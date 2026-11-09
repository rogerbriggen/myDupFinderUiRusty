//! Folder tree pane (§7.4): a virtualised, flattened tree with keyboard navigation.

use std::ops::Range;
use std::sync::Arc;

use dupreport_core::{FolderId, ROOT, Report};
use gpui::{
    AnyElement, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollStrategy, StatefulInteractiveElement, Styled, Subscription,
    UniformListScrollHandle, Window, actions, div, prelude::FluentBuilder, px, uniform_list,
};
use gpui_component::{Disableable, checkbox::Checkbox, tooltip::Tooltip};

use crate::app_state::ReportStore;
use crate::theme::Palette;
use crate::tree_view_model::TreeViewModel;

actions!(
    folder_tree,
    [
        /// Move the selection to the previous visible row.
        SelectPrev,
        /// Move the selection to the next visible row.
        SelectNext,
        /// Expand the selected folder, or move to its first child.
        ExpandOrFirstChild,
        /// Collapse the selected folder, or move to its parent.
        CollapseOrParent,
        /// Select the current row (Enter / Space).
        Activate,
    ]
);

/// Key context of the tree pane; its key bindings are scoped to it.
pub const KEY_CONTEXT: &str = "FolderTree";

/// Fixed height of a tree row.
const ROW_HEIGHT: f32 = 22.0;
const FONT_SIZE: f32 = 12.0;

/// The tree pane view.
pub struct FolderTreeView {
    store: Entity<ReportStore>,
    model: TreeViewModel,
    /// The report the model was built for; a different one resets the view.
    report: Option<Arc<Report>>,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    /// Selection the list was last scrolled to; a different one scrolls it into view.
    scrolled_to: Option<FolderId>,
    /// State of the "green on top" checkbox. Has no effect before step 11.
    green_on_top: bool,
    _store_subscription: Subscription,
}

