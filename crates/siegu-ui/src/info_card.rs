use ely_gpui_component::{
    layout::Card,
    primitives::{Icon, IconName},
    theme::{ActiveTheme, IconSize, Radius, TextSize},
    typography::Paragraph,
};
use gpui::prelude::FluentBuilder;
use gpui::{div, px, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window};

/// A compact card for a non-photo library entity: a person, a place, an album,
/// or a settings entry. Shows an icon, a title, and one line of detail.
#[derive(IntoElement)]
pub struct InfoCard {
    icon: IconName,
    title: SharedString,
    detail: Option<SharedString>,
}

impl InfoCard {
    /// A card with `icon` and `title`.
    pub fn new(icon: IconName, title: impl Into<SharedString>) -> Self {
        Self {
            icon,
            title: title.into(),
            detail: None,
        }
    }

    /// The supporting line under the title, e.g. `12 photos`.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl RenderOnce for InfoCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        Card::new().w(px(220.0)).h(px(96.0)).child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(40.0))
                        .h(px(40.0))
                        .rounded(theme.radius(Radius::Md))
                        .bg(theme.colors.bg)
                        .text_color(theme.colors.fg_muted)
                        .child(Icon::new(self.icon).size(IconSize::Md)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .min_w_0()
                        .child(
                            Paragraph::new(self.title)
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.colors.fg),
                        )
                        .when_some(self.detail, |column, detail| {
                            column.child(
                                Paragraph::new(detail)
                                    .text_size(theme.text_size(TextSize::Sm))
                                    .text_color(theme.colors.fg_muted),
                            )
                        }),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_collects_detail() {
        let card = InfoCard::new(IconName::Users, "Ana").detail("12 faces");
        assert_eq!(card.title.as_ref(), "Ana");
        assert_eq!(card.detail.as_deref(), Some("12 faces"));
    }
}
