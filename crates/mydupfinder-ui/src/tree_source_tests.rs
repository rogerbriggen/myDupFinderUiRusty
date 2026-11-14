//! Tree source dropdown tests: Base + Second / Base only / Second only.

use std::path::PathBuf;

use dupreport_core::{FolderId, ROOT, Source};
use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::app_state::ReportStore;
use crate::tree_view_model::TreeSource;
use crate::views::explorer::Explorer;
use crate::views::folder_tree::FolderTreeView;

const WHOLE_LOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/Demo DiffFindDupsTheWholeLot dupReport.csv"
);
const ONLY_DUPS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/Demo DiffFindOnlyDups dupReport.csv"
);
const TESTDATA: &str = r"O:\DatenP\Programmieren\repos\myDupFinder\testdata";

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn setup(
    cx: &mut TestAppContext,
) -> (
    Entity<ReportStore>,
    Entity<FolderTreeView>,
    &mut VisualTestContext,
) {
    cx.update(gpui_component::init);
    let (view, cx) = cx.add_window_view(|window, cx| Explorer::new(None, window, cx));
    let (store, tree) = view.read_with(cx, |e, _| (e.store.clone(), e.tree.clone()));
    draw(cx);
    (store, tree, cx)
}

fn open(store: &Entity<ReportStore>, path: &str, cx: &mut VisualTestContext) {
    store.update(cx, |s, cx| s.open_report(PathBuf::from(path), cx));
    cx.run_until_parked();
    draw(cx);
}

fn id(store: &Entity<ReportStore>, path: &str, cx: &mut VisualTestContext) -> FolderId {
    store.read_with(cx, |s, _| {
        s.report.as_ref().unwrap().tree.find(path).unwrap()
    })
}

fn set(tree: &Entity<FolderTreeView>, mode: TreeSource, cx: &mut VisualTestContext) {
    tree.update(cx, |t, cx| t.set_tree_source(mode, cx));
    cx.run_until_parked();
    draw(cx);
}

/// Whether `folder` is part of the tree and its file count there.
fn shown(
    store: &Entity<ReportStore>,
    tree: &Entity<FolderTreeView>,
    folder: FolderId,
    cx: &mut VisualTestContext,
) -> Option<u32> {
    let report = store.read_with(cx, |s, _| s.report.clone().unwrap());
    tree.read_with(cx, |t, _| {
        let m = t.model();
        m.shows(&report.tree, folder)
            .then(|| m.file_count(&report.tree, folder))
    })
}

#[gpui::test]
fn base_and_second_only_hide_the_other_side(cx: &mut TestAppContext) {
    let (store, tree, cx) = setup(cx);
    open(&store, WHOLE_LOT, cx);
    let base = id(&store, &format!(r"{TESTDATA}\filesBase"), cx);
    let new = id(&store, &format!(r"{TESTDATA}\filesNew"), cx);
    let testdata = id(&store, TESTDATA, cx);

    assert_eq!(tree.read_with(cx, |t, _| t.tree_source()), TreeSource::Both);
    assert_eq!(shown(&store, &tree, ROOT, cx), Some(64));
    assert_eq!(shown(&store, &tree, base, cx), Some(33));
    assert_eq!(shown(&store, &tree, new, cx), Some(31));

    set(&tree, TreeSource::Base, cx);
    assert_eq!(
        tree.read_with(cx, |t, _| t.model().source()),
        Some(Source::Base)
    );
    assert_eq!(shown(&store, &tree, ROOT, cx), Some(33));
    assert_eq!(shown(&store, &tree, testdata, cx), Some(33));
    assert_eq!(shown(&store, &tree, new, cx), None);

    set(&tree, TreeSource::Second, cx);
    assert_eq!(shown(&store, &tree, ROOT, cx), Some(31));
    assert_eq!(shown(&store, &tree, base, cx), None);
    assert_eq!(shown(&store, &tree, new, cx), Some(31));

    set(&tree, TreeSource::Both, cx);
    assert_eq!(shown(&store, &tree, base, cx), Some(33));
    assert_eq!(shown(&store, &tree, new, cx), Some(31));
}

#[gpui::test]
fn hidden_selection_moves_to_the_nearest_shown_ancestor(cx: &mut TestAppContext) {
    let (store, tree, cx) = setup(cx);
    open(&store, WHOLE_LOT, cx);
    let check = id(&store, &format!(r"{TESTDATA}\filesNew\demo\Check"), cx);
    let testdata = id(&store, TESTDATA, cx);
    tree.update(cx, |t, cx| t.reveal_and_select(check, cx));

    set(&tree, TreeSource::Second, cx);
    assert_eq!(
        store.read_with(cx, |s, _| s.selected_folder),
        check,
        "still shown"
    );

    set(&tree, TreeSource::Base, cx);
    assert_eq!(store.read_with(cx, |s, _| s.selected_folder), testdata);
    assert!(
        tree.read_with(cx, |t, _| t.model().position(testdata))
            .is_some()
    );
}

#[gpui::test]
fn revealing_a_hidden_folder_switches_back_to_both(cx: &mut TestAppContext) {
    let (store, tree, cx) = setup(cx);
    open(&store, WHOLE_LOT, cx);
    let detail = id(
        &store,
        &format!(r"{TESTDATA}\filesBase\demo\Check\CheckDetail"),
        cx,
    );

    set(&tree, TreeSource::Second, cx);
    tree.update(cx, |t, cx| t.reveal_and_select(detail, cx));
    cx.run_until_parked();
    assert_eq!(tree.read_with(cx, |t, _| t.tree_source()), TreeSource::Both);
    assert_eq!(store.read_with(cx, |s, _| s.selected_folder), detail);
    assert!(
        tree.read_with(cx, |t, _| t.model().position(detail))
            .is_some()
    );

    // A folder the current mode shows keeps the mode.
    set(&tree, TreeSource::Base, cx);
    tree.update(cx, |t, cx| t.reveal_and_select(detail, cx));
    assert_eq!(tree.read_with(cx, |t, _| t.tree_source()), TreeSource::Base);
}

#[gpui::test]
fn mode_is_kept_across_reports_but_ignored_for_single_db(cx: &mut TestAppContext) {
    let (store, tree, cx) = setup(cx);
    open(&store, WHOLE_LOT, cx);
    set(&tree, TreeSource::Second, cx);

    open(&store, ONLY_DUPS, cx);
    assert_eq!(
        tree.read_with(cx, |t, _| t.tree_source()),
        TreeSource::Second
    );
    assert_eq!(tree.read_with(cx, |t, _| t.model().source()), None);
    assert_eq!(shown(&store, &tree, ROOT, cx), Some(25));

    open(&store, WHOLE_LOT, cx);
    assert_eq!(
        tree.read_with(cx, |t, _| t.model().source()),
        Some(Source::Second)
    );
    assert_eq!(shown(&store, &tree, ROOT, cx), Some(31));
}
