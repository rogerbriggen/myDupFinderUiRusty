//! myDupFinder UI: a native GPUI viewer for `dupReport.csv` files.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
#[cfg(test)]
mod identical_tests;
mod theme;
#[cfg(test)]
mod toolbar_tests;
#[cfg(test)]
mod tree_source_tests;
mod tree_view_model;
mod version;
mod views;

use std::path::PathBuf;

use gpui::{
    App, AppContext, Bounds, KeyBinding, QuitMode, TitlebarOptions, WindowBounds, WindowOptions,
    px, size,
};
use gpui_component::Root;
use views::explorer::{Explorer, FocusFilter, KEY_CONTEXT, OpenReport};
use views::folder_tree;

/// Ctrl+F everywhere except macOS, where it is Cmd+F.
const FIND_KEYSTROKE: &str = if cfg!(target_os = "macos") {
    "cmd-f"
} else {
    "ctrl-f"
};

/// Window title, e.g. `myDupFinder UI 0.1.0+1bb8187`.
fn window_title() -> String {
    format!("myDupFinder UI {}", version::display_version())
}

/// Ctrl+O everywhere except macOS, where it is Cmd+O.
const OPEN_KEYSTROKE: &str = if cfg!(target_os = "macos") {
    "cmd-o"
} else {
    "ctrl-o"
};

/// Keyboard navigation of the folder tree (§7.4), scoped to its key context.
fn bind_tree_keys(cx: &mut App) {
    let ctx = Some(folder_tree::KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("up", folder_tree::SelectPrev, ctx),
        KeyBinding::new("down", folder_tree::SelectNext, ctx),
        KeyBinding::new("right", folder_tree::ExpandOrFirstChild, ctx),
        KeyBinding::new("left", folder_tree::CollapseOrParent, ctx),
        KeyBinding::new("enter", folder_tree::Activate, ctx),
        KeyBinding::new("space", folder_tree::Activate, ctx),
    ]);
}

