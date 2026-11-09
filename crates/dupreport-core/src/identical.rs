//! Identical-folders detector (§5.8).
//!
//! A Base folder `F` and a Second folder `G` are *identical* when both
//! subtrees have the same shape (same file names, same subfolder names),
//! every Base file in `F` is paired (as `Duplicate`/`Moved`) with the
//! same-named file in `G`, and neither subtree contains a `Unique`,
//! `Missing`, `New` or `Changed` row.
//!
//! # Known limitation
//! Subfolders are matched by **name**, so a renamed subfolder whose files
//! are all `Moved` is not detected as identical.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::model::{Category, IdenticalFolderPair, Row, RowIdx, Source};
use crate::path::{parent_of, split_folder_and_name};

/// Files and subfolders of one folder for one source.
#[derive(Default)]
struct Dir {
    /// File name -> row.
    files: BTreeMap<String, RowIdx>,
    /// Subfolder name -> full child path.
    subfolders: BTreeMap<String, String>,
}

type DirTree = HashMap<String, Dir>;

/// Register `folder` and all its ancestors, linking each to its parent.
///
/// Invariant: every folder in `tree` is already linked to all of its
/// ancestors, so the walk can stop at the first ancestor that existed
/// *before* this call (not at one it just created itself).
fn register_folder(tree: &mut DirTree, folder: &str) {
    if tree.contains_key(folder) {
        return;
    }
    tree.insert(folder.to_string(), Dir::default());
    let mut cur = folder;
    while let Some(parent) = parent_of(cur) {
        let parent_existed = tree.contains_key(parent);
        let name = split_folder_and_name(cur).1;
        tree.entry(parent.to_string())
            .or_default()
            .subfolders
            .insert(name.to_string(), cur.to_string());
        if parent_existed {
            return;
        }
        cur = parent;
    }
}

fn build_tree(rows: &[Row], source: Source) -> DirTree {
    let mut tree = DirTree::new();
    for (i, row) in rows.iter().enumerate() {
        if row.source != source {
            continue;
        }
        register_folder(&mut tree, row.folder());
        if let Some(dir) = tree.get_mut(row.folder()) {
            dir.files.insert(row.filename().to_string(), i as RowIdx);
        }
    }
    tree
}

/// Folders (and all their ancestors) that directly contain a row of a
/// category that disqualifies identity.
fn forbidden_folders(rows: &[Row], source: Source) -> HashSet<&str> {
    let mut set = HashSet::new();
    for row in rows.iter().filter(|r| {
        r.source == source
            && matches!(
                r.category,
                Category::Unique | Category::Missing | Category::New | Category::Changed
            )
    }) {
        let mut cur = Some(row.folder());
        while let Some(f) = cur {
            if !set.insert(f) {
                break;
            }
            cur = parent_of(f);
        }
    }
    set
}

/// Base path -> Second path for the first Base and first Second row of each
/// `Duplicate`/`Moved` group.
fn pair_map(rows: &[Row]) -> HashMap<&str, &str> {
    let mut groups: HashMap<i32, (Option<&str>, Option<&str>)> = HashMap::new();
    for row in rows {
        if !matches!(row.category, Category::Duplicate | Category::Moved) {
            continue;
        }
        let slot = groups.entry(row.group_id).or_default();
        let target = match row.source {
            Source::Base => &mut slot.0,
            Source::Second => &mut slot.1,
        };
        target.get_or_insert(row.path.as_str());
    }
    groups
        .into_values()
        .filter_map(|(b, s)| Some((b?, s?)))
        .collect()
}

/// Bottom-up `(file_count, total_size)` of every Base folder subtree.
fn base_subtree_stats(rows: &[Row], base: &DirTree) -> HashMap<String, (usize, i64)> {
    let mut stats: HashMap<String, (usize, i64)> = base
        .iter()
        .map(|(path, dir)| (path.clone(), (dir.files.len(), 0)))
        .collect();
    for row in rows.iter().filter(|r| r.source == Source::Base) {
        if let Some(s) = stats.get_mut(row.folder()) {
            s.1 += row.size;
        }
    }
    // A child path is always longer than its parent's.
    let mut paths: Vec<&String> = base.keys().collect();
    paths.sort_unstable_by_key(|p| Reverse(p.len()));
    for path in paths {
        if let Some(parent) = parent_of(path) {
            let (count, size) = stats[path];
            if let Some(p) = stats.get_mut(parent) {
                p.0 += count;
                p.1 += size;
            }
        }
    }
    stats
}

