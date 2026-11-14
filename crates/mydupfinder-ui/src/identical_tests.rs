//! Identical-folders tests: scan lifecycle, status ordering in the tree and the panel.

use std::path::PathBuf;

use dupreport_core::{
    FolderId, IdenticalStatus, RowQuery, compute_statuses, filter, find_identical_folders,
};
use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::app_state::ReportStore;
use crate::views::explorer::Explorer;
use crate::views::folder_tree::FolderTreeView;
use crate::views::identical_panel::{EMPTY_TEXT, IdenticalPanel, SCANNING_TEXT, empty_message};
use crate::views::toolbar::identical_button;

const WHOLE_LOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/Demo DiffFindDupsTheWholeLot dupReport.csv"
);
const ONLY_DUPS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/Demo DiffFindOnlyDups dupReport.csv"
);

struct Fixture {
    store: Entity<ReportStore>,
    tree: Entity<FolderTreeView>,
    panel: Entity<IdenticalPanel>,
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn setup(cx: &mut TestAppContext) -> (Fixture, &mut VisualTestContext) {
    cx.update(gpui_component::init);
    let (view, cx) = cx.add_window_view(|window, cx| Explorer::new(None, window, cx));
    let fixture = view.read_with(cx, |e, _| Fixture {
        store: e.store.clone(),
        tree: e.tree.clone(),
        panel: e.identical.clone(),
    });
    draw(cx);
    (fixture, cx)
}

fn open(f: &Fixture, path: &str, cx: &mut VisualTestContext) {
    f.store
        .update(cx, |s, cx| s.open_report(PathBuf::from(path), cx));
    cx.run_until_parked();
    draw(cx);
}

/// Open `path` and run the identical-folders scan to completion.
fn open_and_scan(f: &Fixture, path: &str, cx: &mut VisualTestContext) {
    open(f, path, cx);
    f.store.update(cx, |s, cx| s.load_identical(cx));
    cx.run_until_parked();
    draw(cx);
}

fn report(f: &Fixture, cx: &mut VisualTestContext) -> std::sync::Arc<dupreport_core::Report> {
    f.store.read_with(cx, |s, _| s.report.clone().unwrap())
}

/// Visible folders of the tree, top to bottom.
fn visible(f: &Fixture, cx: &mut VisualTestContext) -> Vec<FolderId> {
    f.tree
        .read_with(cx, |t, _| t.model().rows().iter().map(|r| r.0).collect())
}

fn position(f: &Fixture, id: FolderId, cx: &mut VisualTestContext) -> Option<usize> {
    f.tree.read_with(cx, |t, _| t.model().position(id))
}

#[gpui::test]
fn whole_lot_scan_finds_the_two_pairs(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open_and_scan(&f, WHOLE_LOT, cx);
    let report = report(&f, cx);

    f.store.read_with(cx, |s, _| {
        // Same result as the core detector, in the same order.
        assert_eq!(*s.identical, find_identical_folders(&report.rows));
        assert_eq!(
            *s.statuses,
            compute_statuses(&report.tree, &find_identical_folders(&report.rows))
        );

        assert_eq!(s.identical.len(), 2);
        let (a, b) = (&s.identical[0], &s.identical[1]);
        assert!(a.folder_a.ends_with(r"filesBase\demo\Check\CheckDetail"));
        assert!(a.folder_b.ends_with(r"filesNew\demo\Check\CheckDetail"));
        assert_eq!((a.file_count, a.total_size), (4, 7562));
        assert!(b.folder_a.ends_with(r"filesBase\demo\FindDups\Helper"));
        assert!(b.folder_b.ends_with(r"filesNew\demo\FindDups\Helper"));
        assert_eq!((b.file_count, b.total_size), (3, 6320));
        assert!(s.has_statuses());
        assert!(
            s.statuses.contains(&Some(IdenticalStatus::Green)),
            "paired folders are green"
        );
    });
}

#[gpui::test]
fn scan_lifecycle_flags(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    let flags = |f: &Fixture, cx: &mut VisualTestContext| {
        f.store
            .read_with(cx, |s, _| (s.identical_loading, s.identical_scanned))
    };

    // Without a report the scan does nothing.
    f.store.update(cx, |s, cx| s.load_identical(cx));
    assert_eq!(flags(&f, cx), (false, false));

    open(&f, WHOLE_LOT, cx);
    assert_eq!(flags(&f, cx), (false, false));
    f.store.update(cx, |s, cx| s.load_identical(cx));
    assert_eq!(flags(&f, cx), (true, false));
    // Nothing is stored before the background job has finished.
    assert!(f.store.read_with(cx, |s, _| s.identical.is_empty()));
    draw(cx); // panel shows "Scanning…"
    // A second request while scanning is ignored.
    f.store.update(cx, |s, cx| s.load_identical(cx));
    cx.run_until_parked();
    assert_eq!(flags(&f, cx), (false, true));
    assert_eq!(f.store.read_with(cx, |s, _| s.identical.len()), 2);
    draw(cx);

    // A rescan keeps working and ends with the same result.
    f.store.update(cx, |s, cx| s.load_identical(cx));
    assert_eq!(flags(&f, cx), (true, true));
    cx.run_until_parked();
    assert_eq!(flags(&f, cx), (false, true));
    assert_eq!(f.store.read_with(cx, |s, _| s.identical.len()), 2);
}

#[test]
fn button_label_and_state() {
    assert_eq!(
        identical_button(false, false, false),
        ("Identical folders", true)
    );
    assert_eq!(
        identical_button(true, false, false),
        ("Identical folders", false)
    );
    assert_eq!(identical_button(true, true, true), ("Scanning…", true));
    assert_eq!(
        identical_button(true, false, true),
        ("Hide identical folders", false)
    );
}

#[gpui::test]
fn button_toggles_the_result_off_and_on(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open(&f, WHOLE_LOT, cx);
    let state = |f: &Fixture, cx: &mut VisualTestContext| {
        f.store
            .read_with(cx, |s, _| (s.identical_shown(), s.has_statuses()))
    };
    assert_eq!(state(&f, cx), (false, false));

    // First press scans; a press while scanning is ignored.
    f.store.update(cx, |s, cx| s.toggle_identical(cx));
    assert!(f.store.read_with(cx, |s, _| s.identical_loading));
    f.store.update(cx, |s, cx| s.toggle_identical(cx));
    cx.run_until_parked();
    draw(cx);
    assert_eq!(state(&f, cx), (true, true));

    // Second press hides panel and dots but keeps the result.
    f.store.update(cx, |s, cx| s.toggle_identical(cx));
    draw(cx);
    assert_eq!(state(&f, cx), (false, false));
    f.store.read_with(cx, |s, _| {
        assert!(!s.identical_loading && s.identical_scanned);
        assert_eq!(s.identical.len(), 2);
    });

    // Third press shows it again without rescanning.
    f.store.update(cx, |s, cx| s.toggle_identical(cx));
    assert!(!f.store.read_with(cx, |s, _| s.identical_loading));
    draw(cx);
    assert_eq!(state(&f, cx), (true, true));

    // Hidden, then a new report: the next press scans again.
    f.store.update(cx, |s, cx| s.toggle_identical(cx));
    open(&f, WHOLE_LOT, cx);
    assert!(!f.store.read_with(cx, |s, _| s.identical_hidden));
    f.store.update(cx, |s, cx| s.toggle_identical(cx));
    assert!(f.store.read_with(cx, |s, _| s.identical_loading));
    cx.run_until_parked();
    assert_eq!(state(&f, cx), (true, true));
}

#[test]
fn empty_panel_messages() {
    assert_eq!(empty_message(&[], true), Some(SCANNING_TEXT));
    assert_eq!(empty_message(&[], false), Some(EMPTY_TEXT));
    let pair = dupreport_core::IdenticalFolderPair {
        folder_a: "a".into(),
        folder_b: "b".into(),
        file_count: 1,
        total_size: 1,
    };
    assert_eq!(empty_message(&[pair], false), None);
}

#[gpui::test]
fn opening_a_report_clears_identical_state(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open_and_scan(&f, WHOLE_LOT, cx);
    assert!(f.store.read_with(cx, |s, _| s.has_statuses()));

    f.store
        .update(cx, |s, cx| s.open_report(PathBuf::from(ONLY_DUPS), cx));
    f.store.read_with(cx, |s, _| {
        assert!(s.identical.is_empty() && s.statuses.is_empty());
        assert!(!s.identical_loading && !s.identical_scanned);
        assert!(!s.has_statuses());
    });
    cx.run_until_parked();
    draw(cx);
    f.store.read_with(cx, |s, _| {
        assert_eq!(s.report.as_ref().unwrap().rows.len(), 25);
        assert!(s.identical.is_empty() && !s.identical_scanned);
    });
}

#[gpui::test]
fn stale_scan_result_is_dropped(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open(&f, WHOLE_LOT, cx);
    // Start a scan and open another report before its result arrives.
    f.store.update(cx, |s, cx| s.load_identical(cx));
    assert!(f.store.read_with(cx, |s, _| s.identical_loading));
    f.store
        .update(cx, |s, cx| s.open_report(PathBuf::from(ONLY_DUPS), cx));
    assert!(!f.store.read_with(cx, |s, _| s.identical_loading));
    cx.run_until_parked();
    draw(cx);
    f.store.read_with(cx, |s, _| {
        assert_eq!(s.report.as_ref().unwrap().rows.len(), 25);
        assert!(s.identical.is_empty(), "old report's pairs must not appear");
        assert!(s.statuses.is_empty());
        assert!(!s.identical_loading && !s.identical_scanned);
    });
}

#[gpui::test]
fn only_dups_scan_finds_nothing_and_shows_the_empty_text(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open_and_scan(&f, ONLY_DUPS, cx);
    let report = report(&f, cx);
    f.store.read_with(cx, |s, _| {
        assert!(s.identical.is_empty());
        assert!(s.identical_scanned && !s.identical_loading);
        // §5.9: no pairs, no dots.
        assert_eq!(s.statuses.len(), report.tree.nodes.len());
        assert!(s.statuses.iter().all(Option::is_none));
        assert!(!s.has_statuses());
        assert_eq!(
            empty_message(&s.identical, s.identical_loading),
            Some(EMPTY_TEXT)
        );
    });
    // The finished scan keeps the panel (and its message) around: draws fine,
    // also collapsed.
    f.panel.update(cx, |p, cx| p.set_collapsed(true, cx));
    draw(cx);
    f.panel.update(cx, |p, cx| p.set_collapsed(false, cx));
    draw(cx);
}

#[gpui::test]
fn panel_collapse_and_restore(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open_and_scan(&f, WHOLE_LOT, cx);
    assert!(!f.panel.read_with(cx, |p, _| p.collapsed));
    f.panel.update(cx, |p, cx| p.set_collapsed(true, cx));
    draw(cx);
    assert!(f.panel.read_with(cx, |p, _| p.collapsed));
    // The collapsed state survives a rescan and a new report.
    f.store.update(cx, |s, cx| s.load_identical(cx));
    cx.run_until_parked();
    draw(cx);
    assert!(f.panel.read_with(cx, |p, _| p.collapsed));
    f.panel.update(cx, |p, cx| p.set_collapsed(false, cx));
    draw(cx);
    assert!(!f.panel.read_with(cx, |p, _| p.collapsed));
}

/// A tiny two-source report in a temp dir where the alphabetically first
/// sibling `Aaa` is red and the last one `Zzz` is green (the real fixtures
/// happen to have no folder whose green-first order differs from alphabetical).
/// Removed again when dropped.
struct SyntheticReport {
    dir: PathBuf,
    file: PathBuf,
}

impl SyntheticReport {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("mydupfinder-ui-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("synthetic dupReport.csv");
        let rows = [
            r#""C:\base\Aaa\only.txt";1;ab;Unique;Base;1"#,
            r#""C:\base\Zzz\a.txt";2;cd;Duplicate;Base;2"#,
            r#""C:\second\Zzz\a.txt";2;cd;Duplicate;Second;2"#,
        ];
        let csv = format!(
            "FilenameAndPath;FileSize;FileSha512Hash;Category;Source;GroupId\r\n{}\r\n",
            rows.join("\r\n")
        );
        std::fs::write(&file, csv).unwrap();
        Self { dir, file }
    }

