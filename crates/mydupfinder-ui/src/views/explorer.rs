//! Root layout of the application (§7.2): toolbar, error banner, main area.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    AnyElement, AppContext, Axis, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, Styled, Subscription, Window, actions, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{
    Theme,
    resizable::{h_resizable, resizable_panel, v_resizable},
};

use crate::app_state::ReportStore;
use crate::theme::Palette;
use crate::views::folder_tree::FolderTreeView;
use crate::views::identical_panel::IdenticalPanel;
use crate::views::row_table::RowTable;
use crate::views::toolbar::Toolbar;

actions!(mydupfinder, [OpenReport, FocusFilter]);

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

/// Default and minimum height of the identical panel; the window keeps at least
/// `IDENTICAL_RESERVED_HEIGHT` for everything above it (§7.2).
const IDENTICAL_DEFAULT_HEIGHT: f32 = 220.0;
const IDENTICAL_MIN_HEIGHT: f32 = 80.0;
const IDENTICAL_RESERVED_HEIGHT: f32 = 160.0;

/// Largest identical-panel height for a window of `window_height`:
/// `window_height − 160`, but never below the minimum.
fn identical_max_height(window_height: Pixels) -> Pixels {
    (window_height - px(IDENTICAL_RESERVED_HEIGHT)).max(px(IDENTICAL_MIN_HEIGHT))
}

/// Largest tree width for a window of `window_width`: `window_width − 200`,
/// but never below the minimum.
fn tree_max_width(window_width: Pixels) -> Pixels {
    (window_width - px(TABLE_MIN_WIDTH)).max(px(TREE_MIN_WIDTH))
}

/// The splitter line, painted from the palette instead of the design system's
/// hairline: 5 px thick, `splitter` colour, `splitter_hover` while hovered or
/// dragged. The handle's hit area and cursor stay gpui-component's. `axis` is
/// the resize axis: `Horizontal` for a vertical divider (tree | table),
/// `Vertical` for the horizontal divider above the identical panel.
fn splitter_line(axis: Axis, active: bool, palette: Palette) -> AnyElement {
    // Centre the 5 px line on the 1 px content box of the handle.
    let offset = px(-(SPLITTER_WIDTH - 1.) / 2.);
    div()
        .flex_none()
        .map(|d| match axis {
            Axis::Horizontal => d.h_full().w(px(SPLITTER_WIDTH)).ml(offset),
            Axis::Vertical => d.w_full().h(px(SPLITTER_WIDTH)).mt(offset),
        })
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
    pub(crate) toolbar: Entity<Toolbar>,
    pub(crate) tree: Entity<FolderTreeView>,
    pub(crate) table: Entity<RowTable>,
    pub(crate) identical: Entity<IdenticalPanel>,
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
        let toolbar = cx.new(|cx| Toolbar::new(store.clone(), window, cx));
        let tree = cx.new(|cx| FolderTreeView::new(store.clone(), cx));
        let table = cx.new(|cx| RowTable::new(store.clone(), cx));
        let identical = cx.new(|cx| IdenticalPanel::new(store.clone(), tree.clone(), cx));
        cx.observe(&identical, |_, _, cx| cx.notify()).detach();
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
            identical,
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
        let identical_max = identical_max_height(window.viewport_size().height);
        // §7.2 / decision: also shown for a finished scan without pairs;
        // hidden again when the toolbar button switched the result off.
        let show_identical = store.identical_shown();
        let identical_collapsed = self.identical.read(cx).collapsed;

        // Tree pane | splitter | row table.
        let main_split = h_resizable("main-split")
            .with_handle_appearance(Rc::new(move |handle, _window, _cx| {
                Some(splitter_line(handle.axis(), handle.is_active(), palette))
            }))
            .child(
                resizable_panel()
                    .size(px(TREE_DEFAULT_WIDTH))
                    .size_range(px(TREE_MIN_WIDTH)..tree_max)
                    .fixed()
                    .child(self.tree.clone()),
            )
            .child(
                resizable_panel().child(div().size_full().min_w(px(0.)).child(if has_report {
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
                })),
            );

        div()
            .id("explorer")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &OpenReport, _window, cx| {
                this.store.update(cx, |s, cx| s.pick_and_open(cx));
            }))
            .on_action(cx.listener(|this, _: &FocusFilter, window, cx| {
                this.toolbar
                    .update(cx, |t, cx| t.focus_text_filter(window, cx));
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
            // Main area (§7.2): [tree pane | splitter | row table] above the
            // identical panel, separated by a horizontal splitter. The group
            // is always rendered so the splitters keep their state; the panel
            // is simply not visible while it is hidden or collapsed.
            .child(
                div().flex_1().min_h(px(0.)).child(
                    v_resizable("identical-split")
                        .with_handle_appearance(Rc::new(move |handle, _window, _cx| {
                            Some(splitter_line(handle.axis(), handle.is_active(), palette))
                        }))
                        .child(resizable_panel().child(main_split))
                        .child(
                            resizable_panel()
                                .visible(show_identical && !identical_collapsed)
                                .size(px(IDENTICAL_DEFAULT_HEIGHT))
                                .size_range(px(IDENTICAL_MIN_HEIGHT)..identical_max)
                                .fixed()
                                .child(self.identical.clone()),
                        ),
                ),
            )
            // Collapsed: a full-width bar below the group restores the panel.
            .when(show_identical && identical_collapsed, |d| {
                d.child(self.identical.clone())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitter_limits() {
        assert_eq!(tree_max_width(px(1280.)), px(1080.));
        assert_eq!(tree_max_width(px(300.)), px(TREE_MIN_WIDTH));
        assert_eq!(identical_max_height(px(800.)), px(640.));
        assert_eq!(identical_max_height(px(200.)), px(IDENTICAL_MIN_HEIGHT));
    }
}