struct Detector<'a> {
    rows: &'a [Row],
    base: &'a DirTree,
    second: &'a DirTree,
    pairs: &'a HashMap<&'a str, &'a str>,
    forbidden_base: &'a HashSet<&'a str>,
    forbidden_second: &'a HashSet<&'a str>,
    memo: HashMap<(String, String), bool>,
    /// Memoised [`Self::first_partner`] per Base folder, so the candidate
    /// search costs O(folders) in total instead of one subtree DFS per folder.
    first_partner_memo: HashMap<&'a str, Option<(usize, &'a str)>>,
}

impl<'a> Detector<'a> {
    /// Second-side folder implied by the first paired file in `f`'s subtree.
    fn candidate(&mut self, f: &'a str) -> Option<String> {
        let (depth, partner_folder) = self.first_partner(f)?;
        let mut g = partner_folder;
        for _ in 0..depth {
            g = parent_of(g)?;
        }
        Some(g.to_string())
    }

    /// DFS (files first, then subfolders, both by name) for the first file
    /// with a partner; returns its depth below `f` and its partner's folder.
    fn first_partner(&mut self, f: &'a str) -> Option<(usize, &'a str)> {
        if let Some(&hit) = self.first_partner_memo.get(f) {
            return hit;
        }
        let base: &'a DirTree = self.base;
        let pairs: &'a HashMap<&'a str, &'a str> = self.pairs;
        let rows: &'a [Row] = self.rows;
        let result = base.get(f).and_then(|dir| {
            dir.files
                .values()
                .find_map(|&idx| pairs.get(rows[idx as usize].path.as_str()))
                .map(|&partner| (0, split_folder_and_name(partner).0))
                .or_else(|| {
                    dir.subfolders
                        .values()
                        .find_map(|child| self.first_partner(child))
                        .map(|(depth, partner)| (depth + 1, partner))
                })
        });
        self.first_partner_memo.insert(f, result);
        result
    }

    fn is_identical(&mut self, f: &str, g: &str) -> bool {
        if f.is_empty() || g.is_empty() {
            return false;
        }
        let key = (f.to_string(), g.to_string());
        if let Some(&cached) = self.memo.get(&key) {
            return cached;
        }
        let result = self.compute_identical(f, g);
        self.memo.insert(key, result);
        result
    }

    fn compute_identical(&mut self, f: &str, g: &str) -> bool {
        if self.forbidden_base.contains(f) || self.forbidden_second.contains(g) {
            return false;
        }
        let (Some(fd), Some(gd)) = (self.base.get(f), self.second.get(g)) else {
            return false;
        };
        if fd.files.len() != gd.files.len() || fd.subfolders.len() != gd.subfolders.len() {
            return false;
        }
        for (name, &fi) in &fd.files {
            let Some(&gi) = gd.files.get(name) else {
                return false;
            };
            let f_path = self.rows[fi as usize].path.as_str();
            let g_path = self.rows[gi as usize].path.as_str();
            if self.pairs.get(f_path) != Some(&g_path) {
                return false;
            }
        }
        let children: Vec<(&String, Option<&String>)> = fd
            .subfolders
            .iter()
            .map(|(name, fc)| (fc, gd.subfolders.get(name)))
            .collect();
        children.into_iter().all(|(fc, gc)| match gc {
            Some(gc) => self.is_identical(fc, gc),
            None => false,
        })
    }
}

