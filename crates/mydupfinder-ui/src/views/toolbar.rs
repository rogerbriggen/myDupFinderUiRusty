//! Toolbar (§7.3). Step 7: open button, progress, status text and the hash
//! column toggle. The filter inputs and chips arrive in step 10.

use gpui::{
    Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{Disableable, button::Button, checkbox::Checkbox, progress::Progress};

use crate::app_state::{LoadState, ReportStore};
use crate::theme::Palette;

/// The toolbar view.
pub struct Toolbar {
    store: Entity<ReportStore>,
    _store_subscription: Subscription,
}

impl Toolbar {
    /// Create a toolbar bound to `store`.
    pub fn new(store: Entity<ReportStore>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            store,
            _store_subscription: subscription,
        }
    }
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
        let store = self.store.read(cx);
        let loading = store.load.is_loading();
        let percent = progress_percent(&store.load);
        let show_hash = store.show_hash;
        let has_report = store.report.is_some();
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
