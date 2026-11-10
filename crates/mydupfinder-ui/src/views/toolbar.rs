//! Toolbar (§7.3): open button, progress, status text, path / hash filter inputs,
//! the descendants and hash column toggles, and the category / source chips.
//!
//! The store is the single source of truth for every filter value. The two text
//! inputs push their edits into the store (`InputEvent::Change`) and, whenever the
//! store changes, are re-synced from it, so a value set by anything other than
//! typing is always shown. Filters other than the sources persist across reports.

use dupreport_core::{Category, Report, Source};
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, Size,
    button::Button,
    checkbox::Checkbox,
    input::{Escape, Input, InputEvent, InputState},
    progress::Progress,
};

use crate::app_state::{LoadState, ReportStore};
use crate::theme::Palette;

/// Placeholder of the path / filename filter (§7.3).
pub const TEXT_PLACEHOLDER: &str = "path / filename filter";
/// Placeholder of the hash filter (§7.3).
pub const HASH_PLACEHOLDER: &str = "hash filter (≥8 chars)";

/// The toolbar view.
pub struct Toolbar {
    store: Entity<ReportStore>,
    pub(crate) text_input: Entity<InputState>,
    pub(crate) hash_input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

/// Whether the source chips are shown: only for a report with a second source.
pub(crate) fn show_source_chips(report: Option<&Report>) -> bool {
    report.is_some_and(|r| r.has_second_source)
}

/// Make `input` show `value` and be enabled iff `enabled`. `set_value` emits no
/// change event, so this cannot loop back into the store.
fn sync_input(
    input: &Entity<InputState>,
    value: &str,
    enabled: bool,
    window: &mut Window,
    cx: &mut App,
) {
    input.update(cx, |state, cx| {
        if state.value().as_ref() != value {
            state.set_value(value.to_string(), window, cx);
        }
        state.set_disabled(!enabled, cx);
    });
}

impl Toolbar {
    /// Create a toolbar bound to `store`.
    pub fn new(store: Entity<ReportStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text_input = cx.new(|cx| InputState::new(window, cx).placeholder(TEXT_PLACEHOLDER));
        let hash_input = cx.new(|cx| InputState::new(window, cx).placeholder(HASH_PLACEHOLDER));

        // Disabled until a report is open.
        sync_input(&text_input, "", false, window, cx);
        sync_input(&hash_input, "", false, window, cx);

        let subscriptions = vec![
            cx.observe_in(&store, window, |this, store, window, cx| {
                let s = store.read(cx);
                let (text, hash, enabled) = (
                    s.text_filter.clone(),
                    s.hash_filter.clone(),
                    s.report.is_some(),
                );
                sync_input(&this.text_input, &text, enabled, window, cx);
                sync_input(&this.hash_input, &hash, enabled, window, cx);
                cx.notify();
            }),
            cx.subscribe_in(
                &text_input,
                window,
                |this, input, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = input.read(cx).value().to_string();
                        this.store.update(cx, |s, cx| s.set_text_filter(value, cx));
                    }
                },
            ),
            cx.subscribe_in(
                &hash_input,
                window,
                |this, input, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = input.read(cx).value().to_string();
                        this.store.update(cx, |s, cx| s.set_hash_filter(value, cx));
                    }
                },
            ),
        ];
        Self {
            store,
            text_input,
            hash_input,
            _subscriptions: subscriptions,
        }
    }

    /// Focus the path / filename filter (Ctrl/Cmd+F). Does nothing without a report.
    pub fn focus_text_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.store.read(cx).report.is_some() {
            self.text_input.update(cx, |s, cx| s.focus(window, cx));
        }
    }

    /// Clear the path / filename filter, in the input and in the store (Esc).
    pub fn clear_text_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.text_input
            .update(cx, |s, cx| s.set_value(String::new(), window, cx));
        self.store.update(cx, |s, cx| s.set_text_filter("", cx));
    }

    /// Clear the hash filter, in the input and in the store (Esc).
    pub fn clear_hash_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hash_input
            .update(cx, |s, cx| s.set_value(String::new(), window, cx));
        self.store.update(cx, |s, cx| s.set_hash_filter("", cx));
    }
}

/// A pill-shaped toggle chip: a checkbox with a label, filled and outlined in blue
/// while on (§7.3).
fn chip(
    id: &'static str,
    label: &'static str,
    on: bool,
    enabled: bool,
    palette: Palette,
    on_toggle: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(8.))
        .py(px(1.))
        .rounded_full()
        .border_1()
        .border_color(if on {
            palette.chip_on_border
        } else {
            palette.chip_off_border
        })
        .when(on, |d| d.bg(palette.chip_on_bg))
        .child(
            Checkbox::new(id)
                .label(label)
                .checked(on)
                .disabled(!enabled)
                .on_click(on_toggle),
        )
}

/// Progress in percent, or `None` when the total size is unknown.
fn progress_percent(load: &LoadState) -> Option<f32> {
    let LoadState::Loading {
        progress: Some(p), ..
    } = load
    else {
        return None;
    };
    let total = p.total_bytes.filter(|t| *t > 0)?;
    Some((p.bytes_read as f64 / total as f64 * 100.0) as f32)
}

