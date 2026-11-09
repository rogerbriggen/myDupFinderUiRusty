//! Row filtering over a [`Report`] (§5.7).

use crate::model::{Category, RowIdx, Source};
use crate::report::Report;

/// Minimum hash-query length (in chars) for prefix matching; shorter
/// queries must match the whole hash.
pub const HASH_PREFIX_MIN_LEN: usize = 8;

/// AND-combined filter criteria. `Default` matches every row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowQuery {
    /// Folder to show; `None` or `""` means no folder filter.
    pub folder: Option<String>,
    /// Include rows of descendant folders.
    pub include_descendants: bool,
    /// Case-insensitive substring of the full path.
    pub text: Option<String>,
    /// Case-insensitive hash: prefix match when at least
    /// [`HASH_PREFIX_MIN_LEN`] chars, otherwise exact.
    pub hash: Option<String>,
    /// `None` = no filter; `Some(empty)` matches nothing.
    pub categories: Option<Vec<Category>>,
    /// `None` = no filter; `Some(empty)` matches nothing.
    pub sources: Option<Vec<Source>>,
}

fn non_empty_trimmed(s: Option<&String>) -> Option<&str> {
    s.map(|s| s.trim()).filter(|s| !s.is_empty())
}

/// Indexes of the rows matching `q`, in file order.
///
/// A folder that is not a folder of the report (it has no node in the
/// tree) matches nothing.
#[must_use]
pub fn filter(report: &Report, q: &RowQuery) -> Vec<RowIdx> {
    if q.categories.as_ref().is_some_and(Vec::is_empty)
        || q.sources.as_ref().is_some_and(Vec::is_empty)
    {
        return Vec::new();
    }

    let text = non_empty_trimmed(q.text.as_ref()).map(str::to_lowercase);
    let hash = non_empty_trimmed(q.hash.as_ref());
    let hash_prefix = hash.is_some_and(|h| h.chars().count() >= HASH_PREFIX_MIN_LEN);
    let hash = hash.map(str::to_ascii_uppercase);

    let matches = |idx: RowIdx| -> bool {
        let row = &report.rows[idx as usize];
        if let Some(cats) = &q.categories
            && !cats.contains(&row.category)
        {
            return false;
        }
        if let Some(srcs) = &q.sources
            && !srcs.contains(&row.source)
        {
            return false;
        }
        if let Some(h) = &hash {
            let ok = if hash_prefix {
                row.hash.len() >= h.len()
                    && row.hash.as_bytes()[..h.len()].eq_ignore_ascii_case(h.as_bytes())
            } else {
                row.hash.eq_ignore_ascii_case(h)
            };
            if !ok {
                return false;
            }
        }
        if let Some(t) = &text
            && !row.path.to_lowercase().contains(t.as_str())
        {
            return false;
        }
        true
    };

    let folder = q.folder.as_deref().filter(|f| !f.is_empty());
    let Some(folder) = folder else {
        return (0..report.rows.len() as RowIdx)
            .filter(|&i| matches(i))
            .collect();
    };
    let Some(id) = report.tree.find(folder) else {
        return Vec::new();
    };
    if q.include_descendants {
        let mut candidates: Vec<RowIdx> = report
            .tree
            .subtree_ids(id)
            .into_iter()
            .flat_map(|f| report.rows_by_folder[f as usize].iter().copied())
            .collect();
        // Restore file order after concatenating per-folder lists.
        candidates.sort_unstable();
        candidates.retain(|&i| matches(i));
        candidates
    } else {
        report.rows_by_folder[id as usize]
            .iter()
            .copied()
            .filter(|&i| matches(i))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Row;
    use std::path::PathBuf;

    fn row(path: &str, hash: &str, cat: Category, src: Source) -> Row {
        Row::new(path.to_string(), 1, hash.to_string(), cat, src, 1)
    }

    fn report() -> Report {
        Report::from_rows(
            "t",
            PathBuf::new(),
            vec![
                row(
                    r"C:\a\one.txt",
                    "ABCDEF0123456789",
                    Category::Duplicate,
                    Source::Base,
                ),
                row(
                    r"C:\a\b\Two.TXT",
                    "1111111111111111",
                    Category::Moved,
                    Source::Base,
                ),
                row(r"C:\ab\three.txt", "ABCD", Category::Unique, Source::Base),
                row(
                    r"D:\a\one.txt",
                    "abcdef0123456789",
                    Category::Duplicate,
                    Source::Second,
                ),
                row(r"D:\z\new.txt", "", Category::New, Source::Second),
                row("bare.txt", "9999", Category::Missing, Source::Base),
            ],
        )
    }

    fn q() -> RowQuery {
        RowQuery::default()
    }

    #[test]
    fn default_matches_everything_in_order() {
        assert_eq!(filter(&report(), &q()), [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn exact_folder_is_immediate_children_only() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                folder: Some(r"C:\a".into()),
                ..q()
            },
        );
        assert_eq!(res, [0]);
    }

    #[test]
    fn folder_with_descendants_keeps_file_order_and_excludes_siblings() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                folder: Some(r"C:\a".into()),
                include_descendants: true,
                ..q()
            },
        );
        // `C:\ab\three.txt` (idx 2) must not match folder `C:\a`.
        assert_eq!(res, [0, 1]);
    }

    #[test]
    fn empty_folder_means_no_filter() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                folder: Some(String::new()),
                ..q()
            },
        );
        assert_eq!(res.len(), 6);
    }

    #[test]
    fn root_with_descendants_equals_everything() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                folder: Some("C:".into()),
                include_descendants: true,
                ..q()
            },
        );
        assert_eq!(res, [0, 1, 2]);
    }

    #[test]
    fn unknown_folder_matches_nothing() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                folder: Some(r"C:\nope".into()),
                include_descendants: true,
                ..q()
            },
        );
        assert!(res.is_empty());
    }

    #[test]
    fn category_filter() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                categories: Some(vec![Category::Duplicate, Category::New]),
                ..q()
            },
        );
        assert_eq!(res, [0, 3, 4]);
    }

    #[test]
    fn text_is_case_insensitive_over_full_path() {
        let r = report();
        let by_name = filter(
            &r,
            &RowQuery {
                text: Some("  two.txt ".into()),
                ..q()
            },
        );
        assert_eq!(by_name, [1]);
        let by_folder = filter(
            &r,
            &RowQuery {
                text: Some(r"c:\A\B".into()),
                ..q()
            },
        );
        assert_eq!(by_folder, [1]);
    }

    #[test]
    fn text_uses_unicode_lowercase() {
        let r = Report::from_rows(
            "t",
            PathBuf::new(),
            vec![row("C:\\Zürich\\Ä.txt", "", Category::Unique, Source::Base)],
        );
        let res = filter(
            &r,
            &RowQuery {
                text: Some("ä.TXT".into()),
                ..q()
            },
        );
        assert_eq!(res, [0]);
    }

    #[test]
    fn hash_prefix_with_eight_or_more_chars() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                hash: Some("abcdef01".into()),
                ..q()
            },
        );
        assert_eq!(res, [0, 3]);
    }

    #[test]
    fn short_hash_must_match_exactly() {
        let r = report();
        // 4 chars: exact only, so the 16-char hashes starting with ABCD do not match.
        let res = filter(
            &r,
            &RowQuery {
                hash: Some("abcd".into()),
                ..q()
            },
        );
        assert_eq!(res, [2]);
    }

    #[test]
    fn blank_hash_and_text_are_ignored() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                hash: Some("   ".into()),
                text: Some(String::new()),
                ..q()
            },
        );
        assert_eq!(res.len(), 6);
    }

    #[test]
    fn source_filter() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                sources: Some(vec![Source::Second]),
                ..q()
            },
        );
        assert_eq!(res, [3, 4]);
    }

    #[test]
    fn empty_category_list_matches_nothing() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                categories: Some(vec![]),
                ..q()
            },
        );
        assert!(res.is_empty());
    }

    #[test]
    fn empty_source_list_matches_nothing() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                sources: Some(vec![]),
                ..q()
            },
        );
        assert!(res.is_empty());
    }

    #[test]
    fn none_category_matches_everything() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                categories: None,
                ..q()
            },
        );
        assert_eq!(res.len(), 6);
    }

    #[test]
    fn filters_are_and_combined() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                folder: Some("D:".into()),
                include_descendants: true,
                categories: Some(vec![Category::Duplicate]),
                sources: Some(vec![Source::Second]),
                text: Some("one".into()),
                hash: Some("ABCDEF0123".into()),
            },
        );
        assert_eq!(res, [3]);
    }

    #[test]
    fn bare_filename_matches_with_descendants_from_root_only() {
        let r = report();
        let res = filter(
            &r,
            &RowQuery {
                text: Some("bare".into()),
                ..q()
            },
        );
        assert_eq!(res, [5]);
    }
}
