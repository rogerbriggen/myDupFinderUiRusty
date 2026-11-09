//! Expansion state and flattened row list of the folder tree pane (§7.4).
//!
//! Pure view model without any GPUI dependency, so the flattening, expand /
//! collapse and keyboard navigation rules are unit-testable. It lives in the UI
//! crate (not in `dupreport-core`) because expansion and ordering by status
//! rank are presentation state, not report data. The selection itself is owned
//! by `ReportStore::selected_folder`; the navigation methods take the current
//! selection and return the new one.

use std::collections::HashSet;
use std::sync::Arc;

use dupreport_core::{FolderId, FolderTree, ROOT};

/// One visible tree row: the folder and its depth (root is 0).
pub type FlatRow = (FolderId, u32);

/// Rank of a folder without a known status (sorts last, §7.4).
pub const UNKNOWN_RANK: u8 = 3;

/// Which folders are expanded and the resulting pre-order list of visible rows.
#[derive(Debug, Clone)]
pub struct TreeViewModel {
    expanded: HashSet<FolderId>,
    /// Optional sort rank per `FolderId` (lower first). `None` keeps the
    /// alphabetical order of `FolderTree`. Step 11 plugs the identical-status
    /// ranking (green 0, yellow 1, red 2, unknown 3) in here.
    ranks: Option<Vec<u8>>,
    flat: Arc<Vec<FlatRow>>,
}

impl TreeViewModel {
    /// A model for `tree` with only the root expanded.
    #[must_use]
    pub fn new(tree: &FolderTree) -> Self {
        let mut model = Self {
            expanded: HashSet::new(),
            ranks: None,
            flat: Arc::new(Vec::new()),
        };
        model.reset(tree);
        model
    }

