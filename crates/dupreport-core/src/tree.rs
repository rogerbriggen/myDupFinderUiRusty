//! Arena-based folder tree with recursive aggregates (§5.6).

use std::collections::HashMap;

use crate::model::{Row, RowIdx};
use crate::path::{parent_of, split_folder_and_name};

/// Index of a node in [`FolderTree::nodes`].
pub type FolderId = u32;
/// The synthetic root (path `""`).
pub const ROOT: FolderId = 0;

/// Sort order of sibling folder names: the old UI's
/// `a.localeCompare(b, undefined, { sensitivity: 'accent' })`, i.e. the
/// Unicode (CLDR root) collation ignoring case but not accents. So `Ärger`
/// sorts between `a` and `b` (not after `z`), `Alpha` / `alpha2` / `beta`
/// ignore case. Equal names (`Docs` / `docs`) compare equal; callers sort stably.
struct NameOrder(Option<icu_collator::CollatorBorrowed<'static>>);

impl NameOrder {
    fn new() -> Self {
        let mut options = icu_collator::options::CollatorOptions::default();
        options.strength = Some(icu_collator::options::Strength::Secondary);
        // Compiled-in data: cannot fail in practice; fall back to a plain
        // case-insensitive order rather than panicking.
        Self(icu_collator::Collator::try_new(Default::default(), options).ok())
    }

    fn compare(&self, a: &str, b: &str) -> std::cmp::Ordering {
        match &self.0 {
            Some(collator) => collator.compare(a, b),
            None => a.to_lowercase().cmp(&b.to_lowercase()),
        }
    }
}

/// One folder with aggregates over its whole subtree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderNode {
    /// Full path; `""` for the synthetic root.
    pub path: String,
    /// Last path segment; `""` for the root.
    pub name: String,
    /// Parent node; `None` only for the root.
    pub parent: Option<FolderId>,
    /// Children sorted by name, case-insensitively.
    pub children: Vec<FolderId>,
    /// Files directly in this folder.
    pub direct_file_count: u32,
    /// Files in this folder and all descendants.
    pub total_file_count: u32,
    /// Bytes in this folder and all descendants.
    pub total_size: i64,
    /// Recursive counts, indexed by `Category as usize`.
    pub category_counts: [u32; 6],
}

impl FolderNode {
    fn new(path: &str, name: &str, parent: Option<FolderId>) -> Self {
        Self {
            path: path.to_string(),
            name: name.to_string(),
            parent,
            children: Vec::new(),
            direct_file_count: 0,
            total_file_count: 0,
            total_size: 0,
            category_counts: [0; 6],
        }
    }
}

/// Folder hierarchy of all rows (both sources in one tree).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderTree {
    /// Arena; index 0 is [`ROOT`]. A parent always has a smaller id than
    /// its children.
    pub nodes: Vec<FolderNode>,
    /// Lookup from full folder path to node.
    pub by_path: HashMap<String, FolderId>,
}

impl FolderTree {
    /// Build the tree and, as a by-product, the direct rows of every folder
    /// (indexed by `FolderId`, in file order).
    #[must_use]
    pub fn build(rows: &[Row]) -> (Self, Vec<Vec<RowIdx>>) {
        let mut tree = Self {
            nodes: vec![FolderNode::new("", "", None)],
            by_path: HashMap::new(),
        };
        let mut rows_by_folder: Vec<Vec<RowIdx>> = vec![Vec::new()];

        for (i, row) in rows.iter().enumerate() {
            let id = tree.ensure_path(row.folder());
            if rows_by_folder.len() < tree.nodes.len() {
                rows_by_folder.resize_with(tree.nodes.len(), Vec::new);
            }
            let node = &mut tree.nodes[id as usize];
            node.direct_file_count += 1;
            node.total_file_count += 1;
            node.total_size += row.size;
            node.category_counts[row.category as usize] += 1;
            rows_by_folder[id as usize].push(i as RowIdx);
        }

        // Children always have larger ids than their parent, so a reverse
        // sweep rolls every subtree up exactly once.
        for id in (1..tree.nodes.len()).rev() {
            let (total_files, total_size, counts, parent) = {
                let n = &tree.nodes[id];
                (
                    n.total_file_count,
                    n.total_size,
                    n.category_counts,
                    n.parent.unwrap_or(ROOT),
                )
            };
            let p = &mut tree.nodes[parent as usize];
            p.total_file_count += total_files;
            p.total_size += total_size;
            for (dst, src) in p.category_counts.iter_mut().zip(counts) {
                *dst += src;
            }
        }

        let order = NameOrder::new();
        for id in 0..tree.nodes.len() {
            let mut children = std::mem::take(&mut tree.nodes[id].children);
            // Stable: names equal for the collator (e.g. `Docs` / `docs`) keep
            // their creation order, i.e. first appearance in the file, like the
            // old UI's stable `Array.sort` over a `Map` in insertion order.
            children.sort_by(|&a, &b| {
                order.compare(&tree.nodes[a as usize].name, &tree.nodes[b as usize].name)
            });
            tree.nodes[id].children = children;
        }

        (tree, rows_by_folder)
    }

