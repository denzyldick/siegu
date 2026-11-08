use ely_gpui_component::{
    layout::{AppShell, Sidebar},
    theme::{ActiveTheme, TextSize, Theme},
    typography::Heading,
};
use gpui::{div, px, AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window};
use std::rc::Rc;

use crate::siegu_nav_items;

/// Siegu's standard application shell: a title bar, the sidebar navigation,
/// and the page body.
///
/// Consumers supply the page body as children, an `on_nav` handler that runs
/// when the user selects a section, and the `active` section id to highlight.
#[derive(IntoElement)]
pub struct SieguAppShell {
    title: String,
    body: Vec<AnyElement>,
    on_nav: Option<Rc<dyn Fn(&str, &mut App)>>,
    active: Option<String>,
}

impl SieguAppShell {
    /// A shell titled `title` (shown in the top bar next to the Siegu mark).
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: Vec::new(),
            on_nav: None,
            active: None,
        }
    }

    /// Called with the id of the nav item the user selects.
    pub fn on_nav(mut self, handler: Rc<dyn Fn(&str, &mut App)>) -> Self {
        self.on_nav = Some(handler);
        self
    }

    /// Marks the nav item for the section currently being shown.
    pub fn active(mut self, id: impl Into<String>) -> Self {
        self.active = Some(id.into());
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
            active,
        } = self;
        let theme = cx.theme();
        let on_nav = on_nav.unwrap_or_else(|| Rc::new(|_, _| {}));
        let mut shell = AppShell::new().title_bar(title_bar(&title, theme)).sidebar(
            Sidebar::new("siegu-sidebar", false).child(siegu_nav_items(on_nav, active.as_deref())),
        );
        shell.extend(body.into_iter());
        shell
    }
}

/// The bar across the top: the Siegu mark and the page title.
fn title_bar(title: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_4()
        .h(px(48.0))
        .bg(theme.colors.bg)
        .text_color(theme.colors.fg)
        .child(Heading::h1("Siegu"))
        .child(
            div()
                .text_size(theme.text_size(TextSize::Sm))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.colors.fg_muted)
                .child(title.to_string()),
        )
}