    /// An empty model (no report).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            expanded: HashSet::new(),
            ranks: None,
            flat: Arc::new(Vec::new()),
        }
    }

    /// Collapse everything except the root and drop any ranking (new report).
    pub fn reset(&mut self, tree: &FolderTree) {
        self.expanded.clear();
        self.expanded.insert(ROOT);
        self.ranks = None;
        self.rebuild(tree);
    }

    /// Cheap shared handle on the visible rows, for use inside list callbacks.
    #[must_use]
    pub fn rows_arc(&self) -> Arc<Vec<FlatRow>> {
        self.flat.clone()
    }

    /// The visible rows in pre-order.
    #[must_use]
    pub fn rows(&self) -> &[FlatRow] {
        &self.flat
    }

    /// Whether `id` is currently expanded.
    #[must_use]
    pub fn is_expanded(&self, id: FolderId) -> bool {
        self.expanded.contains(&id)
    }

    /// Index of `id` in the visible rows, if it is visible.
    #[must_use]
    pub fn position(&self, id: FolderId) -> Option<usize> {
        self.flat.iter().position(|&(f, _)| f == id)
    }

    /// Set (or clear) the per-folder sort rank and rebuild. `ranks[id]` is the
    /// rank of folder `id`; missing entries count as [`UNKNOWN_RANK`]. Equal
    /// ranks keep the alphabetical order (stable sort).
    // Step 11 (status dots / green on top) is the first caller.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by green-on-top (step 11)")
    )]
    pub fn set_ranks(&mut self, tree: &FolderTree, ranks: Option<Vec<u8>>) {
        self.ranks = ranks;
        self.rebuild(tree);
    }

    /// Expand `id`. Returns `true` when something changed (leaves never expand).
    pub fn expand(&mut self, tree: &FolderTree, id: FolderId) -> bool {
        if tree.node(id).children.is_empty() || !self.expanded.insert(id) {
            return false;
        }
        self.rebuild(tree);
        true
    }

    /// Collapse `id`. Returns `true` when something changed.
    pub fn collapse(&mut self, tree: &FolderTree, id: FolderId) -> bool {
        if !self.expanded.remove(&id) {
            return false;
        }
        self.rebuild(tree);
        true
    }

    /// Toggle `id`; a no-op on leaves. Returns `true` when something changed.
    pub fn toggle(&mut self, tree: &FolderTree, id: FolderId) -> bool {
        if self.is_expanded(id) {
            self.collapse(tree, id)
        } else {
            self.expand(tree, id)
        }
    }

    /// Expand all ancestors of `id` so that it becomes visible.
    // Step 11 (identical panel: "expand ancestors and scroll into view").
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by the identical panel (step 11)")
    )]
    pub fn reveal(&mut self, tree: &FolderTree, id: FolderId) {
        let mut changed = false;
        let mut cur = tree.node(id).parent;
        while let Some(p) = cur {
            changed |= self.expanded.insert(p);
            cur = tree.node(p).parent;
        }
        if changed {
            self.rebuild(tree);
        }
    }

    /// The nearest visible folder at or above `id` (`id` itself when visible).
    /// A selection can be hidden after one of its ancestors was collapsed.
    fn visible_anchor(&self, tree: &FolderTree, id: FolderId) -> FolderId {
        let mut cur = id;
        loop {
            if self.position(cur).is_some() {
                return cur;
            }
            match tree.node(cur).parent {
                Some(p) => cur = p,
                None => return ROOT,
            }
        }
    }

    /// ↑: the previous visible row (stays at the first row).
    #[must_use]
    pub fn select_prev(&self, tree: &FolderTree, selected: FolderId) -> FolderId {
        let anchor = self.visible_anchor(tree, selected);
        match self.position(anchor) {
            Some(i) if i > 0 => self.flat[i - 1].0,
            _ => anchor,
        }
    }

    /// ↓: the next visible row (stays at the last row).
    #[must_use]
    pub fn select_next(&self, tree: &FolderTree, selected: FolderId) -> FolderId {
        let anchor = self.visible_anchor(tree, selected);
        match self.position(anchor) {
            Some(i) if i + 1 < self.flat.len() => self.flat[i + 1].0,
            _ => anchor,
        }
    }

    /// →: expand a collapsed folder (selection stays); on an expanded folder
    /// move to its first child; on a leaf do nothing. Returns the new selection.
    pub fn key_right(&mut self, tree: &FolderTree, selected: FolderId) -> FolderId {
        let id = self.visible_anchor(tree, selected);
        let node = tree.node(id);
        if node.children.is_empty() {
            return id;
        }
        if self.is_expanded(id) {
            // The first *displayed* child (rank order may differ from `children`).
            return self
                .position(id)
                .and_then(|i| self.flat.get(i + 1))
                .map_or(id, |&(child, _)| child);
        }
        self.expand(tree, id);
        id
    }

    /// ←: collapse an expanded folder (selection stays); otherwise move to the
    /// parent (the root has none). Returns the new selection.
    pub fn key_left(&mut self, tree: &FolderTree, selected: FolderId) -> FolderId {
        let id = self.visible_anchor(tree, selected);
        if self.is_expanded(id) && !tree.node(id).children.is_empty() {
            self.collapse(tree, id);
            return id;
        }
        tree.node(id).parent.unwrap_or(id)
    }

    /// Recompute the flattened list: pre-order over expanded nodes.
    fn rebuild(&mut self, tree: &FolderTree) {
        let mut flat = Vec::with_capacity(self.flat.len());
        let mut stack: Vec<FlatRow> = vec![(ROOT, 0)];
        while let Some((id, depth)) = stack.pop() {
            flat.push((id, depth));
            if !self.expanded.contains(&id) {
                continue;
            }
            let children = &tree.node(id).children;
            match &self.ranks {
                None => stack.extend(children.iter().rev().map(|&c| (c, depth + 1))),
                Some(ranks) => {
                    let rank = |c: FolderId| ranks.get(c as usize).copied().unwrap_or(UNKNOWN_RANK);
                    let mut ordered = children.clone();
                    ordered.sort_by_key(|&c| rank(c)); // stable: keeps alphabetical ties
                    stack.extend(ordered.iter().rev().map(|&c| (c, depth + 1)));
                }
            }
        }
        self.flat = Arc::new(flat);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dupreport_core::{Category, Row, Source};

    fn row(path: &str) -> Row {
        Row::new(
            path.to_string(),
            1,
            String::new(),
            Category::Unique,
            Source::Base,
            1,
        )
    }

    /// root -> `C:`, `D:`; `C:` -> alpha, Beta; alpha -> sub -> deep; `D:` -> d.
    fn sample() -> (FolderTree, [FolderId; 8]) {
        let rows = [
            row(r"C:\Beta\f.txt"),
            row(r"C:\alpha\x.txt"),
            row(r"C:\alpha\sub\y.txt"),
            row(r"C:\alpha\sub\deep\z.txt"),
            row(r"D:\d\f.txt"),
        ];
        let t = FolderTree::from_rows(&rows);
        let id = |p: &str| t.find(p).unwrap();
        let ids = [
            id("C:"),
            id("D:"),
            id(r"C:\alpha"),
            id(r"C:\Beta"),
            id(r"C:\alpha\sub"),
            id(r"C:\alpha\sub\deep"),
            id(r"D:\d"),
            ROOT,
        ];
        (t, ids)
    }

    fn visible(m: &TreeViewModel) -> Vec<FolderId> {
        m.rows().iter().map(|r| r.0).collect()
    }

    #[test]
    fn initially_root_and_its_children_are_visible() {
        let (t, [c, d, ..]) = sample();
        let m = TreeViewModel::new(&t);
        assert_eq!(m.rows(), &[(ROOT, 0), (c, 1), (d, 1)]);
        assert!(m.is_expanded(ROOT));
        assert!(!m.is_expanded(c));
    }

    #[test]
    fn empty_model_has_no_rows() {
        assert!(TreeViewModel::empty().rows().is_empty());
    }

    #[test]
    fn expand_and_collapse_children_in_case_insensitive_order() {
        let (t, [c, d, alpha, beta, ..]) = sample();
        let mut m = TreeViewModel::new(&t);
        assert!(m.expand(&t, c));
        // "alpha" < "Beta" case-insensitively although 'B' < 'a' in ASCII.
        assert_eq!(
            m.rows(),
            &[(ROOT, 0), (c, 1), (alpha, 2), (beta, 2), (d, 1)]
        );
        assert!(!m.expand(&t, c), "already expanded");
        assert!(m.collapse(&t, c));
        assert_eq!(visible(&m), [ROOT, c, d]);
        assert!(!m.collapse(&t, c), "already collapsed");
    }

    #[test]
    fn collapse_hides_the_whole_subtree_but_keeps_inner_expansion() {
        let (t, [c, d, alpha, beta, sub, ..]) = sample();
        let mut m = TreeViewModel::new(&t);
        m.expand(&t, c);
        m.expand(&t, alpha);
        assert_eq!(visible(&m), [ROOT, c, alpha, sub, beta, d]);
        m.collapse(&t, c);
        assert_eq!(visible(&m), [ROOT, c, d]);
        m.expand(&t, c);
        assert_eq!(visible(&m), [ROOT, c, alpha, sub, beta, d]);
    }

    #[test]
    fn toggling_a_leaf_is_a_no_op() {
        let (t, [c, _, _, beta, ..]) = sample();
        let mut m = TreeViewModel::new(&t);
        m.expand(&t, c);
        let before = m.rows().to_vec();
        assert!(!m.toggle(&t, beta));
        assert!(!m.is_expanded(beta));
        assert_eq!(m.rows(), before.as_slice());
        assert!(m.toggle(&t, c));
        assert!(m.toggle(&t, c));
    }

    #[test]
    fn up_and_down_clamp_at_the_ends() {
        let (t, [c, d, ..]) = sample();
        let m = TreeViewModel::new(&t);
        assert_eq!(m.select_prev(&t, ROOT), ROOT);
        assert_eq!(m.select_next(&t, ROOT), c);
        assert_eq!(m.select_next(&t, c), d);
        assert_eq!(m.select_next(&t, d), d);
        assert_eq!(m.select_prev(&t, d), c);
        assert_eq!(m.select_prev(&t, c), ROOT);
    }

    #[test]
    fn right_expands_then_moves_to_first_child_then_stops_at_leaf() {
        let (t, [c, _, alpha, beta, sub, deep, ..]) = sample();
        let mut m = TreeViewModel::new(&t);
        assert_eq!(m.key_right(&t, c), c, "first press expands");
        assert!(m.is_expanded(c));
        assert_eq!(
            m.key_right(&t, c),
            alpha,
            "second press goes to first child"
        );
        assert_eq!(m.key_right(&t, beta), beta, "leaf: nothing");
        m.expand(&t, alpha);
        m.expand(&t, sub);
        assert_eq!(m.key_right(&t, sub), deep);
        assert_eq!(m.key_right(&t, deep), deep);
    }

    #[test]
    fn left_collapses_then_moves_to_parent_root_stays() {
        let (t, [c, _, alpha, ..]) = sample();
        let mut m = TreeViewModel::new(&t);
        m.expand(&t, c);
        m.expand(&t, alpha);
        assert_eq!(m.key_left(&t, alpha), alpha, "collapses first");
        assert!(!m.is_expanded(alpha));
        assert_eq!(m.key_left(&t, alpha), c, "then moves to the parent");
        assert_eq!(m.key_left(&t, c), c, "collapses c");
        assert!(!m.is_expanded(c));
        assert_eq!(m.key_left(&t, c), ROOT);
        assert_eq!(m.key_left(&t, ROOT), ROOT, "collapses the root");
        assert_eq!(visible(&m), [ROOT]);
        assert_eq!(m.key_left(&t, ROOT), ROOT, "root has no parent");
    }

    #[test]
    fn hidden_selection_navigates_from_its_visible_ancestor() {
        let (t, [c, d, alpha, _, sub, ..]) = sample();
        let mut m = TreeViewModel::new(&t);
        m.expand(&t, c);
        m.expand(&t, alpha);
        m.collapse(&t, c); // `sub` and `alpha` are now hidden
        assert_eq!(m.position(sub), None);
        assert_eq!(m.select_next(&t, sub), d);
        assert_eq!(m.select_prev(&t, sub), ROOT);
    }

    #[test]
    fn reveal_expands_all_ancestors() {
        let (t, [c, _, alpha, _, sub, deep, ..]) = sample();
        let mut m = TreeViewModel::new(&t);
        m.reveal(&t, deep);
        assert!(m.is_expanded(c) && m.is_expanded(alpha) && m.is_expanded(sub));
        assert!(m.position(deep).is_some());
    }

    #[test]
    fn reset_collapses_everything_but_the_root() {
        let (t, [c, d, alpha, ..]) = sample();
        let mut m = TreeViewModel::new(&t);
        m.expand(&t, c);
        m.expand(&t, alpha);
        m.reset(&t);
        assert_eq!(visible(&m), [ROOT, c, d]);
    }

    #[test]
    fn ranks_reorder_children_stably() {
        let (t, [c, d, alpha, beta, ..]) = sample();
        let mut m = TreeViewModel::new(&t);
        m.expand(&t, c);
        let mut ranks = vec![UNKNOWN_RANK; t.nodes.len()];
        ranks[d as usize] = 0;
        ranks[beta as usize] = 1;
        m.set_ranks(&t, Some(ranks));
        assert_eq!(visible(&m), [ROOT, d, c, beta, alpha]);
        m.set_ranks(&t, None);
        assert_eq!(visible(&m), [ROOT, c, alpha, beta, d]);
    }

    #[test]
    fn right_on_expanded_folder_follows_displayed_order() {
        let (t, [c, _, _, beta, ..]) = sample();
        let mut m = TreeViewModel::new(&t);
        m.expand(&t, c);
        let mut ranks = vec![UNKNOWN_RANK; t.nodes.len()];
        ranks[beta as usize] = 0;
        m.set_ranks(&t, Some(ranks));
        assert_eq!(m.key_right(&t, c), beta);
    }
}