    /// Build only the tree (see [`FolderTree::build`]).
    #[must_use]
    pub fn from_rows(rows: &[Row]) -> Self {
        Self::build(rows).0
    }

    /// Node for `folder`, creating it and any missing ancestors.
    fn ensure_path(&mut self, folder: &str) -> FolderId {
        if folder.is_empty() {
            return ROOT;
        }
        if let Some(&id) = self.by_path.get(folder) {
            return id;
        }
        let parent = match parent_of(folder) {
            Some(p) => self.ensure_path(p),
            None => ROOT,
        };
        let name = split_folder_and_name(folder).1;
        let id = FolderId::try_from(self.nodes.len()).expect("fewer than 2^32 folders");
        self.nodes.push(FolderNode::new(folder, name, Some(parent)));
        self.nodes[parent as usize].children.push(id);
        self.by_path.insert(folder.to_string(), id);
        id
    }

    /// Node by id.
    #[must_use]
    pub fn node(&self, id: FolderId) -> &FolderNode {
        &self.nodes[id as usize]
    }

    /// Node id for a full folder path (`""` is the root).
    #[must_use]
    pub fn find(&self, path: &str) -> Option<FolderId> {
        if path.is_empty() {
            Some(ROOT)
        } else {
            self.by_path.get(path).copied()
        }
    }

