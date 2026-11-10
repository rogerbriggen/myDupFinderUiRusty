//! Folder tree pane (§7.4): a virtualised, flattened tree with keyboard navigation.

use std::ops::Range;
use std::sync::Arc;

use dupreport_core::{FolderId, IdenticalStatus, ROOT, Report};
use gpui::{
    AnyElement, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollStrategy, StatefulInteractiveElement, Styled, Subscription,
    UniformListScrollHandle, Window, actions, div, prelude::FluentBuilder, px, uniform_list,
};
use gpui_component::{Disableable, checkbox::Checkbox, tooltip::Tooltip};

use crate::app_state::ReportStore;
use crate::theme::Palette;
use crate::tree_view_model::{TreeViewModel, UNKNOWN_RANK};

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
/// Diameter of the identical-status dot (§7.4).
const DOT_SIZE: f32 = 8.0;

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
    /// "auto-expand chains" setting; survives report changes (the model does not).
    auto_expand_chains: bool,
    /// State of the "green on top" checkbox (default on, kept across reports).
    green_on_top: bool,
    /// The statuses the model is currently ranked with; `None` = alphabetical.
    ranked_with: Option<Arc<Vec<Option<IdenticalStatus>>>>,
    _store_subscription: Subscription,
}

impl FolderTreeView {
    /// Create a tree bound to `store`.
    pub fn new(store: Entity<ReportStore>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&store, |this, _, cx| {
            this.sync_report(cx);
            this.sync_ranks(cx);
            cx.notify();
        });
        let mut view = Self {
            store,
            model: TreeViewModel::empty(),
            report: None,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            scrolled_to: None,
            auto_expand_chains: true,
            green_on_top: true,
            ranked_with: None,
            _store_subscription: subscription,
        };
        view.sync_report(cx);
        view.sync_ranks(cx);
        view
    }

    /// The visible rows, for tests.
    #[cfg(test)]
    pub(crate) fn model(&self) -> &TreeViewModel {
        &self.model
    }

    /// Whether a scan produced statuses (dots are shown, the checkbox is enabled).
    fn has_statuses(&self, cx: &gpui::App) -> bool {
        self.store.read(cx).has_statuses()
    }

    /// Bring the model's ordering in line with the "green on top" checkbox and
    /// the store's statuses (ranks: green 0, yellow 1, red 2, unknown 3). Only
    /// the order changes: expansion and selection survive.
    fn sync_ranks(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let want = (self.green_on_top && store.has_statuses()).then(|| store.statuses.clone());
        let same = match (&self.ranked_with, &want) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if same {
            return;
        }
        let Some(report) = self.report.clone() else {
            return;
        };
        let ranks = want.as_ref().map(|statuses| {
            statuses
                .iter()
                .map(|s| s.map_or(UNKNOWN_RANK, IdenticalStatus::rank))
                .collect()
        });
        self.model.set_ranks(&report.tree, ranks);
        self.ranked_with = want;
        // The selected row may have moved: scroll it into view again.
        self.scrolled_to = None;
    }

    /// Switch "green on top" on or off.
    pub(crate) fn set_green_on_top(&mut self, on: bool, cx: &mut Context<Self>) {
        self.green_on_top = on;
        self.sync_ranks(cx);
        cx.notify();
    }

    /// Select `id`, expand its ancestors and scroll it into view (identical
    /// panel links, §7.6).
    pub(crate) fn reveal_and_select(&mut self, id: FolderId, cx: &mut Context<Self>) {
        if let Some(report) = self.report.clone() {
            self.model.reveal(&report.tree, id);
        }
        // Scroll even when `id` was already selected but hidden.
        self.scrolled_to = None;
        self.select(id, cx);
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
        self.model = current.as_ref().map_or_else(TreeViewModel::empty, |r| {
            TreeViewModel::new(&r.tree, self.auto_expand_chains)
        });
        self.report = current;
        // The new model starts alphabetical; `sync_ranks` re-applies a ranking.
        self.ranked_with = None;
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

    /// Turn chain auto-expansion on or off for this and every later report.
    pub(crate) fn set_auto_expand_chains(&mut self, on: bool, cx: &mut Context<Self>) {
        self.auto_expand_chains = on;
        self.model.set_auto_expand_chains(on);
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
        let store = self.store.read(cx);
        let statuses = store.has_statuses().then(|| store.statuses.clone());
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
                // Status dot: only once a scan produced statuses.
                let dot = statuses
                    .as_ref()
                    .map(|s| s.get(id as usize).copied().flatten());
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
                            // Toggle only: the click must not reach the row
                            // (select). A leaf's `·` has no handler, so its
                            // click falls through and selects the row.
                            .when(has_children, |d| {
                                d.on_click(cx.listener(move |this, _, window, cx| {
                                    window.focus(&this.focus, cx);
                                    this.toggle(id, cx);
                                    cx.stop_propagation();
                                }))
                            }),
                    )
                    .when_some(dot, |d, status| {
                        d.child(
                            div()
                                .id(("tree-dot", id as usize))
                                .flex_none()
                                .size(px(DOT_SIZE))
                                .mr(px(6.))
                                .rounded_full()
                                // A folder without a status (unknown) gets a hollow dot.
                                .when_some(status, |d, s| {
                                    let tip = s.tooltip();
                                    d.bg(palette.status[s.rank() as usize]).tooltip(
                                        move |window, cx| Tooltip::new(tip).build(window, cx),
                                    )
                                })
                                .when(status.is_none(), |d| {
                                    d.border_1().border_color(palette.muted_fg)
                                }),
                        )
                    })
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
        let has_statuses = self.has_statuses(cx);

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
                    // Wraps onto a second line when the pane is narrow (min 160 px).
                    .flex_wrap()
                    .items_center()
                    .min_h(px(ROW_HEIGHT + 6.))
                    .px_2()
                    .py_1()
                    .gap_x(px(12.))
                    .gap_y(px(4.))
                    .bg(palette.surface)
                    .border_b_1()
                    .border_color(palette.border)
                    .child(
                        Checkbox::new("green-on-top")
                            .label("● green on top")
                            .checked(self.green_on_top)
                            .disabled(!has_statuses)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.set_green_on_top(*checked, cx);
                            })),
                    )
                    .child(
                        Checkbox::new("auto-expand-chains")
                            .label("auto-expand chains")
                            .checked(self.auto_expand_chains)
                            .tooltip(
                                "Expanding a folder also expands its chain of single subfolders",
                            )
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.set_auto_expand_chains(*checked, cx);
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
