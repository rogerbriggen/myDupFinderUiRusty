//! Proves the shared `testdata/` fixtures are reachable from the core crate.

use std::fs;
use std::path::PathBuf;

const EXPECTED_HEADER: &str = "FilenameAndPath;FileSize;FileSha512Hash;Category;Source;GroupId";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(name)
}

fn first_line(name: &str) -> String {
    let text = fs::read_to_string(fixture(name)).expect("fixture readable");
    text.lines().next().unwrap_or_default().to_string()
}

#[test]
fn only_dups_fixture_has_expected_header() {
    assert_eq!(
        first_line("Demo DiffFindOnlyDups dupReport.csv"),
        EXPECTED_HEADER
    );
}

#[test]
fn whole_lot_fixture_has_expected_header() {
    assert_eq!(
        first_line("Demo DiffFindDupsTheWholeLot dupReport.csv"),
        EXPECTED_HEADER
    );
}