    fn path(&self) -> &str {
        self.file.to_str().unwrap()
    }
}

impl Drop for SyntheticReport {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// `(C:\base, C:\base\Aaa (red), C:\base\Zzz (green))` of an open synthetic report.
fn synthetic_ids(f: &Fixture, cx: &mut VisualTestContext) -> (FolderId, FolderId, FolderId) {
    let report = report(f, cx);
    let id = |p: &str| report.tree.find(p).unwrap();
    (id(r"C:\base"), id(r"C:\base\Aaa"), id(r"C:\base\Zzz"))
}

#[gpui::test]
fn green_on_top_reorders_the_tree(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    let syn = SyntheticReport::new("green-on-top");
    open_and_scan(&f, syn.path(), cx);
    let (parent, early, late) = synthetic_ids(&f, cx);

    // Select the better ranked sibling through the panel's route so its parent
    // is expanded (and selection must survive re-ranking).
    f.tree.update(cx, |t, cx| t.reveal_and_select(late, cx));
    draw(cx);
    let order = |f: &Fixture, cx: &mut VisualTestContext| {
        (
            position(f, early, cx).unwrap(),
            position(f, late, cx).unwrap(),
        )
    };

    // On by default once statuses exist: the better ranked sibling comes first.
    let (e, l) = order(&f, cx);
    assert!(l < e, "green-ish child before its worse sibling");

    // Off: alphabetical again.
    f.tree.update(cx, |t, cx| t.set_green_on_top(false, cx));
    draw(cx);
    let (e, l) = order(&f, cx);
    assert!(e < l, "alphabetical order restored");
    // Selection and expansion survived.
    assert_eq!(f.store.read_with(cx, |s, _| s.selected_folder), late);
    assert!(f.tree.read_with(cx, |t, _| t.model().is_expanded(parent)));

    // On again.
    f.tree.update(cx, |t, cx| t.set_green_on_top(true, cx));
    draw(cx);
    let (e, l) = order(&f, cx);
    assert!(l < e);
    assert_eq!(f.store.read_with(cx, |s, _| s.selected_folder), late);
}

#[gpui::test]
fn new_report_resets_the_ranking(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    let syn = SyntheticReport::new("rank-reset");
    open_and_scan(&f, syn.path(), cx);
    let (_, early, late) = synthetic_ids(&f, cx);
    f.tree.update(cx, |t, cx| t.reveal_and_select(late, cx));
    draw(cx);
    assert!(position(&f, late, cx) < position(&f, early, cx));

    // A new report starts alphabetical (no statuses yet) ...
    open(&f, syn.path(), cx);
    f.tree.update(cx, |t, cx| t.reveal_and_select(late, cx));
    draw(cx);
    assert!(
        position(&f, early, cx) < position(&f, late, cx),
        "no statuses, no ranking"
    );
    // ... and ranks again after its own scan (the checkbox is still on).
    f.store.update(cx, |s, cx| s.load_identical(cx));
    cx.run_until_parked();
    draw(cx);
    assert!(position(&f, late, cx) < position(&f, early, cx));

    // With the checkbox off a scan does not rank.
    f.tree.update(cx, |t, cx| t.set_green_on_top(false, cx));
    open(&f, syn.path(), cx);
    f.store.update(cx, |s, cx| s.load_identical(cx));
    cx.run_until_parked();
    f.tree.update(cx, |t, cx| t.reveal_and_select(late, cx));
    draw(cx);
    assert!(position(&f, early, cx) < position(&f, late, cx));
}

#[gpui::test]
fn panel_row_click_selects_and_reveals_the_folder(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open_and_scan(&f, WHOLE_LOT, cx);
    let report = report(&f, cx);
    let pairs = f.store.read_with(cx, |s, _| s.identical.clone());
    let first = report.tree.find(&pairs[0].folder_b).unwrap();
    assert!(position(&f, first, cx).is_none(), "hidden before the click");

    for pair in pairs.iter() {
        for path in [&pair.folder_b, &pair.folder_a] {
            let id = report.tree.find(path).unwrap();
            f.panel.update(cx, |p, cx| p.activate_folder(path, cx));
            draw(cx);
            // Selected, every ancestor expanded, hence visible in the tree.
            assert_eq!(f.store.read_with(cx, |s, _| s.selected_folder), id);
            for ancestor in report.tree.ancestors(id) {
                assert!(f.tree.read_with(cx, |t, _| t.model().is_expanded(ancestor)));
            }
            assert!(position(&f, id, cx).is_some(), "{path} is visible");
            assert!(visible(&f, cx).contains(&id));
            // The table was refiltered to the folder.
            let want = filter(
                &report,
                &RowQuery {
                    folder: Some(path.clone()),
                    include_descendants: true,
                    ..RowQuery::default()
                },
            )
            .len();
            assert!(want > 0);
            assert_eq!(f.store.read_with(cx, |s, _| s.filtered.len()), want);
        }
    }

    // An unknown path does nothing.
    let before = f.store.read_with(cx, |s, _| s.selected_folder);
    f.panel
        .update(cx, |p, cx| p.activate_folder(r"Z:\no\such\folder", cx));
    assert_eq!(f.store.read_with(cx, |s, _| s.selected_folder), before);
}
