use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use rayon::prelude::*;
use siegu_core::database::{Album, LocationGroup, PhotoFilter};
use siegu_core::{
    config::default_config_dir,
    memory_pressure,
    scanner::{extract_photo_metadata, is_media_file, photo_from_metadata, ScanGuard},
    Database, PersonWithFace, Photo,
};
use siegu_ui::{
    ely::{
        self,
        buttons::Button,
        feedback::Banner,
        forms::{InputEvent, SearchInput, Switch, TextInput},
        layout::ScrollArea,
        motion::ProgressBar,
        overlays::Dialog,
        primitives::{FocusScope, IconName, Severity},
        theme::{Mode, Theme},
        typography::{Heading, Paragraph},
    },
    gpui::{
        div, px, size, AnyElement, App, AppContext, AsyncApp, Bounds, Context, Entity, FocusHandle,
        Image, ImageFormat, ImageSource, InteractiveElement, IntoElement, KeyDownEvent,
        ParentElement, PathPromptOptions, Render, Styled, TitlebarOptions, WeakEntity, Window,
        WindowBounds, WindowOptions,
    },
    gpui_platform, InfoCard, MediaViewer, PhotoCard, PhotoGrid, SieguAppShell,
};
use tokio::sync::mpsc::{self, UnboundedSender};

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
            Self::Settings => "Local appearance, library, and AI model configuration.",
        }
    }
}

const PAGE_SIZE: usize = 60;

/// A library photo paired with its pre-resolved thumbnail source.
struct PhotoEntry {
    photo: Photo,
    thumbnail: Option<ImageSource>,
}

/// A person paired with their pre-resolved representative face thumbnail.
struct PersonEntry {
    person: PersonWithFace,
    thumbnail: Option<ImageSource>,
}

/// A place paired with its pre-resolved representative photo thumbnail.
struct PlaceEntry {
    place: LocationGroup,
    thumbnail: Option<ImageSource>,
}

/// An album paired with its pre-resolved cover thumbnail.
struct AlbumEntry {
    album: Album,
    thumbnail: Option<ImageSource>,
}

/// Everything the desktop shell reads from the local library once at startup.
#[derive(Default)]
struct LibraryData {
    photos: Vec<PhotoEntry>,
    total_photos: usize,
    people: Vec<PersonEntry>,
    places: Vec<PlaceEntry>,
    albums: Vec<AlbumEntry>,
    settings: Vec<(String, String)>,
    models: Vec<(String, String, bool)>,
    dark_mode: bool,
    auto_scan: bool,
    sync_enabled: bool,
}

/// Config keys that are safe to show in the desktop settings view. Tokens and
/// passwords are deliberately excluded.
const DISPLAY_SETTINGS: &[(&str, &str)] = &[
    ("language", "Language"),
    ("tier", "Tier"),
    ("indexing_mode", "Indexing mode"),
    ("scan_threads", "Scan threads"),
    ("ml_threads", "ML threads"),
    ("ml_memory_budget_mb", "ML memory budget (MB)"),
    ("max_storage_mb", "Max storage (MB)"),
    ("last_scan_completed", "Last scan completed"),
];

/// A filtered photo grid opened from a Map, People, or Collections card.
struct DrillDown {
    title: String,
    subtitle: String,
    photos: Vec<PhotoEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScanPhase {
    Idle,
    Discovering,
    Complete,
    Cancelled,
    Blocked,
    Failed,
}

impl ScanPhase {
    fn banner_severity(self) -> Option<Severity> {
        match self {
            Self::Idle => None,
            Self::Discovering => Some(Severity::Info),
            Self::Complete => Some(Severity::Success),
            Self::Cancelled | Self::Blocked => Some(Severity::Warning),
            Self::Failed => Some(Severity::Danger),
        }
    }
}

#[derive(Clone, Debug)]
struct ScanStatus {
    phase: ScanPhase,
    message: String,
    current: usize,
    total: usize,
    files_found: usize,
}

impl Default for ScanStatus {
    fn default() -> Self {
        Self {
            phase: ScanPhase::Idle,
            message: String::new(),
            current: 0,
            total: 0,
            files_found: 0,
        }
    }
}

/// Progress reported by the background discovery thread.
enum ScanMessage {
    Discovering {
        current: usize,
        total: usize,
        directory: String,
        files_found: usize,
    },
    Complete {
        files_found: usize,
    },
    Cancelled,
    Failed(String),
}

/// The desktop shell. Reads the local library and renders one section at a time.
struct Siegu {
    focus: FocusHandle,
    section: Section,
    search_input: Entity<TextInput>,
    search_query: String,
    favorites_only: bool,
    videos_only: bool,
    library_limit: usize,
    drill_down: Option<DrillDown>,
    selected_photo: Option<usize>,
    onboarding: bool,
    folders: Vec<String>,
    scan_status: ScanStatus,
    scan_running: bool,
    scan_abort: Option<Arc<AtomicBool>>,
    data: LibraryData,
}

impl Siegu {
    fn active_photos(&self) -> &[PhotoEntry] {
        self.drill_down
            .as_ref()
            .map(|dd| dd.photos.as_slice())
            .unwrap_or(&self.data.photos)
    }

    fn reload_library_photos(&mut self) {
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            self.data.photos.clear();
            self.data.total_photos = 0;
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let (photos, total) = query_library_photos(
            &db,
            &self.search_query,
            self.library_limit,
            self.favorites_only,
            self.videos_only,
        );
        self.data.photos = photos;
        self.data.total_photos = total;
        self.selected_photo = None;
    }

    fn refresh_data(&mut self) {
        self.data = load_library();
        self.selected_photo = None;
    }

