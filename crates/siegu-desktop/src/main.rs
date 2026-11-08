use std::collections::HashMap;
use std::rc::Rc;

use siegu_core::database::{Album, LocationGroup};
use siegu_core::{config::default_config_dir, Database, PersonWithFace, Photo};
use siegu_ui::{
    ely::{
        self,
        primitives::{FocusScope, IconName},
        theme::{Mode, Theme},
        typography::{Heading, Paragraph},
    },
    gpui::{
        div, px, size, AnyElement, App, AppContext, Bounds, Context, FocusHandle, IntoElement,
        ParentElement, Render, Styled, TitlebarOptions, Window, WindowBounds, WindowOptions,
    },
    gpui_platform, InfoCard, PhotoCard, PhotoGrid, SieguAppShell,
};

/// A top-level place in the app. Keep the ids in sync with `siegu_ui::nav`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
            Self::Map => "Places from your geotagged photos.",
            Self::People => "Detected faces, grouped by person.",
            Self::Collections => "Albums and saved searches.",
            Self::Settings => "A read-only view of your local configuration.",
        }
    }
}

/// Everything the desktop shell reads from the local library once at startup.
#[derive(Default)]
struct LibraryData {
    photos: Vec<Photo>,
    people: Vec<PersonWithFace>,
    places: Vec<LocationGroup>,
    albums: Vec<Album>,
    settings: Vec<(String, String)>,
    models: Vec<String>,
}

/// Config keys that are safe to show in a read-only desktop view. Tokens and
/// passwords are deliberately excluded.
const DISPLAY_SETTINGS: &[(&str, &str)] = &[
    ("theme", "Theme"),
    ("language", "Language"),
    ("tier", "Tier"),
    ("indexing_mode", "Indexing mode"),
    ("scan_threads", "Scan threads"),
    ("ml_threads", "ML threads"),
    ("ml_memory_budget_mb", "ML memory budget (MB)"),
    ("max_storage_mb", "Max storage (MB)"),
    ("auto_scan", "Auto scan"),
    ("sync_enabled", "Sync enabled"),
    ("last_scan_completed", "Last scan completed"),
];

/// The desktop shell. Reads the local library and renders one section at a time.
struct Siegu {
    focus: FocusHandle,
    section: Section,
    data: LibraryData,
}

