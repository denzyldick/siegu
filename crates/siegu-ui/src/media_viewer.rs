use std::rc::Rc;

use crate::i18n;
use ely_gpui_component::{
    buttons::Button,
    media::ImageViewer,
    overlays::Dialog,
    primitives::{Icon, IconName, Image},
    theme::{ActiveTheme, IconSize, TextSize},
    typography::Paragraph,
};
use gpui::{
    div, px, App, ElementId, ImageSource, InteractiveElement, IntoElement, KeyDownEvent, ObjectFit,
    ParentElement, RenderOnce, Resource, SharedString, Styled, Window,
};

/// A fullscreen modal viewer for a single library photo, with zoom, pan, and
/// rotation via `ely::media::ImageViewer`, or a thumbnail fallback when the
/// full-size file is not present locally.
#[derive(IntoElement)]
pub struct MediaViewer {
    id: ElementId,
    title: SharedString,
    detail: Option<SharedString>,
    source: Option<Resource>,
    fallback: Option<ImageSource>,
    favorite: bool,
    on_toggle_favorite: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_prev: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_next: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_show_in_explorer: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_close: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl MediaViewer {
    /// Creates a viewer dialog keyed by `id` for the photo titled `title`.
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            source: None,
            fallback: None,
            favorite: false,
            on_toggle_favorite: None,
            on_prev: None,
            on_next: None,
            on_show_in_explorer: None,
            on_close: Rc::new(on_close),
        }
    }

    /// Supporting subtitle shown under the title (e.g. capture date).
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Full-resolution image resource (typically a local `PathBuf`).
    pub fn source(mut self, source: impl Into<Resource>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Cached thumbnail shown when the full-resolution file is unavailable locally.
    pub fn fallback(mut self, fallback: impl Into<ImageSource>) -> Self {
        self.fallback = Some(fallback.into());
        self
    }

    /// Whether the photo is currently marked as a favorite.
    pub fn favorite(mut self, favorite: bool) -> Self {
        self.favorite = favorite;
        self
    }

    /// Invoked when the user clicks the Favorite toggle action.
    pub fn on_toggle_favorite(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_favorite = Some(Rc::new(handler));
        self
    }

    /// Invoked when the user clicks Previous.
    pub fn on_prev(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_prev = Some(Rc::new(handler));
        self
    }

    /// Invoked when the user clicks Next.
    pub fn on_next(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_next = Some(Rc::new(handler));
        self
    }

    /// Invoked when the user asks to reveal the original file in the OS file
    /// manager. Omit it when the file is not available locally.
    pub fn on_show_in_explorer(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_show_in_explorer = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for MediaViewer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let on_close = self.on_close;
        let on_prev = self.on_prev.clone();
        let on_next = self.on_next.clone();
        let mut dialog = Dialog::new(self.id.clone(), self.title, move |window, cx| {
            on_close(window, cx);
        })
        .fullscreen();

        if let Some(detail) = self.detail {
            dialog = dialog.detail(detail);
        }

        if let Some(on_toggle_favorite) = self.on_toggle_favorite {
            let fav = self.favorite;
            dialog = dialog.action(move |_| {
                let label = if fav { "Favorited" } else { "Favorite" };
                let btn = Button::new("mv-favorite", label)
                    .icon(IconName::Heart)
                    .on_click(move |_, window, cx| on_toggle_favorite(window, cx));
                if fav {
                    btn.primary()
                } else {
                    btn
                }
            });
        }

        if let Some(on_show_in_explorer) = self.on_show_in_explorer.clone() {
            let label = i18n::text(cx, "media_viewer.show_in_explorer", &[]);
            dialog = dialog.action(move |_| {
                let handler = on_show_in_explorer.clone();
                Button::new("mv-show-in-explorer", label)
                    .icon(IconName::FolderOpen)
                    .on_click(move |_, window, cx| handler(window, cx))
            });
        }

        if let Some(on_prev) = on_prev.clone() {
            dialog = dialog.action(move |_| {
                Button::new("mv-prev", "Previous")
                    .icon(IconName::ArrowLeft)
                    .on_click(move |_, window, cx| on_prev(window, cx))
            });
        }

        if let Some(on_next) = on_next.clone() {
            dialog = dialog.action(move |_| {
                Button::new("mv-next", "Next")
                    .icon(IconName::ArrowRight)
                    .on_click(move |_, window, cx| on_next(window, cx))
            });
        }

        let stage = match (self.source, self.fallback) {
            (Some(resource), _) => div()
                .w_full()
                .h_full()
                .min_h(px(420.0))
                .child(ImageViewer::new((self.id, "viewer"), resource))
                .into_any_element(),
            (None, Some(fallback)) => div()
                .w_full()
                .h_full()
                .min_h(px(420.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    Image::new((self.id, "fallback"), fallback)
                        .fit(ObjectFit::Contain)
                        .w_full()
                        .h(px(360.0)),
                )
                .child(
                    Paragraph::new("Showing cached preview — original file is not stored locally.")
                        .text_size(theme.text_size(TextSize::Sm))
                        .text_color(theme.colors.fg_muted),
                )
                .into_any_element(),
            (None, None) => div()
                .w_full()
                .min_h(px(360.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::ImageOff)
                        .size(IconSize::Lg)
                        .color(theme.colors.fg_subtle),
                )
                .child(
                    Paragraph::new("Original file is not available on this device.")
                        .text_size(theme.text_size(TextSize::Sm))
                        .text_color(theme.colors.fg_muted),
                )
                .into_any_element(),
        };

        let dialog = dialog.child(stage);
        let (keys_prev, keys_next) = (on_prev, on_next);
        div()
            .size_full()
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let callback = match event.keystroke.key.as_str() {
                    "left" => keys_prev.as_ref(),
                    "right" => keys_next.as_ref(),
                    _ => return,
                };
                if let Some(callback) = callback {
                    cx.stop_propagation();
                    callback(window, cx);
                }
            })
            .child(dialog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn media_viewer_collects_fields() {
        let viewer = MediaViewer::new("mv", "Lake", |_, _| {})
            .detail("2026-08-01")
            .source(PathBuf::from("/tmp/lake.jpg"))
            .on_prev(|_, _| {})
            .on_next(|_, _| {})
            .on_show_in_explorer(|_, _| {});
        assert_eq!(viewer.title.as_ref(), "Lake");
        assert_eq!(viewer.detail.as_deref(), Some("2026-08-01"));
        assert!(viewer.source.is_some());
        assert!(viewer.on_prev.is_some());
        assert!(viewer.on_next.is_some());
        assert!(viewer.on_show_in_explorer.is_some());
    }
}
