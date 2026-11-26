use std::rc::Rc;

use ely_gpui_component::{
    layout::{AspectRatio, Card, CardHeader},
    primitives::{Icon, IconName, Image},
    theme::{ActiveTheme, IconSize, Radius, TextSize},
};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, App, ElementId, ImageSource, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
};

/// A single library photo: a thumbnail or placeholder, with optional caption and date.
#[derive(IntoElement)]
pub struct PhotoCard {
    id: ElementId,
    title: SharedString,
    date: Option<SharedString>,
    source: Option<ImageSource>,
    image_only: bool,
    on_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl PhotoCard {
    /// A card for the photo titled `title`, keyed by `id`.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            date: None,
            source: None,
            image_only: false,
            on_click: None,
        }
    }

    /// The date the photo was taken, shown under the title.
    pub fn date(mut self, date: impl Into<SharedString>) -> Self {
        self.date = Some(date.into());
        self
    }

    /// Show only the media, without caption or date text. This is the library
    /// grid treatment: the date lives in the section header instead.
    pub fn image_only(mut self) -> Self {
        self.image_only = true;
        self
    }

    /// The thumbnail image to display inside the card.
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

impl RenderOnce for PhotoCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let aspect = if self.image_only { 1.0 } else { 4.0 / 3.0 };
        let height = if self.image_only {
            px(220.0)
        } else {
            px(260.0)
        };

        let thumb = AspectRatio::new(aspect)
            .rounded(theme.radius(Radius::Sm))
            .overflow_hidden()
            .bg(theme.colors.sunken)
            .child(match self.source {
                Some(source) => Image::new((self.id.clone(), "thumb"), source)
                    .size_full()
                    .into_any_element(),
                None => div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.colors.surface)
                    .text_color(theme.colors.fg_muted)
                    .text_size(theme.text_size(TextSize::Sm))
                    .child(Icon::new(IconName::Image).size(IconSize::Md))
                    .into_any_element(),
            });

        let mut card = Card::new().w(px(220.0)).h(height);
        if !self.image_only {
            let mut header = CardHeader::new(self.title);
            if let Some(date) = self.date {
                header = header.description(date);
            }
            card = card.child(header);
        }
        card = card.child(thumb);

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
    use std::path::PathBuf;

    #[test]
    fn card_collects_metadata_and_source() {
        let card = PhotoCard::new("p1", "Sunset")
            .date("2026-08-01")
            .source(PathBuf::from("/tmp/sunset.jpg"));
        assert_eq!(card.title.as_ref(), "Sunset");
        assert_eq!(card.date.as_deref(), Some("2026-08-01"));
        assert!(card.source.is_some());
        assert!(!card.image_only);
    }

    #[test]
    fn image_only_card_drops_caption_and_date() {
        let card = PhotoCard::new("p1", "Sunset")
            .date("2026-08-01")
            .image_only();
        assert!(card.image_only);
    }
}
