//! myDupFinder UI: a native GPUI viewer for `dupReport.csv` files.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod theme;
mod views;

use std::path::PathBuf;

use gpui::{
    App, AppContext, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use gpui_component::Root;
use views::explorer::{Explorer, KEY_CONTEXT, OpenReport};

const WINDOW_TITLE: &str = "myDupFinder UI";

/// Ctrl+O everywhere except macOS, where it is Cmd+O.
const OPEN_KEYSTROKE: &str = if cfg!(target_os = "macos") {
    "cmd-o"
} else {
    "ctrl-o"
};

fn main() {
    // Optional first argument: a report to open at startup.
    let initial: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);

    gpui_platform::application().run(move |cx: &mut App| {
        gpui_component::init(cx);
        cx.bind_keys([KeyBinding::new(
            OPEN_KEYSTROKE,
            OpenReport,
            Some(KEY_CONTEXT),
        )]);

        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(WINDOW_TITLE.into()),
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
