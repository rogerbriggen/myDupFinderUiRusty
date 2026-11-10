//! Identical-folders panel (§7.6): a virtualised table of the folder pairs found
//! by the scan, shown at the bottom of the window.
//!
//! The panel decides only what is inside it. Its height and splitter are the
//! explorer's business (it places the panel in a vertical resizable group, or
//! below the group as a bar while the panel is collapsed).

use std::ops::Range;
use std::sync::Arc;

use dupreport_core::{IdenticalFolderPair, format::format_size};
use gpui::{
    Context, Div, Entity, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, UniformListScrollHandle, Window, div,
    prelude::FluentBuilder, px, uniform_list,
};
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    tooltip::Tooltip,
};

use crate::app_state::ReportStore;
use crate::theme::Palette;
use crate::views::folder_tree::FolderTreeView;

/// Header text of the expanded panel.
pub const TITLE: &str = "IDENTICAL FOLDERS";
/// Text of the bar shown while the panel is collapsed.
/// (`▲`/`▼` instead of the brief's `▴`/`▾`: those are the *small* triangles and
/// render as a barely visible dot at this size.)
pub const COLLAPSED_BAR: &str = "▲ Identical folders";
/// Shown while a scan runs and no pair is known yet.
pub const SCANNING_TEXT: &str = "Scanning…";
/// Shown after a finished scan that found no pair.
pub const EMPTY_TEXT: &str = "No identical folder pairs found.";

const ROW_HEIGHT: f32 = 22.0;
const FONT_SIZE: f32 = 12.0;

/// What the panel body shows for `pairs`: a message instead of the table when
/// there is no pair.
pub(crate) fn empty_message(pairs: &[IdenticalFolderPair], loading: bool) -> Option<&'static str> {
    if !pairs.is_empty() {
        None
    } else if loading {
        Some(SCANNING_TEXT)
    } else {
        Some(EMPTY_TEXT)
    }
}

#[derive(Clone, Copy)]
enum Col {
    Base,
    Second,
    Files,
    Size,
}

/// One table cell (also used for the header): the two folder columns share the
/// width, the number columns are fixed and right-aligned.
fn cell<E: Styled>(el: E, col: Col) -> E {
    let el = el
        .px_1()
        .min_w(px(0.))
        .flex()
        .overflow_hidden()
        .whitespace_nowrap();
    match col {
        Col::Base | Col::Second => el.flex_1().min_w(px(160.)),
        Col::Files => el.w(px(80.)).flex_none().justify_end(),
        Col::Size => el.w(px(100.)).flex_none().justify_end(),
    }
}

fn header_row(palette: &Palette) -> Div {
    let head = |text: &'static str, col| cell(div(), col).child(text);
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(ROW_HEIGHT))
        .bg(palette.surface)
        .border_b_1()
        .border_color(palette.border)
        .text_color(palette.muted_fg)
        .child(head("Base folder", Col::Base))
        .child(head("Second folder", Col::Second))
        .child(head("Files", Col::Files))
        .child(head("Total size", Col::Size))
}

/// The panel view.
pub struct IdenticalPanel {
    store: Entity<ReportStore>,
    tree: Entity<FolderTreeView>,
    /// Collapsed to the `▴ Identical folders` bar. Kept across scans and reports.
    pub(crate) collapsed: bool,
    scroll: UniformListScrollHandle,
    /// The pair list the table was last rendered with; a different one scrolls to the top.
    last_pairs: Option<Arc<Vec<IdenticalFolderPair>>>,
    _store_subscription: Subscription,
}

