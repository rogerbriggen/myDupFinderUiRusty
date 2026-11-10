//! Root layout of the application (§7.2): toolbar, error banner, main area.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    AnyElement, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, Styled, Subscription, Window, actions, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{
    Theme,
    resizable::{h_resizable, resizable_panel},
};

use crate::app_state::ReportStore;
use crate::theme::Palette;
use crate::views::folder_tree::FolderTreeView;
use crate::views::row_table::RowTable;
use crate::views::toolbar::Toolbar;

actions!(mydupfinder, [OpenReport]);

/// Text shown while no report is open (§7.2).
pub const PLACEHOLDER: &str =
    "No report open. Click Open report… in the toolbar above to load a dupReport.csv.";

/// Key context of the root view; key bindings are scoped to it.
pub const KEY_CONTEXT: &str = "Explorer";

/// Default, minimum width of the tree pane and the width kept for the table (§7.2).
const TREE_DEFAULT_WIDTH: f32 = 320.0;
const TREE_MIN_WIDTH: f32 = 160.0;
const TABLE_MIN_WIDTH: f32 = 200.0;
/// Width of the splitter's painted line, in px (§7.2).
const SPLITTER_WIDTH: f32 = 5.0;

/// Largest tree width for a window of `window_width`: `window_width − 200`,
/// but never below the minimum.
fn tree_max_width(window_width: Pixels) -> Pixels {
    (window_width - px(TABLE_MIN_WIDTH)).max(px(TREE_MIN_WIDTH))
}

/// The splitter line, painted from the palette instead of the design system's
/// hairline: 5 px wide, `splitter` colour, `splitter_hover` while hovered or
/// dragged. The handle's hit area and cursor stay gpui-component's.
fn splitter_line(active: bool, palette: Palette) -> AnyElement {
    div()
        .flex_none()
        .h_full()
        .w(px(SPLITTER_WIDTH))
        // Centre the 5 px line on the 1 px content box of the handle.
        .ml(px(-(SPLITTER_WIDTH - 1.) / 2.))
        .bg(if active {
            palette.splitter_hover
        } else {
            palette.splitter
        })
        .group_hover("handle", move |s| s.bg(palette.splitter_hover))
        .into_any_element()
}

/// The main explorer view.
pub struct Explorer {
    pub(crate) store: Entity<ReportStore>,
    toolbar: Entity<Toolbar>,
    pub(crate) tree: Entity<FolderTreeView>,
    table: Entity<RowTable>,
    focus: FocusHandle,
    _appearance_subscription: Subscription,
}

impl Explorer {
    /// Create the explorer; opens `initial` (e.g. a command-line argument) right away.
    pub fn new(initial: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // gpui-component widgets take their colours from its own theme, which
        // starts out light; keep it in step with the window appearance.
        Theme::sync_system_appearance(Some(window), cx);
        let appearance_subscription = window.observe_window_appearance(|window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
            window.refresh();
        });

        let store = cx.new(|_| ReportStore::new());
        let toolbar = cx.new(|cx| Toolbar::new(store.clone(), cx));
        let tree = cx.new(|cx| FolderTreeView::new(store.clone(), cx));
        let table = cx.new(|cx| RowTable::new(store.clone(), cx));
        cx.observe(&store, |_, _, cx| cx.notify()).detach();

        let focus = cx.focus_handle();
        window.focus(&focus, cx);

        if let Some(path) = initial {
            store.update(cx, |s, cx| s.open_report(path, cx));
        }
        Self {
            store,
            toolbar,
            tree,
            table,
            focus,
            _appearance_subscription: appearance_subscription,
        }
    }
}

impl Focusable for Explorer {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Explorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::of(window);
        let store = self.store.read(cx);
        let error = store.error.clone();
        let has_report = store.report.is_some();
        let tree_max = tree_max_width(window.viewport_size().width);

        div()
            .id("explorer")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &OpenReport, _window, cx| {
                this.store.update(cx, |s, cx| s.pick_and_open(cx));
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(palette.bg)
            .text_color(palette.fg)
            .child(self.toolbar.clone())
            .when_some(error, |d, message| {
                d.child(
                    div()
                        .flex_none()
                        .px_2()
                        .py_1()
                        .text_size(px(12.))
                        .bg(palette.error_bg)
                        .text_color(palette.error_fg)
                        .child(message),
                )
            })
            // Main area: tree pane | splitter | row table (§7.2).
            .child(
                div().flex_1().min_h(px(0.)).child(
                    h_resizable("main-split")
                        .with_handle_appearance(Rc::new(move |handle, _window, _cx| {
                            Some(splitter_line(handle.is_active(), palette))
                        }))
                        .child(
                            resizable_panel()
                                .size(px(TREE_DEFAULT_WIDTH))
                                .size_range(px(TREE_MIN_WIDTH)..tree_max)
                                .fixed()
                                .child(self.tree.clone()),
                        )
                        .child(
                            resizable_panel().child(div().size_full().min_w(px(0.)).child(
                                if has_report {
                                    self.table.clone().into_any_element()
                                } else {
                                    div()
                                        .size_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(palette.muted_fg)
                                        .child(PLACEHOLDER)
                                        .into_any_element()
                                },
                            )),
                        ),
                ),
            )
    }
}
