use std::rc::Rc;

use siegu_core::{config::default_config_dir, Database, Photo};
use siegu_ui::{
    ely::{
        self,
        primitives::FocusScope,
        theme::{Mode, Theme},
        typography::{Heading, Paragraph},
    },
    gpui::{
        div, px, size, AnyElement, App, AppContext, Bounds, Context, FocusHandle, IntoElement,
        ParentElement, Render, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions,
    },
    gpui_platform, PhotoCard, PhotoGrid, SieguAppShell,
};

/// A top-level place in the app. Keep the ids in sync with `siegu_ui::nav`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Library,
    Map,
    People,
    Collections,
    Settings,
}

impl Section {
    fn from_id(id: &str) -> Self {
        match id {
            "map" => Self::Map,
            "people" => Self::People,
            "collections" => Self::Collections,
            "settings" => Self::Settings,
            _ => Self::Library,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Map => "map",
            Self::People => "people",
            Self::Collections => "collections",
            Self::Settings => "settings",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Map => "Map",
            Self::People => "People",
            Self::Collections => "Collections",
            Self::Settings => "Settings",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Library => "Every photo and video, all in one place.",
            Self::Map => "Photos with location data will appear here.",
            Self::People => "Detected faces will be grouped here.",
            Self::Collections => "Albums and saved searches will appear here.",
            Self::Settings => "Library folders, AI models, and sync settings.",
        }
    }
}

/// The desktop shell. Reads the local library for the Library section; the
/// remaining sections are placeholders until they are ported from the Tauri UI.
struct Siegu {
    focus: FocusHandle,
    section: Section,
    photos: Vec<Photo>,
}

impl Siegu {
    fn body(&self) -> AnyElement {
        let content: AnyElement = match self.section {
            Section::Library if self.photos.is_empty() => empty_library(),
            Section::Library => {
                let mut grid = PhotoGrid::new();
                for photo in &self.photos {
                    grid = grid.child(photo_card(photo));
                }
                grid.into_any_element()
            }
            other => placeholder(other),
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Heading::h2(self.section.title()))
                    .child(Paragraph::new(self.section.subtitle())),
            )
            .child(content)
            .into_any_element()
    }
}

/// Opens the existing local library, if any. We never create a database here:
/// the desktop spike must not leave side effects in a user's real config dir.
fn load_photos() -> Vec<Photo> {
    let dir = default_config_dir();
    if !dir.join("siegu.db").exists() {
        return Vec::new();
    }
    let db = Database::new(&dir.display().to_string());
    db.list_photos("", 0, 60, false, false)
}

fn photo_card(photo: &Photo) -> PhotoCard {
    PhotoCard::new(display_name(photo)).date(display_date(&photo.created))
}

/// A caption when the AI has written one, otherwise the file name.
fn display_name(photo: &Photo) -> String {
    if let Some(caption) = photo.caption.as_deref() {
        let caption = caption.trim();
        if !caption.is_empty() {
            return caption.to_string();
        }
    }
    std::path::Path::new(&photo.location)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| photo.id.clone())
}

/// Timestamps are stored as `YYYY-MM-DD HH:MM:SS` (or ISO-8601); show the date.
fn display_date(created: &str) -> String {
    created
        .split([' ', 'T'])
        .next()
        .unwrap_or(created)
        .to_string()
}

fn empty_library() -> AnyElement {
    div()
        .flex()
        .flex_1()
        .min_h(px(240.0))
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .child(Paragraph::new("No photos yet."))
        .child(Paragraph::new(
            "Add folders and run a scan in Siegu, then reopen this window.",
        ))
        .into_any_element()
}

fn placeholder(section: Section) -> AnyElement {
    div()
        .flex()
        .flex_1()
        .min_h(px(240.0))
        .items_center()
        .justify_center()
        .child(Paragraph::new(section.subtitle()))
        .into_any_element()
}

impl Render for Siegu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let on_nav: Rc<dyn Fn(&str, &mut App)> = Rc::new(move |id, cx| {
            if let Some(entity) = weak.upgrade() {
                entity.update(cx, |state, cx| {
                    state.section = Section::from_id(id);
                    cx.notify();
                });
            }
        });

        FocusScope::new(&self.focus).root().size_full().child(
            SieguAppShell::new(self.section.title())
                .active(self.section.id())
                .on_nav(on_nav)
                .child(self.body()),
        )
    }
}

fn main() {
    gpui_platform::application()
        .with_assets(ely::Assets)
        .run(|cx: &mut App| {
            ely::init(cx).expect("Ely failed to start");
            Theme::set_mode_now(Mode::Dark, cx);

            let bounds = Bounds::centered(None, size(px(900.0), px(560.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Siegu".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            cx.open_window(options, |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Siegu {
                        focus,
                        section: Section::Library,
                        photos: load_photos(),
                    }
                })
            })
            .expect("window failed to open");
            cx.activate(true);
        });
}
