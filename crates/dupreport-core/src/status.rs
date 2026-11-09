//! Green / yellow / red identical status per folder (§5.9).

use std::collections::HashSet;

use crate::model::IdenticalFolderPair;
use crate::tree::FolderTree;

/// Whether a folder's subtree has identical matches elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdenticalStatus {
    /// The entire subtree has an identical match.
    Green,
    /// Only some descendants have an identical match.
    Yellow,
    /// Nothing in the subtree has an identical match.
    Red,
}

impl IdenticalStatus {
    /// Tooltip text for the status dot.
    #[must_use]
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Green => "Entire subtree has an identical match elsewhere",
            Self::Yellow => "Some descendants have an identical match elsewhere",
            Self::Red => "No identical match in this subtree",
        }
    }

    /// Sort rank for "green on top": Green 0, Yellow 1, Red 2.
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Self::Green => 0,
            Self::Yellow => 1,
            Self::Red => 2,
        }
    }
}

/// Status of every folder, indexed by `FolderId`; all `None` when `pairs`
/// is empty ("no scan yet").
#[must_use]
pub fn compute_statuses(
    tree: &FolderTree,
    pairs: &[IdenticalFolderPair],
) -> Vec<Option<IdenticalStatus>> {
    let n = tree.nodes.len();
    if pairs.is_empty() {
        return vec![None; n];
    }
    let paired: HashSet<&str> = pairs
        .iter()
        .flat_map(|p| [p.folder_a.as_str(), p.folder_b.as_str()])
        .collect();

    // Parents have smaller ids than children: a forward sweep paints the
    // subtree of every paired folder.
    let mut forced_green = vec![false; n];
    for (id, node) in tree.nodes.iter().enumerate() {
        forced_green[id] = paired.contains(node.path.as_str())
            || node.parent.is_some_and(|p| forced_green[p as usize]);
    }

    // A reverse sweep is a valid post-order.
    let mut out: Vec<Option<IdenticalStatus>> = vec![None; n];
    for id in (0..n).rev() {
        let node = &tree.nodes[id];
        let status = if forced_green[id] {
            IdenticalStatus::Green
        } else if node.children.is_empty() {
            IdenticalStatus::Red
        } else {
            let mut has_green = false;
            let mut has_yellow = false;
            let mut has_red = node.direct_file_count > 0;
            for &c in &node.children {
                match out[c as usize] {
                    Some(IdenticalStatus::Green) => has_green = true,
                    Some(IdenticalStatus::Yellow) => has_yellow = true,
                    _ => has_red = true,
                }
            }
            if has_yellow || (has_green && has_red) {
                IdenticalStatus::Yellow
            } else if has_green {
                IdenticalStatus::Green
            } else {
                IdenticalStatus::Red
            }
        };
        out[id] = Some(status);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Category, Row, Source};
    use crate::tree::{FolderId, ROOT};

    fn tree(paths: &[&str]) -> FolderTree {
        let rows: Vec<Row> = paths
            .iter()
            .map(|p| {
                Row::new(
                    (*p).to_string(),
                    1,
                    String::new(),
                    Category::Unique,
                    Source::Base,
                    1,
                )
            })
            .collect();
        FolderTree::from_rows(&rows)
    }

    fn pair(a: &str) -> IdenticalFolderPair {
        IdenticalFolderPair {
            folder_a: a.to_string(),
            folder_b: format!("{a}-other"),
            file_count: 1,
            total_size: 1,
        }
    }

    fn status(t: &FolderTree, s: &[Option<IdenticalStatus>], path: &str) -> IdenticalStatus {
        let id: FolderId = t.find(path).unwrap();
        s[id as usize].unwrap()
    }

    #[test]
    fn empty_pairs_give_all_none() {
        let t = tree(&[r"C:\a\f"]);
        let s = compute_statuses(&t, &[]);
        assert_eq!(s.len(), t.nodes.len());
        assert!(s.iter().all(Option::is_none));
    }

    #[test]
    fn paired_folder_and_subtree_are_green() {
        let t = tree(&[r"C:\a\b\f", r"C:\a\c\g", r"C:\z\f"]);
        let s = compute_statuses(&t, &[pair(r"C:\a")]);
        assert_eq!(status(&t, &s, r"C:\a"), IdenticalStatus::Green);
        assert_eq!(status(&t, &s, r"C:\a\b"), IdenticalStatus::Green);
        assert_eq!(status(&t, &s, r"C:\a\c"), IdenticalStatus::Green);
        assert_eq!(status(&t, &s, r"C:\z"), IdenticalStatus::Red);
    }

    #[test]
    fn parent_yellow_when_only_some_children_green() {
        let t = tree(&[r"C:\p\a\f", r"C:\p\b\f"]);
        let s = compute_statuses(&t, &[pair(r"C:\p\a")]);
        assert_eq!(status(&t, &s, r"C:\p\a"), IdenticalStatus::Green);
        assert_eq!(status(&t, &s, r"C:\p\b"), IdenticalStatus::Red);
        assert_eq!(status(&t, &s, r"C:\p"), IdenticalStatus::Yellow);
        assert_eq!(status(&t, &s, "C:"), IdenticalStatus::Yellow);
        assert_eq!(s[ROOT as usize], Some(IdenticalStatus::Yellow));
    }

    #[test]
    fn parent_green_when_all_children_green() {
        let t = tree(&[r"C:\p\a\f", r"C:\p\b\f"]);
        let s = compute_statuses(&t, &[pair(r"C:\p\a"), pair(r"C:\p\b")]);
        assert_eq!(status(&t, &s, r"C:\p"), IdenticalStatus::Green);
        assert_eq!(status(&t, &s, "C:"), IdenticalStatus::Green);
        assert_eq!(s[ROOT as usize], Some(IdenticalStatus::Green));
    }

    #[test]
    fn loose_files_downgrade_green_parent_to_yellow() {
        let t = tree(&[r"C:\p\loose.txt", r"C:\p\a\f"]);
        let s = compute_statuses(&t, &[pair(r"C:\p\a")]);
        assert_eq!(status(&t, &s, r"C:\p"), IdenticalStatus::Yellow);
    }

    #[test]
    fn paired_folder_with_direct_files_stays_green() {
        let t = tree(&[r"C:\p\loose.txt", r"C:\p\a\f"]);
        let s = compute_statuses(&t, &[pair(r"C:\p")]);
        assert_eq!(status(&t, &s, r"C:\p"), IdenticalStatus::Green);
        assert_eq!(status(&t, &s, r"C:\p\a"), IdenticalStatus::Green);
    }

    #[test]
    fn red_when_nothing_matches() {
        let t = tree(&[r"C:\a\f", r"C:\b\f"]);
        let s = compute_statuses(&t, &[pair(r"D:\elsewhere")]);
        for p in ["C:", r"C:\a", r"C:\b"] {
            assert_eq!(status(&t, &s, p), IdenticalStatus::Red);
        }
        assert_eq!(s[ROOT as usize], Some(IdenticalStatus::Red));
    }

    #[test]
    fn second_side_folder_is_also_painted() {
        let t = tree(&[r"C:\a\f", r"D:\x-other\f"]);
        let s = compute_statuses(&t, &[pair(r"C:\a")]);
        assert_eq!(status(&t, &s, r"D:\x-other"), IdenticalStatus::Red);
        let t2 = tree(&[r"C:\a\f", r"C:\a-other\f"]);
        let s2 = compute_statuses(&t2, &[pair(r"C:\a")]);
        assert_eq!(status(&t2, &s2, r"C:\a-other"), IdenticalStatus::Green);
    }

    #[test]
    fn tooltips_and_rank() {
        assert_eq!(
            IdenticalStatus::Green.tooltip(),
            "Entire subtree has an identical match elsewhere"
        );
        assert_eq!(
            IdenticalStatus::Yellow.tooltip(),
            "Some descendants have an identical match elsewhere"
        );
        assert_eq!(
            IdenticalStatus::Red.tooltip(),
            "No identical match in this subtree"
        );
        assert!(IdenticalStatus::Green.rank() < IdenticalStatus::Yellow.rank());
        assert!(IdenticalStatus::Yellow.rank() < IdenticalStatus::Red.rank());
    }
}
