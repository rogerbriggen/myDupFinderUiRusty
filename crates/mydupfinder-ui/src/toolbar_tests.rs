//! Toolbar filter tests: the inputs, chips and clear actions drive the store.

use std::path::PathBuf;

use dupreport_core::{Category, RowQuery, Source, filter};
use gpui::{Entity, Focusable, TestAppContext, VisualTestContext};
use gpui_component::input::InputState;

use crate::app_state::ReportStore;
use crate::views::explorer::Explorer;
use crate::views::toolbar::{Toolbar, show_source_chips};

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
    toolbar: Entity<Toolbar>,
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn setup(cx: &mut TestAppContext) -> (Fixture, &mut VisualTestContext) {
    cx.update(gpui_component::init);
    let (view, cx) = cx.add_window_view(|window, cx| Explorer::new(None, window, cx));
    let fixture = view.read_with(cx, |e, _| Fixture {
        store: e.store.clone(),
        toolbar: e.toolbar.clone(),
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

/// Replace the content of an input like typing would (emits `Change`).
fn type_into(input: &Entity<InputState>, text: &str, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        input.update(cx, |s, cx| s.replace_all(text.to_string(), window, cx));
    });
    cx.run_until_parked();
    draw(cx);
}

fn input_value(input: &Entity<InputState>, cx: &mut VisualTestContext) -> String {
    input.read_with(cx, |s, _| s.value().to_string())
}

fn inputs(f: &Fixture, cx: &mut VisualTestContext) -> (Entity<InputState>, Entity<InputState>) {
    f.toolbar
        .read_with(cx, |t, _| (t.text_input.clone(), t.hash_input.clone()))
}

/// Row count `dupreport_core::filter` gives for `query` on the open report.
fn expected(f: &Fixture, query: RowQuery, cx: &mut VisualTestContext) -> usize {
    let report = f.store.read_with(cx, |s, _| s.report.clone().unwrap());
    filter(&report, &query).len()
}

fn filtered_len(f: &Fixture, cx: &mut VisualTestContext) -> usize {
    f.store.read_with(cx, |s, _| s.filtered.len())
}

fn source_chips_shown(f: &Fixture, cx: &mut VisualTestContext) -> bool {
    f.store
        .read_with(cx, |s, _| show_source_chips(s.report.as_deref()))
}

#[gpui::test]
fn inputs_are_disabled_without_report(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    let (text, hash) = inputs(&f, cx);
    assert!(!text.read_with(cx, |s, _| s.is_editable()));
    assert!(!hash.read_with(cx, |s, _| s.is_editable()));
    open(&f, WHOLE_LOT, cx);
    assert!(text.read_with(cx, |s, _| s.is_editable()));
    assert!(hash.read_with(cx, |s, _| s.is_editable()));
}

#[gpui::test]
fn typing_text_filter_refilters(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open(&f, WHOLE_LOT, cx);
    let (text, _) = inputs(&f, cx);

    type_into(&text, "  FILESNEW ", cx);
    let want = expected(
        &f,
        RowQuery {
            include_descendants: true,
            text: Some("filesnew".into()),
            ..RowQuery::default()
        },
        cx,
    );
    assert_eq!(want, 31);
    assert_eq!(filtered_len(&f, cx), want);
    assert_eq!(
        f.store.read_with(cx, |s, _| s.text_filter.clone()),
        "  FILESNEW "
    );

    type_into(&text, "", cx);
    assert_eq!(filtered_len(&f, cx), 64);
}

#[gpui::test]
fn hash_filter_prefix_and_exact(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open(&f, WHOLE_LOT, cx);
    let (_, hash) = inputs(&f, cx);
    let full = f
        .store
        .read_with(cx, |s, _| s.report.as_ref().unwrap().rows[0].hash.clone());
    let query = |h: &str| RowQuery {
        include_descendants: true,
        hash: Some(h.to_string()),
        ..RowQuery::default()
    };

    // A prefix of at least 8 characters matches.
    let prefix = &full[..8];
    type_into(&hash, prefix, cx);
    let want = expected(&f, query(prefix), cx);
    assert!(want >= 1);
    assert_eq!(filtered_len(&f, cx), want);

    // The exact hash matches.
    type_into(&hash, &full, cx);
    let want = expected(&f, query(&full), cx);
    assert!(want >= 1);
    assert_eq!(filtered_len(&f, cx), want);

    // Shorter than 8 characters only matches exactly, so nothing does.
    let short = &full[..4];
    type_into(&hash, short, cx);
    assert_eq!(filtered_len(&f, cx), expected(&f, query(short), cx));
    assert_eq!(filtered_len(&f, cx), 0);
}

#[gpui::test]
fn chips_toggle_filters_and_source_chips_follow_report(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    assert!(!source_chips_shown(&f, cx), "no report");
    open(&f, ONLY_DUPS, cx);
    assert!(!source_chips_shown(&f, cx), "single-source fixture");

    open(&f, WHOLE_LOT, cx);
    assert!(source_chips_shown(&f, cx), "whole-lot fixture");

    // Chips call the store setters; the table follows.
    f.store
        .update(cx, |s, cx| s.set_category(Category::Duplicate, false, cx));
    draw(cx);
    assert_eq!(filtered_len(&f, cx), 16);
    f.store
        .update(cx, |s, cx| s.set_source(Source::Second, false, cx));
    draw(cx);
    let want = expected(
        &f,
        RowQuery {
            include_descendants: true,
            categories: Some(Category::ALL[1..].to_vec()),
            sources: Some(vec![Source::Base]),
            ..RowQuery::default()
        },
        cx,
    );
    assert_eq!(filtered_len(&f, cx), want);
}

#[gpui::test]
fn escape_clears_filters(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open(&f, WHOLE_LOT, cx);
    let (text, hash) = inputs(&f, cx);
    type_into(&text, "filesnew", cx);
    assert_eq!(filtered_len(&f, cx), 31);

    cx.update(|window, cx| {
        f.toolbar
            .update(cx, |t, cx| t.clear_text_filter(window, cx))
    });
    draw(cx);
    assert_eq!(input_value(&text, cx), "");
    assert_eq!(f.store.read_with(cx, |s, _| s.text_filter.clone()), "");
    assert_eq!(filtered_len(&f, cx), 64);

    type_into(&hash, "02f16dc4", cx);
    assert_eq!(filtered_len(&f, cx), 2);
    cx.update(|window, cx| {
        f.toolbar
            .update(cx, |t, cx| t.clear_hash_filter(window, cx))
    });
    draw(cx);
    assert_eq!(input_value(&hash, cx), "");
    assert_eq!(f.store.read_with(cx, |s, _| s.hash_filter.clone()), "");
    assert_eq!(filtered_len(&f, cx), 64);
}

#[gpui::test]
fn store_changes_and_new_reports_keep_inputs_in_sync(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open(&f, WHOLE_LOT, cx);
    let (text, _) = inputs(&f, cx);

    // A value set on the store shows up in the input.
    f.store
        .update(cx, |s, cx| s.set_text_filter("filesnew", cx));
    draw(cx);
    assert_eq!(input_value(&text, cx), "filesnew");

    // Text filter and categories persist across reports; sources reset.
    f.store
        .update(cx, |s, cx| s.set_category(Category::New, false, cx));
    f.store
        .update(cx, |s, cx| s.set_source(Source::Second, false, cx));
    open(&f, WHOLE_LOT, cx);
    assert_eq!(input_value(&text, cx), "filesnew");
    f.store.read_with(cx, |s, _| {
        assert_eq!(s.text_filter, "filesnew");
        assert!(!s.categories.contains(&Category::New));
        assert_eq!(s.sources.len(), 2);
    });
    assert_eq!(filtered_len(&f, cx), 31);
}

#[gpui::test]
fn escape_action_in_focused_input_clears_it(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open(&f, WHOLE_LOT, cx);
    let (text, _) = inputs(&f, cx);
    type_into(&text, "filesnew", cx);
    assert_eq!(filtered_len(&f, cx), 31);

    // Ctrl/Cmd+F focuses the path filter, Esc then clears it and the store filter.
    cx.update(|window, cx| {
        f.toolbar
            .update(cx, |t, cx| t.focus_text_filter(window, cx))
    });
    draw(cx);
    cx.update(|window, cx| {
        let focused = text.focus_handle(cx).is_focused(window);
        assert!(focused, "Ctrl/Cmd+F focuses the path filter");
    });
    cx.dispatch_action(gpui_component::input::Escape);
    cx.run_until_parked();
    assert_eq!(input_value(&text, cx), "");
    assert_eq!(f.store.read_with(cx, |s, _| s.text_filter.clone()), "");
    assert_eq!(filtered_len(&f, cx), 64);
}

/// Real key events through the platform input path: Ctrl+F (bound like in
/// `main`) focuses the path filter, every typed character arrives (no
/// keystroke is lost to the refilter/resync round trip) and Esc clears it.
#[gpui::test]
fn keyboard_typing_into_path_filter(cx: &mut TestAppContext) {
    let (f, cx) = setup(cx);
    open(&f, WHOLE_LOT, cx);
    cx.update(|_, cx| {
        cx.bind_keys([gpui::KeyBinding::new(
            "ctrl-f",
            crate::views::explorer::FocusFilter,
            Some(crate::views::explorer::KEY_CONTEXT),
        )])
    });
    cx.simulate_keystrokes("ctrl-f");
    cx.run_until_parked();
    cx.simulate_input("checkreport");
    cx.run_until_parked();
    draw(cx);
    let (text, _) = inputs(&f, cx);
    assert_eq!(input_value(&text, cx), "checkreport");
    assert_eq!(
        f.store.read_with(cx, |s, _| s.text_filter.clone()),
        "checkreport"
    );
    let want = expected(
        &f,
        RowQuery {
            text: Some("checkreport".into()),
            include_descendants: true,
            ..RowQuery::default()
        },
        cx,
    );
    assert_eq!(filtered_len(&f, cx), want);
    assert!(want > 0 && want < 64);

    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(input_value(&text, cx), "");
    assert_eq!(filtered_len(&f, cx), 64);
}
