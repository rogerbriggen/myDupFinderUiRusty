//! Streaming parser for `dupReport.csv` files (§4.5, §5.4).

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::error::ReportError;
use crate::model::{Category, Row, Source};

/// Exact header line of a dup report.
pub const EXPECTED_HEADER: &str = "FilenameAndPath;FileSize;FileSha512Hash;Category;Source;GroupId";
/// First-line prefix of the (unsupported) check report format.
pub const CHECK_REPORT_MARKER: &str = "# myDupFinder check report";
/// Number of rows between progress callbacks.
pub const PROGRESS_INTERVAL: usize = 5_000;

const FIELD_COUNT: usize = 6;
const BUFFER_CAPACITY: usize = 64 * 1024;
const JOB_SUFFIX: &str = " dupReport";

/// Snapshot passed to the progress callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseProgress {
    /// Data rows parsed so far.
    pub rows_read: usize,
    /// Bytes consumed from the input so far (monotonic).
    pub bytes_read: u64,
    /// Total input size, when known.
    pub total_bytes: Option<u64>,
}

/// Parse the report at `path`, streaming; `total_bytes` is the file size.
///
/// # Errors
/// See [`parse_reader`]; additionally fails with [`ReportError::Io`] when
/// the file cannot be opened.
pub fn parse_file(
    path: &Path,
    on_progress: impl FnMut(ParseProgress),
) -> Result<Vec<Row>, ReportError> {
    let file = File::open(path)?;
    let total = file.metadata().ok().map(|m| m.len());
    parse_reader(file, total, on_progress)
}

/// Parse a report from any reader.
///
/// `on_progress` fires every [`PROGRESS_INTERVAL`] rows and once after the
/// last row.
///
/// # Errors
/// - [`ReportError::Unsupported`] for a check report,
/// - [`ReportError::InvalidReport`] for a wrong header or an invalid row,
/// - [`ReportError::Csv`] / [`ReportError::Io`] for lower-level failures.
pub fn parse_reader<R: Read>(
    reader: R,
    total_bytes: Option<u64>,
    mut on_progress: impl FnMut(ParseProgress),
) -> Result<Vec<Row>, ReportError> {
    let mut buf = BufReader::with_capacity(BUFFER_CAPACITY, reader);

    // Peek without consuming so the CSV reader still sees the first line.
    if buf.fill_buf()?.starts_with(CHECK_REPORT_MARKER.as_bytes()) {
        return Err(ReportError::Unsupported(
            "myDupFinder check reports are not supported yet.".to_string(),
        ));
    }

    // `flexible(true)` so a wrong field count surfaces as our own message.
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b';')
        .quote(b'"')
        .has_headers(true)
        .flexible(true)
        .from_reader(buf);

    let header = rdr.headers()?.iter().collect::<Vec<_>>().join(";");
    if header != EXPECTED_HEADER {
        return Err(ReportError::InvalidReport(format!(
            "Unexpected CSV header: '{header}'"
        )));
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut record = csv::StringRecord::new();
    while rdr.read_record(&mut record)? {
        let n = rows.len() + 2;
        rows.push(row_from_record(&record, n)?);
        if rows.len().is_multiple_of(PROGRESS_INTERVAL) {
            on_progress(ParseProgress {
                rows_read: rows.len(),
                bytes_read: rdr.position().byte(),
                total_bytes,
            });
        }
    }
    on_progress(ParseProgress {
        rows_read: rows.len(),
        bytes_read: rdr.position().byte(),
        total_bytes,
    });
    Ok(rows)
}

fn row_from_record(rec: &csv::StringRecord, n: usize) -> Result<Row, ReportError> {
    let invalid = |msg: String| ReportError::InvalidReport(msg);
    if rec.len() != FIELD_COUNT {
        return Err(invalid(format!(
            "Row {n} has {} fields, expected {FIELD_COUNT}",
            rec.len()
        )));
    }
    let size = rec[1]
        .parse::<i64>()
        .map_err(|e| invalid(format!("Row {n}: invalid FileSize: {e}")))?;
    let category = Category::parse(&rec[3])
        .ok_or_else(|| invalid(format!("Row {n}: unknown Category '{}'", &rec[3])))?;
    let source = Source::parse(&rec[4])
        .ok_or_else(|| invalid(format!("Row {n}: unknown Source '{}'", &rec[4])))?;
    let group_id = rec[5]
        .parse::<i32>()
        .map_err(|e| invalid(format!("Row {n}: invalid GroupId: {e}")))?;
    Ok(Row::new(
        rec[0].to_string(),
        size,
        rec[2].to_string(),
        category,
        source,
        group_id,
    ))
}

