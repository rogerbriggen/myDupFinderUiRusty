//! Virtualised row table over `ReportStore::filtered` (§7.5).

use std::sync::Arc;

use dupreport_core::{Row, RowIdx, format::format_hash_short, format::format_size};
use gpui::{
    ClipboardItem, Context, Div, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Subscription,
    UniformListScrollHandle, Window, div, prelude::FluentBuilder, px, uniform_list,
};
use gpui_component::tooltip::Tooltip;

use crate::app_state::ReportStore;
use crate::theme::Palette;

/// Fixed height of every table row.
pub const ROW_HEIGHT: f32 = 22.0;
const FONT_SIZE: f32 = 12.0;

#[derive(Clone, Copy)]
enum Col {
    Folder,
    Filename,
    Size,
    Category,
    Hash,
    Source,
    Group,
}

/// Apply the column's width rules (§7.5) so header and body cells line up.
fn sized<E: Styled>(mut cell: E, col: Col) -> E {
    match col {
        Col::Folder => {
            cell = cell.flex_1().min_w(px(160.));
            cell.style().flex_grow = Some(2.);
        }
        Col::Filename => cell = cell.flex_1().min_w(px(140.)),
        Col::Size => cell = cell.w(px(90.)).flex_none().text_right(),
        Col::Category => cell = cell.w(px(100.)).flex_none(),
        Col::Hash => cell = cell.w(px(130.)).flex_none(),
        Col::Source => cell = cell.w(px(80.)).flex_none(),
        Col::Group => cell = cell.w(px(70.)).flex_none().text_right(),
    }
    cell
}

fn text_cell<E: Styled>(cell: E, col: Col) -> E {
    // Common styles first so the column's own `min_w` (§7.5) wins over the
    // `min_w(0)` that lets fixed cells clip instead of growing with content.
    let cell = cell
        .px_1()
        .min_w(px(0.))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis();
    sized(cell, col)
}

/// A cell showing `text`, with the same text as hover tooltip when `tooltip` is set.
fn body_cell(
    id: (&'static str, usize),
    col: Col,
    text: impl Into<SharedString>,
    tooltip: Option<SharedString>,
) -> impl IntoElement {
    text_cell(div().id(id), col)
        .child(text.into())
        .when_some(tooltip, |d, tip| {
            d.tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        })
}

fn header_row(show_hash: bool, palette: &Palette) -> Div {
    let head = |text: &'static str, col| text_cell(div(), col).child(text);
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(ROW_HEIGHT))
        .bg(palette.surface)
        .border_b_1()
        .border_color(palette.border)
        .text_color(palette.muted_fg)
        .child(head("Folder", Col::Folder))
        .child(head("Filename", Col::Filename))
        .child(head("Size", Col::Size))
        .child(head("Category", Col::Category))
        .when(show_hash, |d| d.child(head("Hash", Col::Hash)))
        .child(head("Source", Col::Source))
        .child(head("Group", Col::Group))
}

/// One body row. Formats only this row's data; the `Row` itself is only borrowed.
fn body_row(ix: usize, row: &Row, show_hash: bool, palette: Palette) -> impl IntoElement + use<> {
    // Ids only have to be unique among siblings; each cell kind gets its own base.
    let folder = row.folder();
    let filename = row.filename();
    let hash_cell = show_hash.then(|| {
        let full = row.hash.clone();
        let tip = if full.is_empty() {
            None
        } else {
            Some(SharedString::from(format!("{full} (click to copy)")))
        };
        text_cell(div().id(("hash", ix)), Col::Hash)
            .child(SharedString::from(format_hash_short(&row.hash).to_string()))
            .when(!full.is_empty(), |d| {
                d.cursor_pointer().on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(full.clone()));
                })
            })
            .when_some(tip, |d, tip| {
                d.tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
            })
    });
    div()
        .id(("row", ix))
        .flex()
        .flex_none()
        // Rows must span the list width like the header, otherwise they size to
        // their content and the columns no longer line up.
        .w_full()
        .items_center()
        .h(px(ROW_HEIGHT))
        .border_b_1()
        .border_color(palette.border)
        .hover(|s| s.bg(palette.row_hover))
        .child(body_cell(
            ("folder", ix),
            Col::Folder,
            folder.to_string(),
            Some(folder.to_string().into()),
        ))
        .child(body_cell(
            ("filename", ix),
            Col::Filename,
            filename.to_string(),
            Some(filename.to_string().into()),
        ))
        .child(body_cell(
            ("size", ix),
            Col::Size,
            format_size(row.size),
            Some(format!("{} B", row.size).into()),
        ))
        .child(
            text_cell(div(), Col::Category)
                .text_color(palette.category(row.category))
                .child(row.category.as_str()),
        )
        .children(hash_cell)
        .child(text_cell(div(), Col::Source).child(row.source.as_str()))
        .child(text_cell(div(), Col::Group).child(row.group_id.to_string()))
}

/// The table view.
pub struct RowTable {
    store: Entity<ReportStore>,
    scroll: UniformListScrollHandle,
    /// The `filtered` vector the list was last rendered with; a different one
    /// resets the scroll position so it can never point past the end.
    last_filtered: Option<Arc<Vec<RowIdx>>>,
    _store_subscription: Subscription,
}

impl RowTable {
    /// Create a table bound to `store`.
    pub fn new(store: Entity<ReportStore>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            store,
            scroll: UniformListScrollHandle::new(),
            last_filtered: None,
            _store_subscription: subscription,
        }
    }
}

impl Render for RowTable {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::of(window);
        let store = self.store.read(cx);
        let Some(report) = store.report.clone() else {
            return div().size_full().into_any_element();
        };
        let filtered = store.filtered.clone();
        let show_hash = store.show_hash;
        let folder = &report.tree.node(store.selected_folder).path;

        let count = filtered.len();
        let mut meta = format!(
            "Showing {count} {}",
            if count == 1 { "row" } else { "rows" }
        );
        meta.push_str(" — folder: ");
        meta.push_str(if folder.is_empty() { "(root)" } else { folder });

        let changed = self
            .last_filtered
            .as_ref()
            .is_none_or(|prev| !Arc::ptr_eq(prev, &filtered));
        if changed {
            self.last_filtered = Some(filtered.clone());
            self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        }

        let list_filtered = filtered.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(FONT_SIZE))
            .child(
                div()
                    .flex_none()
                    .px_2()
                    .py_1()
                    .text_color(palette.muted_fg)
                    .child(meta),
            )
            .child(header_row(show_hash, &palette))
            .child(
                uniform_list("rows", count, move |range, _window, _cx| {
                    range
                        .map(|ix| {
                            let row = &report.rows[list_filtered[ix] as usize];
                            body_row(ix, row, show_hash, palette)
                        })
                        .collect::<Vec<_>>()
                })
                .flex_1()
                .track_scroll(&self.scroll),
            )
            .into_any_element()
    }
}