impl FolderTreeView {
    /// Create a tree bound to `store`.
    pub fn new(store: Entity<ReportStore>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&store, |this, _, cx| {
            this.sync_report(cx);
            cx.notify();
        });
        let mut view = Self {
            store,
            model: TreeViewModel::empty(),
            report: None,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            scrolled_to: None,
            green_on_top: true,
            _store_subscription: subscription,
        };
        view.sync_report(cx);
        view
    }

    /// The visible rows, for tests.
    #[cfg(test)]
    pub(crate) fn model(&self) -> &TreeViewModel {
        &self.model
    }

    /// Statuses arrive with step 11; until then the checkbox stays disabled.
    fn has_statuses(&self) -> bool {
        false
    }

    /// Rebuild the model when the store holds a different report: only the root
    /// expanded, scrolled to the top.
    fn sync_report(&mut self, cx: &mut Context<Self>) {
        let current = self.store.read(cx).report.clone();
        let same = match (&self.report, &current) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if same {
            return;
        }
        self.model = current
            .as_ref()
            .map_or_else(TreeViewModel::empty, |r| TreeViewModel::new(&r.tree));
        self.report = current;
        self.scrolled_to = None;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
    }

    fn selected(&self, cx: &gpui::App) -> FolderId {
        self.store.read(cx).selected_folder
    }

    /// Select `id` (refilters the table); nothing happens when it already is.
    pub(crate) fn select(&mut self, id: FolderId, cx: &mut Context<Self>) {
        if self.selected(cx) != id {
            self.store.update(cx, |s, cx| s.select_folder(id, cx));
        }
        cx.notify();
    }

    /// Expand or collapse `id` without selecting it (leaves do nothing).
    pub(crate) fn toggle(&mut self, id: FolderId, cx: &mut Context<Self>) {
        if let Some(report) = self.report.clone() {
            self.model.toggle(&report.tree, id);
        }
        cx.notify();
    }

    /// ↑
    pub(crate) fn press_up(&mut self, cx: &mut Context<Self>) {
        if let Some(report) = self.report.clone() {
            let next = self.model.select_prev(&report.tree, self.selected(cx));
            self.select(next, cx);
        }
    }

    /// ↓
    pub(crate) fn press_down(&mut self, cx: &mut Context<Self>) {
        if let Some(report) = self.report.clone() {
            let next = self.model.select_next(&report.tree, self.selected(cx));
            self.select(next, cx);
        }
    }

    /// →
    pub(crate) fn press_right(&mut self, cx: &mut Context<Self>) {
        if let Some(report) = self.report.clone() {
            let selected = self.selected(cx);
            let next = self.model.key_right(&report.tree, selected);
            self.select(next, cx);
        }
    }

    /// ←
    pub(crate) fn press_left(&mut self, cx: &mut Context<Self>) {
        if let Some(report) = self.report.clone() {
            let selected = self.selected(cx);
            let next = self.model.key_left(&report.tree, selected);
            self.select(next, cx);
        }
    }

    /// Enter / Space: (re-)select the current row.
    pub(crate) fn press_activate(&mut self, cx: &mut Context<Self>) {
        let selected = self.selected(cx);
        self.select(selected, cx);
    }

    /// The elements of the visible `range` of rows.
    fn render_rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(report) = self.report.clone() else {
            return Vec::new();
        };
        let palette = Palette::of(window);
        let selected = self.selected(cx);
        let rows = self.model.rows_arc();
        range
            .filter_map(|ix| rows.get(ix).copied())
            .map(|(id, depth)| {
                let node = report.tree.node(id);
                let has_children = !node.children.is_empty();
                let glyph = if !has_children {
                    "·"
                } else if self.model.is_expanded(id) {
                    "▾"
                } else {
                    "▸"
                };
                let name = if id == ROOT { "/" } else { node.name.as_str() };
                let is_selected = id == selected;
                let tip = (id != ROOT).then(|| node.path.clone());
                div()
                    .id(("tree-row", id as usize))
                    .flex()
                    .flex_none()
                    .w_full()
                    .items_center()
                    .h(px(ROW_HEIGHT))
                    .pl(px(8. + depth as f32 * 14.))
                    .cursor_pointer()
                    .when(is_selected, |d| d.bg(palette.row_selected))
                    .when(!is_selected, |d| d.hover(|s| s.bg(palette.row_hover)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        window.focus(&this.focus, cx);
                        this.select(id, cx);
                    }))
                    .when_some(tip, |d, tip| {
                        d.tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                    })
                    .child(
                        div()
                            .id(("tree-toggle", id as usize))
                            .flex_none()
                            .w(px(16.))
                            .text_center()
                            .child(glyph)
                            // Toggle only: the click must not reach the row (select).
                            .on_click(cx.listener(move |this, _, window, cx| {
                                window.focus(&this.focus, cx);
                                this.toggle(id, cx);
                                cx.stop_propagation();
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(name.to_string()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .px_2()
                            .text_size(px(11.))
                            .opacity(0.6)
                            .child(node.total_file_count.to_string()),
                    )
                    .into_any_element()
            })
            .collect()
    }
}

impl Focusable for FolderTreeView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FolderTreeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::of(window);
        let count = self.model.rows().len();

        // Keep the selected row in view (keyboard moves, new report).
        let selected = self.selected(cx);
        if self.scrolled_to != Some(selected)
            && let Some(pos) = self.model.position(selected)
        {
            self.scroll.scroll_to_item(pos, ScrollStrategy::Nearest);
            self.scrolled_to = Some(selected);
        }

        div()
            .id("folder-tree")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.press_up(cx)))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.press_down(cx)))
            .on_action(cx.listener(|this, _: &ExpandOrFirstChild, _, cx| this.press_right(cx)))
            .on_action(cx.listener(|this, _: &CollapseOrParent, _, cx| this.press_left(cx)))
            .on_action(cx.listener(|this, _: &Activate, _, cx| this.press_activate(cx)))
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(FONT_SIZE))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .h(px(ROW_HEIGHT + 6.))
                    .px_2()
                    .bg(palette.surface)
                    .border_b_1()
                    .border_color(palette.border)
                    .child(
                        Checkbox::new("green-on-top")
                            .label("● green on top")
                            .checked(self.green_on_top)
                            .disabled(!self.has_statuses())
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.green_on_top = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                uniform_list(
                    "tree-rows",
                    count,
                    cx.processor(|this, range, window, cx| this.render_rows(range, window, cx)),
                )
                .flex_1()
                .track_scroll(&self.scroll),
            )
    }
}