impl Render for Toolbar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::of(window);
        let mono = cx.theme().mono_font_family.clone();
        let store = self.store.read(cx);
        let loading = store.load.is_loading();
        let percent = progress_percent(&store.load);
        let show_hash = store.show_hash;
        let include_descendants = store.include_descendants;
        let has_report = store.report.is_some();
        let source_chips = show_source_chips(store.report.as_deref());
        let categories = store.categories.clone();
        let sources = store.sources.clone();
        let status = store
            .report
            .as_ref()
            .map(|r| format!("{} — {} rows", r.job_name, r.rows.len()));

        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .px_2()
            .py_1()
            .text_size(px(12.))
            .bg(palette.surface)
            .border_b_1()
            .border_color(palette.border)
            .child(
                Button::new("open-report")
                    .label(if loading {
                        "Loading…"
                    } else {
                        "Open report…"
                    })
                    .disabled(loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.store.update(cx, |s, cx| s.pick_and_open(cx));
                    })),
            )
            .when(loading, |d| {
                d.child(
                    Progress::new("load-progress")
                        .w(px(160.))
                        .loading(percent.is_none())
                        .value(percent.unwrap_or(0.)),
                )
            })
            .when_some(status, |d, text| d.child(div().child(text)))
            // Esc is not consumed by the input unless it clears itself, so it
            // bubbles up to these wrappers, which clear the filter and the store.
            .child(
                div()
                    .w(px(200.))
                    .on_action(cx.listener(|this, _: &Escape, window, cx| {
                        this.clear_text_filter(window, cx);
                    }))
                    .child(
                        Input::new(&self.text_input)
                            .with_size(Size::Small)
                            .text_size(px(12.))
                            .disabled(!has_report),
                    ),
            )
            .child(
                div()
                    .w(px(200.))
                    .on_action(cx.listener(|this, _: &Escape, window, cx| {
                        this.clear_hash_filter(window, cx);
                    }))
                    .child(
                        Input::new(&self.hash_input)
                            .with_size(Size::Small)
                            .font_family(mono)
                            .text_size(px(12.))
                            .disabled(!has_report),
                    ),
            )
            .child(
                Checkbox::new("include-descendants")
                    .label("descendants")
                    .checked(include_descendants)
                    .disabled(!has_report)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        let checked = *checked;
                        this.store
                            .update(cx, |s, cx| s.set_include_descendants(checked, cx));
                    })),
            )
            .child(
                Checkbox::new("show-hash")
                    .label("hash column")
                    .checked(show_hash)
                    .disabled(!has_report)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        let checked = *checked;
                        this.store.update(cx, |s, cx| s.set_show_hash(checked, cx));
                    })),
            )
            // The chips form one group: it wraps onto the next toolbar line as a
            // whole (not chip by chip), is right-aligned on its line (the
            // flexible spacer of §7.3) and only wraps internally when even a
            // full line is too narrow.
            .child(
                div()
                    .ml_auto()
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .items_center()
                    .gap(px(6.))
                    .children(Category::ALL.into_iter().map(|category| {
                        chip(
                            category_chip_id(category),
                            category.as_str(),
                            categories.contains(&category),
                            has_report,
                            palette,
                            cx.listener(move |this, checked: &bool, _, cx| {
                                let checked = *checked;
                                this.store
                                    .update(cx, |s, cx| s.set_category(category, checked, cx));
                            }),
                        )
                    }))
                    .when(source_chips, |d| {
                        d.children(Source::ALL.into_iter().map(|source| {
                            chip(
                                source_chip_id(source),
                                source.as_str(),
                                sources.contains(&source),
                                has_report,
                                palette,
                                cx.listener(move |this, checked: &bool, _, cx| {
                                    let checked = *checked;
                                    this.store
                                        .update(cx, |s, cx| s.set_source(source, checked, cx));
                                }),
                            )
                        }))
                    }),
            )
    }
}

/// Stable element id of a category chip.
fn category_chip_id(category: Category) -> &'static str {
    match category {
        Category::Duplicate => "chip-category-duplicate",
        Category::Moved => "chip-category-moved",
        Category::Unique => "chip-category-unique",
        Category::Missing => "chip-category-missing",
        Category::New => "chip-category-new",
        Category::Changed => "chip-category-changed",
    }
}

/// Stable element id of a source chip.
fn source_chip_id(source: Source) -> &'static str {
    match source {
        Source::Base => "chip-source-base",
        Source::Second => "chip-source-second",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dupreport_core::ParseProgress;

    #[test]
    fn percent_needs_total() {
        assert_eq!(progress_percent(&LoadState::Idle), None);
        let p = |bytes_read, total_bytes| LoadState::Loading {
            progress: Some(ParseProgress {
                rows_read: 1,
                bytes_read,
                total_bytes,
            }),
        };
        assert_eq!(
            progress_percent(&LoadState::Loading { progress: None }),
            None
        );
        assert_eq!(progress_percent(&p(10, None)), None);
        assert_eq!(progress_percent(&p(10, Some(0))), None);
        assert_eq!(progress_percent(&p(50, Some(200))), Some(25.0));
    }
}
