//! Synthetic 200k-row load test (§9). Run in release:
//! `cargo test -p dupreport-core --release -- --ignored`

use std::fmt::Write as _;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use dupreport_core::parser::{EXPECTED_HEADER, parse_reader};
use dupreport_core::{Report, RowQuery, filter};

const ROWS: usize = 200_000;

fn synthetic_csv() -> String {
    let mut s = String::with_capacity(ROWS * 160);
    s.push_str(EXPECTED_HEADER);
    s.push_str("\r\n");
    let hash = "0123456789ABCDEF".repeat(8);
    for i in 0..ROWS {
        let source = if i % 2 == 0 { "Base" } else { "Second" };
        let drive = if i % 2 == 0 { "C" } else { "D" };
        write!(
            s,
            "\"{drive}:\\photos\\{}\\{}\\file{i}.jpg\";{};{hash};Duplicate;{source};{}\r\n",
            i % 50,
            i % 400,
            i * 3,
            i / 2
        )
        .unwrap();
    }
    s
}

#[test]
#[ignore = "timing test; run in release with --ignored"]
fn loads_200k_rows_in_under_two_seconds() {
    let csv = synthetic_csv();
    let total = csv.len() as u64;

    let start = Instant::now();
    let rows = parse_reader(Cursor::new(csv.into_bytes()), Some(total), |_| {}).unwrap();
    let report = Report::from_rows("synthetic", PathBuf::new(), rows);
    let elapsed = start.elapsed();

    eprintln!("200k-row parse + index build: {elapsed:?}");
    assert_eq!(report.rows.len(), ROWS);
    assert!(report.has_second_source);

    let t = Instant::now();
    let hits = filter(
        &report,
        &RowQuery {
            folder: Some(r"C:\photos".into()),
            include_descendants: true,
            text: Some("file1".into()),
            ..RowQuery::default()
        },
    );
    eprintln!("filter: {:?} ({} hits)", t.elapsed(), hits.len());
    assert!(!hits.is_empty());

    assert!(
        elapsed < Duration::from_secs(2),
        "loading took {elapsed:?}, budget is 2 s"
    );
}