impl Siegu {
    fn body(&self) -> AnyElement {
        let content: AnyElement = match self.section {
            Section::Library => self.library_view(),
            Section::Map => self.map_view(),
            Section::People => self.people_view(),
            Section::Collections => self.collections_view(),
            Section::Settings => self.settings_view(),
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

    fn library_view(&self) -> AnyElement {
        if self.data.photos.is_empty() {
            return empty_state(
                "No photos yet.",
                "Add folders and run a scan in Siegu, then reopen this window.",
            );
        }
        let mut grid = PhotoGrid::new();
        for photo in &self.data.photos {
            grid = grid.child(photo_card(photo));
        }
        grid.into_any_element()
    }

    fn map_view(&self) -> AnyElement {
        if self.data.places.is_empty() {
            return empty_state(
                "No places yet.",
                "Photos with GPS or resolved location data will appear here.",
            );
        }
        let mut grid = PhotoGrid::new();
        for place in &self.data.places {
            grid = grid.child(
                InfoCard::new(IconName::MapPin, place.name.clone())
                    .detail(plural(place.count as usize, "photo")),
            );
        }
        grid.into_any_element()
    }

    fn people_view(&self) -> AnyElement {
        if self.data.people.is_empty() {
            return empty_state(
                "No people yet.",
                "Named faces from your library will be grouped here.",
            );
        }
        let mut grid = PhotoGrid::new();
        for person in &self.data.people {
            grid = grid.child(
                InfoCard::new(IconName::Users, person.name.clone())
                    .detail(plural(person.face_count as usize, "face")),
            );
        }
        grid.into_any_element()
    }

    fn collections_view(&self) -> AnyElement {
        if self.data.albums.is_empty() {
            return empty_state(
                "No collections yet.",
                "Albums you create in Siegu will appear here.",
            );
        }
        let mut grid = PhotoGrid::new();
        for album in &self.data.albums {
            grid = grid.child(
                InfoCard::new(IconName::Layers, album.name.clone())
                    .detail(plural(album.item_count as usize, "photo")),
            );
        }
        grid.into_any_element()
    }

    fn settings_view(&self) -> AnyElement {
        if self.data.settings.is_empty() && self.data.models.is_empty() {
            return empty_state(
                "Nothing configured yet.",
                "Open Siegu and change a setting; it will show up here.",
            );
        }
        let mut grid = PhotoGrid::new();
        for (label, value) in &self.data.settings {
            grid = grid
                .child(InfoCard::new(IconName::Settings, label.clone()).detail(clip(value, 42)));
        }
        if !self.data.models.is_empty() {
            grid = grid.child(
                InfoCard::new(IconName::Sparkles, "AI models")
                    .detail(clip(&self.data.models.join(", "), 42)),
            );
        }
        grid.into_any_element()
    }
}

/// Opens the existing local library, if any. We never create a database here:
/// the desktop spike must not leave side effects in a user's real config dir.
fn load_library() -> LibraryData {
    let dir = default_config_dir();
    if !dir.join("siegu.db").exists() {
        return LibraryData::default();
    }
    let db = Database::new(&dir.display().to_string());
    let state = db.get_state();
    LibraryData {
        photos: db.list_photos("", 0, 60, false, false),
        people: db.get_people(),
        places: db.get_location_groups(60),
        albums: db.list_albums(),
        settings: safe_settings(&state),
        models: enabled_models(&state),
    }
}

/// Only the curated, non-secret settings that are actually present.
fn safe_settings(state: &HashMap<String, String>) -> Vec<(String, String)> {
    DISPLAY_SETTINGS
        .iter()
        .filter_map(|(key, label)| {
            state
                .get(*key)
                .filter(|value| !value.is_empty())
                .map(|value| ((*label).to_string(), value.clone()))
        })
        .collect()
}

/// Human-readable names of the AI models turned on in the config.
fn enabled_models(state: &HashMap<String, String>) -> Vec<String> {
    let mut models: Vec<String> = state
        .iter()
        .filter(|(key, value)| {
            key.starts_with("model_enabled_") && matches!(value.as_str(), "1" | "true" | "True")
        })
        .map(|(key, _)| {
            let name = key.trim_start_matches("model_enabled_");
            let mut chars = name.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .filter(|name| !name.is_empty())
        .collect();
    models.sort();
    models
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

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Shorten a value so it fits on one line without splitting a character.
fn clip(value: &str, max: usize) -> String {
    let mut chars = value.chars();
    let clipped: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{clipped}…")
    } else {
        clipped
    }
}

fn empty_state(title: &str, hint: &str) -> AnyElement {
    div()
        .flex()
        .flex_1()
        .min_h(px(240.0))
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .child(Paragraph::new(title.to_string()))
        .child(Paragraph::new(hint.to_string()))
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
                        data: load_library(),
                    }
                })
            })
            .expect("window failed to open");
            cx.activate(true);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_ids_round_trip() {
        for section in [
            Section::Library,
            Section::Map,
            Section::People,
            Section::Collections,
            Section::Settings,
        ] {
            assert_eq!(Section::from_id(section.id()), section);
        }
    }

    #[test]
    fn unknown_section_falls_back_to_library() {
        assert_eq!(Section::from_id("nope"), Section::Library);
    }

    #[test]
    fn plural_handles_singular_and_plural() {
        assert_eq!(plural(1, "face"), "1 face");
        assert_eq!(plural(2, "face"), "2 faces");
    }

    #[test]
    fn clip_respects_character_boundaries() {
        assert_eq!(clip("hello", 10), "hello");
        assert_eq!(clip("héllo", 3), "hél…");
    }

    #[test]
    fn safe_settings_hides_secrets() {
        let mut state = HashMap::new();
        state.insert("theme".to_string(), "dark".to_string());
        state.insert("signaling_token".to_string(), "secret".to_string());
        let settings = safe_settings(&state);
        assert_eq!(settings, vec![("Theme".to_string(), "dark".to_string())]);
    }

    #[test]
    fn enabled_models_reads_model_toggles() {
        let mut state = HashMap::new();
        state.insert("model_enabled_clip".to_string(), "true".to_string());
        state.insert("model_enabled_face".to_string(), "false".to_string());
        state.insert("model_enabled_arcface".to_string(), "1".to_string());
        assert_eq!(enabled_models(&state), vec!["Arcface", "Clip"]);
    }
}