    fn add_folders(
        &mut self,
        paths: Vec<PathBuf>,
        complete_onboarding: bool,
        cx: &mut Context<Self>,
    ) {
        let dir = default_config_dir();
        let db = Database::new(&dir.display().to_string());
        let mut added = Vec::new();
        for path in paths {
            let canonical = canonical_folder(&path);
            if !db.list_directories().contains(&canonical) {
                db.add_directory(&canonical);
                added.push(canonical);
            }
        }
        self.folders = db.list_directories();
        if complete_onboarding {
            db.set_onboarding_complete();
            self.onboarding = false;
            self.start_scan(cx);
        } else if !added.is_empty() {
            self.scan_status = ScanStatus {
                phase: ScanPhase::Complete,
                message: format!(
                    "Added {} folder{} to your library.",
                    added.len(),
                    if added.len() == 1 { "" } else { "s" }
                ),
                current: 0,
                total: 0,
                files_found: 0,
            };
            cx.notify();
        }
    }

    fn start_scan(&mut self, cx: &mut Context<Self>) {
        if self.scan_running {
            return;
        }
        if let Some(reason) = memory_pressure::headroom_block_reason() {
            self.scan_status = ScanStatus {
                phase: ScanPhase::Blocked,
                message: reason,
                current: 0,
                total: 0,
                files_found: 0,
            };
            cx.notify();
            return;
        }

        let config_dir = default_config_dir();
        let db = Database::new(&config_dir.display().to_string());
        let folders = db.list_directories();
        if folders.is_empty() {
            self.scan_status = ScanStatus {
                phase: ScanPhase::Failed,
                message: "Add a folder before scanning.".to_string(),
                current: 0,
                total: 0,
                files_found: 0,
            };
            cx.notify();
            return;
        }

        let (tx, mut rx) = mpsc::unbounded_channel();
        let abort = Arc::new(AtomicBool::new(false));
        self.scan_running = true;
        self.scan_abort = Some(abort.clone());
        self.scan_status = ScanStatus {
            phase: ScanPhase::Discovering,
            message: format!(
                "Scanning {} folder{}…",
                folders.len(),
                if folders.len() == 1 { "" } else { "s" }
            ),
            current: 0,
            total: folders.len(),
            files_found: 0,
        };

        let worker_abort = abort.clone();
        let _ = std::thread::Builder::new()
            .name("siegu-scan".to_string())
            .spawn(move || run_scan(config_dir, folders, tx, worker_abort));

        cx.spawn(move |this: WeakEntity<Siegu>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                while let Some(message) = rx.recv().await {
                    let alive = cx.update(|app| match this.upgrade() {
                        Some(entity) => {
                            entity.update(app, |state, cx| {
                                state.apply_scan_message(message, cx);
                            });
                            true
                        }
                        None => false,
                    });
                    if !alive {
                        break;
                    }
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn cancel_scan(&mut self, cx: &mut Context<Self>) {
        if let Some(abort) = &self.scan_abort {
            abort.store(true, Ordering::SeqCst);
        }
        self.scan_running = false;
        self.scan_abort = None;
        self.scan_status = ScanStatus {
            phase: ScanPhase::Cancelled,
            message: "Scan stopping…".to_string(),
            current: self.scan_status.current,
            total: self.scan_status.total,
            files_found: self.scan_status.files_found,
        };
        cx.notify();
    }

    fn apply_scan_message(&mut self, message: ScanMessage, cx: &mut Context<Self>) {
        let finished = matches!(
            message,
            ScanMessage::Complete { .. } | ScanMessage::Cancelled | ScanMessage::Failed(_)
        );
        match message {
            ScanMessage::Discovering {
                current,
                total,
                directory,
                files_found,
            } => {
                self.scan_status = ScanStatus {
                    phase: ScanPhase::Discovering,
                    message: format!("Scanning {}…", folder_name(&directory)),
                    current,
                    total,
                    files_found,
                };
            }
            ScanMessage::Complete { files_found } => {
                self.scan_status = ScanStatus {
                    phase: ScanPhase::Complete,
                    message: format!(
                        "Scan complete — {} new media file{}.",
                        files_found,
                        if files_found == 1 { "" } else { "s" }
                    ),
                    current: self.scan_status.total,
                    total: self.scan_status.total,
                    files_found,
                };
            }
            ScanMessage::Cancelled => {
                self.scan_status = ScanStatus {
                    phase: ScanPhase::Cancelled,
                    message: "Scan stopped.".to_string(),
                    current: self.scan_status.current,
                    total: self.scan_status.total,
                    files_found: self.scan_status.files_found,
                };
            }
            ScanMessage::Failed(message) => {
                self.scan_status = ScanStatus {
                    phase: ScanPhase::Failed,
                    message,
                    current: 0,
                    total: 0,
                    files_found: 0,
                };
            }
        }
        if finished {
            self.scan_running = false;
            self.scan_abort = None;
            self.refresh_data();
        }
        cx.notify();
    }

    fn clear_scan_status(&mut self, cx: &mut Context<Self>) {
        self.scan_status = ScanStatus::default();
        cx.notify();
    }

    fn toggle_photo_favorite(&mut self, photo_id: &str) {
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let new_fav = db.toggle_favorite(photo_id);
        for entry in &mut self.data.photos {
            if entry.photo.id == photo_id {
                entry.photo.favorite = new_fav;
            }
        }
        if let Some(dd) = &mut self.drill_down {
            for entry in &mut dd.photos {
                if entry.photo.id == photo_id {
                    entry.photo.favorite = new_fav;
                }
            }
        }
    }

    fn open_drill_down(
        &mut self,
        title: String,
        subtitle: String,
        query: impl FnOnce(&Database) -> Vec<Photo>,
    ) {
        let dir = default_config_dir();
        let photos = if dir.join("siegu.db").exists() {
            let db = Database::new(&dir.display().to_string());
            query(&db)
                .into_iter()
                .map(|photo| {
                    let thumbnail = resolve_thumbnail(&photo);
                    PhotoEntry { photo, thumbnail }
                })
                .collect()
        } else {
            Vec::new()
        };
        self.drill_down = Some(DrillDown {
            title,
            subtitle,
            photos,
        });
        self.selected_photo = None;
    }

    fn status_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let severity = self.scan_status.phase.banner_severity()?;
        let running = self.scan_status.phase == ScanPhase::Discovering;
        let weak = cx.entity().downgrade();
        let label = if running { "Cancel" } else { "Dismiss" };
        let banner = Banner::new("scan-status", severity, self.scan_status.message.clone()).action(
            label,
            move |_, cx| {
                if let Some(entity) = weak.upgrade() {
                    entity.update(cx, |state, cx| {
                        if running {
                            state.cancel_scan(cx);
                        } else {
                            state.clear_scan_status(cx);
                        }
                    });
                }
            },
        );
        let progress = running.then(|| {
            if self.scan_status.total > 0 {
                let fraction = self.scan_status.current as f32 / self.scan_status.total as f32;
                ProgressBar::new("scan-progress", fraction.clamp(0.0, 1.0))
            } else {
                ProgressBar::indeterminate("scan-progress")
            }
        });
        Some(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(banner)
                .children(progress)
                .into_any_element(),
        )
    }

    fn onboarding_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak_close = cx.entity().downgrade();
        let weak_choose = cx.entity().downgrade();
        let folders = if self.folders.is_empty() {
            "No folders chosen yet.".to_string()
        } else {
            format!("Ready to scan: {}", self.folders.join(", "))
        };

        Dialog::new("onboarding", "Welcome to Siegu", move |_, app| {
            if let Some(entity) = weak_close.upgrade() {
                entity.update(app, |state, cx| {
                    let dir = default_config_dir();
                    let db = Database::new(&dir.display().to_string());
                    db.set_onboarding_complete();
                    state.onboarding = false;
                    cx.notify();
                });
            }
        })
        .detail("Pick a folder to start your local library. Everything stays on this device.")
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(Paragraph::new(
                    "Siegu will find your photos and videos, extract their metadata, and build a searchable library. Nothing is uploaded.",
                ))
                .child(Paragraph::new(folders)),
        )
        .action(move |_| {
            Button::new("onboarding-choose", "Choose folders")
                .primary()
                .icon(IconName::FolderPlus)
                .on_click(move |_, _, app| prompt_for_folders(weak_choose.clone(), app, true))
        })
        .action(|close| {
            Button::new("onboarding-skip", "Start with an empty library")
                .on_click(move |_, window, app| close(window, app))
        })
        .into_any_element()
    }

    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(dd) = &self.drill_down {
            let weak = cx.entity().downgrade();
            let content = self.photo_grid_view(
                &dd.photos,
                "No photos here yet.",
                "Matching photos will appear here once indexed.",
                cx,
            );
            return div()
                .flex()
                .flex_col()
                .gap_4()
                .p_6()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            Button::new("drill-back", "Back")
                                .icon(IconName::ArrowLeft)
                                .on_click(move |_, _, cx| {
                                    if let Some(entity) = weak.upgrade() {
                                        entity.update(cx, |state, cx| {
                                            state.drill_down = None;
                                            state.selected_photo = None;
                                            cx.notify();
                                        });
                                    }
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(Heading::h2(dd.title.clone()))
                                .child(Paragraph::new(dd.subtitle.clone())),
                        ),
                )
                .child(content)
                .into_any_element();
        }

