use std::rc::Rc;

use ely_gpui_component::{
    layout::Card,
    primitives::{Icon, IconName, Image},
    theme::{ActiveTheme, IconSize, Radius, TextSize},
    typography::Paragraph,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, App, ElementId, ImageSource, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
};

/// A compact card for a non-photo library entity: a person, a place, an album,
/// or a settings entry. Shows a thumbnail (or icon), a title, and one line of detail.
#[derive(IntoElement)]
pub struct InfoCard {
    id: ElementId,
    icon: IconName,
    title: SharedString,
    detail: Option<SharedString>,
    source: Option<ImageSource>,
    on_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl InfoCard {
    /// A card keyed by `id` with `icon` and `title`.
    pub fn new(id: impl Into<ElementId>, icon: IconName, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon,
            title: title.into(),
            detail: None,
            source: None,
            on_click: None,
        }
    }

    /// The supporting line under the title, e.g. `12 photos`.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Optional thumbnail image shown in place of the leading icon.
    pub fn source(mut self, source: impl Into<ImageSource>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Invoked when the user clicks the card.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for InfoCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let leading = match self.source {
            Some(source) => Image::new((self.id.clone(), "thumb"), source)
                .w(px(44.0))
                .h(px(44.0))
                .flex_none()
                .rounded(theme.radius(Radius::Md))
                .into_any_element(),
            None => div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .w(px(44.0))
                .h(px(44.0))
                .rounded(theme.radius(Radius::Md))
                .bg(theme.colors.bg)
                .text_color(theme.colors.fg_muted)
                .child(Icon::new(self.icon).size(IconSize::Md))
                .into_any_element(),
        };

        let card = Card::new().w(px(220.0)).h(px(96.0)).child(
            div().flex().items_center().gap_3().child(leading).child(
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
        );

        div()
            .id(self.id)
            .when_some(self.on_click, |el, handler| {
                el.cursor_pointer()
                    .on_click(move |_, window, cx| handler(window, cx))
            })
            .child(card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_collects_detail() {
        let card = InfoCard::new("ana", IconName::Users, "Ana").detail("12 faces");
        assert_eq!(card.title.as_ref(), "Ana");
        assert_eq!(card.detail.as_deref(), Some("12 faces"));
    }
}