/// Job name from a report path: the file stem without a trailing
/// `" dupReport"`.
#[must_use]
pub fn job_name_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match stem.strip_suffix(JOB_SUFFIX) {
        Some(name) => name.to_string(),
        None => stem,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata")
            .join(name)
    }

    fn parse_str(s: &str) -> Result<Vec<Row>, ReportError> {
        parse_reader(Cursor::new(s.as_bytes().to_vec()), None, |_| {})
    }

    fn invalid_message(r: Result<Vec<Row>, ReportError>) -> String {
        match r {
            Err(ReportError::InvalidReport(m)) => m,
            other => panic!("expected InvalidReport, got {other:?}"),
        }
    }

    #[test]
    fn parses_small_report_with_quotes_and_lf() {
        let text = format!(
            "{EXPECTED_HEADER}\n\"C:\\a\\\"\"q\"\".txt\";12;ABCDEF;Duplicate;Base;1\n\"D:\\b.txt\";-1;;New;Second;2\n"
        );
        let rows = parse_str(&text).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].path, "C:\\a\\\"q\".txt");
        assert_eq!(rows[0].size, 12);
        assert_eq!(rows[0].hash, "ABCDEF");
        assert_eq!(rows[0].category, Category::Duplicate);
        assert_eq!(rows[0].source, Source::Base);
        assert_eq!(rows[0].group_id, 1);
        assert_eq!(rows[1].hash, "");
        assert_eq!(rows[1].size, -1);
        assert_eq!(rows[1].category, Category::New);
        assert_eq!(rows[1].source, Source::Second);
    }

    #[test]
    fn parses_crlf() {
        let text = format!("{EXPECTED_HEADER}\r\n\"C:\\a.txt\";1;AB;Unique;Base;3\r\n");
        let rows = parse_str(&text).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "C:\\a.txt");
        assert_eq!(rows[0].group_id, 3);
    }

    #[test]
    fn header_only_gives_no_rows() {
        assert!(parse_str(EXPECTED_HEADER).unwrap().is_empty());
    }

    #[test]
    fn rejects_check_report() {
        let err =
            parse_str("# myDupFinder check report\r\n# Key=Value\r\nCategory,x\r\n").unwrap_err();
        match err {
            ReportError::Unsupported(m) => {
                assert_eq!(m, "myDupFinder check reports are not supported yet.");
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn rejects_wrong_header() {
        let m = invalid_message(parse_str("a;b;c\n1;2;3\n"));
        assert_eq!(m, "Unexpected CSV header: 'a;b;c'");
    }

    #[test]
    fn rejects_empty_input() {
        let m = invalid_message(parse_str(""));
        assert_eq!(m, "Unexpected CSV header: ''");
    }

    #[test]
    fn rejects_unknown_category() {
        let text = format!(
            "{EXPECTED_HEADER}\n\"a\";1;;Duplicate;Base;1\n\"b\";1;;Duplicate;Base;1\n\"c\";1;;Duplicate;Base;1\n\"d\";1;;Duplicate;Base;1\n\"e\";1;;Duplicate;Base;1\n\"f\";1;;Foo;Base;1\n"
        );
        assert_eq!(
            invalid_message(parse_str(&text)),
            "Row 7: unknown Category 'Foo'"
        );
    }

    #[test]
    fn rejects_unknown_source() {
        let text = format!("{EXPECTED_HEADER}\n\"a\";1;;Duplicate;Third;1\n");
        assert_eq!(
            invalid_message(parse_str(&text)),
            "Row 2: unknown Source 'Third'"
        );
    }

    #[test]
    fn rejects_bad_size_and_group() {
        let text = format!("{EXPECTED_HEADER}\n\"a\";x;;Duplicate;Base;1\n");
        assert!(invalid_message(parse_str(&text)).starts_with("Row 2: invalid FileSize: "));
        let text = format!("{EXPECTED_HEADER}\n\"a\";1;;Duplicate;Base;1.5\n");
        assert!(invalid_message(parse_str(&text)).starts_with("Row 2: invalid GroupId: "));
    }

    #[test]
    fn rejects_wrong_field_count() {
        let text = format!("{EXPECTED_HEADER}\n\"a\";1;;Duplicate;Base\n");
        assert_eq!(
            invalid_message(parse_str(&text)),
            "Row 2 has 5 fields, expected 6"
        );
    }

    #[test]
    fn only_dups_fixture_parses() {
        let rows = parse_file(&fixture("Demo DiffFindOnlyDups dupReport.csv"), |_| {}).unwrap();
        assert_eq!(rows.len(), 25);
        assert!(
            rows.iter()
                .all(|r| r.category == Category::Duplicate && r.source == Source::Base)
        );
    }

    #[test]
    fn whole_lot_fixture_parses() {
        let rows = parse_file(
            &fixture("Demo DiffFindDupsTheWholeLot dupReport.csv"),
            |_| {},
        )
        .unwrap();
        assert_eq!(rows.len(), 64);
        let count = |c: Category| rows.iter().filter(|r| r.category == c).count();
        assert_eq!(count(Category::Duplicate), 48);
        assert_eq!(count(Category::Changed), 12);
        assert_eq!(count(Category::Missing), 2);
        assert_eq!(count(Category::Moved), 2);
        assert_eq!(count(Category::Unique) + count(Category::New), 0);
        assert!(rows.iter().any(|r| r.source == Source::Base));
        assert!(rows.iter().any(|r| r.source == Source::Second));
    }

    #[test]
    fn fixture_progress_reaches_file_size() {
        let path = fixture("Demo DiffFindDupsTheWholeLot dupReport.csv");
        let size = std::fs::metadata(&path).unwrap().len();
        let mut last = None;
        parse_file(&path, |p| last = Some(p)).unwrap();
        let last = last.expect("final progress call");
        assert_eq!(last.rows_read, 64);
        assert_eq!(last.total_bytes, Some(size));
        assert_eq!(last.bytes_read, size);
    }

    #[test]
    fn job_names() {
        assert_eq!(
            job_name_from_path(Path::new("/tmp/Foo Bar dupReport.csv")),
            "Foo Bar"
        );
        assert_eq!(job_name_from_path(Path::new("/tmp/Other.csv")), "Other");
        assert_eq!(
            job_name_from_path(Path::new(r"C:\x\Demo DiffFindOnlyDups dupReport.csv")),
            if cfg!(windows) {
                "Demo DiffFindOnlyDups"
            } else {
                // On POSIX a backslash is a normal character.
                "C:\\x\\Demo DiffFindOnlyDups"
            }
        );
    }

    #[test]
    fn progress_fires_at_interval_and_at_end() {
        let total_rows = PROGRESS_INTERVAL * 2 + 123;
        let mut text = String::from(EXPECTED_HEADER);
        text.push_str("\r\n");
        for i in 0..total_rows {
            text.push_str(&format!("\"C:\\d\\f{i}.txt\";{i};ABCD;Unique;Base;{i}\r\n"));
        }
        let total = text.len() as u64;
        let mut calls = Vec::new();
        let rows = parse_reader(Cursor::new(text.into_bytes()), Some(total), |p| {
            calls.push(p);
        })
        .unwrap();
        assert_eq!(rows.len(), total_rows);
        let rows_seen: Vec<_> = calls.iter().map(|p| p.rows_read).collect();
        assert_eq!(
            rows_seen,
            [PROGRESS_INTERVAL, PROGRESS_INTERVAL * 2, total_rows]
        );
        assert!(calls.windows(2).all(|w| w[0].bytes_read < w[1].bytes_read));
        assert!(calls.iter().all(|p| p.total_bytes == Some(total)));
        assert_eq!(calls.last().unwrap().bytes_read, total);
    }

    #[test]
    fn progress_fires_once_for_small_input() {
        let text = format!("{EXPECTED_HEADER}\n\"a\";1;;Unique;Base;1\n");
        let mut n = 0;
        parse_reader(Cursor::new(text.into_bytes()), None, |p| {
            n += 1;
            assert_eq!(p.rows_read, 1);
            assert_eq!(p.total_bytes, None);
        })
        .unwrap();
        assert_eq!(n, 1);
    }
}