        let content: AnyElement = match self.section {
            Section::Library => self.library_view(cx),
            Section::Map => self.map_view(cx),
            Section::People => self.people_view(cx),
            Section::Collections => self.collections_view(cx),
            Section::Settings => self.settings_view(cx),
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

    fn photo_grid_view(
        &self,
        photos: &[PhotoEntry],
        empty_title: &str,
        empty_hint: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if photos.is_empty() {
            return empty_state(empty_title, empty_hint);
        }
        let mut grid = PhotoGrid::new();
        for (idx, entry) in photos.iter().enumerate() {
            let weak = cx.entity().downgrade();
            grid = grid.child(photo_card(idx, entry, move |_, cx| {
                if let Some(entity) = weak.upgrade() {
                    entity.update(cx, |state, cx| {
                        state.selected_photo = Some(idx);
                        cx.notify();
                    });
                }
            }));
        }
        grid.into_any_element()
    }

    fn library_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak_fav = cx.entity().downgrade();
        let weak_vid = cx.entity().downgrade();
        let weak_more = cx.entity().downgrade();
        let weak_add = cx.entity().downgrade();
        let weak_scan = cx.entity().downgrade();

        let has_filters = !self.search_query.is_empty() || self.favorites_only || self.videos_only;
        let (empty_title, empty_hint) = if has_filters {
            (
                "No matching photos.",
                "Try clearing your search or filters.",
            )
        } else {
            (
                "No photos yet.",
                "Add folders and run a scan in Siegu, then reopen this window.",
            )
        };

        let toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w(px(220.0))
                    .max_w(px(420.0))
                    .child(SearchInput::new("library-search", &self.search_input)),
            )
            .child(
                Button::new("library-add-folder", "Add folder")
                    .icon(IconName::FolderPlus)
                    .on_click(move |_, _, app| prompt_for_folders(weak_add.clone(), app, false)),
            )
            .child(
                Button::new(
                    "library-scan",
                    if self.scan_running {
                        "Scanning…"
                    } else {
                        "Scan now"
                    },
                )
                .icon(IconName::RefreshCw)
                .loading(self.scan_running)
                .disabled(self.scan_running)
                .on_click(move |_, _, app| {
                    if let Some(entity) = weak_scan.upgrade() {
                        entity.update(app, |state, cx| state.start_scan(cx));
                    }
                }),
            )
            .child(
                Switch::new("filter-favorites", self.favorites_only)
                    .label("Favorites")
                    .on_change(move |on, _, cx| {
                        if let Some(entity) = weak_fav.upgrade() {
                            entity.update(cx, |state, cx| {
                                state.favorites_only = on;
                                state.library_limit = PAGE_SIZE;
                                state.reload_library_photos();
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                Switch::new("filter-videos", self.videos_only)
                    .label("Videos")
                    .on_change(move |on, _, cx| {
                        if let Some(entity) = weak_vid.upgrade() {
                            entity.update(cx, |state, cx| {
                                state.videos_only = on;
                                state.library_limit = PAGE_SIZE;
                                state.reload_library_photos();
                                cx.notify();
                            });
                        }
                    }),
            )
            .children((self.data.total_photos > 0).then(|| {
                Paragraph::new(format!(
                    "{} of {}",
                    self.data.photos.len(),
                    self.data.total_photos
                ))
            }));

        let grid = self.photo_grid_view(&self.data.photos, empty_title, empty_hint, cx);
        let has_more = self.data.photos.len() < self.data.total_photos;

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(toolbar)
            .child(grid)
            .children(has_more.then(|| {
                div().flex().justify_center().pt_2().child(
                    Button::new("library-load-more", "Load more").on_click(move |_, _, cx| {
                        if let Some(entity) = weak_more.upgrade() {
                            entity.update(cx, |state, cx| {
                                state.library_limit += PAGE_SIZE;
                                state.reload_library_photos();
                                cx.notify();
                            });
                        }
                    }),
                )
            }))
            .into_any_element()
    }

    fn map_view(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.data.places.is_empty() {
            return empty_state(
                "No places yet.",
                "Photos with GPS or resolved location data will appear here.",
            );
        }
        let mut grid = PhotoGrid::new();
        for (idx, entry) in self.data.places.iter().enumerate() {
            let weak = cx.entity().downgrade();
            let name = entry.place.name.clone();
            let detail = plural(entry.place.count as usize, "photo");
            let mut card = InfoCard::new(("place", idx), IconName::MapPin, name.clone())
                .detail(detail.clone())
                .on_click(move |_, cx| {
                    if let Some(entity) = weak.upgrade() {
                        let place_name = name.clone();
                        let subtitle = detail.clone();
                        entity.update(cx, |state, cx| {
                            state.open_drill_down(place_name.clone(), subtitle, |db| {
                                db.list_photos_filtered(
                                    "",
                                    0,
                                    60,
                                    false,
                                    false,
                                    &PhotoFilter {
                                        location: Some(place_name),
                                        ..Default::default()
                                    },
                                    true,
                                )
                            });
                            cx.notify();
                        });
                    }
                });
            if let Some(thumb) = &entry.thumbnail {
                card = card.source(thumb.clone());
            }
            grid = grid.child(card);
        }
        grid.into_any_element()
    }

    fn people_view(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.data.people.is_empty() {
            return empty_state(
                "No people yet.",
                "Named faces from your library will be grouped here.",
            );
        }
        let mut grid = PhotoGrid::new();
        for (idx, entry) in self.data.people.iter().enumerate() {
            let weak = cx.entity().downgrade();
            let person_id = entry.person.id.clone();
            let name = entry.person.name.clone();
            let detail = plural(entry.person.face_count as usize, "face");
            let mut card = InfoCard::new(("person", idx), IconName::Users, name.clone())
                .detail(detail.clone())
                .on_click(move |_, cx| {
                    if let Some(entity) = weak.upgrade() {
                        let id = person_id.clone();
                        let title = name.clone();
                        let subtitle = detail.clone();
                        entity.update(cx, |state, cx| {
                            state.open_drill_down(title, subtitle, |db| {
                                db.get_photos_for_person(&id, 0, 60)
                            });
                            cx.notify();
                        });
                    }
                });
            if let Some(thumb) = &entry.thumbnail {
                card = card.source(thumb.clone());
            }
            grid = grid.child(card);
        }
        grid.into_any_element()
    }

    fn collections_view(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.data.albums.is_empty() {
            return empty_state(
                "No collections yet.",
                "Albums you create in Siegu will appear here.",
            );
        }
        let mut grid = PhotoGrid::new();
        for (idx, entry) in self.data.albums.iter().enumerate() {
            let weak = cx.entity().downgrade();
            let album_id = entry.album.id.clone();
            let name = entry.album.name.clone();
            let detail = plural(entry.album.item_count as usize, "photo");
            let mut card = InfoCard::new(("album", idx), IconName::Layers, name.clone())
                .detail(detail.clone())
                .on_click(move |_, cx| {
                    if let Some(entity) = weak.upgrade() {
                        let id = album_id.clone();
                        let title = name.clone();
                        let subtitle = detail.clone();
                        entity.update(cx, |state, cx| {
                            state.open_drill_down(title, subtitle, |db| {
                                db.get_album_contents(&id, 0, 60)
                            });
                            cx.notify();
                        });
                    }
                });
            if let Some(thumb) = &entry.thumbnail {
                card = card.source(thumb.clone());
            }
            grid = grid.child(card);
        }
        grid.into_any_element()
    }

    fn settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak_theme = cx.entity().downgrade();
        let weak_scan = cx.entity().downgrade();
        let weak_sync = cx.entity().downgrade();
        let weak_folder = cx.entity().downgrade();
        let weak_add_folder = cx.entity().downgrade();

        let mut folder_rows = div().flex().flex_col().gap_2();
        for (idx, folder) in self.folders.iter().enumerate() {
            let weak = weak_folder.clone();
            let path = folder.clone();
            folder_rows = folder_rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().child(Paragraph::new(folder.clone())))
                    .child(
                        Button::new(("remove-folder", idx), "Remove")
                            .icon(IconName::Trash2)
                            .on_click(move |_, _, app| {
                                if let Some(entity) = weak.upgrade() {
                                    let path = path.clone();
                                    entity.update(app, move |state, cx| {
                                        let dir = default_config_dir();
                                        let db = Database::new(&dir.display().to_string());
                                        db.remove_directory(path.clone());
                                        state.folders = db.list_directories();
                                        state.scan_status = ScanStatus {
                                            phase: ScanPhase::Complete,
                                            message: format!("Removed {path} from the watch list."),
                                            current: 0,
                                            total: 0,
                                            files_found: 0,
                                        };
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            );
        }
        let folders_section = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Heading::h3("Library folders"))
                    .child(
                        Button::new("settings-add-folder", "Add folder")
                            .icon(IconName::FolderPlus)
                            .on_click(move |_, _, app| {
                                prompt_for_folders(weak_add_folder.clone(), app, false)
                            }),
                    ),
            )
            .children((!self.folders.is_empty()).then_some(folder_rows))
            .children(self.folders.is_empty().then(|| {
                Paragraph::new("No folders yet. Add one to start building your library.")
            }));

        let toggles = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_6()
            .child(
                Switch::new("setting-dark-mode", self.data.dark_mode)
                    .label("Dark theme")
                    .on_change(move |on, _, cx| {
                        Theme::set_mode_now(if on { Mode::Dark } else { Mode::Light }, cx);
                        if let Some(entity) = weak_theme.upgrade() {
                            entity.update(cx, |state, cx| {
                                state.data.dark_mode = on;
                                persist_setting("theme", if on { "dark" } else { "light" });
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                Switch::new("setting-auto-scan", self.data.auto_scan)
                    .label("Auto scan")
                    .on_change(move |on, _, cx| {
                        if let Some(entity) = weak_scan.upgrade() {
                            entity.update(cx, |state, cx| {
                                state.data.auto_scan = on;
                                persist_setting("auto_scan", if on { "true" } else { "false" });
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                Switch::new("setting-sync-enabled", self.data.sync_enabled)
                    .label("Sync enabled")
                    .on_change(move |on, _, cx| {
                        if let Some(entity) = weak_sync.upgrade() {
                            entity.update(cx, |state, cx| {
                                state.data.sync_enabled = on;
                                persist_setting("sync_enabled", if on { "true" } else { "false" });
                                cx.notify();
                            });
                        }
                    }),
            );

        let mut model_row = div().flex().flex_wrap().items_center().gap_5();
        for (idx, (key, label, enabled)) in self.data.models.iter().enumerate() {
            let weak_model = cx.entity().downgrade();
            let key = key.clone();
            model_row = model_row.child(
                Switch::new(("setting-model", idx), *enabled)
                    .label(label.clone())
                    .on_change(move |on, _, cx| {
                        if let Some(entity) = weak_model.upgrade() {
                            let key = key.clone();
                            entity.update(cx, |state, cx| {
                                if let Some((_, _, flag)) =
                                    state.data.models.iter_mut().find(|(k, _, _)| k == &key)
                                {
                                    *flag = on;
                                }
                                persist_setting(&key, if on { "true" } else { "false" });
                                cx.notify();
                            });
                        }
                    }),
            );
        }

        let mut grid = PhotoGrid::new();
        for (idx, (label, value)) in self.data.settings.iter().enumerate() {
            grid = grid.child(
                InfoCard::new(("setting", idx), IconName::Settings, label.clone())
                    .detail(clip(value, 42)),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(folders_section)
            .child(toggles)
            .children((!self.data.models.is_empty()).then(|| {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Heading::h3("AI Models"))
                    .child(model_row)
            }))
            .children((!self.data.settings.is_empty()).then_some(grid))
            .into_any_element()
    }
}

fn step_index(current: Option<usize>, delta: isize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let next = match current {
        Some(index) => (index as isize + delta).rem_euclid(len as isize) as usize,
        None if delta > 0 => 0,
        None => len - 1,
    };
    Some(next)
}

fn canonical_folder(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn folder_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn is_initialized() -> bool {
    let dir = default_config_dir();
    if !dir.join("siegu.db").exists() {
        return false;
    }
    let db = Database::new(&dir.display().to_string());
    !db.list_directories().is_empty() || db.is_onboarding_complete() || db.has_any_photos()
}

fn prompt_for_folders(weak: WeakEntity<Siegu>, app: &mut App, complete_onboarding: bool) {
    let receiver = app.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: true,
        prompt: Some("Choose folders to add to your library".into()),
    });
    app.spawn(move |cx: &mut AsyncApp| {
        let cx = cx.clone();
        async move {
            match receiver.await {
                Ok(Ok(Some(paths))) if !paths.is_empty() => {
                    cx.update(|app| {
                        weak.update(app, |state, cx| {
                            state.add_folders(paths, complete_onboarding, cx)
                        })
                        .ok();
                    });
                }
                Ok(Ok(_)) => {}
                Ok(Err(error)) => {
                    cx.update(|app| {
                        weak.update(app, |state, cx| {
                            state.scan_status = ScanStatus {
                                phase: ScanPhase::Failed,
                                message: format!("Could not open the folder picker: {error}"),
                                current: 0,
                                total: 0,
                                files_found: 0,
                            };
                            cx.notify();
                        })
                        .ok();
                    });
                }
                Err(error) => {
                    cx.update(|app| {
                        weak.update(app, |state, cx| {
                            state.scan_status = ScanStatus {
                                phase: ScanPhase::Failed,
                                message: format!("Folder picker closed unexpectedly: {error}"),
                                current: 0,
                                total: 0,
                                files_found: 0,
                            };
                            cx.notify();
                        })
                        .ok();
                    });
                }
            }
        }
    })
    .detach();
}

fn run_scan(
    config_dir: PathBuf,
    folders: Vec<String>,
    tx: UnboundedSender<ScanMessage>,
    abort: Arc<AtomicBool>,
) {
    let guard = ScanGuard::new();
    let Some(_session) = guard.try_start() else {
        let _ = tx.send(ScanMessage::Failed(
            "A scan is already running.".to_string(),
        ));
        return;
    };

    let config = config_dir.display().to_string();
    let mut db = Database::new(&config);
    let mut existing = db.existing_locations();
    let mut files_found = 0usize;
    let total = folders.len();
    let mut last_emit = Instant::now();

    for (index, folder) in folders.iter().enumerate() {
        if abort.load(Ordering::SeqCst) {
            let _ = tx.send(ScanMessage::Cancelled);
            return;
        }

        let paths: Vec<PathBuf> = jwalk::WalkDir::new(folder)
            .follow_links(false)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| is_media_file(path))
            .collect();

        for chunk in paths.chunks(250) {
            if abort.load(Ordering::SeqCst) {
                let _ = tx.send(ScanMessage::Cancelled);
                return;
            }
            let batch: Vec<Photo> = chunk
                .par_iter()
                .filter_map(|path| {
                    if abort.load(Ordering::SeqCst) {
                        return None;
                    }
                    let location = path.to_string_lossy().to_string();
                    if existing.contains(&location) {
                        return None;
                    }
                    let metadata = extract_photo_metadata(path);
                    Some(photo_from_metadata(&location, &metadata))
                })
                .collect();
            if batch.is_empty() {
                continue;
            }
            if let Err(error) = db.store_photo_batch(&batch) {
                let _ = tx.send(ScanMessage::Failed(format!(
                    "Could not save scanned photos: {error}"
                )));
                return;
            }
            existing.extend(batch.iter().map(|photo| photo.location.clone()));
            files_found += batch.len();
            if last_emit.elapsed() >= Duration::from_millis(200) {
                let _ = tx.send(ScanMessage::Discovering {
                    current: index + 1,
                    total,
                    directory: folder.clone(),
                    files_found,
                });
                last_emit = Instant::now();
            }
        }

        let _ = tx.send(ScanMessage::Discovering {
            current: index + 1,
            total,
            directory: folder.clone(),
            files_found,
        });
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();
    db.set_last_scan_time(timestamp);
    let _ = tx.send(ScanMessage::Complete { files_found });
}

fn query_library_photos(
    db: &Database,
    query: &str,
    limit: usize,
    favorites_only: bool,
    videos_only: bool,
) -> (Vec<PhotoEntry>, usize) {
    let total = db
        .count_photos_filtered(query, favorites_only, videos_only, &PhotoFilter::default())
        .max(0) as usize;
    let photos = db
        .list_photos(query, 0, limit, favorites_only, videos_only)
        .into_iter()
        .map(|photo| {
            let thumbnail = resolve_thumbnail(&photo);
            PhotoEntry { photo, thumbnail }
        })
        .collect();
    (photos, total)
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
    let (photos, total_photos) = query_library_photos(&db, "", PAGE_SIZE, false, false);
    let people = db
        .get_people()
        .into_iter()
        .map(|person| {
            let thumbnail = resolve_image_source(
                person.encoded.as_deref(),
                person.representative_crop.as_deref(),
            );
            PersonEntry { person, thumbnail }
        })
        .collect();
    let places = db
        .get_location_groups(60)
        .into_iter()
        .map(|place| {
            let thumbnail =
                resolve_image_source(place.encoded.as_deref(), place.photo_location.as_deref());
            PlaceEntry { place, thumbnail }
        })
        .collect();
    let albums = db
        .list_albums()
        .into_iter()
        .map(|album| {
            let cover = album
                .cover_photo_id
                .as_deref()
                .and_then(|id| db.get_photo_by_id(id))
                .or_else(|| db.get_album_contents(&album.id, 0, 1).into_iter().next());
            let thumbnail = cover.as_ref().and_then(resolve_thumbnail);
            AlbumEntry { album, thumbnail }
        })
        .collect();
    let dark_mode = state
        .get("theme")
        .map(|v| !v.eq_ignore_ascii_case("light"))
        .unwrap_or(true);
    let auto_scan = state
        .get("auto_scan")
        .map(|v| is_truthy(v))
        .unwrap_or(false);
    let sync_enabled = state
        .get("sync_enabled")
        .map(|v| is_truthy(v))
        .unwrap_or(false);

    let mut settings = safe_settings(&state);
    if let Some(last_scan) = db.get_last_scan_time() {
        settings.insert(0, ("Last scan".to_string(), last_scan));
    }

    LibraryData {
        photos,
        total_photos,
        people,
        places,
        albums,
        settings,
        models: model_toggles(&state),
        dark_mode,
        auto_scan,
        sync_enabled,
    }
}

fn persist_setting(key: &str, value: &str) {
    let dir = default_config_dir();
    if dir.join("siegu.db").exists() {
        let db = Database::new(&dir.display().to_string());
        db.set_state_value(key, value);
    }
}

fn is_truthy(value: &str) -> bool {
    matches!(value.trim(), "1" | "true" | "True" | "TRUE")
}

/// Resolves a photo's thumbnail: prefers the pre-encoded thumbnail (fast in-memory
/// decode, works for videos and evicted view-only items) and falls back to the
/// local file path when present.
fn resolve_thumbnail(photo: &Photo) -> Option<ImageSource> {
    let path = (!photo.view_only).then_some(photo.location.as_str());
    resolve_image_source(Some(photo.encoded.as_str()), path)
}

/// Resolves an image source from an optional data URL or local file path.
fn resolve_image_source(encoded: Option<&str>, path: Option<&str>) -> Option<ImageSource> {
    if let Some(data_url) = encoded {
        if let Some(image) = decode_data_url(data_url) {
            return Some(ImageSource::Image(image));
        }
    }
    if let Some(path) = path {
        if !path.is_empty() && Path::new(path).exists() {
            return Some(ImageSource::from(PathBuf::from(path)));
        }
    }
    None
}

/// Decodes a `data:image/<fmt>;base64,...` URI into an in-memory `gpui::Image`.
fn decode_data_url(encoded: &str) -> Option<Arc<Image>> {
    let rest = encoded.strip_prefix("data:image/")?;
    let (mime_sub, payload) = rest.split_once(";base64,")?;
    let format = match mime_sub.to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::Webp,
        "gif" => ImageFormat::Gif,
        "bmp" => ImageFormat::Bmp,
        "svg+xml" => ImageFormat::Svg,
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(Arc::new(Image::from_bytes(format, bytes)))
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

/// All AI model toggles present in the config as `(key, label, enabled)`.
fn model_toggles(state: &HashMap<String, String>) -> Vec<(String, String, bool)> {
    let mut models: Vec<(String, String, bool)> = state
        .iter()
        .filter(|(key, _)| key.starts_with("model_enabled_"))
        .filter_map(|(key, value)| {
            let name = key.trim_start_matches("model_enabled_");
            let mut chars = name.chars();
            let label = match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => return None,
            };
            Some((key.clone(), label, is_truthy(value)))
        })
        .collect();
    models.sort_by(|a, b| a.1.cmp(&b.1));
    models
}

fn photo_card(
    idx: usize,
    entry: &PhotoEntry,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> PhotoCard {
    let mut card = PhotoCard::new(("photo", idx), display_name(&entry.photo))
        .date(display_date(&entry.photo.created))
        .on_click(on_click);
    if let Some(thumb) = &entry.thumbnail {
        card = card.source(thumb.clone());
    }
    card
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
                    state.drill_down = None;
                    state.selected_photo = None;
                    cx.notify();
                });
            }
        });

        let viewer = self.selected_photo.and_then(|idx| {
            let active = self.active_photos();
            let total = active.len();
            let entry = active.get(idx)?;
            let weak_close = cx.entity().downgrade();
            let weak_fav = cx.entity().downgrade();
            let photo_id = entry.photo.id.clone();
            let detail = format!(
                "{}  •  {} of {}",
                display_date(&entry.photo.created),
                idx + 1,
                total
            );

            let mut modal = MediaViewer::new(
                ("media-viewer", idx),
                display_name(&entry.photo),
                move |_, cx| {
                    if let Some(entity) = weak_close.upgrade() {
                        entity.update(cx, |state, cx| {
                            state.selected_photo = None;
                            cx.notify();
                        });
                    }
                },
            )
            .detail(detail)
            .favorite(entry.photo.favorite)
            .on_toggle_favorite(move |_, cx| {
                if let Some(entity) = weak_fav.upgrade() {
                    let id = photo_id.clone();
                    entity.update(cx, |state, cx| {
                        state.toggle_photo_favorite(&id);
                        cx.notify();
                    });
                }
            });

            if idx > 0 {
                let weak_prev = cx.entity().downgrade();
                modal = modal.on_prev(move |_, cx| {
                    if let Some(entity) = weak_prev.upgrade() {
                        entity.update(cx, |state, cx| {
                            state.selected_photo = Some(idx - 1);
                            cx.notify();
                        });
                    }
                });
            }

            if idx + 1 < total {
                let weak_next = cx.entity().downgrade();
                modal = modal.on_next(move |_, cx| {
                    if let Some(entity) = weak_next.upgrade() {
                        entity.update(cx, |state, cx| {
                            state.selected_photo = Some(idx + 1);
                            cx.notify();
                        });
                    }
                });
            }

            if !entry.photo.view_only
                && !entry.photo.location.is_empty()
                && Path::new(&entry.photo.location).exists()
            {
                modal = modal.source(PathBuf::from(&entry.photo.location));
            }
            if let Some(thumb) = &entry.thumbnail {
                modal = modal.fallback(thumb.clone());
            }
            Some(modal)
        });

        let shell_title = self
            .drill_down
            .as_ref()
            .map(|dd| dd.title.as_str())
            .unwrap_or_else(|| self.section.title());
        let status = self.status_view(cx);
        let onboarding = if self.onboarding {
            Some(self.onboarding_view(cx))
        } else {
            None
        };
        let weak_keys = cx.entity().downgrade();

        FocusScope::new(&self.focus)
            .root()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .on_key_down(move |event: &KeyDownEvent, _, cx: &mut App| {
                        if event.keystroke.modifiers.modified() {
                            return;
                        }
                        let entity = match weak_keys.upgrade() {
                            Some(entity) => entity,
                            None => return,
                        };
                        let handled = match event.keystroke.key.as_str() {
                            "escape" => entity.update(cx, |state, cx| {
                                let handled = if state.selected_photo.is_some() {
                                    state.selected_photo = None;
                                    true
                                } else if state.drill_down.is_some() {
                                    state.drill_down = None;
                                    state.selected_photo = None;
                                    true
                                } else {
                                    false
                                };
                                if handled {
                                    cx.notify();
                                }
                                handled
                            }),
                            "left" | "right" => entity.update(cx, |state, cx| {
                                if state.selected_photo.is_none() {
                                    return false;
                                }
                                let len = state.active_photos().len();
                                if len == 0 {
                                    return false;
                                }
                                let delta = if event.keystroke.key == "left" { -1 } else { 1 };
                                state.selected_photo = step_index(state.selected_photo, delta, len);
                                cx.notify();
                                true
                            }),
                            _ => false,
                        };
                        if handled {
                            cx.stop_propagation();
                        }
                    })
                    .children(status)
                    .child(
                        div().flex_1().min_h_0().child(
                            SieguAppShell::new(shell_title)
                                .active(self.section.id())
                                .on_nav(on_nav)
                                .child(
                                    ScrollArea::new("siegu-shell-scroll")
                                        .vertical()
                                        .size_full()
                                        .child(self.body(cx)),
                                ),
                        ),
                    ),
            )
            .children(viewer)
            .children(onboarding)
    }
}

fn main() {
    gpui_platform::application()
        .with_assets(ely::Assets)
        .run(|cx: &mut App| {
            ely::init(cx).expect("Ely failed to start");
            let onboarding = !is_initialized();
            let folders = {
                let dir = default_config_dir();
                if dir.join("siegu.db").exists() {
                    Database::new(&dir.display().to_string()).list_directories()
                } else {
                    Vec::new()
                }
            };
            let data = load_library();
            Theme::set_mode_now(
                if data.dark_mode {
                    Mode::Dark
                } else {
                    Mode::Light
                },
                cx,
            );

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
                    let search_input = cx.new(|cx| {
                        TextInput::new(window, cx).placeholder("Search photos, objects, captions…")
                    });
                    cx.subscribe(&search_input, |state: &mut Siegu, input, event, cx| {
                        if matches!(event, InputEvent::Changed | InputEvent::Submit) {
                            let query = input.read(cx).text().to_string();
                            if query != state.search_query {
                                state.search_query = query;
                                state.library_limit = PAGE_SIZE;
                                state.reload_library_photos();
                                cx.notify();
                            }
                        }
                    })
                    .detach();

                    Siegu {
                        focus,
                        section: Section::Library,
                        search_input,
                        search_query: String::new(),
                        favorites_only: false,
                        videos_only: false,
                        library_limit: PAGE_SIZE,
                        drill_down: None,
                        selected_photo: None,
                        onboarding,
                        folders,
                        scan_status: ScanStatus::default(),
                        scan_running: false,
                        scan_abort: None,
                        data,
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
    fn step_index_wraps_within_the_active_grid() {
        assert_eq!(step_index(Some(0), -1, 3), Some(2));
        assert_eq!(step_index(Some(2), 1, 3), Some(0));
        assert_eq!(step_index(Some(1), 1, 3), Some(2));
        assert_eq!(step_index(None, 1, 3), Some(0));
        assert_eq!(step_index(None, -1, 3), Some(2));
        assert_eq!(step_index(Some(0), 1, 0), None);
    }

    #[test]
    fn scan_status_starts_idle_without_a_banner() {
        let status = ScanStatus::default();
        assert_eq!(status.phase, ScanPhase::Idle);
        assert!(status.phase.banner_severity().is_none());
    }

    #[test]
    fn folder_name_uses_the_last_path_component() {
        assert_eq!(folder_name("/tmp/photos/summer"), "summer");
        assert_eq!(folder_name("photos"), "photos");
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
        state.insert("language".to_string(), "en".to_string());
        state.insert("signaling_token".to_string(), "secret".to_string());
        let settings = safe_settings(&state);
        assert_eq!(settings, vec![("Language".to_string(), "en".to_string())]);
    }

    #[test]
    fn model_toggles_reads_all_models() {
        let mut state = HashMap::new();
        state.insert("model_enabled_clip".to_string(), "true".to_string());
        state.insert("model_enabled_face".to_string(), "false".to_string());
        state.insert("model_enabled_arcface".to_string(), "1".to_string());
        assert_eq!(
            model_toggles(&state),
            vec![
                (
                    "model_enabled_arcface".to_string(),
                    "Arcface".to_string(),
                    true
                ),
                ("model_enabled_clip".to_string(), "Clip".to_string(), true),
                ("model_enabled_face".to_string(), "Face".to_string(), false),
            ]
        );
    }

    #[test]
    fn decode_data_url_handles_valid_and_invalid_inputs() {
        assert!(decode_data_url("").is_none());
        assert!(decode_data_url("not-a-data-url").is_none());
        assert!(decode_data_url("data:image/jpeg;base64,").is_none());
        assert!(decode_data_url("data:image/jpeg;base64,!!!").is_none());

        let img = decode_data_url("data:image/png;base64,AQID").expect("valid png bytes");
        assert_eq!(img.format, ImageFormat::Png);
        assert_eq!(img.bytes(), &[1, 2, 3]);
    }
}
