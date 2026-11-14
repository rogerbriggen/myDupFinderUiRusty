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

use dupreport_core::{FolderId, FolderTree, ROOT, Source};

/// Which rows the tree is built from: both sources (default), or only the
/// folders holding files of one source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TreeSource {
    /// Base and Second in one tree.
    #[default]
    Both,
    /// Only folders with Base files.
    Base,
    /// Only folders with Second files.
    Second,
}

impl TreeSource {
    /// All modes in menu order.
    pub const ALL: [TreeSource; 3] = [TreeSource::Both, TreeSource::Base, TreeSource::Second];

    /// The source filter of this mode (`None` = both).
    #[must_use]
    pub fn source(self) -> Option<Source> {
        match self {
            Self::Both => None,
            Self::Base => Some(Source::Base),
            Self::Second => Some(Source::Second),
        }
    }

    /// Menu / button label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Both => "Base + Second",
            Self::Base => "Base only",
            Self::Second => "Second only",
        }
    }
}

/// One visible tree row: the folder and its depth (root is 0).
pub type FlatRow = (FolderId, u32);

/// Rank of a folder without a known status (sorts last, §7.4).
pub const UNKNOWN_RANK: u8 = 3;

/// Which folders are expanded and the resulting pre-order list of visible rows.
#[derive(Debug, Clone)]
pub struct TreeViewModel {
    expanded: HashSet<FolderId>,
    /// Optional sort rank per `FolderId` (lower first). `None` keeps the
    /// alphabetical order of `FolderTree`. The identical-status
    /// ranking (green 0, yellow 1, red 2, unknown 3) is plugged in here.
    ranks: Option<Vec<u8>>,
    /// Whether expanding a folder also expands its single-subfolder chain.
    auto_expand_chains: bool,
    /// Only folders holding files of this source are shown (`None` = all).
    /// The root is always shown.
    source: Option<Source>,
    flat: Arc<Vec<FlatRow>>,
}

impl TreeViewModel {
    /// A model for `tree` with only the root expanded (plus the root's chain
    /// when `auto_expand_chains` is on, see [`Self::set_auto_expand_chains`]).
    #[cfg(test)]
    #[must_use]
    pub fn new(tree: &FolderTree, auto_expand_chains: bool) -> Self {
        Self::with_source(tree, auto_expand_chains, None)
    }

    /// Like [`Self::new`], restricted to folders holding files of `source`.
    #[must_use]
    pub fn with_source(
        tree: &FolderTree,
        auto_expand_chains: bool,
        source: Option<Source>,
    ) -> Self {
        let mut model = Self::empty();
        model.auto_expand_chains = auto_expand_chains;
        model.source = source;
        model.reset(tree);
        model
    }

