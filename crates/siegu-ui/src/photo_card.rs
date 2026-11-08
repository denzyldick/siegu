use ely_gpui_component::{
    layout::{Card, CardHeader},
    primitives::{Icon, IconName},
    theme::{ActiveTheme, IconSize, TextSize},
    typography::Paragraph,
};
use gpui::{div, px, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window};

/// A single library photo: a thumbnail placeholder, title, and the date it was taken.
#[derive(IntoElement)]
pub struct PhotoCard {
    title: SharedString,
    date: Option<SharedString>,
}

impl PhotoCard {
    /// A card for the photo titled `title`.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            date: None,
        }
    }

    /// The date the photo was taken, shown under the title.
    pub fn date(mut self, date: impl Into<SharedString>) -> Self {
        self.date = Some(date.into());
        self
    }
}

impl RenderOnce for PhotoCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let mut header = CardHeader::new(self.title);
        if let Some(date) = self.date {
            header = header.description(date);
        }
        Card::new()
            .w(px(220.0))
            .h(px(260.0))
            .child(header)
            .child(
                div()
                    .size_full()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.colors.surface)
                    .text_color(theme.colors.fg_muted)
                    .text_size(theme.text_size(TextSize::Sm))
                    .child(Icon::new(IconName::Image).size(IconSize::Md)),
            )
            .child(
                Paragraph::new("Tap to open.")
                    .text_size(theme.text_size(TextSize::Sm))
                    .text_color(theme.colors.fg_subtle),
            )
    }
}
