//! myDupFinder UI: a native GPUI viewer for `dupReport.csv` files.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod theme;
mod views;

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;
use views::explorer::Explorer;

const WINDOW_TITLE: &str = "myDupFinder UI";

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        gpui_component::init(cx);

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
            let view = cx.new(|_| Explorer);
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

    #[gpui::test]
    fn explorer_renders_placeholder(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (_view, cx) = cx.add_window_view(|_, _| Explorer);
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(PLACEHOLDER.starts_with("No report open."));
    }
}