    /// An empty model (no report). Chain auto-expansion is on.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            expanded: HashSet::new(),
            ranks: None,
            auto_expand_chains: true,
            source: None,
            flat: Arc::new(Vec::new()),
        }
    }

    /// The source the tree is restricted to (`None` = both).
    #[cfg(test)]
    #[must_use]
    pub fn source(&self) -> Option<Source> {
        self.source
    }

    /// Restrict the tree to folders holding files of `source` (`None` = all)
    /// and rebuild. Expansion and ranking are kept; with chain auto-expansion
    /// on, the root's chain in the new mode is expanded too (e.g. the Second
    /// tree's `D:` → `backup` → … when switching from Base to Second).
    pub fn set_source(&mut self, tree: &FolderTree, source: Option<Source>) {
        if self.source == source {
            return;
        }
        self.source = source;
        if self.auto_expand_chains && self.expanded.contains(&ROOT) {
            self.expand_chain(tree, ROOT);
        }
        self.rebuild(tree);
    }

    /// Whether `id` is part of the tree in the current source mode.
    #[must_use]
    pub fn shows(&self, tree: &FolderTree, id: FolderId) -> bool {
        id == ROOT || tree.node(id).total_files_of(self.source) > 0
    }

    /// The displayed subfolders of `id` (alphabetical, unranked).
    fn children<'t>(
        &self,
        tree: &'t FolderTree,
        id: FolderId,
    ) -> impl Iterator<Item = FolderId> + 't {
        let source = self.source;
        tree.node(id)
            .children
            .iter()
            .copied()
            .filter(move |&c| tree.node(c).total_files_of(source) > 0)
    }

    /// Whether `id` has displayed subfolders (otherwise it is a leaf).
    #[must_use]
    pub fn has_children(&self, tree: &FolderTree, id: FolderId) -> bool {
        self.children(tree, id).next().is_some()
    }

    /// The file count shown next to `id`: its subtree, current source only.
    #[must_use]
    pub fn file_count(&self, tree: &FolderTree, id: FolderId) -> u32 {
        tree.node(id).total_files_of(self.source)
    }

    /// Collapse everything except the root (and the root's chain when
    /// auto-expansion is on) and drop any ranking (new report).
    pub fn reset(&mut self, tree: &FolderTree) {
        self.expanded.clear();
        self.expanded.insert(ROOT);
        if self.auto_expand_chains {
            self.expand_chain(tree, ROOT);
        }
        self.ranks = None;
        self.rebuild(tree);
    }

    /// Whether expanding a folder also expands its single-subfolder chain.
    #[cfg(test)]
    #[must_use]
    pub fn auto_expand_chains(&self) -> bool {
        self.auto_expand_chains
    }

    /// Turn chain auto-expansion on or off. Only later expansions are
    /// affected; the current expansion state is kept.
    pub fn set_auto_expand_chains(&mut self, on: bool) {
        self.auto_expand_chains = on;
    }

    /// Below the (already expanded) `id`, expand a chain of pass-through
    /// folders: while the current folder has exactly one subfolder, and that
    /// subfolder holds no files itself and has subfolders of its own, the
    /// subfolder is expanded too. The walk stops at the first folder that
    /// branches, holds files or is a leaf; that folder stays visible but
    /// collapsed (or, if it branches, expanded by the previous step), so the
    /// first "interesting" level becomes visible in one step.
    fn expand_chain(&mut self, tree: &FolderTree, id: FolderId) {
        let mut cur = id;
        loop {
            let mut children = self.children(tree, cur);
            let (Some(child), None) = (children.next(), children.next()) else {
                return;
            };
            if tree.node(child).direct_files_of(self.source) > 0 || !self.has_children(tree, child)
            {
                return;
            }
            self.expanded.insert(child);
            cur = child;
        }
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
    pub fn set_ranks(&mut self, tree: &FolderTree, ranks: Option<Vec<u8>>) {
        self.ranks = ranks;
        self.rebuild(tree);
    }

    /// Expand `id` (and its chain when auto-expansion is on). Returns `true`
    /// when something changed (leaves never expand).
    pub fn expand(&mut self, tree: &FolderTree, id: FolderId) -> bool {
        if !self.has_children(tree, id) || !self.expanded.insert(id) {
            return false;
        }
        if self.auto_expand_chains {
            self.expand_chain(tree, id);
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

    /// The nearest folder at or above `id` that is part of the tree in the
    /// current source mode (`id` itself when it is; at worst the root).
    #[must_use]
    pub fn nearest_shown(&self, tree: &FolderTree, id: FolderId) -> FolderId {
        let mut cur = id;
        while !self.shows(tree, cur) {
            cur = tree.node(cur).parent.unwrap_or(ROOT);
        }
        cur
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
        if !self.has_children(tree, id) {
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
        if self.is_expanded(id) && self.has_children(tree, id) {
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
            let mut children: Vec<FolderId> = self.children(tree, id).collect();
            if let Some(ranks) = &self.ranks {
                let rank = |c: FolderId| ranks.get(c as usize).copied().unwrap_or(UNKNOWN_RANK);
                children.sort_by_key(|&c| rank(c)); // stable: keeps alphabetical ties
            }
            stack.extend(children.iter().rev().map(|&c| (c, depth + 1)));
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
        let m = TreeViewModel::new(&t, false);
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
        let mut m = TreeViewModel::new(&t, false);
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
        let mut m = TreeViewModel::new(&t, false);
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
        let mut m = TreeViewModel::new(&t, false);
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
        let m = TreeViewModel::new(&t, false);
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
        let mut m = TreeViewModel::new(&t, false);
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
        let mut m = TreeViewModel::new(&t, false);
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
        let mut m = TreeViewModel::new(&t, false);
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
        let mut m = TreeViewModel::new(&t, false);
        m.reveal(&t, deep);
        assert!(m.is_expanded(c) && m.is_expanded(alpha) && m.is_expanded(sub));
        assert!(m.position(deep).is_some());
    }

    #[test]
    fn reset_collapses_everything_but_the_root() {
        let (t, [c, d, alpha, ..]) = sample();
        let mut m = TreeViewModel::new(&t, false);
        m.expand(&t, c);
        m.expand(&t, alpha);
        m.reset(&t);
        assert_eq!(visible(&m), [ROOT, c, d]);
    }

    #[test]
    fn ranks_reorder_children_stably() {
        let (t, [c, d, alpha, beta, ..]) = sample();
        let mut m = TreeViewModel::new(&t, false);
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
        let mut m = TreeViewModel::new(&t, false);
        m.expand(&t, c);
        let mut ranks = vec![UNKNOWN_RANK; t.nodes.len()];
        ranks[beta as usize] = 0;
        m.set_ranks(&t, Some(ranks));
        assert_eq!(m.key_right(&t, c), beta);
    }

    /// root -> `O:` -> a -> b (holds a file) -> c -> {x, y}.
    fn chain() -> (FolderTree, [FolderId; 6]) {
        let rows = [
            row(r"O:\a\b\f.txt"),
            row(r"O:\a\b\c\x\g.txt"),
            row(r"O:\a\b\c\y\h.txt"),
        ];
        let t = FolderTree::from_rows(&rows);
        let id = |p: &str| t.find(p).unwrap();
        let ids = [
            id("O:"),
            id(r"O:\a"),
            id(r"O:\a\b"),
            id(r"O:\a\b\c"),
            id(r"O:\a\b\c\x"),
            id(r"O:\a\b\c\y"),
        ];
        (t, ids)
    }

    #[test]
    fn reset_expands_the_root_chain_down_to_a_folder_with_files() {
        let (t, [o, a, b, c, ..]) = chain();
        let m = TreeViewModel::new(&t, true);
        assert_eq!(visible(&m), [ROOT, o, a, b]);
        assert!(m.is_expanded(o) && m.is_expanded(a));
        assert!(!m.is_expanded(b), "b holds files: the walk stops at it");
        assert_eq!(m.position(c), None);
    }

    #[test]
    fn expanding_continues_through_the_chain_until_it_branches() {
        let (t, [o, a, b, c, x, y]) = chain();
        let mut m = TreeViewModel::new(&t, true);
        // b itself holds files, but its only subfolder c does not: c is
        // expanded too, and c branches, so x and y become visible.
        assert!(m.expand(&t, b));
        assert!(m.is_expanded(c));
        assert_eq!(visible(&m), [ROOT, o, a, b, c, x, y]);

        // Collapsed chain links are expanded again by expanding the top.
        m.collapse(&t, a);
        m.collapse(&t, o);
        assert_eq!(visible(&m), [ROOT, o]);
        assert!(m.expand(&t, o));
        assert_eq!(visible(&m), [ROOT, o, a, b, c, x, y]);
    }

    #[test]
    fn chain_stops_before_a_leaf() {
        let t = FolderTree::from_rows(&[row(r"O:\a\leaf\f.txt")]);
        let id = |p: &str| t.find(p).unwrap();
        let (o, a, leaf) = (id("O:"), id(r"O:\a"), id(r"O:\a\leaf"));
        let m = TreeViewModel::new(&t, true);
        assert_eq!(visible(&m), [ROOT, o, a, leaf]);
        assert!(m.is_expanded(a));
        assert!(!m.is_expanded(leaf));
    }

    fn second(path: &str) -> Row {
        Row::new(
            path.to_string(),
            1,
            String::new(),
            Category::New,
            Source::Second,
            2,
        )
    }

    /// Base: `C:\base\a\f`, `C:\base\b\f`; Second: `D:\backup\a\f`, plus one
    /// Second file in the otherwise Base-only `C:\base\b`.
    fn two_sources() -> (FolderTree, [FolderId; 7]) {
        let rows = [
            row(r"C:\base\a\f.txt"),
            row(r"C:\base\b\f.txt"),
            second(r"D:\backup\a\f.txt"),
            second(r"C:\base\b\g.txt"),
        ];
        let t = FolderTree::from_rows(&rows);
        let id = |p: &str| t.find(p).unwrap();
        let ids = [
            id("C:"),
            id(r"C:\base"),
            id(r"C:\base\a"),
            id(r"C:\base\b"),
            id("D:"),
            id(r"D:\backup"),
            id(r"D:\backup\a"),
        ];
        (t, ids)
    }

    #[test]
    fn source_mode_hides_folders_without_files_of_that_source() {
        let (t, [c, base, a, b, d, backup, da]) = two_sources();
        let mut m = TreeViewModel::new(&t, false);
        m.expand(&t, c);
        m.expand(&t, base);
        m.expand(&t, d);
        m.expand(&t, backup);
        assert_eq!(visible(&m), [ROOT, c, base, a, b, d, backup, da]);
        assert_eq!(m.file_count(&t, ROOT), 4);

        m.set_source(&t, Some(Source::Base));
        assert_eq!(visible(&m), [ROOT, c, base, a, b], "D: has no Base files");
        assert_eq!(m.file_count(&t, ROOT), 2);
        assert_eq!(
            m.file_count(&t, b),
            1,
            "the Second file in b is not counted"
        );
        assert!(!m.shows(&t, d));

        m.set_source(&t, Some(Source::Second));
        assert_eq!(visible(&m), [ROOT, c, base, b, d, backup, da]);
        assert_eq!(m.file_count(&t, base), 1);
        assert!(!m.shows(&t, a));
        assert_eq!(m.nearest_shown(&t, a), base);
        assert_eq!(m.nearest_shown(&t, da), da);

        m.set_source(&t, None);
        assert_eq!(visible(&m), [ROOT, c, base, a, b, d, backup, da]);
    }

    #[test]
    fn hidden_children_make_a_leaf_in_source_mode() {
        let (t, [c, base, a, ..]) = two_sources();
        let mut m = TreeViewModel::with_source(&t, false, Some(Source::Second));
        m.expand(&t, c);
        m.expand(&t, base);
        let b = t.find(r"C:\base\b").unwrap();
        assert!(!m.has_children(&t, b));
        assert!(!m.expand(&t, b), "a leaf in this mode");
        assert_eq!(
            m.key_right(&t, base),
            b,
            "first displayed child, a is hidden"
        );
        assert_eq!(m.position(a), None);
    }

    #[test]
    fn switching_source_expands_the_new_root_chain() {
        let (t, [c, base, _, _, d, ..]) = two_sources();
        let mut m = TreeViewModel::with_source(&t, true, Some(Source::Base));
        // Base: root -> C: -> base (branches): expanded down to base.
        assert!(m.is_expanded(c) && m.is_expanded(base));
        assert!(!m.is_expanded(d));
        // Second: root has two children (C:, D:), so the root chain stops at once.
        m.set_source(&t, Some(Source::Second));
        assert!(!m.is_expanded(d));
        // Only D: in a pure Second tree: its chain opens on switching.
        let t2 = FolderTree::from_rows(&[row(r"C:\x\f.txt"), second(r"D:\backup\a\f.txt")]);
        let d2 = t2.find("D:").unwrap();
        let backup2 = t2.find(r"D:\backup").unwrap();
        let mut m2 = TreeViewModel::with_source(&t2, true, Some(Source::Base));
        assert!(!m2.is_expanded(d2));
        m2.set_source(&t2, Some(Source::Second));
        assert!(m2.is_expanded(d2) && m2.is_expanded(backup2));
    }

    #[test]
    fn auto_expansion_off_expands_one_level_and_can_be_switched_on() {
        let (t, [o, a, b, ..]) = chain();
        let mut m = TreeViewModel::new(&t, false);
        assert!(!m.auto_expand_chains());
        assert_eq!(visible(&m), [ROOT, o]);
        m.expand(&t, o);
        assert_eq!(visible(&m), [ROOT, o, a]);

        m.set_auto_expand_chains(true);
        assert!(m.auto_expand_chains());
        assert_eq!(visible(&m), [ROOT, o, a], "switching on keeps the state");
        m.collapse(&t, o);
        m.expand(&t, o);
        assert_eq!(visible(&m), [ROOT, o, a, b]);
    }
}
