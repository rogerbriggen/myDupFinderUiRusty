//! Root layout of the application (§7.2). Currently only the empty-state placeholder.

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};

/// Text shown while no report is open (§7.2).
pub const PLACEHOLDER: &str =
    "No report open. Click Open report… in the toolbar above to load a dupReport.csv.";

/// The main explorer view.
#[derive(Default)]
pub struct Explorer;

impl Render for Explorer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(PLACEHOLDER)
    }
}