/// Find all maximal identical folder pairs, sorted by total size descending
/// (ties by Base folder path).
///
/// Subtree sizes are computed bottom-up once, so the cost is roughly linear
/// in the number of rows.
#[must_use]
pub fn find_identical_folders(rows: &[Row]) -> Vec<IdenticalFolderPair> {
    let pairs = pair_map(rows);
    // Without a single Base/Second pair (e.g. a single-source report) no
    // folder can have a candidate partner.
    if pairs.is_empty() {
        return Vec::new();
    }
    let base = build_tree(rows, Source::Base);
    let second = build_tree(rows, Source::Second);
    let forbidden_base = forbidden_folders(rows, Source::Base);
    let forbidden_second = forbidden_folders(rows, Source::Second);
    let stats = base_subtree_stats(rows, &base);

    let mut det = Detector {
        rows,
        base: &base,
        second: &second,
        pairs: &pairs,
        forbidden_base: &forbidden_base,
        forbidden_second: &forbidden_second,
        memo: HashMap::new(),
        first_partner_memo: HashMap::new(),
    };

    let mut folders: Vec<&String> = base.keys().collect();
    folders.sort_unstable();

    let mut found: Vec<(String, String)> = Vec::new();
    for f in folders {
        // A forbidden folder can never be identical; skip the DFS.
        if f.is_empty() || forbidden_base.contains(f.as_str()) {
            continue;
        }
        let Some(g) = det.candidate(f) else {
            continue;
        };
        if det.is_identical(f, &g) {
            found.push((f.clone(), g));
        }
    }

    // Keep only maximal pairs: drop (A, B) when the lockstep ancestors
    // (parent^k(A), parent^k(B)) are also a pair.
    let found_set: HashSet<(&str, &str)> = found
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut result: Vec<IdenticalFolderPair> = found
        .iter()
        .filter(|(a, b)| {
            let (mut pa, mut pb) = (a.as_str(), b.as_str());
            while let (Some(na), Some(nb)) = (parent_of(pa), parent_of(pb)) {
                if found_set.contains(&(na, nb)) {
                    return false;
                }
                (pa, pb) = (na, nb);
            }
            true
        })
        .map(|(a, b)| {
            let (file_count, total_size) = stats.get(a).copied().unwrap_or_default();
            IdenticalFolderPair {
                folder_a: a.clone(),
                folder_b: b.clone(),
                file_count,
                total_size,
            }
        })
        .collect();

    result.sort_by(|x, y| {
        y.total_size
            .cmp(&x.total_size)
            .then_with(|| x.folder_a.cmp(&y.folder_a))
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;
    use std::path::PathBuf;

    fn row(path: &str, size: i64, cat: Category, src: Source, group: i32) -> Row {
        Row::new(path.to_string(), size, "H".to_string(), cat, src, group)
    }

    fn dup_pair(rows: &mut Vec<Row>, base: &str, second: &str, size: i64, group: i32) {
        rows.push(row(base, size, Category::Duplicate, Source::Base, group));
        rows.push(row(
            second,
            size,
            Category::Duplicate,
            Source::Second,
            group,
        ));
    }

    #[test]
    fn simple_identical_pair() {
        let mut rows = Vec::new();
        dup_pair(&mut rows, r"C:\a\b\one.txt", r"D:\x\y\one.txt", 100, 1);
        dup_pair(&mut rows, r"C:\a\b\two.txt", r"D:\x\y\two.txt", 200, 2);
        let pairs = find_identical_folders(&rows);
        assert_eq!(
            pairs,
            vec![IdenticalFolderPair {
                folder_a: r"C:\a\b".into(),
                folder_b: r"D:\x\y".into(),
                file_count: 2,
                total_size: 300,
            }]
        );
    }

    #[test]
    fn single_source_report_has_no_pairs() {
        // Same-DB duplicates: every row is Base, so no Base/Second pair exists.
        let rows = vec![
            row(r"C:\a\one.txt", 10, Category::Duplicate, Source::Base, 1),
            row(r"C:\b\one.txt", 10, Category::Duplicate, Source::Base, 1),
        ];
        assert!(find_identical_folders(&rows).is_empty());
    }

    #[test]
    fn deep_chain_finds_maximal_pair() {
        // 300 nested folders on each side. Regression test: every ancestor
        // must be registered (an early exit once linked only the immediate
        // parent, so pairs above it were never found), and the candidate
        // search must stay cheap on deep trees (memoised).
        let depth = 300;
        let chain = |root: &str| (0..depth).fold(root.to_string(), |acc, i| format!(r"{acc}\d{i}"));
        let (base, second) = (chain("C:"), chain("D:"));
        let mut rows = Vec::new();
        dup_pair(
            &mut rows,
            &format!(r"{base}\f.txt"),
            &format!(r"{second}\f.txt"),
            5,
            1,
        );
        // A Unique file on the Base drive root keeps `C:` itself from pairing,
        // so the maximal pair is the top of the chain.
        rows.push(row(r"C:\other.txt", 1, Category::Unique, Source::Base, 2));
        let pairs = find_identical_folders(&rows);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].folder_a, r"C:\d0");
        assert_eq!(pairs[0].folder_b, r"D:\d0");
        assert_eq!((pairs[0].file_count, pairs[0].total_size), (1, 5));
    }

    #[test]
    fn rejected_when_missing_row_in_subtree() {
        let mut rows = Vec::new();
        dup_pair(&mut rows, r"C:\a\b\one.txt", r"D:\x\y\one.txt", 100, 1);
        rows.push(row(
            r"C:\a\b\gone.txt",
            5,
            Category::Missing,
            Source::Base,
            2,
        ));
        assert!(find_identical_folders(&rows).is_empty());
    }

    #[test]
    fn rejected_when_missing_row_in_nested_subfolder() {
        let mut rows = Vec::new();
        dup_pair(&mut rows, r"C:\a\one.txt", r"D:\x\one.txt", 100, 1);
        rows.push(row(
            r"C:\a\sub\gone.txt",
            5,
            Category::Missing,
            Source::Base,
            2,
        ));
        assert!(find_identical_folders(&rows).is_empty());
    }

    #[test]
    fn rejected_when_second_has_extra_file() {
        let mut rows = Vec::new();
        dup_pair(&mut rows, r"C:\a\one.txt", r"D:\x\one.txt", 100, 1);
        rows.push(row(r"D:\x\extra.txt", 5, Category::New, Source::Second, 2));
        assert!(find_identical_folders(&rows).is_empty());
    }

    #[test]
    fn maximal_pair_preferred_over_subfolder_pair() {
        let mut rows = Vec::new();
        dup_pair(&mut rows, r"C:\a\b\f1.txt", r"D:\x\b\f1.txt", 10, 1);
        dup_pair(&mut rows, r"C:\a\c\f2.txt", r"D:\x\c\f2.txt", 20, 2);
        let pairs = find_identical_folders(&rows);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].folder_a, r"C:\a");
        assert_eq!(pairs[0].folder_b, r"D:\x");
        assert_eq!(pairs[0].file_count, 2);
        assert_eq!(pairs[0].total_size, 30);
    }

    #[test]
    fn sorted_by_total_size_descending() {
        let mut rows = Vec::new();
        dup_pair(&mut rows, r"C:\small\f.txt", r"D:\small2\f.txt", 1, 1);
        dup_pair(&mut rows, r"C:\big\f.txt", r"D:\big2\f.txt", 1000, 2);
        let pairs = find_identical_folders(&rows);
        let sizes: Vec<_> = pairs.iter().map(|p| p.total_size).collect();
        assert_eq!(sizes, [1000, 1]);
    }

    #[test]
    fn same_db_duplicates_yield_no_pairs() {
        let rows = vec![
            row(r"C:\a\f.txt", 1, Category::Duplicate, Source::Base, 1),
            row(r"C:\b\f.txt", 1, Category::Duplicate, Source::Base, 1),
        ];
        assert!(find_identical_folders(&rows).is_empty());
    }

    #[test]
    fn moved_files_count_as_partners() {
        let rows = vec![
            row(r"C:\a\f.txt", 7, Category::Moved, Source::Base, 1),
            row(r"D:\x\f.txt", 7, Category::Moved, Source::Second, 1),
        ];
        let pairs = find_identical_folders(&rows);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].total_size, 7);
    }

    #[test]
    fn renamed_subfolder_is_not_detected() {
        // Documented limitation: child names must match.
        let mut rows = Vec::new();
        dup_pair(&mut rows, r"C:\a\old\f.txt", r"D:\x\new\f.txt", 1, 1);
        dup_pair(&mut rows, r"C:\a\g.txt", r"D:\x\g.txt", 1, 2);
        let pairs = find_identical_folders(&rows);
        assert!(pairs.iter().all(|p| p.folder_a != r"C:\a"));
    }

    #[test]
    fn empty_input_is_fine() {
        assert!(find_identical_folders(&[]).is_empty());
    }

    #[test]
    fn whole_lot_fixture_runs_without_panic() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/Demo DiffFindDupsTheWholeLot dupReport.csv");
        let rows = parse_file(&path, |_| {}).unwrap();
        let pairs = find_identical_folders(&rows);
        let root = r"O:\DatenP\Programmieren\repos\myDupFinder\testdata";
        let summary: Vec<(String, String, usize, i64)> = pairs
            .iter()
            .map(|p| {
                (
                    p.folder_a.clone(),
                    p.folder_b.clone(),
                    p.file_count,
                    p.total_size,
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                (
                    format!(r"{root}\filesBase\demo\Check\CheckDetail"),
                    format!(r"{root}\filesNew\demo\Check\CheckDetail"),
                    4,
                    7562
                ),
                (
                    format!(r"{root}\filesBase\demo\FindDups\Helper"),
                    format!(r"{root}\filesNew\demo\FindDups\Helper"),
                    3,
                    6320
                ),
            ]
        );
        for w in pairs.windows(2) {
            assert!(w[0].total_size >= w[1].total_size);
        }
        for p in &pairs {
            assert!(p.file_count > 0);
        }
    }
}