fn main() {
    // Optional first argument: a report to open at startup.
    let initial: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);

    gpui_platform::application()
        // gpui-component loads its icons (e.g. the checkbox tick) from `icons/*.svg`.
        .with_assets(gpui_kit_assets::Assets)
        // The app has one window and no way to reopen it, so closing it quits,
        // on macOS too (GPUI's default there keeps the process running).
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            cx.bind_keys([
                KeyBinding::new(OPEN_KEYSTROKE, OpenReport, Some(KEY_CONTEXT)),
                KeyBinding::new(FIND_KEYSTROKE, FocusFilter, Some(KEY_CONTEXT)),
            ]);
            bind_tree_keys(cx);

            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(window_title().into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(800.), px(500.))),
                ..Default::default()
            };

            cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| Explorer::new(initial.clone(), window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("failed to open main window");
            cx.activate(true);
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::explorer::PLACEHOLDER;
    use gpui::TestAppContext;

    const WHOLE_LOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/Demo DiffFindDupsTheWholeLot dupReport.csv"
    );

    fn draw(cx: &mut gpui::VisualTestContext) {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    #[gpui::test]
    fn explorer_renders_placeholder(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (view, cx) = cx.add_window_view(|window, cx| Explorer::new(None, window, cx));
        draw(cx);
        assert!(PLACEHOLDER.starts_with("No report open."));
        assert!(view.read_with(cx, |e, cx| e.store.read(cx).report.is_none()));
    }

    #[gpui::test]
    fn explorer_renders_opened_report(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (view, cx) = cx.add_window_view(|window, cx| Explorer::new(None, window, cx));
        let store = view.read_with(cx, |e, _| e.store.clone());
        store.update(cx, |s, cx| s.open_report(PathBuf::from(WHOLE_LOT), cx));
        draw(cx); // loading state
        cx.run_until_parked();
        draw(cx); // table with rows
        store.update(cx, |s, cx| s.set_show_hash(true, cx));
        draw(cx); // with hash column
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), 64);
    }

    #[gpui::test]
    fn tree_selection_matches_core_filter(cx: &mut TestAppContext) {
        use dupreport_core::{FolderId, RowQuery, filter};

        cx.update(gpui_component::init);
        let (view, cx) = cx.add_window_view(|window, cx| Explorer::new(None, window, cx));
        let store = view.read_with(cx, |e, _| e.store.clone());
        let tree = view.read_with(cx, |e, _| e.tree.clone());
        store.update(cx, |s, cx| s.open_report(PathBuf::from(WHOLE_LOT), cx));
        cx.run_until_parked();
        draw(cx);

        let report = store.read_with(cx, |s, _| s.report.clone().unwrap());
        let expected = |id: FolderId, descendants: bool| {
            let q = RowQuery {
                folder: Some(report.tree.node(id).path.clone()),
                include_descendants: descendants,
                ..RowQuery::default()
            };
            filter(&report, &q).len()
        };

        for descendants in [true, false] {
            store.update(cx, |s, cx| s.set_include_descendants(descendants, cx));
            for id in 0..report.tree.nodes.len() as FolderId {
                tree.update(cx, |t, cx| t.select(id, cx));
                draw(cx);
                assert_eq!(
                    store.read_with(cx, |s, _| (s.selected_folder, s.filtered.len())),
                    (id, expected(id, descendants)),
                    "folder {id}, descendants {descendants}"
                );
            }
        }
    }

    #[gpui::test]
    fn tree_expansion_keyboard_and_reset(cx: &mut TestAppContext) {
        use dupreport_core::ROOT;

        cx.update(gpui_component::init);
        let (view, cx) = cx.add_window_view(|window, cx| Explorer::new(None, window, cx));
        let store = view.read_with(cx, |e, _| e.store.clone());
        let tree = view.read_with(cx, |e, _| e.tree.clone());
        draw(cx);
        assert!(tree.read_with(cx, |t, _| t.model().rows().is_empty()));
        // One level per expansion; chain auto-expansion is covered separately.
        tree.update(cx, |t, cx| t.set_auto_expand_chains(false, cx));

        store.update(cx, |s, cx| s.open_report(PathBuf::from(WHOLE_LOT), cx));
        cx.run_until_parked();
        draw(cx);
        let report = store.read_with(cx, |s, _| s.report.clone().unwrap());
        let root_children = report.tree.node(ROOT).children.clone();
        let visible =
            |cx: &mut gpui::VisualTestContext| tree.read_with(cx, |t, _| t.model().rows().to_vec());
        assert_eq!(visible(cx).len(), 1 + root_children.len());
        assert_eq!(
            store.read_with(cx, |s, _| s.selected_folder),
            ROOT,
            "new report selects the root"
        );

        // Toggle expands without selecting; a second toggle collapses again.
        let first = root_children[0];
        let child_count = report.tree.node(first).children.len();
        tree.update(cx, |t, cx| t.toggle(first, cx));
        draw(cx);
        assert_eq!(visible(cx).len(), 1 + root_children.len() + child_count);
        assert_eq!(store.read_with(cx, |s, _| s.selected_folder), ROOT);
        tree.update(cx, |t, cx| t.toggle(first, cx));
        assert_eq!(visible(cx).len(), 1 + root_children.len());

        // Keyboard: down selects the first child, right expands it, up returns to the root.
        tree.update(cx, |t, cx| t.press_down(cx));
        assert_eq!(store.read_with(cx, |s, _| s.selected_folder), first);
        tree.update(cx, |t, cx| t.press_right(cx));
        assert_eq!(visible(cx).len(), 1 + root_children.len() + child_count);
        tree.update(cx, |t, cx| t.press_left(cx));
        assert_eq!(visible(cx).len(), 1 + root_children.len());
        tree.update(cx, |t, cx| t.press_left(cx));
        assert_eq!(store.read_with(cx, |s, _| s.selected_folder), ROOT);
        tree.update(cx, |t, cx| t.press_activate(cx));
        assert_eq!(store.read_with(cx, |s, _| s.filtered.len()), 64);

        // Opening another report resets expansion and selection.
        tree.update(cx, |t, cx| t.toggle(first, cx));
        tree.update(cx, |t, cx| t.press_down(cx));
        store.update(cx, |s, cx| s.open_report(PathBuf::from(WHOLE_LOT), cx));
        cx.run_until_parked();
        draw(cx);
        assert_eq!(visible(cx).len(), 1 + root_children.len());
        assert_eq!(store.read_with(cx, |s, _| s.selected_folder), ROOT);
    }

    #[gpui::test]
    fn new_report_auto_expands_the_root_chain(cx: &mut TestAppContext) {
        use dupreport_core::ROOT;

        cx.update(gpui_component::init);
        let (view, cx) = cx.add_window_view(|window, cx| Explorer::new(None, window, cx));
        let store = view.read_with(cx, |e, _| e.store.clone());
        let tree = view.read_with(cx, |e, _| e.tree.clone());
        assert!(
            tree.read_with(cx, |t, _| t.model().auto_expand_chains()),
            "on by default"
        );
        store.update(cx, |s, cx| s.open_report(PathBuf::from(WHOLE_LOT), cx));
        cx.run_until_parked();
        draw(cx);
        let report = store.read_with(cx, |s, _| s.report.clone().unwrap());
        let t = &report.tree;

        // Follow the chain from the root: every pass-through folder is expanded,
        // and the walk ends at a folder that branches, holds files or is a leaf.
        let mut cur = ROOT;
        let mut depth = 0;
        while let &[child] = t.node(cur).children.as_slice() {
            let next = t.node(child);
            if next.direct_file_count > 0 || next.children.is_empty() {
                break;
            }
            assert!(tree.read_with(cx, |v, _| v.model().is_expanded(child)));
            cur = child;
            depth += 1;
        }
        assert!(
            depth > 1,
            "the fixture starts with a long chain (O:\\DatenP\\…)"
        );
        let pos = tree.read_with(cx, |v, _| v.model().position(cur));
        assert!(pos.is_some(), "the end of the chain is visible");
        for &child in &t.node(cur).children {
            assert!(tree.read_with(cx, |v, _| v.model().position(child).is_some()));
        }
    }

    #[test]
    fn component_icons_are_bundled() {
        use gpui::AssetSource;
        // The checkbox tick (and other component icons) must load from the
        // asset source registered in `main`.
        let svg = gpui_kit_assets::Assets.load("icons/check.svg").unwrap();
        assert!(svg.is_some_and(|data| data.starts_with(b"<svg")));
    }

    #[gpui::test]
    fn narrow_table_scrolls_horizontally(cx: &mut TestAppContext) {
        use crate::views::row_table::min_table_width;

        cx.update(gpui_component::init);
        let (view, cx) = cx.add_window_view(|window, cx| Explorer::new(None, window, cx));
        let store = view.read_with(cx, |e, _| e.store.clone());
        let table = view.read_with(cx, |e, _| e.table.clone());
        store.update(cx, |s, cx| s.open_report(PathBuf::from(WHOLE_LOT), cx));
        cx.run_until_parked();
        let hscroll =
            |cx: &mut gpui::VisualTestContext| table.read_with(cx, |t, _| t.hscroll().clone());

        cx.simulate_resize(size(px(1600.), px(800.)));
        draw(cx);
        assert_eq!(
            hscroll(cx).max_offset().x,
            px(0.),
            "wide pane: nothing to scroll"
        );

        // Minimum window size: the table pane is narrower than the columns.
        cx.simulate_resize(size(px(800.), px(500.)));
        for show_hash in [false, true] {
            store.update(cx, |s, cx| s.set_show_hash(show_hash, cx));
            draw(cx);
            let h = hscroll(cx);
            let viewport = h.bounds().size.width;
            assert!(
                viewport < px(min_table_width(show_hash)),
                "pane {viewport:?}"
            );
            let content = viewport + h.max_offset().x;
            assert!(
                (content - px(min_table_width(show_hash))).abs() < px(1.),
                "scrollable width {content:?}, hash {show_hash}"
            );
        }

        // A vertical wheel scrolls only the rows; a horizontal one (Shift+wheel
        // on Windows) only moves header and body sideways.
        let vscroll =
            |cx: &mut gpui::VisualTestContext| table.read_with(cx, |t, _| t.vscroll().clone());
        let wheel = |cx: &mut gpui::VisualTestContext, x: f32, y: f32| {
            let position = hscroll(cx).bounds().center();
            cx.simulate_event(gpui::ScrollEvent {
                position,
                delta: gpui::ScrollDelta::Pixels(gpui::point(px(x), px(y))),
                ..Default::default()
            });
            draw(cx);
        };
        wheel(cx, 0., -60.);
        assert_eq!(vscroll(cx).offset().y, px(-60.));
        assert_eq!(hscroll(cx).offset().x, px(0.));
        wheel(cx, -50., 0.);
        assert_eq!(hscroll(cx).offset().x, px(-50.));
        assert_eq!(vscroll(cx).offset().y, px(-60.));
    }

    #[gpui::test]
    fn explorer_renders_error_banner(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (view, cx) = cx.add_window_view(|window, cx| Explorer::new(None, window, cx));
        let store = view.read_with(cx, |e, _| e.store.clone());
        store.update(cx, |s, cx| {
            s.open_report(PathBuf::from("no/such/file.csv"), cx)
        });
        cx.run_until_parked();
        draw(cx);
        assert!(store.read_with(cx, |s, _| s.error.is_some()));
    }
}
