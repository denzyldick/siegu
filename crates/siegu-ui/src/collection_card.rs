use std::rc::Rc;

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    data_display::{Badge, Tone},
    primitives::{FocusRing, Icon, IconName, Image as ElyImage},
    theme::{ActiveTheme, ControlSize, IconSize, Radius},
    typography::EllipsisTooltip,
};
use gpui::{
    div, hsla, prelude::FluentBuilder, px, App, ElementId, ImageSource, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Window,
};

/// A collection tile with a cover, a quiet count chip, and contextual actions.
/// The whole tile opens the collection; rename and delete sit above it so they
/// never compete with the primary action.
#[derive(IntoElement)]
pub struct CollectionCard {
    id: ElementId,
    title: SharedString,
    detail: SharedString,
    badge: Option<(SharedString, Tone)>,
    source: Option<ImageSource>,
    rename_label: SharedString,
    delete_label: SharedString,
    on_open: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_rename: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_delete: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl CollectionCard {
    /// A card keyed by `id` for the collection named `title`.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: String::new().into(),
            badge: None,
            source: None,
            rename_label: "Rename".into(),
            delete_label: "Delete".into(),
            on_open: None,
            on_rename: None,
            on_delete: None,
        }
    }

    /// The short line on the cover, usually an item count.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = detail.into();
        self
    }

    /// A kind marker in the footer, such as Smart or Trip.
    pub fn badge(mut self, label: impl Into<SharedString>, tone: Tone) -> Self {
        self.badge = Some((label.into(), tone));
        self
    }

    /// The cover image, if the collection has a cover photo.
    pub fn source(mut self, source: impl Into<ImageSource>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Accessible names for the icon actions.
    pub fn action_labels(
        mut self,
        rename: impl Into<SharedString>,
        delete: impl Into<SharedString>,
    ) -> Self {
        self.rename_label = rename.into();
        self.delete_label = delete.into();
        self
    }

    /// Invoked when the tile is pressed or activated from the keyboard.
    pub fn on_open(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }

    /// Invoked by the rename icon.
    pub fn on_rename(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_rename = Some(Rc::new(handler));
        self
    }

    /// Invoked by the delete icon.
    pub fn on_delete(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_delete = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CollectionCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let group = SharedString::from(format!("collection-card-{}", self.id));

        let cover = match self.source.clone() {
            Some(source) => ElyImage::new((self.id.clone(), "cover"), source)
                .w_full()
                .h(px(112.0))
                .into_any_element(),
            None => div()
                .w_full()
                .h(px(112.0))
                .flex()
                .items_center()
                .justify_center()
                .bg(colors.sunken)
                .text_color(colors.fg_subtle)
                .child(Icon::new(IconName::Layers).size(IconSize::Lg))
                .into_any_element(),
        };

        let open = self.on_open.clone();
        let open_key = self.on_open.clone();
        let region = div()
            .id((self.id.clone(), "open"))
            .role(Role::Button)
            .aria_label(self.title.clone())
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .cursor_pointer()
            .tab_index(0)
            .focus_ring(cx)
            .hover(|style| style.bg(colors.hover.opacity(0.35)))
            .on_click(move |_, window, cx| {
                if let Some(handler) = &open {
                    handler(window, cx);
                }
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    if let Some(handler) = &open_key {
                        cx.stop_propagation();
                        handler(window, cx);
                    }
                }
            })
            .child(
                div()
                    .relative()
                    .h(px(112.0))
                    .w_full()
                    .overflow_hidden()
                    .child(cover)
                    .child(
                        div()
                            .absolute()
                            .bottom_2()
                            .left_2()
                            .child(Badge::new(self.detail.clone()).tone(Tone::Neutral)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_1()
                    .px_3()
                    .py_2()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .min_w_0()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(EllipsisTooltip::new(
                                (self.id.clone(), "title"),
                                self.title.clone(),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .children(self.badge.map(|(label, tone)| {
                                Badge::new(label).tone(tone).into_any_element()
                            }))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .opacity(0.0)
                                    .group_hover(group.clone(), |style| style.opacity(1.0))
                                    .child(
                                        Icon::new(IconName::ArrowUpRight)
                                            .size(IconSize::Xs)
                                            .color(colors.fg_subtle),
                                    ),
                            ),
                    ),
            );

        let mut actions = div()
            .absolute()
            .top_2()
            .right_2()
            .flex()
            .gap_1()
            .p_1()
            .rounded(theme.radius(Radius::Md))
            .bg(hsla(0.0, 0.0, 0.0, 0.42))
            .opacity(0.45)
            .group_hover(group.clone(), |style| style.opacity(1.0));
        let mut has_actions = false;

        if let Some(handler) = self.on_rename.clone() {
            has_actions = true;
            actions = actions.child(
                IconButton::new((self.id.clone(), "rename"), IconName::Pencil)
                    .tooltip(self.rename_label.clone())
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .on_click(move |_, window, cx| handler(window, cx)),
            );
        }
        if let Some(handler) = self.on_delete.clone() {
            has_actions = true;
            actions = actions.child(
                IconButton::new((self.id.clone(), "delete"), IconName::Trash2)
                    .tooltip(self.delete_label.clone())
                    .variant(ButtonVariant::Danger)
                    .size(ControlSize::Sm)
                    .on_click(move |_, window, cx| handler(window, cx)),
            );
        }

        div()
            .id(self.id)
            .group(group)
            .relative()
            .w_full()
            .h(px(184.0))
            .flex_none()
            .rounded(theme.radius(Radius::Lg))
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface)
            .overflow_hidden()
            .hover(|style| style.border_color(colors.border_strong))
            .child(region)
            .when(has_actions, |card| card.child(actions))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_collects_metadata() {
        let card = CollectionCard::new("holiday", "Holiday")
            .detail("12 items")
            .badge("Smart", Tone::Accent);
        assert_eq!(card.title.as_ref(), "Holiday");
        assert_eq!(card.detail.as_ref(), "12 items");
        assert_eq!(
            card.badge.as_ref().map(|(label, _)| label.as_ref()),
            Some("Smart")
        );
        assert!(card.on_open.is_none());
    }
}
