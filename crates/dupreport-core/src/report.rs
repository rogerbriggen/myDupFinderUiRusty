//! A loaded report with precomputed indexes (§5.5).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use smallvec::SmallVec;

use crate::error::ReportError;
use crate::model::{Row, RowIdx, Source};
use crate::parser::{ParseProgress, job_name_from_path, parse_file};
use crate::tree::FolderTree;

/// All rows of one `dupReport.csv` plus the indexes built once after parsing.
#[derive(Debug, Clone)]
pub struct Report {
    /// Job name derived from the file name.
    pub job_name: String,
    /// Where the report was loaded from.
    pub source_path: PathBuf,
    /// Rows in file order.
    pub rows: Vec<Row>,
    /// True when any row comes from the Second database.
    pub has_second_source: bool,
    /// Folder tree over all rows.
    pub tree: FolderTree,
    /// Direct rows of each folder (indexed by `FolderId`), in file order.
    pub rows_by_folder: Vec<Vec<RowIdx>>,
    /// Rows sharing a `GroupId`, in file order.
    pub rows_by_group: HashMap<i32, SmallVec<[RowIdx; 2]>>,
}

impl Report {
    /// Parse the file at `path` and build all indexes.
    ///
    /// # Errors
    /// Any [`ReportError`] from the parser.
    pub fn load(path: &Path, on_progress: impl FnMut(ParseProgress)) -> Result<Self, ReportError> {
        let rows = parse_file(path, on_progress)?;
        Ok(Self::from_rows(
            job_name_from_path(path),
            path.to_path_buf(),
            rows,
        ))
    }

    /// Build a report (and its indexes) from already parsed rows.
    #[must_use]
    pub fn from_rows(job_name: impl Into<String>, source_path: PathBuf, rows: Vec<Row>) -> Self {
        let has_second_source = rows.iter().any(|r| r.source == Source::Second);
        let (tree, rows_by_folder) = FolderTree::build(&rows);
        let mut rows_by_group: HashMap<i32, SmallVec<[RowIdx; 2]>> = HashMap::new();
        for (i, row) in rows.iter().enumerate() {
            rows_by_group
                .entry(row.group_id)
                .or_default()
                .push(i as RowIdx);
        }
        Self {
            job_name: job_name.into(),
            source_path,
            rows,
            has_second_source,
            tree,
            rows_by_folder,
            rows_by_group,
        }
    }

    /// Initial source filter: `{Base, Second}` for a two-source report,
    /// `{Base}` for a single-DB report (so a stale `{Second}` filter can
    /// never hide every row). Returned in canonical order.
    #[must_use]
    pub fn default_sources(&self) -> Vec<Source> {
        if self.has_second_source {
            vec![Source::Base, Source::Second]
        } else {
            vec![Source::Base]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Category;
    use crate::tree::ROOT;

    fn row(path: &str, cat: Category, src: Source, group: i32) -> Row {
        Row::new(path.to_string(), 10, String::new(), cat, src, group)
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    #[test]
    fn default_sources_two_source() {
        let r = Report::from_rows(
            "j",
            PathBuf::new(),
            vec![
                row(r"C:\a\f", Category::Duplicate, Source::Base, 1),
                row(r"D:\a\f", Category::Duplicate, Source::Second, 1),
            ],
        );
        assert!(r.has_second_source);
        assert_eq!(r.default_sources(), [Source::Base, Source::Second]);
    }

    #[test]
    fn default_sources_single_db() {
        let r = Report::from_rows(
            "j",
            PathBuf::new(),
            vec![row(r"C:\a\f", Category::Duplicate, Source::Base, 1)],
        );
        assert!(!r.has_second_source);
        assert_eq!(r.default_sources(), [Source::Base]);
    }

    #[test]
    fn groups_are_indexed() {
        let r = Report::from_rows(
            "j",
            PathBuf::new(),
            vec![
                row(r"C:\a\f", Category::Duplicate, Source::Base, 1),
                row(r"C:\b\g", Category::Unique, Source::Base, 2),
                row(r"D:\a\f", Category::Duplicate, Source::Second, 1),
            ],
        );
        assert_eq!(r.rows_by_group[&1].as_slice(), [0, 2]);
        assert_eq!(r.rows_by_group[&2].as_slice(), [1]);
    }

    #[test]
    fn loads_whole_lot_fixture() {
        let path = fixture("Demo DiffFindDupsTheWholeLot dupReport.csv");
        let mut progress_calls = 0;
        let r = Report::load(&path, |_| progress_calls += 1).unwrap();
        assert_eq!(r.job_name, "Demo DiffFindDupsTheWholeLot");
        assert_eq!(r.rows.len(), 64);
        assert!(r.has_second_source);
        assert_eq!(r.default_sources(), [Source::Base, Source::Second]);
        assert_eq!(r.tree.node(ROOT).total_file_count, 64);
        assert_eq!(
            r.rows_by_folder.iter().map(Vec::len).sum::<usize>(),
            r.rows.len()
        );
        assert_eq!(progress_calls, 1);
    }

    #[test]
    fn loads_only_dups_fixture_as_single_db() {
        let path = fixture("Demo DiffFindOnlyDups dupReport.csv");
        let r = Report::load(&path, |_| {}).unwrap();
        assert_eq!(r.rows.len(), 25);
        assert!(!r.has_second_source);
        assert_eq!(r.default_sources(), [Source::Base]);
    }

    #[test]
    fn load_missing_file_is_io_error() {
        let err = Report::load(Path::new("definitely/not/here.csv"), |_| {}).unwrap_err();
        assert_eq!(err.kind(), "Io");
    }
}
