use std::{
    rc::Rc,
    sync::{Arc, OnceLock},
};

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    layout::{AppShell, Sidebar},
    primitives::{IconName, Image as ElyImage},
    theme::{ActiveTheme, ControlSize, Radius, TextSize},
};
use gpui::{
    div, hsla, px, AnyElement, App, Image, ImageFormat, ImageSource, IntoElement, ParentElement,
    RenderOnce, SharedString, Styled, Window,
};

use crate::{i18n, siegu_nav_items};

/// Siegu's standard application shell: a title bar with the brand mark and
/// menu control, the sidebar navigation, and the page body.
///
/// Consumers supply the page body as children, an `on_nav` handler that runs
/// when the user selects a section, and the active section id to highlight.
#[derive(IntoElement)]
pub struct SieguAppShell {
    title: String,
    body: Vec<AnyElement>,
    on_nav: Option<Rc<dyn Fn(&str, &mut App)>>,
    on_toggle_menu: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    active: Option<String>,
    collapsed: bool,
}

impl SieguAppShell {
    /// A shell titled `title` (shown in the top bar beside the brand mark).
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: Vec::new(),
            on_nav: None,
            on_toggle_menu: None,
            active: None,
            collapsed: false,
        }
    }

    /// Called with the id of the nav item the user selects.
    pub fn on_nav(mut self, handler: Rc<dyn Fn(&str, &mut App)>) -> Self {
        self.on_nav = Some(handler);
        self
    }

    /// Called when the menu control is pressed; the shell renders the state
    /// supplied by `collapsed`.
    pub fn on_toggle_menu(mut self, handler: Rc<dyn Fn(&mut Window, &mut App)>) -> Self {
        self.on_toggle_menu = Some(handler);
        self
    }

    /// Marks the nav item for the section currently being shown.
    pub fn active(mut self, id: impl Into<String>) -> Self {
        self.active = Some(id.into());
        self
    }

    /// Folds the sidebar into an icon rail.
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl ParentElement for SieguAppShell {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.body.extend(elements);
    }
}

impl RenderOnce for SieguAppShell {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            title,
            body,
            on_nav,
            on_toggle_menu,
            active,
            collapsed,
        } = self;
        let on_nav = on_nav.unwrap_or_else(|| Rc::new(|_, _| {}));
        let on_toggle_menu = on_toggle_menu.unwrap_or_else(|| Rc::new(|_, _| {}));
        let mut shell = AppShell::new()
            .title_bar(title_bar(&title, collapsed, &on_toggle_menu, cx))
            .sidebar(
                Sidebar::new("siegu-sidebar", collapsed).child(siegu_nav_items(
                    on_nav,
                    active.as_deref(),
                    collapsed,
                )),
            );
        shell.extend(body.into_iter());
        shell
    }
}

/// The bar across the top: the brand mark, the menu toggle, and the page title.
fn title_bar(
    title: &str,
    collapsed: bool,
    on_toggle: &Rc<dyn Fn(&mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let toggle = Rc::clone(on_toggle);
    let toggle_label: SharedString = i18n::text(
        cx,
        if collapsed {
            "common.expand_menu"
        } else {
            "common.collapse_menu"
        },
        &[],
    );

    div()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .h(px(48.0))
        .bg(theme.colors.bg)
        .text_color(theme.colors.fg)
        .child(brand_mark(cx))
        .child(
            IconButton::new(
                "siegu-menu-toggle",
                if collapsed {
                    IconName::ChevronsRight
                } else {
                    IconName::ChevronsLeft
                },
            )
            .tooltip(toggle_label)
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Sm)
            .on_click(move |_, window, app| toggle(window, app)),
        )
        .child(
            div()
                .text_size(theme.text_size(TextSize::Sm))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.colors.fg_muted)
                .child(title.to_string()),
        )
}

/// Decoded once so repaints reuse the same cached GPU image.
fn brand_image() -> &'static Arc<Image> {
    static IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
    IMAGE.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../../../branding/logo_mark_padded.png").to_vec(),
        ))
    })
}

/// The logo sits on a light chip so the dark mark stays legible in both themes.
fn brand_mark(cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let source = ImageSource::Image(Arc::clone(brand_image()));
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(30.0))
        .rounded(theme.radius(Radius::Md))
        .border_1()
        .border_color(theme.colors.border)
        .bg(hsla(0.0, 0.0, 1.0, 0.94))
        .child(ElyImage::new("siegu-brand-mark", source).size(px(20.0)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn brand_mark_is_embedded() {
        let bytes = include_bytes!("../../../branding/logo_mark_padded.png");
        assert_eq!(&bytes[1..4], b"PNG");
    }
}