impl IdenticalPanel {
    /// Create a panel bound to `store`; folder links drive `tree`.
    pub fn new(
        store: Entity<ReportStore>,
        tree: Entity<FolderTreeView>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            store,
            tree,
            collapsed: false,
            scroll: UniformListScrollHandle::new(),
            last_pairs: None,
            _store_subscription: subscription,
        }
    }

    /// Collapse to the bar or restore the panel.
    pub(crate) fn set_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        if self.collapsed != collapsed {
            self.collapsed = collapsed;
            cx.notify();
        }
    }

    /// A folder link was clicked: select the folder in the tree, expand its
    /// ancestors and scroll it into view. Unknown paths do nothing.
    pub(crate) fn activate_folder(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(id) = self
            .store
            .read(cx)
            .report
            .as_ref()
            .and_then(|r| r.tree.find(path))
        else {
            return;
        };
        self.tree.update(cx, |t, cx| t.reveal_and_select(id, cx));
    }

    /// A clickable, link-styled folder path with the full path as tooltip.
    fn link(
        &self,
        id: (&'static str, usize),
        path: &str,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let target = path.to_string();
        let tip = SharedString::from(path.to_string());
        div()
            .id(id)
            .min_w(px(0.))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_color(palette.link)
            .underline()
            .cursor_pointer()
            .child(SharedString::from(path.to_string()))
            .on_click(cx.listener(move |this, _, _, cx| this.activate_folder(&target, cx)))
            .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
    }
}

impl Render for IdenticalPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::of(window);

        if self.collapsed {
            return div()
                .id("identical-bar")
                .flex_none()
                .w_full()
                .h(px(ROW_HEIGHT + 4.))
                .flex()
                .items_center()
                .px_2()
                .text_size(px(FONT_SIZE))
                .bg(palette.surface)
                .border_t_1()
                .border_color(palette.border)
                .cursor_pointer()
                .hover(|s| s.bg(palette.row_hover))
                .on_click(cx.listener(|this, _, _, cx| this.set_collapsed(false, cx)))
                .child(COLLAPSED_BAR)
                .into_any_element();
        }

        let store = self.store.read(cx);
        let pairs = store.identical.clone();
        let message = empty_message(&pairs, store.identical_loading);
        let changed = self
            .last_pairs
            .as_ref()
            .is_none_or(|prev| !Arc::ptr_eq(prev, &pairs));
        if changed {
            self.last_pairs = Some(pairs.clone());
            self.scroll.scroll_to_item(0, gpui::ScrollStrategy::Top);
        }
        let count = pairs.len();

        div()
            .id("identical-panel")
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(FONT_SIZE))
            .border_t_1()
            .border_color(palette.border)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(ROW_HEIGHT + 4.))
                    .px_2()
                    .bg(palette.surface)
                    .text_color(palette.muted_fg)
                    .child(TITLE)
                    .child(
                        Button::new("identical-collapse")
                            .ghost()
                            .xsmall()
                            .label("▼")
                            .tooltip("Collapse")
                            .on_click(cx.listener(|this, _, _, cx| this.set_collapsed(true, cx))),
                    ),
            )
            .map(|d| match message {
                Some(text) => d.child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(palette.muted_fg)
                        .child(text),
                ),
                None => d.child(header_row(&palette)).child(
                    uniform_list(
                        "identical-rows",
                        count,
                        cx.processor(move |this, range: Range<usize>, _window, cx| {
                            range
                                .filter_map(|ix| pairs.get(ix).map(|p| (ix, p)))
                                .map(|(ix, pair)| {
                                    div()
                                        .id(("pair", ix))
                                        .flex()
                                        .flex_none()
                                        .w_full()
                                        .items_center()
                                        .h(px(ROW_HEIGHT))
                                        .border_b_1()
                                        .border_color(palette.border)
                                        .hover(|s| s.bg(palette.row_hover))
                                        .child(cell(div(), Col::Base).child(this.link(
                                            ("base", ix),
                                            &pair.folder_a,
                                            palette,
                                            cx,
                                        )))
                                        .child(cell(div(), Col::Second).child(this.link(
                                            ("second", ix),
                                            &pair.folder_b,
                                            palette,
                                            cx,
                                        )))
                                        .child(
                                            cell(div(), Col::Files)
                                                .child(pair.file_count.to_string()),
                                        )
                                        .child(
                                            cell(div(), Col::Size)
                                                .child(format_size(pair.total_size)),
                                        )
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .flex_1()
                    .track_scroll(&self.scroll),
                ),
            })
            .into_any_element()
    }
}