    /// `id` followed by all its descendants (pre-order).
    #[must_use]
    pub fn subtree_ids(&self, id: FolderId) -> Vec<FolderId> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            out.push(n);
            stack.extend(self.nodes[n as usize].children.iter().rev().copied());
        }
        out
    }

    /// Ancestors of `id` from its parent up to the root.
    #[must_use]
    pub fn ancestors(&self, id: FolderId) -> Vec<FolderId> {
        let mut out = Vec::new();
        let mut cur = self.nodes[id as usize].parent;
        while let Some(p) = cur {
            out.push(p);
            cur = self.nodes[p as usize].parent;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Category, Source};

    fn row(path: &str, size: i64, cat: Category) -> Row {
        Row::new(path.to_string(), size, String::new(), cat, Source::Base, 1)
    }

    fn child_names(t: &FolderTree, id: FolderId) -> Vec<&str> {
        t.node(id)
            .children
            .iter()
            .map(|&c| t.node(c).name.as_str())
            .collect()
    }

    #[test]
    fn aggregates_counts_and_sizes_recursively() {
        let rows = [
            row(r"C:\a\one.txt", 100, Category::Duplicate),
            row(r"C:\a\b\two.txt", 200, Category::Duplicate),
            row(r"C:\a\b\three.txt", 50, Category::Missing),
            row(r"D:\x\four.txt", 7, Category::New),
        ];
        let (t, by_folder) = FolderTree::build(&rows);
        let root = t.node(ROOT);
        assert_eq!(root.total_file_count, 4);
        assert_eq!(root.total_size, 357);
        assert_eq!(root.direct_file_count, 0);
        let a = t.node(t.find(r"C:\a").unwrap());
        assert_eq!(a.direct_file_count, 1);
        assert_eq!(a.total_file_count, 3);
        assert_eq!(a.total_size, 350);
        assert_eq!(a.category_counts[Category::Duplicate as usize], 2);
        assert_eq!(a.category_counts[Category::Missing as usize], 1);
        assert_eq!(a.category_counts[Category::New as usize], 0);
        let c = t.node(t.find("C:").unwrap());
        assert_eq!(c.total_file_count, 3);
        assert_eq!(c.parent, Some(ROOT));
        assert_eq!(root.category_counts.iter().sum::<u32>(), 4);

        let b = t.find(r"C:\a\b").unwrap();
        assert_eq!(by_folder[b as usize], vec![1, 2]);
        assert_eq!(by_folder[t.find(r"C:\a").unwrap() as usize], vec![0]);
        assert_eq!(by_folder.len(), t.nodes.len());
    }

    #[test]
    fn sorts_children_case_insensitively() {
        let rows = [
            row(r"C:\beta\f", 1, Category::Unique),
            row(r"C:\Alpha\f", 1, Category::Unique),
            row(r"C:\charlie\f", 1, Category::Unique),
            row(r"C:\alpha2\f", 1, Category::Unique),
        ];
        let t = FolderTree::from_rows(&rows);
        let c = t.find("C:").unwrap();
        assert_eq!(child_names(&t, c), ["Alpha", "alpha2", "beta", "charlie"]);
    }

    /// Same order as the old UI's `localeCompare(…, {sensitivity: 'accent'})`
    /// (values checked against Node's ICU): accents sort next to their base
    /// letter, case is ignored, case-only twins keep their file order.
    #[test]
    fn sorts_children_like_locale_compare() {
        let names = [
            "zebra",
            "Übersicht",
            "Ärger",
            "set1",
            "café",
            "cafe",
            "Docs",
            "docs",
            "A b",
            "a",
            "b",
            "Österreich",
            "oz",
            "10",
            "9",
            "_tmp",
            "Fotos 2024",
        ];
        let rows: Vec<Row> = names
            .iter()
            .map(|n| row(&format!(r"C:\{n}\f"), 1, Category::Unique))
            .collect();
        let t = FolderTree::from_rows(&rows);
        let c = t.find("C:").unwrap();
        assert_eq!(
            child_names(&t, c),
            [
                "_tmp",
                "10",
                "9",
                "a",
                "A b",
                "Ärger",
                "b",
                "cafe",
                "café",
                "Docs",
                "docs",
                "Fotos 2024",
                "Österreich",
                "oz",
                "set1",
                "Übersicht",
                "zebra",
            ]
        );
    }

    #[test]
    fn bare_filenames_hang_off_the_root() {
        let rows = [
            row("bare.txt", 5, Category::Unique),
            row(r"C:\a\f", 1, Category::Unique),
        ];
        let (t, by_folder) = FolderTree::build(&rows);
        let root = t.node(ROOT);
        assert_eq!(root.direct_file_count, 1);
        assert_eq!(root.total_file_count, 2);
        assert_eq!(by_folder[ROOT as usize], vec![0]);
        assert_eq!(root.children.len(), 1);
    }

    #[test]
    fn posix_paths_build_a_tree() {
        let rows = [row("/x/y/z.txt", 3, Category::Unique)];
        let t = FolderTree::from_rows(&rows);
        let x = t.find("/x").unwrap();
        let y = t.find("/x/y").unwrap();
        assert_eq!(t.node(x).parent, Some(ROOT));
        assert_eq!(t.node(y).parent, Some(x));
        assert_eq!(t.node(y).name, "y");
        assert_eq!(t.node(y).direct_file_count, 1);
    }

    #[test]
    fn subtree_and_ancestors() {
        let rows = [
            row(r"C:\a\b\f", 1, Category::Unique),
            row(r"C:\a\c\f", 1, Category::Unique),
            row(r"D:\f", 1, Category::Unique),
        ];
        let t = FolderTree::from_rows(&rows);
        let a = t.find(r"C:\a").unwrap();
        let sub: Vec<_> = t
            .subtree_ids(a)
            .into_iter()
            .map(|i| t.node(i).path.as_str())
            .collect();
        assert_eq!(sub, [r"C:\a", r"C:\a\b", r"C:\a\c"]);
        let b = t.find(r"C:\a\b").unwrap();
        let anc: Vec<_> = t
            .ancestors(b)
            .into_iter()
            .map(|i| t.node(i).path.as_str())
            .collect();
        assert_eq!(anc, [r"C:\a", "C:", ""]);
        assert_eq!(t.find(""), Some(ROOT));
        assert_eq!(t.find(r"C:\nope"), None);
    }

    #[test]
    fn empty_rows_give_root_only() {
        let (t, by_folder) = FolderTree::build(&[]);
        assert_eq!(t.nodes.len(), 1);
        assert_eq!(by_folder.len(), 1);
        assert_eq!(t.node(ROOT).total_file_count, 0);
    }
}
