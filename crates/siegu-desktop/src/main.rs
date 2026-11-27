use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod app_ipc;
mod tile_http;

use base64::Engine as _;
use rayon::prelude::*;
use siegu_core::database::{Album, DeviceInfo, PhotoDateGroup, PhotoFilter};
use siegu_core::devices::{
    list_devices as list_core_devices, remove_device as remove_core_device,
    rename_device as rename_core_device, storage_percent,
};
use siegu_core::duplicates::{self, DuplicateGroupView, DuplicateStats};
use siegu_core::map::{cluster_map_points, frame_map_points, MapCluster, MapFraming};
use siegu_core::{
    config::default_config_dir,
    memory_pressure,
    scanner::{extract_photo_metadata, is_media_file, photo_from_metadata, ScanGuard},
    Database, PersonWithFace, Photo,
};
use siegu_ui::{
    ely::{
        self,
        buttons::{Button, ButtonVariant},
        feedback::Banner,
        forms::{Choice, InputEvent, SearchInput, Select, Switch, TextInput},
        layout::{columns_for, ScrollArea},
        maps::{LatLon, MapMarker, MapView, MapViewport, Tile, WorldMap},
        motion::ProgressBar,
        overlays::{ConfirmDialog, Dialog, PromptDialog},
        primitives::{FocusScope, Icon, IconName, Image as ElyImage, Severity},
        theme::{ActiveTheme, Mode, Theme},
        typography::{Heading, Paragraph},
    },
    gpui::{
        canvas, div, list, px, relative, size, AnyElement, App, AppContext, AsyncApp, Bounds,
        Context, Entity, FocusHandle, Image, ImageFormat, ImageSource, InteractiveElement,
        IntoElement, KeyDownEvent, ListAlignment, ListOffset, ListState, ParentElement,
        PathPromptOptions, Pixels, Render, SharedString, Styled, Subscription, SystemNotification,
        TitlebarOptions, WeakEntity, Window, WindowAppearance, WindowBounds, WindowOptions,
    },
    gpui_platform, i18n, insight_percent, InfoCard, MediaViewer, PhotoCard, PhotoGrid,
    SieguAppShell, ViewerField, ViewerInsight, ViewerMetadata, ViewerPerson,
};
use tokio::sync::mpsc::{self, UnboundedSender};
use url::Url;

/// A top-level place in the app. Keep the ids in sync with `siegu_ui::nav`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Section {
    Library,
    Map,
    People,
    Collections,
    Devices,
    Duplicates,
    Settings,
}

impl Section {
    fn from_id(id: &str) -> Self {
        match id {
            "map" => Self::Map,
            "people" => Self::People,
            "collections" => Self::Collections,
            "devices" => Self::Devices,
            "duplicates" => Self::Duplicates,
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
            Self::Devices => "devices",
            Self::Duplicates => "duplicates",
            Self::Settings => "settings",
        }
    }

    fn subtitle(self, cx: &App) -> SharedString {
        match self {
            Self::Library => "Every photo and video, all in one place.".into(),
            Self::Map => i18n::text(cx, "map.desc", &[]),
            Self::People => i18n::text(cx, "people.desc", &[]),
            Self::Collections => i18n::text(cx, "albums.desc", &[]),
            Self::Devices => i18n::text(cx, "devices.desc", &[]),
            Self::Duplicates => i18n::text(cx, "duplicates.desc", &[]),
            Self::Settings => i18n::text(cx, "settings.desc", &[]),
        }
    }
}

/// How the desktop palette chooses light or dark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ThemePreference {
    /// Follow the OS window appearance.
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    fn from_setting(value: &str) -> Self {
        if value.eq_ignore_ascii_case("light") {
            Self::Light
        } else if value.eq_ignore_ascii_case("dark") {
            Self::Dark
        } else {
            Self::System
        }
    }

    fn as_setting(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn appearance(self, system: WindowAppearance) -> WindowAppearance {
        match self {
            Self::System => system,
            Self::Light => WindowAppearance::Light,
            Self::Dark => WindowAppearance::Dark,
        }
    }

    fn mode(self, system: WindowAppearance) -> Mode {
        self.appearance(system).into()
    }
}

const PHOTO_WINDOW_SIZE: usize = 180;
const PHOTO_GAP: Pixels = px(16.0);
const PHOTO_CARD_WIDTH: Pixels = px(220.0);
const PHOTO_ROW_HEIGHT: Pixels = px(236.0);
const LIBRARY_MONTH_HEADER_HEIGHT: Pixels = px(64.0);
const LIBRARY_DAY_HEADER_HEIGHT: Pixels = px(40.0);
const MAP_CLUSTER_DECIMALS: u32 = 2;
const DEFAULT_MAP_TILE_URL: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
const CARTO_LIGHT_MAP_TILE_URL: &str =
    "https://{s}.basemaps.cartocdn.com/light_all/{z}/{x}/{y}{r}.png";
const CARTO_VOYAGER_MAP_TILE_URL: &str =
    "https://{s}.basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}{r}.png";

/// A built-in online tile provider. OpenStreetMap is the safe default: it works
/// without an API key and is selected the first time online tiles are enabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum MapTileProvider {
    #[default]
    Osm,
    CartoLight,
    CartoVoyager,
    Custom,
}

impl MapTileProvider {
    fn from_setting(value: &str) -> Self {
        match value {
            "carto_light" => Self::CartoLight,
            "carto_voyager" => Self::CartoVoyager,
            "custom" => Self::Custom,
            _ => Self::Osm,
        }
    }

    fn as_setting(self) -> &'static str {
        match self {
            Self::Osm => "osm",
            Self::CartoLight => "carto_light",
            Self::CartoVoyager => "carto_voyager",
            Self::Custom => "custom",
        }
    }

    fn from_url(url: &str) -> Self {
        match url.trim() {
            "" | DEFAULT_MAP_TILE_URL => Self::Osm,
            CARTO_LIGHT_MAP_TILE_URL => Self::CartoLight,
            CARTO_VOYAGER_MAP_TILE_URL => Self::CartoVoyager,
            _ => Self::Custom,
        }
    }

    fn url(self) -> Option<&'static str> {
        match self {
            Self::Osm => Some(DEFAULT_MAP_TILE_URL),
            Self::CartoLight => Some(CARTO_LIGHT_MAP_TILE_URL),
            Self::CartoVoyager => Some(CARTO_VOYAGER_MAP_TILE_URL),
            Self::Custom => None,
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Self::Osm => "settings.map_provider_osm",
            Self::CartoLight => "settings.map_provider_carto_light",
            Self::CartoVoyager => "settings.map_provider_carto_voyager",
            Self::Custom => "settings.map_provider_custom",
        }
    }

    fn accepts_key(self) -> bool {
        self != Self::Osm
    }
}

/// A library photo paired with its pre-resolved thumbnail source.
#[derive(Clone)]
struct PhotoEntry {
    photo: Photo,
    thumbnail: Option<ImageSource>,
}

/// A bounded, contiguous slice of library photos. The virtual grid only needs
/// the rows in view, so we never hold the whole library in memory.
#[derive(Default)]
struct PhotoWindow {
    query: String,
    favorites_only: bool,
    videos_only: bool,
    start: usize,
    photos: Vec<PhotoEntry>,
}

impl PhotoWindow {
    fn matches(&self, query: &str, favorites_only: bool, videos_only: bool) -> bool {
        self.query == query
            && self.favorites_only == favorites_only
            && self.videos_only == videos_only
    }

    fn contains(&self, index: usize) -> bool {
        index >= self.start && index - self.start < self.photos.len()
    }

    fn get(&self, index: usize) -> Option<&PhotoEntry> {
        self.contains(index)
            .then(|| &self.photos[index - self.start])
    }

    fn clear(&mut self) {
        self.photos.clear();
        self.start = 0;
    }
}

/// What one virtual item in the date-grouped library represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LibraryItemKind {
    Month,
    Day,
    Photos,
}

/// One virtual item in the date-grouped library. Month/day headers and photo
/// rows have different heights, so they live in variable-height `ListState`.
#[derive(Clone, Copy, Debug)]
struct LibraryItem {
    kind: LibraryItemKind,
    group: usize,
    row: usize,
    group_start: usize,
    group_count: usize,
}

/// A person paired with their pre-resolved representative face thumbnail.
struct PersonEntry {
    person: PersonWithFace,
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
    total_photos: usize,
    date_groups: Vec<PhotoDateGroup>,
    people: Vec<PersonEntry>,
    map_point_count: usize,
    map_clusters: Vec<MapCluster>,
    map_framing: MapFraming,
    map_tiles_enabled: bool,
    map_tile_provider: MapTileProvider,
    map_tile_url: String,
    map_tile_key: String,
    albums: Vec<AlbumEntry>,
    settings: Vec<(String, String)>,
    models: Vec<(String, String, bool)>,
    theme_preference: ThemePreference,
    auto_scan: bool,
    sync_enabled: bool,
    language: String,
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

/// Duplicate detection is lazy: the first visit to Space saver hashes files in
/// a background thread, and the result is cached until something changes it.
struct DuplicateData {
    groups: Vec<DuplicateGroupView>,
    stats: DuplicateStats,
    library_bytes: u64,
}

#[derive(Default)]
struct DuplicateStatus {
    running: bool,
    done: usize,
    total: usize,
    error: Option<String>,
}

enum DuplicateMessage {
    Progress { done: usize, total: usize },
    Complete(DuplicateData),
    Failed(String),
}

/// A pending device management dialog. The rename field is an entity so its
/// text and selection survive repaints while the dialog is open.
#[derive(Clone)]
enum DeviceDialog {
    Rename {
        id: String,
        field: Entity<TextInput>,
    },
    Remove {
        id: String,
        title: String,
    },
}

/// The desktop shell. Reads the local library and renders one section at a time.
struct Siegu {
    focus: FocusHandle,
    section: Section,
    search_input: Entity<TextInput>,
    search_query: String,
    map_tile_url_input: Entity<TextInput>,
    map_tile_key_input: Entity<TextInput>,
    favorites_only: bool,
    videos_only: bool,
    library_window: PhotoWindow,
    library_items: Vec<LibraryItem>,
    library_columns: usize,
    library_list: ListState,
    grid_width: Entity<Pixels>,
    drill_down: Option<DrillDown>,
    selected_photo: Option<usize>,
    viewer_override: Option<PhotoEntry>,
    /// Cached metadata for the open viewer, keyed by photo id. Opening a viewer
    /// reads a few linked SQLite tables once instead of on every repaint.
    viewer_metadata: Option<(String, ViewerMetadata)>,
    onboarding: bool,
    folders: Vec<String>,
    scan_status: ScanStatus,
    scan_running: bool,
    scan_abort: Option<Arc<AtomicBool>>,
    duplicates: Option<DuplicateData>,
    duplicate_status: DuplicateStatus,
    /// Devices are read from SQLite on demand; no networking is involved.
    devices: Vec<DeviceInfo>,
    device_dialog: Option<DeviceDialog>,
    device_error: Option<String>,
    data: LibraryData,
    _appearance: Subscription,
}

impl Siegu {
    fn section_title(&self, cx: &App) -> SharedString {
        match self.section {
            Section::Library => i18n::text(cx, "dock.home", &[]),
            Section::Map => i18n::text(cx, "dock.map", &[]),
            Section::People => i18n::text(cx, "search.people_section", &[]),
            Section::Collections => i18n::text(cx, "dock.collections", &[]),
            Section::Devices => i18n::text(cx, "dock.devices", &[]),
            Section::Duplicates => i18n::text(cx, "dock.duplicates", &[]),
            Section::Settings => i18n::text(cx, "dock.settings", &[]),
        }
    }

    fn active_count(&self) -> usize {
        self.drill_down
            .as_ref()
            .map(|dd| dd.photos.len())
            .unwrap_or(self.data.total_photos)
    }

    /// Returns one absolute photo from the active grid, loading a small window
    /// from SQLite when the virtual list has not reached it yet.
    fn photo_at(&mut self, index: usize) -> Option<PhotoEntry> {
        if let Some(drill_down) = &self.drill_down {
            return drill_down.photos.get(index).cloned();
        }
        if index >= self.data.total_photos {
            return None;
        }
        if !self.library_window.contains(index) {
            self.ensure_library_window(index, index + 1);
        }
        self.library_window.get(index).cloned()
    }

    /// Opens a media file from "Open with…" or a second app launch.
    fn open_media_file(&mut self, path: PathBuf) {
        let Some(entry) = photo_entry_from_path(&path) else {
            return;
        };
        self.section = Section::Library;
        self.drill_down = None;
        self.selected_photo = None;
        self.viewer_override = Some(entry);
    }

    fn viewer_for(
        &mut self,
        idx: usize,
        entry: PhotoEntry,
        total: usize,
        cx: &Context<Self>,
    ) -> MediaViewer {
        let metadata = self.viewer_metadata_for(&entry.photo, cx);
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
                        state.viewer_override = None;
                        cx.notify();
                    });
                }
            },
        )
        .detail(detail)
        .favorite(entry.photo.favorite)
        .metadata(metadata)
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
            let location = PathBuf::from(&entry.photo.location);
            modal = modal.source(location.clone());
            let weak_explorer = cx.entity().downgrade();
            modal = modal.on_show_in_explorer(move |_, cx| {
                if let Some(entity) = weak_explorer.upgrade() {
                    entity.update(cx, |_, cx| reveal_in_file_manager(&location, cx));
                }
            });
        }
        if let Some(thumb) = &entry.thumbnail {
            modal = modal.fallback(thumb.clone());
        }
        modal
    }

    /// Opens a map marker's photos in the same local drill-down grid used by
    /// People and Collections. Only SQLite is involved; no map tile request.
    fn open_map_cluster(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(cluster) = self
            .data
            .map_clusters
            .iter()
            .find(|cluster| cluster.key == key)
            .cloned()
        else {
            return;
        };
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let photos = db
            .get_photos_by_ids(&cluster.photo_ids)
            .into_iter()
            .map(|photo| {
                let thumbnail = resolve_thumbnail(&photo);
                PhotoEntry { photo, thumbnail }
            })
            .collect::<Vec<_>>();
        let title = if cluster.label.is_empty() {
            i18n::text(cx, "dock.map", &[]).to_string()
        } else {
            cluster.label
        };
        let subtitle = i18n::text(
            cx,
            "media.items_count",
            &[("count", &cluster.count.to_string())],
        )
        .to_string();
        self.drill_down = Some(DrillDown {
            title,
            subtitle,
            photos,
        });
        self.selected_photo = None;
        self.viewer_override = None;
        cx.notify();
    }

    /// Metadata for the open viewer, cached per photo id so repaints do not
    /// reopen SQLite. Faces and OCR live in linked tables, not on the photo row.
    fn viewer_metadata_for(&mut self, photo: &Photo, cx: &App) -> ViewerMetadata {
        if self
            .viewer_metadata
            .as_ref()
            .is_some_and(|(id, _)| id == &photo.id)
        {
            return self.viewer_metadata.as_ref().unwrap().1.clone();
        }

        let dir = default_config_dir();
        let db = Database::new(&dir.display().to_string());
        let is_video = is_video_path(&photo.location);
        let recognized_text = if is_video {
            String::new()
        } else {
            db.get_photo_ocr(&photo.id)
        };
        let transcript = photo
            .properties
            .get("transcript")
            .filter(|text| !text.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| db.get_photo_transcript(&photo.id));

        let unnamed = i18n::text(cx, "media_viewer.unnamed", &[]).to_string();
        let mut seen = HashSet::new();
        let people = db
            .get_faces_for_photo(&photo.id)
            .into_iter()
            .filter_map(|face| {
                let key = face
                    .person_id
                    .clone()
                    .unwrap_or_else(|| face.face_id.clone());
                if !seen.insert(key) {
                    return None;
                }
                let name = face
                    .person_name
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| unnamed.clone());
                Some(ViewerPerson {
                    name,
                    thumbnail: resolve_image_source(
                        Some(face.encoded.as_str()),
                        Some(face.crop_path.as_str()),
                    ),
                })
            })
            .collect();

        let mut insights = photo
            .objects
            .iter()
            .map(|(name, score)| ViewerInsight {
                name: name.clone(),
                percent: insight_percent(*score),
            })
            .collect::<Vec<_>>();
        insights.sort_by(|a, b| b.percent.cmp(&a.percent).then_with(|| a.name.cmp(&b.name)));

        let metadata = ViewerMetadata {
            file_path: photo.location.clone(),
            caption: photo
                .caption
                .as_deref()
                .map(str::trim)
                .filter(|caption| !caption.is_empty())
                .map(str::to_string),
            aesthetics_score: photo.aesthetics_score.filter(|score| score.is_finite()),
            recognized_text,
            transcript,
            is_video,
            camera: camera_fields(photo, cx),
            people,
            insights,
        };
        self.viewer_metadata = Some((photo.id.clone(), metadata.clone()));
        metadata
    }

    /// Render one virtual item: a month header, day header, or photo row.
    fn library_item(
        &mut self,
        item_ix: usize,
        columns: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(item) = self.library_items.get(item_ix).copied() else {
            return div().h(PHOTO_ROW_HEIGHT).into_any_element();
        };
        match item.kind {
            LibraryItemKind::Month => return self.library_month_header(item, cx),
            LibraryItemKind::Day => return self.library_day_header(item, cx),
            LibraryItemKind::Photos => {}
        }

        let (first, last) = row_bounds(item.row, columns, item.group_count);
        let first = item.group_start + first;
        let last = item.group_start + last;
        self.ensure_library_window(first, last);

        let mut row_view = div().flex().items_start().gap_4().h(PHOTO_ROW_HEIGHT);
        for index in first..last {
            let Some(entry) = self.library_window.get(index).cloned() else {
                continue;
            };
            let weak = cx.entity().downgrade();
            row_view = row_view.child(photo_card(index, &entry, move |_, app| {
                if let Some(entity) = weak.upgrade() {
                    entity.update(app, |state, cx| {
                        state.selected_photo = Some(index);
                        cx.notify();
                    });
                }
            }));
        }
        row_view.into_any_element()
    }

    fn library_month_header(&self, item: LibraryItem, cx: &Context<Self>) -> AnyElement {
        let Some(group) = self.data.date_groups.get(item.group) else {
            return div().h(LIBRARY_MONTH_HEADER_HEIGHT).into_any_element();
        };
        let title = month_label(&group.key, cx);
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(LIBRARY_MONTH_HEADER_HEIGHT)
            .px_1()
            .child(Heading::h3(title))
            .child(self.library_count_pill(item.group_count, cx))
            .into_any_element()
    }

    fn library_day_header(&self, item: LibraryItem, cx: &Context<Self>) -> AnyElement {
        let Some(group) = self.data.date_groups.get(item.group) else {
            return div().h(LIBRARY_DAY_HEADER_HEIGHT).into_any_element();
        };
        let title = day_label(&group.key, cx);
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(LIBRARY_DAY_HEADER_HEIGHT)
            .pl_4()
            .pr_1()
            .child(Heading::h4(title))
            .child(self.library_count_pill(item.group_count, cx))
            .into_any_element()
    }

    fn library_count_pill(&self, count: usize, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let label = i18n::text(cx, "media.items_count", &[("count", &count.to_string())]);
        div()
            .rounded_full()
            .border_1()
            .border_color(theme.colors.border)
            .px_2()
            .py_1()
            .child(Paragraph::new(label))
            .into_any_element()
    }

    fn ensure_library_window(&mut self, first: usize, last: usize) {
        let last = last.min(self.data.total_photos);
        if last <= first {
            return;
        }
        if self
            .library_window
            .matches(&self.search_query, self.favorites_only, self.videos_only)
            && first >= self.library_window.start
            && last <= self.library_window.start + self.library_window.photos.len()
        {
            return;
        }

        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let start = first.saturating_sub(PHOTO_WINDOW_SIZE / 3);
        let end = (last + PHOTO_WINDOW_SIZE * 2 / 3).min(self.data.total_photos);
        if end <= start {
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let mut photos = db.list_photos_filtered(
            &self.search_query,
            start,
            end - start,
            self.favorites_only,
            self.videos_only,
            &PhotoFilter::default(),
            false,
        );
        let ids: Vec<String> = photos.iter().map(|photo| photo.id.clone()).collect();
        let encoded = db.get_photo_encoded_batch(&ids);
        for photo in &mut photos {
            if let Some(value) = encoded.get(&photo.id) {
                photo.encoded = value.clone();
            }
        }
        let photos = photos
            .into_iter()
            .map(|photo| {
                let thumbnail = resolve_thumbnail(&photo);
                PhotoEntry { photo, thumbnail }
            })
            .collect();
        self.library_window = PhotoWindow {
            query: self.search_query.clone(),
            favorites_only: self.favorites_only,
            videos_only: self.videos_only,
            start,
            photos,
        };
    }

    fn reset_library_grid(&mut self) {
        self.library_window.clear();
        self.rebuild_library_items(self.library_columns);
        self.library_list.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: Pixels::ZERO,
        });
        self.selected_photo = None;
        self.viewer_override = None;
    }

    /// Flatten day buckets into month/day virtual list items. The bucket counts
    /// come from SQLite, so this stays small even for a 100k-photo library.
    fn rebuild_library_items(&mut self, columns: usize) {
        let columns = columns.max(1);
        self.library_items = library_items_for_groups(&self.data.date_groups, columns);
        self.library_columns = columns;
        self.library_list.reset(self.library_items.len());
    }

    fn reload_library_photos(&mut self) {
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            self.data.total_photos = 0;
            self.data.date_groups.clear();
        } else {
            let db = Database::new(&dir.display().to_string());
            self.data.total_photos = db
                .count_photos_filtered(
                    &self.search_query,
                    self.favorites_only,
                    self.videos_only,
                    &PhotoFilter::default(),
                )
                .max(0) as usize;
            self.data.date_groups = db.list_photo_date_groups(
                &self.search_query,
                self.favorites_only,
                self.videos_only,
                &PhotoFilter::default(),
            );
        }
        self.reset_library_grid();
    }

    fn refresh_data(&mut self) {
        self.data = load_library();
        self.library_window.clear();
        self.rebuild_library_items(self.library_columns);
        self.library_list.scroll_to(ListOffset {
            item_ix: 0,
            offset_in_item: Pixels::ZERO,
        });
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
                let message = format!(
                    "Scan complete — {} new media file{}.",
                    files_found,
                    if files_found == 1 { "" } else { "s" }
                );
                notify_scan("Siegu scan complete", &message, cx);
                self.scan_status = ScanStatus {
                    phase: ScanPhase::Complete,
                    message,
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
                notify_scan("Siegu scan failed", &message, cx);
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

    /// Kicks off duplicate detection the first time Space saver is opened, or
    /// when the user asks for a rescan after removing duplicates.
    fn ensure_duplicates(&mut self, cx: &mut Context<Self>) {
        if self.duplicates.is_none() && !self.duplicate_status.running {
            self.start_duplicates_scan(cx);
        }
    }

    fn start_duplicates_scan(&mut self, cx: &mut Context<Self>) {
        if self.duplicate_status.running {
            return;
        }
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.duplicate_status = DuplicateStatus {
            running: true,
            ..Default::default()
        };
        let _ = std::thread::Builder::new()
            .name("siegu-duplicates".to_string())
            .spawn(move || run_duplicate_scan(dir, tx));

        cx.spawn(move |this: WeakEntity<Siegu>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                while let Some(message) = rx.recv().await {
                    let alive = cx.update(|app| match this.upgrade() {
                        Some(entity) => {
                            entity.update(app, |state, cx| {
                                state.apply_duplicate_message(message, cx);
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

    fn apply_duplicate_message(&mut self, message: DuplicateMessage, cx: &mut Context<Self>) {
        match message {
            DuplicateMessage::Progress { done, total } => {
                self.duplicate_status.done = done;
                self.duplicate_status.total = total;
            }
            DuplicateMessage::Complete(data) => {
                self.duplicate_status = DuplicateStatus::default();
                self.duplicates = Some(data);
            }
            DuplicateMessage::Failed(message) => {
                self.duplicate_status = DuplicateStatus {
                    error: Some(message),
                    ..Default::default()
                };
            }
        }
        cx.notify();
    }

    /// Trashes every non-best member of one group, then refreshes the library
    /// and recomputes the remaining groups in the background.
    fn trash_duplicate_group(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(group) = self
            .duplicates
            .as_ref()
            .and_then(|data| data.groups.get(idx))
            .cloned()
        else {
            return;
        };
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let trashed = duplicates::trash_group_non_best(&db, &group);
        if trashed == 0 {
            return;
        }
        self.duplicates = None;
        self.refresh_data();
        self.start_duplicates_scan(cx);
    }

    fn toggle_photo_favorite(&mut self, photo_id: &str) {
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let new_fav = db.toggle_favorite(photo_id);
        for entry in &mut self.library_window.photos {
            if entry.photo.id == photo_id {
                entry.photo.favorite = new_fav;
            }
        }
        if let Some(drill_down) = &mut self.drill_down {
            for entry in &mut drill_down.photos {
                if entry.photo.id == photo_id {
                    entry.photo.favorite = new_fav;
                }
            }
        }
        if self.drill_down.is_none() && self.favorites_only {
            self.reload_library_photos();
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
        self.viewer_override = None;
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

    fn body(&mut self, cx: &mut Context<Self>) -> AnyElement {
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
                                            state.viewer_override = None;
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
            Section::Devices => self.devices_view(cx),
            Section::Duplicates => self.duplicates_view(cx),
            Section::Settings => self.settings_view(cx),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .gap_4()
            .p_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Heading::h2(self.section_title(cx)))
                    .child(Paragraph::new(self.section.subtitle(cx))),
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

    fn library_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let weak_fav = cx.entity().downgrade();
        let weak_vid = cx.entity().downgrade();
        let weak_add = cx.entity().downgrade();
        let weak_scan = cx.entity().downgrade();

        let has_filters = !self.search_query.is_empty() || self.favorites_only || self.videos_only;
        let (empty_title, empty_hint): (SharedString, SharedString) = if has_filters {
            (
                i18n::text(cx, "media.no_results", &[]),
                i18n::text(cx, "search.clear_all", &[]),
            )
        } else {
            (
                i18n::text(cx, "media.your_library_empty", &[]),
                i18n::text(cx, "media.add_folder_hint", &[]),
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
                Button::new(
                    "library-add-folder",
                    i18n::text(cx, "settings.add_folder", &[]),
                )
                .icon(IconName::FolderPlus)
                .on_click(move |_, _, app| prompt_for_folders(weak_add.clone(), app, false)),
            )
            .child(
                Button::new(
                    "library-scan",
                    if self.scan_running {
                        i18n::text(cx, "sync.scanning", &[])
                    } else {
                        i18n::text(cx, "scan.scan_button", &[])
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
                    .label(i18n::text(cx, "search.magic.favorites", &[]))
                    .on_change(move |on, _, cx| {
                        if let Some(entity) = weak_fav.upgrade() {
                            entity.update(cx, |state, cx| {
                                state.favorites_only = on;
                                state.reload_library_photos();
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                Switch::new("filter-videos", self.videos_only)
                    .label(i18n::text(cx, "search.magic.videos", &[]))
                    .on_change(move |on, _, cx| {
                        if let Some(entity) = weak_vid.upgrade() {
                            entity.update(cx, |state, cx| {
                                state.videos_only = on;
                                state.reload_library_photos();
                                cx.notify();
                            });
                        }
                    }),
            )
            .children((self.data.total_photos > 0).then(|| {
                Paragraph::new(i18n::text(
                    cx,
                    "search.photos_count",
                    &[("count", &self.data.total_photos.to_string())],
                ))
            }));

        let width = self.grid_width.clone();
        let columns = columns_for(*width.read(cx), PHOTO_CARD_WIDTH, PHOTO_GAP).max(1) as usize;
        if self.library_columns != columns
            || (self.library_items.is_empty() && self.data.total_photos > 0)
        {
            self.rebuild_library_items(columns);
        }
        let grid = if self.data.total_photos == 0 {
            empty_state(&empty_title, &empty_hint)
        } else {
            list(
                self.library_list.clone(),
                cx.processor(move |state: &mut Siegu, ix: usize, _, cx| {
                    state.library_item(ix, columns, cx)
                }),
            )
            .flex_1()
            .min_h_0()
            .into_any_element()
        };

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .gap_4()
            .child(toolbar)
            .child(grid)
            .child(
                canvas(
                    move |bounds, window, cx| {
                        if *width.read(cx) != bounds.size.width {
                            width.update(cx, |value, cx| {
                                *value = bounds.size.width;
                                cx.notify();
                            });
                            window.request_animation_frame();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element()
    }

    fn map_view(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.data.map_clusters.is_empty() {
            return empty_state(
                &i18n::text(cx, "map.no_photos", &[]),
                &i18n::text(cx, "map.no_photos_desc", &[]),
            );
        }

        let framing = self.data.map_framing;
        let viewport = MapViewport::new(
            LatLon::new(
                framing.latitude.clamp(-85.0, 85.0),
                framing.longitude.clamp(-180.0, 180.0),
            ),
            framing.zoom,
        );
        let markers = self.data.map_clusters.iter().map(|cluster| {
            let at = LatLon::new(
                cluster.latitude.clamp(-85.0, 85.0),
                cluster.longitude.clamp(-180.0, 180.0),
            );
            let marker = MapMarker::new(cluster.key.clone(), at);
            if cluster.count > 1 {
                marker.label(cluster.count.to_string())
            } else {
                marker
            }
        });
        let weak = cx.entity().downgrade();
        let mut map = MapView::new(("map-view", self.data.map_point_count), viewport)
            .cluster_markers()
            .markers(markers)
            .on_marker(move |key, _, app| {
                if let Some(entity) = weak.upgrade() {
                    let key = key.to_string();
                    entity.update(app, |state, cx| state.open_map_cluster(&key, cx));
                }
            });

        let template = self
            .data
            .map_tiles_enabled
            .then(|| map_tile_template(&self.data.map_tile_url, &self.data.map_tile_key))
            .flatten();
        let online_tiles = template.is_some();
        if let Some(template) = template {
            let attribution = if template.contains("cartocdn.com") {
                "Tiles: CARTO, OpenStreetMap contributors"
            } else if template.contains("openstreetmap.org") {
                "Tiles: OpenStreetMap contributors"
            } else {
                "Online tiles: custom provider"
            };
            map = map
                .tiles(move |tile| ImageSource::from(render_map_tile_url(&template, tile)))
                .attribution(attribution);
        } else {
            map = map.attribution("Offline: Natural Earth");
        }

        let notice_key = if online_tiles {
            "map.online_notice"
        } else {
            "map.offline_notice"
        };
        // WorldMap paints every country with a "no data" wash. That is the
        // offline basemap, but it would hide real tiles, so online maps render
        // the tile view directly.
        let map_body = if online_tiles {
            div().flex_1().min_h_0().child(map).into_any_element()
        } else {
            let world = WorldMap::new(
                ("map-world", self.data.map_point_count),
                map,
                std::iter::empty::<(&str, f64)>(),
            );
            div().flex_1().min_h_0().child(world).into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_4()
                    .child(Paragraph::new(i18n::text(
                        cx,
                        "search.photos_count",
                        &[("count", &self.data.map_point_count.to_string())],
                    )))
                    .child(Paragraph::new(format!(
                        "{} locations",
                        self.data.map_clusters.len()
                    ))),
            )
            .child(Banner::new(
                "map-offline-notice",
                Severity::Info,
                i18n::text(cx, notice_key, &[]),
            ))
            .child(map_body)
            .into_any_element()
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

    fn duplicates_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.ensure_duplicates(cx);

        let weak_scan = cx.entity().downgrade();
        let toolbar = div().flex().flex_wrap().items_center().gap_3().child(
            Button::new("duplicates-scan", i18n::text(cx, "duplicates.refresh", &[]))
                .icon(IconName::RefreshCw)
                .loading(self.duplicate_status.running)
                .disabled(self.duplicate_status.running)
                .on_click(move |_, _, app| {
                    if let Some(entity) = weak_scan.upgrade() {
                        entity.update(app, |state, cx| state.start_duplicates_scan(cx));
                    }
                }),
        );

        let error = self.duplicate_status.error.as_ref().map(|message| {
            Banner::new("duplicates-error", Severity::Danger, message.clone()).into_any_element()
        });
        let progress = self.duplicate_status.running.then(|| {
            let done = self.duplicate_status.done;
            let total = self.duplicate_status.total;
            let bar = if total > 0 {
                ProgressBar::new(
                    "duplicates-progress",
                    (done as f32 / total as f32).clamp(0.0, 1.0),
                )
                .into_any_element()
            } else {
                ProgressBar::indeterminate("duplicates-progress").into_any_element()
            };
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(Paragraph::new(i18n::text(cx, "duplicates.scanning", &[])))
                .child(bar)
                .child(Paragraph::new(i18n::text(
                    cx,
                    "duplicates.scanning_count",
                    &[("done", &done.to_string()), ("total", &total.to_string())],
                )))
                .into_any_element()
        });

        let Some(data) = self.duplicates.as_ref() else {
            let hint = if self.duplicate_status.running {
                i18n::text(cx, "duplicates.scanning_hint", &[])
            } else {
                "Add a folder and scan your library to find duplicates.".into()
            };
            return div()
                .flex()
                .flex_col()
                .size_full()
                .min_h_0()
                .gap_4()
                .child(toolbar)
                .children(error)
                .children(progress)
                .child(empty_state(&i18n::text(cx, "duplicates.title", &[]), &hint))
                .into_any_element();
        };

        let stats = div()
            .flex()
            .flex_wrap()
            .gap_3()
            .child(
                InfoCard::new(
                    "duplicate-library-size",
                    IconName::HardDrive,
                    i18n::text(cx, "duplicates.storage_total", &[]),
                )
                .detail(human_bytes(data.library_bytes)),
            )
            .child(
                InfoCard::new(
                    "duplicate-groups",
                    IconName::Copy,
                    i18n::text(cx, "duplicates.groups", &[]),
                )
                .detail(data.stats.group_count.to_string()),
            )
            .child(
                InfoCard::new(
                    "duplicate-count",
                    IconName::Layers,
                    i18n::text(cx, "duplicates.duplicates", &[]),
                )
                .detail(data.stats.duplicate_count.to_string()),
            )
            .child(
                InfoCard::new(
                    "duplicate-reclaimable",
                    IconName::Zap,
                    i18n::text(cx, "duplicates.reclaimable", &[]),
                )
                .detail(human_bytes(data.stats.reclaimable_bytes)),
            );

        let groups: AnyElement = if data.groups.is_empty() {
            empty_state(
                &i18n::text(cx, "duplicates.no_duplicates", &[]),
                &i18n::text(cx, "duplicates.no_duplicates_desc", &[]),
            )
        } else {
            let mut grid = div().flex().flex_wrap().gap_3();
            for (idx, group) in data.groups.iter().enumerate() {
                let kind = duplicate_kind_label(cx, &group.kind);
                let count = plural(group.members.len(), "photo");
                let reclaimable = human_bytes(group.reclaimable_bytes);
                let thumb = group
                    .members
                    .iter()
                    .find(|member| member.is_best)
                    .or_else(|| group.members.first())
                    .and_then(|member| resolve_image_source(None, Some(member.location.as_str())));
                let leading = match thumb {
                    Some(source) => ElyImage::new(("duplicate-thumb", idx), source)
                        .w(px(44.0))
                        .h(px(44.0))
                        .flex_none()
                        .rounded(px(8.0))
                        .into_any_element(),
                    None => div()
                        .w(px(44.0))
                        .h(px(44.0))
                        .flex_none()
                        .child(Icon::new(IconName::Copy))
                        .into_any_element(),
                };

                let ids: Vec<String> = group
                    .members
                    .iter()
                    .map(|member| member.id.clone())
                    .collect();
                let drill_title = kind.to_string();
                let drill_subtitle = format!("{} · {}", count, reclaimable);
                let weak_view = cx.entity().downgrade();
                let view_button = Button::new(
                    ("duplicate-view", idx),
                    i18n::text(cx, "duplicates.contents", &[]),
                )
                .icon(IconName::FolderOpen)
                .on_click(move |_, _, app| {
                    if let Some(entity) = weak_view.upgrade() {
                        let ids = ids.clone();
                        let title = drill_title.clone();
                        let subtitle = drill_subtitle.clone();
                        entity.update(app, |state, cx| {
                            state.open_drill_down(title, subtitle, |db| {
                                ids.iter().filter_map(|id| db.get_photo_by_id(id)).collect()
                            });
                            cx.notify();
                        });
                    }
                });

                let remove_count = group.members.len().saturating_sub(1);
                let weak_trash = cx.entity().downgrade();
                let trash_button = Button::new(
                    ("duplicate-trash", idx),
                    i18n::text(
                        cx,
                        "duplicates.trash_duplicates",
                        &[("count", &remove_count.to_string())],
                    ),
                )
                .variant(ButtonVariant::Danger)
                .icon(IconName::Trash2)
                .on_click(move |_, _, app| {
                    if let Some(entity) = weak_trash.upgrade() {
                        entity.update(app, |state, cx| state.trash_duplicate_group(idx, cx));
                    }
                });

                grid = grid.child(
                    div()
                        .id(("duplicate-group", idx))
                        .w(px(320.0))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_3()
                        .child(
                            div().flex().items_center().gap_3().child(leading).child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(Paragraph::new(kind))
                                    .child(Paragraph::new(count))
                                    .child(Paragraph::new(reclaimable)),
                            ),
                        )
                        .child(div().flex().gap_2().child(view_button).child(trash_button)),
                );
            }
            grid.into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .gap_4()
            .child(toolbar)
            .children(error)
            .children(progress)
            .child(stats)
            .child(groups)
            .into_any_element()
    }

    fn settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak_theme = cx.entity().downgrade();
        let weak_scan = cx.entity().downgrade();
        let weak_sync = cx.entity().downgrade();
        let weak_folder = cx.entity().downgrade();
        let weak_add_folder = cx.entity().downgrade();
        let weak_language = cx.entity().downgrade();
        let weak_map = cx.entity().downgrade();
        let weak_map_url_input = self.map_tile_url_input.downgrade();

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
                        Button::new(("remove-folder", idx), i18n::text(cx, "common.remove", &[]))
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
                    .child(Heading::h3(i18n::text(cx, "settings.folders", &[])))
                    .child(
                        Button::new(
                            "settings-add-folder",
                            i18n::text(cx, "settings.add_folder", &[]),
                        )
                        .icon(IconName::FolderPlus)
                        .on_click(move |_, _, app| {
                            prompt_for_folders(weak_add_folder.clone(), app, false)
                        }),
                    ),
            )
            .children((!self.folders.is_empty()).then_some(folder_rows))
            .children(
                self.folders
                    .is_empty()
                    .then(|| Paragraph::new(i18n::text(cx, "settings.no_folders", &[]))),
            );

        let toggles = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_6()
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

        let theme_choices = [
            Choice::new(
                ThemePreference::System.as_setting(),
                i18n::text(cx, "settings.theme_system", &[]),
            ),
            Choice::new(
                ThemePreference::Light.as_setting(),
                i18n::text(cx, "settings.theme_light", &[]),
            ),
            Choice::new(
                ThemePreference::Dark.as_setting(),
                i18n::text(cx, "settings.theme_dark", &[]),
            ),
        ];
        let appearance_section = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Heading::h3(i18n::text(cx, "settings.appearance", &[])))
            .child(
                Select::new("setting-theme", theme_choices)
                    .label(i18n::text(cx, "settings.appearance", &[]))
                    .selected(self.data.theme_preference.as_setting())
                    .on_change(move |value, window, app| {
                        let preference = ThemePreference::from_setting(value.as_ref());
                        let appearance = preference.appearance(window.appearance());
                        app.set_window_appearance(match preference {
                            ThemePreference::System => None,
                            _ => Some(appearance),
                        });
                        Theme::set_mode_now(preference.mode(window.appearance()), app);
                        if let Some(entity) = weak_theme.upgrade() {
                            entity.update(app, |state, cx| {
                                state.data.theme_preference = preference;
                                persist_setting("theme", preference.as_setting());
                                cx.notify();
                            });
                        }
                    }),
            );

        let weak_map_switch = weak_map.clone();
        let map_provider = self.data.map_tile_provider;
        let map_provider_choices = [
            MapTileProvider::Osm,
            MapTileProvider::CartoLight,
            MapTileProvider::CartoVoyager,
            MapTileProvider::Custom,
        ]
        .map(|provider| {
            Choice::new(
                provider.as_setting(),
                i18n::text(cx, provider.label_key(), &[]),
            )
        });
        let weak_provider_url_input = weak_map_url_input;
        let map_section = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Heading::h3(i18n::text(cx, "settings.map", &[])))
            .child(Paragraph::new(i18n::text(cx, "settings.map_hint", &[])))
            .child(
                Switch::new("setting-map-tiles", self.data.map_tiles_enabled)
                    .label(i18n::text(cx, "settings.map_tiles_enabled", &[]))
                    .on_change(move |on, _, app| {
                        if let Some(entity) = weak_map_switch.upgrade() {
                            entity.update(app, |state, cx| {
                                state.data.map_tiles_enabled = on;
                                persist_setting(
                                    "map_tiles_enabled",
                                    if on { "true" } else { "false" },
                                );
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                Select::new("setting-map-provider", map_provider_choices)
                    .label(i18n::text(cx, "settings.map_tile_provider", &[]))
                    .selected(map_provider.as_setting())
                    .on_change(move |value, _, app| {
                        let provider = MapTileProvider::from_setting(value.as_ref());
                        let url = provider.url().map(str::to_string).unwrap_or_else(|| {
                            weak_provider_url_input
                                .upgrade()
                                .map(|input| input.read(app).text().trim().to_string())
                                .filter(|text| !text.is_empty())
                                .unwrap_or_else(|| DEFAULT_MAP_TILE_URL.to_string())
                        });
                        if let Some(input) = weak_provider_url_input.upgrade() {
                            input.update(app, |input, cx| input.set_text(url.clone(), cx));
                        }
                        if let Some(entity) = weak_map.upgrade() {
                            entity.update(app, |state, cx| {
                                state.data.map_tile_provider = provider;
                                state.data.map_tile_url = url;
                                persist_setting("map_tile_url", &state.data.map_tile_url);
                                persist_setting("map_tile_provider", provider.as_setting());
                                cx.notify();
                            });
                        }
                    }),
            )
            .children(
                (map_provider == MapTileProvider::Custom).then(|| self.map_tile_url_input.clone()),
            )
            .children(
                map_provider
                    .accepts_key()
                    .then(|| self.map_tile_key_input.clone()),
            );

        let language_choices = i18n::CODES
            .iter()
            .map(|code| Choice::new(*code, i18n::language_label(cx, code)));
        let language_section = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Heading::h3(i18n::text(cx, "language.label", &[])))
            .child(
                Select::new("setting-language", language_choices)
                    .label(i18n::text(cx, "language.label", &[]))
                    .selected(self.data.language.clone())
                    .on_change(move |value, _, cx| {
                        let code = value.to_string();
                        if let Some(entity) = weak_language.upgrade() {
                            entity.update(cx, |state, cx| {
                                let code = i18n::normalize(&code);
                                state.data.language = code.clone();
                                for (label, value) in &mut state.data.settings {
                                    if label == "Language" {
                                        *value = code.clone();
                                    }
                                }
                                persist_setting("language", &code);
                                i18n::set_language(&code, cx);
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
            .child(appearance_section)
            .child(map_section)
            .child(language_section)
            .children((!self.data.models.is_empty()).then(|| {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Heading::h3(i18n::text(cx, "settings.ai_models", &[])))
                    .child(model_row)
            }))
            .children((!self.data.settings.is_empty()).then_some(grid))
            .into_any_element()
    }

    fn reload_devices(&mut self, cx: &mut Context<Self>) {
        let dir = default_config_dir();
        let config_path = dir.display().to_string();
        let db = Database::new(&config_path);
        self.devices = list_core_devices(&db, &config_path);
        self.device_error = None;
        cx.notify();
    }

    fn rename_device(&mut self, id: &str, name: &str, cx: &mut Context<Self>) {
        let dir = default_config_dir();
        let db = Database::new(&dir.display().to_string());
        match rename_core_device(&db, id, name) {
            Ok(()) => self.reload_devices(cx),
            Err(error) => {
                self.device_error = Some(error);
                cx.notify();
            }
        }
    }

    fn remove_device(&mut self, id: &str, cx: &mut Context<Self>) {
        let dir = default_config_dir();
        let db = Database::new(&dir.display().to_string());
        match remove_core_device(&db, id) {
            Ok(()) => self.reload_devices(cx),
            Err(error) => {
                self.device_error = Some(error);
                cx.notify();
            }
        }
    }

    fn devices_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let weak_reload = cx.entity().downgrade();
        let toolbar = div().flex().flex_wrap().items_center().gap_3().child(
            Button::new("devices-reload", i18n::text(cx, "sync.refresh", &[]))
                .icon(IconName::RefreshCw)
                .on_click(move |_, _, app| {
                    if let Some(entity) = weak_reload.upgrade() {
                        entity.update(app, |state, cx| state.reload_devices(cx));
                    }
                }),
        );

        let weak_dismiss = cx.entity().downgrade();
        let error = self.device_error.as_ref().map(|message| {
            Banner::new("devices-error", Severity::Danger, message.clone())
                .action(i18n::text(cx, "common.close", &[]), move |_, cx| {
                    if let Some(entity) = weak_dismiss.upgrade() {
                        entity.update(cx, |state, cx| {
                            state.device_error = None;
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        });

        if self.devices.is_empty() {
            return div()
                .flex()
                .flex_col()
                .size_full()
                .min_h_0()
                .gap_4()
                .child(toolbar)
                .children(error)
                .child(empty_state(
                    &i18n::text(cx, "devices.no_devices", &[]),
                    &i18n::text(cx, "devices.no_devices_desc", &[]),
                ))
                .into_any_element();
        }

        let mut cards = div().flex().flex_wrap().gap_3();
        for (idx, device) in self.devices.iter().enumerate() {
            cards = cards.child(self.device_card(device, idx, cx));
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .gap_4()
            .child(toolbar)
            .children(error)
            .child(cards)
            .into_any_element()
    }

    fn device_card(&self, device: &DeviceInfo, idx: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let status_severity = if device.host {
            Severity::Success
        } else {
            Severity::Info
        };
        let status_label = if device.host {
            i18n::text(cx, "devices.this_device", &[])
        } else {
            i18n::text(cx, "devices.status_idle", &[])
        };
        let system = if device.os.trim().is_empty() {
            "—".to_string()
        } else {
            device.os.clone()
        };
        let photos = effective_count(device.remote_photo_count, device.photo_count);
        let videos = effective_count(device.remote_video_count, device.video_count);
        let percent = storage_percent(device.storage_used, device.storage_capacity);
        let storage_severity = storage_severity(percent);
        let storage_label = if device.storage_capacity > 0 {
            format!(
                "{} / {}",
                human_bytes(device.storage_used),
                human_bytes(device.storage_capacity)
            )
        } else {
            "—".to_string()
        };

        let mut actions = div().flex().flex_wrap().items_center().gap_2().child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .rounded_full()
                .border_1()
                .border_color(theme.colors.border)
                .px_2()
                .py_1()
                .child(
                    div()
                        .size(px(8.0))
                        .rounded_full()
                        .bg(status_severity.color(&theme.colors)),
                )
                .child(Paragraph::new(status_label)),
        );

        if !device.host {
            let weak_rename = cx.entity().downgrade();
            let rename_id = device.id.clone();
            let rename_title = device.title.clone();
            let rename = Button::new(
                ("device-rename", idx),
                i18n::text(cx, "devices.rename_device", &[]),
            )
            .icon(IconName::Pencil)
            .on_click(move |_, window, app| {
                if let Some(entity) = weak_rename.upgrade() {
                    let id = rename_id.clone();
                    let title = rename_title.clone();
                    entity.update(app, |state, cx| {
                        let field = cx.new(|cx| TextInput::new(window, cx));
                        field.update(cx, |input, cx| input.set_text(title, cx));
                        state.device_dialog = Some(DeviceDialog::Rename { id, field });
                        state.device_error = None;
                        cx.notify();
                    });
                }
            });

            let weak_remove = cx.entity().downgrade();
            let remove_id = device.id.clone();
            let remove_title = device.title.clone();
            let remove = Button::new(
                ("device-remove", idx),
                i18n::text(cx, "devices.remove_device", &[]),
            )
            .icon(IconName::Trash2)
            .variant(ButtonVariant::Ghost)
            .on_click(move |_, _, app| {
                if let Some(entity) = weak_remove.upgrade() {
                    let id = remove_id.clone();
                    let title = remove_title.clone();
                    entity.update(app, |state, cx| {
                        state.device_dialog = Some(DeviceDialog::Remove { id, title });
                        state.device_error = None;
                        cx.notify();
                    });
                }
            });
            actions = actions.child(rename).child(remove);
        }

        div()
            .id(("device-card", idx))
            .w(px(320.0))
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded(px(12.0))
            .border_1()
            .border_color(theme.colors.border)
            .bg(theme.colors.surface)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(40.0))
                            .rounded(px(10.0))
                            .bg(theme.colors.hover)
                            .child(Icon::new(device_icon(device))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Heading::h4(device.title.clone()))
                            .child(Paragraph::new(format!(
                                "{} {}",
                                i18n::text(cx, "devices.system", &[]),
                                system
                            ))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Paragraph::new(format!(
                        "{} {}",
                        i18n::text(cx, "media.label_photos", &[]),
                        photos
                    )))
                    .child(Paragraph::new(format!(
                        "{} {}",
                        i18n::text(cx, "media.label_videos", &[]),
                        videos
                    )))
                    .child(Paragraph::new(format!(
                        "{} {}",
                        i18n::text(cx, "devices.storage", &[]),
                        storage_label
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Paragraph::new(format!("{percent}%")))
                    .child(
                        div()
                            .relative()
                            .h(px(6.0))
                            .rounded_full()
                            .bg(theme.colors.sunken)
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .h_full()
                                    .rounded_full()
                                    .w(relative(percent as f32 / 100.0))
                                    .bg(storage_severity.color(&theme.colors)),
                            ),
                    ),
            )
            .child(actions)
            .into_any_element()
    }

    fn device_dialogs(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(dialog) = self.device_dialog.clone() else {
            return Vec::new();
        };
        let weak_close = cx.entity().downgrade();
        let close = move |_: &mut Window, cx: &mut App| {
            if let Some(entity) = weak_close.upgrade() {
                entity.update(cx, |state, cx| {
                    state.device_dialog = None;
                    state.device_error = None;
                    cx.notify();
                });
            }
        };

        match dialog {
            DeviceDialog::Rename { id, field } => {
                let weak_submit = cx.entity().downgrade();
                let submit_id = id.clone();
                vec![PromptDialog::new(
                    format!("device-rename-{id}"),
                    i18n::text(cx, "devices.rename_device_title", &[]),
                    &field,
                    close,
                )
                .label(i18n::text(cx, "devices.device_name", &[]))
                .submit(i18n::text(cx, "common.save", &[]))
                .check(|text| {
                    if text.trim().is_empty() {
                        Err("Name cannot be empty".into())
                    } else {
                        Ok(())
                    }
                })
                .on_submit(move |text, _, app| {
                    if let Some(entity) = weak_submit.upgrade() {
                        let id = submit_id.clone();
                        entity.update(app, |state, cx| state.rename_device(&id, text, cx));
                    }
                })
                .into_any_element()]
            }
            DeviceDialog::Remove { id, title } => {
                let weak_confirm = cx.entity().downgrade();
                let confirm_id = id.clone();
                let message = i18n::text(
                    cx,
                    "devices.remove_device_confirm",
                    &[("name", title.as_str())],
                );
                vec![ConfirmDialog::new(
                    format!("device-remove-{id}"),
                    i18n::text(cx, "devices.remove_device_title", &[]),
                    message,
                    close,
                )
                .confirm(i18n::text(cx, "common.remove", &[]))
                .destructive()
                .on_confirm(move |_, app| {
                    if let Some(entity) = weak_confirm.upgrade() {
                        let id = confirm_id.clone();
                        entity.update(app, |state, cx| state.remove_device(&id, cx));
                    }
                })
                .into_any_element()]
            }
        }
    }
}

const OPEN_WITH_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "heic", "heif", "avif", "mp4", "webm",
    "mov", "avi", "mkv", "m4v",
];

fn notify_scan(title: &str, body: &str, cx: &mut App) {
    cx.show_system_notification(SystemNotification {
        tag: "siegu-scan".into(),
        title: title.into(),
        body: body.into(),
        actions: Vec::new(),
    });
}

/// Reveal a local media file in the platform's file manager. We spawn the
/// process instead of blocking the UI, then reap it on a short-lived thread.
fn reveal_in_file_manager(path: &Path, cx: &mut App) {
    let body = match spawn_file_manager(path) {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            i18n::text(cx, "media_viewer.opened_in_explorer", &[])
        }
        Err(error) => {
            eprintln!("failed to open file manager: {error}");
            i18n::text(cx, "media_viewer.explorer_error", &[])
        }
    };
    cx.show_system_notification(SystemNotification {
        tag: "siegu-explorer".into(),
        title: i18n::text(cx, "app.name", &[]),
        body,
        actions: Vec::new(),
    });
}

fn spawn_file_manager(path: &Path) -> io::Result<Child> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let _ = parent;

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        command.arg(format!("/select,{}", path.display()));
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg("-R").arg(path);
        command
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(parent);
        command
    };

    command.stdout(Stdio::null()).stderr(Stdio::null()).spawn()
}

fn path_from_open_arg(arg: &str) -> Option<PathBuf> {
    // A Windows drive path such as `C:\photos\x.jpg` parses as a URL with the
    // scheme `c`, so only attempt URL parsing when an explicit `://` delimiter
    // is present.
    if arg.contains("://") {
        let url = Url::parse(arg).ok()?;
        if url.scheme() == "file" {
            return url.to_file_path().ok();
        }
        return None;
    }
    Some(PathBuf::from(arg))
}

fn path_is_openable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| OPEN_WITH_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Find the first supported media file in a process argument list.
fn opened_media_arg(args: &[String]) -> Option<PathBuf> {
    args.iter()
        .filter_map(|arg| path_from_open_arg(arg))
        .find(|path| path_is_openable(path))
}

/// Find the first supported media file in a single-instance payload, whose
/// first field is the sending process's working directory.
fn opened_media_request(request: &[String]) -> Option<PathBuf> {
    let cwd = request.first().map(PathBuf::from).unwrap_or_default();
    let resolved: Vec<String> = request
        .iter()
        .skip(1)
        .map(|arg| {
            path_from_open_arg(arg)
                .map(|path| {
                    if path.is_relative() {
                        cwd.join(path)
                    } else {
                        path
                    }
                })
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| arg.clone())
        })
        .collect();
    opened_media_arg(&resolved)
}

fn photo_entry_from_path(path: &Path) -> Option<PhotoEntry> {
    let location = path.to_string_lossy().into_owned();
    let dir = default_config_dir();
    let indexed = dir.join("siegu.db").exists().then(|| {
        let db = Database::new(&dir.display().to_string());
        db.find_photo_id_by_location(&location)
            .and_then(|id| db.get_photo_by_id(&id))
    });
    let photo = indexed.flatten().unwrap_or_else(|| {
        let metadata = extract_photo_metadata(path);
        photo_from_metadata(&location, &metadata)
    });
    let thumbnail = resolve_thumbnail(&photo);
    Some(PhotoEntry { photo, thumbnail })
}

fn row_bounds(row: usize, columns: usize, total: usize) -> (usize, usize) {
    let first = row * columns;
    (first, (first + columns).min(total))
}

fn library_items_for_groups(groups: &[PhotoDateGroup], columns: usize) -> Vec<LibraryItem> {
    let columns = columns.max(1);
    let days: Vec<(usize, &PhotoDateGroup)> = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| group.count > 0)
        .collect();
    let total: usize = days.iter().map(|(_, group)| group.count).sum();
    let mut items = Vec::with_capacity(total / columns + days.len() * 2);
    let mut start = 0usize;
    let mut ix = 0usize;

    while ix < days.len() {
        // Day buckets arrive newest-first, so a month's days are contiguous.
        let key = month_key(&days[ix].1.key);
        let first_day = ix;
        let month_start = start;
        let mut month_count = 0usize;
        while ix < days.len() && month_key(&days[ix].1.key) == key {
            month_count += days[ix].1.count;
            ix += 1;
        }

        items.push(LibraryItem {
            kind: LibraryItemKind::Month,
            group: days[first_day].0,
            row: 0,
            group_start: month_start,
            group_count: month_count,
        });

        for (group_ix, group) in &days[first_day..ix] {
            items.push(LibraryItem {
                kind: LibraryItemKind::Day,
                group: *group_ix,
                row: 0,
                group_start: start,
                group_count: group.count,
            });
            for row in 0..group.count.div_ceil(columns) {
                items.push(LibraryItem {
                    kind: LibraryItemKind::Photos,
                    group: *group_ix,
                    row,
                    group_start: start,
                    group_count: group.count,
                });
            }
            start += group.count;
        }
    }
    items
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

fn is_video_path(path: &str) -> bool {
    let extension = path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    matches!(
        extension.as_deref(),
        Some("mp4" | "webm" | "mov" | "avi" | "mkv" | "m4v")
    )
}

fn non_empty_prop<'a>(props: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    props
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
}

/// The legacy viewer's resolution line: PixelXDimension/Y first, then ImageWidth/Length.
fn exif_dimensions(props: &HashMap<String, String>) -> Option<String> {
    [
        ("PixelXDimension", "PixelYDimension"),
        ("ImageWidth", "ImageLength"),
    ]
    .into_iter()
    .find_map(|(width, height)| {
        let width = non_empty_prop(props, width)?;
        let height = non_empty_prop(props, height)?;
        Some(format!("{width} × {height}"))
    })
}

fn push_prop_field(
    fields: &mut Vec<ViewerField>,
    props: &HashMap<String, String>,
    key: &str,
    label_key: &str,
    cx: &App,
) {
    if let Some(value) = non_empty_prop(props, key) {
        fields.push(ViewerField {
            label: i18n::text(cx, label_key, &[]),
            value: value.to_string(),
        });
    }
}

/// Localized EXIF rows for the viewer's Camera Settings section.
fn camera_fields(photo: &Photo, cx: &App) -> Vec<ViewerField> {
    let props = &photo.properties;
    let mut fields = Vec::new();

    let make = non_empty_prop(props, "Make");
    let model = non_empty_prop(props, "Model");
    let camera = match (make, model) {
        (Some(make), Some(model)) => Some(format!("{make} {model}")),
        (Some(value), None) | (None, Some(value)) => Some(value.to_string()),
        (None, None) => None,
    };
    if let Some(camera) = camera {
        fields.push(ViewerField {
            label: i18n::text(cx, "search.cameras", &[]),
            value: camera,
        });
    }
    if let Some(date) =
        non_empty_prop(props, "DateTimeOriginal").or_else(|| non_empty_prop(props, "DateTime"))
    {
        fields.push(ViewerField {
            label: i18n::text(cx, "media_viewer.date_taken", &[]),
            value: date.to_string(),
        });
    }
    if let Some(dimensions) = exif_dimensions(props) {
        fields.push(ViewerField {
            label: i18n::text(cx, "media_viewer.resolution", &[]),
            value: dimensions,
        });
    }
    for (key, label_key) in [
        ("PhotographicSensitivity", "media_viewer.iso"),
        ("ExposureTime", "media_viewer.shutter"),
        ("FNumber", "media_viewer.aperture"),
        ("LensModel", "media_viewer.lens"),
        ("FocalLength", "media_viewer.focal_length"),
        ("Flash", "media_viewer.flash"),
        ("WhiteBalance", "media_viewer.white_balance"),
        ("MeteringMode", "media_viewer.metering_mode"),
        ("Software", "media_viewer.software"),
    ] {
        push_prop_field(&mut fields, props, key, label_key, cx);
    }
    fields
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

/// Computes duplicate groups off the UI thread. The first pass hashes every
/// file; later passes reuse the stored hashes. Progress is throttled so a large
/// library does not flood the UI channel.
fn run_duplicate_scan(config_dir: PathBuf, tx: UnboundedSender<DuplicateMessage>) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let db = Database::new(&config_dir.display().to_string());
        let groups = duplicates::detect_all_view_progress(&db, false, true, &mut |done, total| {
            if done % 100 == 0 || done == total {
                let _ = tx.send(DuplicateMessage::Progress { done, total });
            }
        });
        let stats = duplicates::duplicate_stats_from_views(&groups);
        let library_bytes = duplicates::library_overview(&db).library_bytes;
        (groups, stats, library_bytes)
    }));
    let message = match result {
        Ok((groups, stats, library_bytes)) => DuplicateMessage::Complete(DuplicateData {
            groups,
            stats,
            library_bytes,
        }),
        Err(_) => {
            DuplicateMessage::Failed("Duplicate detection failed. Try scanning again.".to_string())
        }
    };
    let _ = tx.send(message);
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
    let total_photos = db
        .count_photos_filtered("", false, false, &PhotoFilter::default())
        .max(0) as usize;
    let date_groups = db.list_photo_date_groups("", false, false, &PhotoFilter::default());
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
    let map_points = db.get_heatmap_points();
    let map_point_count = map_points.len();
    let map_clusters = cluster_map_points(&map_points, MAP_CLUSTER_DECIMALS);
    let map_framing = frame_map_points(&map_points);
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
    let theme_preference = state
        .get("theme")
        .map(|value| ThemePreference::from_setting(value))
        .unwrap_or_default();
    let auto_scan = state
        .get("auto_scan")
        .map(|v| is_truthy(v))
        .unwrap_or(false);
    let sync_enabled = state
        .get("sync_enabled")
        .map(|v| is_truthy(v))
        .unwrap_or(false);
    let map_tiles_enabled = state
        .get("map_tiles_enabled")
        .map(|value| is_truthy(value))
        .unwrap_or(false);
    let map_tile_url = map_tile_url_from_state(&state);
    let map_tile_provider = state
        .get("map_tile_provider")
        .map(|value| MapTileProvider::from_setting(value))
        .unwrap_or_else(|| MapTileProvider::from_url(&map_tile_url));
    let map_tile_key = state.get("map_tile_key").cloned().unwrap_or_default();
    let language = language_from_state(&state);

    let mut settings = safe_settings(&state);
    if let Some(last_scan) = db.get_last_scan_time() {
        settings.insert(0, ("Last scan".to_string(), last_scan));
    }

    LibraryData {
        total_photos,
        date_groups,
        people,
        map_point_count,
        map_clusters,
        map_framing,
        map_tiles_enabled,
        map_tile_provider,
        map_tile_url,
        map_tile_key,
        albums,
        settings,
        models: model_toggles(&state),
        theme_preference,
        auto_scan,
        sync_enabled,
        language,
    }
}

fn language_from_state(state: &HashMap<String, String>) -> String {
    state
        .get("language")
        .map(|value| i18n::normalize(value))
        .unwrap_or_else(|| "en".to_string())
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

/// A blank stored URL means the keyless OpenStreetMap provider, so the Settings
/// field always shows a usable value instead of making users retype it.
fn map_tile_url_from_state(state: &HashMap<String, String>) -> String {
    state
        .get("map_tile_url")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_MAP_TILE_URL)
        .to_string()
}

/// Builds the optional online tile template. An empty setting stays on the
/// keyless OpenStreetMap tiles; anything else must be an http(s) URL.
fn map_tile_template(custom: &str, key: &str) -> Option<String> {
    let custom = custom.trim();
    let key = key.trim();
    let mut template = if custom.is_empty() {
        DEFAULT_MAP_TILE_URL.to_string()
    } else {
        custom.to_string()
    };
    // OpenStreetMap is keyless; appending a saved provider key would break it.
    let key = if template == DEFAULT_MAP_TILE_URL {
        ""
    } else {
        key
    };
    if !key.is_empty() && !template.contains("key=") {
        let separator = if template.contains('?') { '&' } else { '?' };
        template.push_str(&format!(
            "{separator}key={}",
            url::form_urlencoded::byte_serialize(key.as_bytes()).collect::<String>()
        ));
    }
    let probe = render_map_tile_url(&template, Tile { z: 0, x: 0, y: 0 });
    let parsed = Url::parse(&probe).ok()?;
    matches!(parsed.scheme(), "http" | "https").then_some(template)
}

/// Fills Ely's tile placeholders with the provider's URL conventions.
fn render_map_tile_url(template: &str, tile: Tile) -> String {
    let subdomains = ["a", "b", "c"];
    let subdomain = subdomains[(tile.x as usize + tile.y as usize) % subdomains.len()];
    template
        .replace("{s}", subdomain)
        .replace("{r}", "")
        .replace("{z}", &tile.z.to_string())
        .replace("{x}", &tile.x.to_string())
        .replace("{y}", &tile.y.to_string())
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
        .image_only()
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

/// The `YYYY-MM` month part of a `YYYY-MM-DD` bucket, or empty for unknown dates.
fn month_key(day_key: &str) -> &str {
    day_key
        .get(..7)
        .filter(|key| key.as_bytes().get(4) == Some(&b'-'))
        .unwrap_or("")
}

/// A month section title, e.g. `March 2026`.
fn month_label(day_key: &str, cx: &App) -> SharedString {
    let (year, month) = month_key(day_key).split_once('-').unwrap_or(("", ""));
    let Some(name_key) = month_name_key(month) else {
        return i18n::text(cx, "media.recent", &[]);
    };
    let name = i18n::text(cx, &format!("media.months.{name_key}"), &[]);
    if year.is_empty() {
        name
    } else {
        format!("{name} {year}").into()
    }
}

/// A day section title, e.g. `March 5, 2026`.
fn day_label(day_key: &str, cx: &App) -> SharedString {
    let (Some(year), Some(month), Some(day)) =
        (day_key.get(0..4), day_key.get(5..7), day_key.get(8..10))
    else {
        return i18n::text(cx, "media.recent", &[]);
    };
    let Some(name_key) = month_name_key(month) else {
        return i18n::text(cx, "media.recent", &[]);
    };
    let day = day.trim_start_matches('0');
    let day = if day.is_empty() { "0" } else { day };
    let month = i18n::text(cx, &format!("media.months.{name_key}"), &[]);
    i18n::text(
        cx,
        "media.day_label",
        &[("month", &month), ("day", day), ("year", year)],
    )
}

fn month_name_key(month: &str) -> Option<&'static str> {
    match month.parse::<u8>() {
        Ok(1) => Some("january"),
        Ok(2) => Some("february"),
        Ok(3) => Some("march"),
        Ok(4) => Some("april"),
        Ok(5) => Some("may"),
        Ok(6) => Some("june"),
        Ok(7) => Some("july"),
        Ok(8) => Some("august"),
        Ok(9) => Some("september"),
        Ok(10) => Some("october"),
        Ok(11) => Some("november"),
        Ok(12) => Some("december"),
        _ => None,
    }
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// A compact size label for storage counters.
fn human_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Prefer the peer's self-reported library size, falling back to the local
/// count for the host and for peers that have not announced yet.
fn effective_count(remote: i64, local: i64) -> i64 {
    if remote > 0 {
        remote
    } else {
        local
    }
}

fn device_icon(device: &DeviceInfo) -> IconName {
    icon_for_os(&device.os, device.host)
}

fn icon_for_os(os: &str, host: bool) -> IconName {
    if host {
        return IconName::Monitor;
    }
    match os {
        "android" | "ios" => IconName::Smartphone,
        "linux" | "freebsd" | "openbsd" | "netbsd" => IconName::Server,
        "" => IconName::HardDrive,
        _ => IconName::Monitor,
    }
}

fn storage_severity(percent: u8) -> Severity {
    if percent >= 90 {
        Severity::Danger
    } else if percent >= 75 {
        Severity::Warning
    } else {
        Severity::Success
    }
}

fn duplicate_kind_label(cx: &App, kind: &str) -> SharedString {
    match kind {
        "exact" => i18n::text(cx, "duplicates.kind_exact", &[]),
        "perceptual" => i18n::text(cx, "duplicates.kind_perceptual", &[]),
        _ => i18n::text(cx, "duplicates.kind_clip", &[]),
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
                    state.viewer_override = None;
                    state.device_dialog = None;
                    state.device_error = None;
                    if state.section == Section::Devices {
                        state.reload_devices(cx);
                    }
                    cx.notify();
                });
            }
        });

        let viewer = if let Some(entry) = self.viewer_override.clone() {
            Some(self.viewer_for(0, entry, 1, cx))
        } else {
            self.selected_photo.and_then(|idx| {
                let total = self.active_count();
                let entry = self.photo_at(idx)?;
                Some(self.viewer_for(idx, entry, total, cx))
            })
        };

        let shell_title: SharedString = self
            .drill_down
            .as_ref()
            .map(|dd| dd.title.clone().into())
            .unwrap_or_else(|| self.section_title(cx));
        let status = self.status_view(cx);
        let onboarding = if self.onboarding {
            Some(self.onboarding_view(cx))
        } else {
            None
        };
        let weak_keys = cx.entity().downgrade();
        // Library and Map manage their own height; wrapping them in a scroll
        // area collapses the map's flex box to zero and no tiles are requested.
        let fills_height =
            self.drill_down.is_none() && matches!(self.section, Section::Library | Section::Map);
        let shell_body = self.body(cx);
        let shell_body = if fills_height {
            shell_body
        } else {
            ScrollArea::new("siegu-shell-scroll")
                .vertical()
                .size_full()
                .child(shell_body)
                .into_any_element()
        };
        let device_dialogs = if self.device_dialog.is_some() {
            self.device_dialogs(cx)
        } else {
            Vec::new()
        };

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
                                let handled = if state.viewer_override.is_some() {
                                    state.viewer_override = None;
                                    true
                                } else if state.selected_photo.is_some() {
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
                                let len = state.active_count();
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
                                .child(shell_body),
                        ),
                    ),
            )
            .children(viewer)
            .children(onboarding)
            .children(device_dialogs)
    }
}

fn main() {
    const DESKTOP_APP_ID: &str = "io.denzyl.siegu.gpui";

    let initial_request = app_ipc::current_request();
    let (request_tx, mut request_rx) = mpsc::unbounded_channel::<Vec<String>>();
    let url_tx = request_tx.clone();
    match app_ipc::claim() {
        app_ipc::Instance::Secondary => return,
        app_ipc::Instance::Primary(Some(instance)) => {
            instance.serve(move |request| {
                let _ = request_tx.send(request);
            });
        }
        app_ipc::Instance::Primary(None) => {}
    }

    let application = gpui_platform::application();
    application.on_open_urls(move |urls| {
        let mut request = vec![std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()];
        request.extend(urls);
        let _ = url_tx.send(request);
    });
    let http_runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tile HTTP runtime"),
    );
    application
        .with_assets(ely::Assets)
        .with_http_client(Arc::new(tile_http::TileHttpClient::new(http_runtime)))
        .run(move |cx: &mut App| {
            cx.set_app_identity(DESKTOP_APP_ID, "Siegu");
            cx.on_system_notification_response(|_, cx| {
                for window in cx.windows() {
                    if let Some(handle) = window.downcast::<Siegu>() {
                        let _ = handle.update(cx, |_, window, _| window.activate_window());
                    }
                }
            });
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
            i18n::install(cx, &data.language);
            let system_appearance = cx.window_appearance();
            let theme_appearance = data.theme_preference.appearance(system_appearance);
            cx.set_window_appearance(match data.theme_preference {
                ThemePreference::System => None,
                _ => Some(theme_appearance),
            });
            Theme::set_mode_now(data.theme_preference.mode(system_appearance), cx);

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
                    let appearance =
                        cx.observe_window_appearance(window, |state: &mut Siegu, window, cx| {
                            if state.data.theme_preference == ThemePreference::System {
                                Theme::set_mode_now(
                                    ThemePreference::System.mode(window.appearance()),
                                    cx,
                                );
                            }
                        });
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    let search_input = cx.new(|cx| {
                        TextInput::new(window, cx).placeholder(i18n::text(
                            cx,
                            "search.placeholder",
                            &[],
                        ))
                    });
                    cx.subscribe(&search_input, |state: &mut Siegu, input, event, cx| {
                        if matches!(event, InputEvent::Changed | InputEvent::Submit) {
                            let query = input.read(cx).text().to_string();
                            if query != state.search_query {
                                state.search_query = query;
                                state.reload_library_photos();
                                cx.notify();
                            }
                        }
                    })
                    .detach();

                    let map_tile_url_input = cx.new(|cx| {
                        let mut input = TextInput::new(window, cx)
                            .label(i18n::text(cx, "settings.map_tile_url", &[]))
                            .placeholder(DEFAULT_MAP_TILE_URL);
                        input.set_text(data.map_tile_url.clone(), cx);
                        input
                    });
                    let map_tile_key_input = cx.new(|cx| {
                        let mut input = TextInput::new(window, cx)
                            .label(i18n::text(cx, "settings.map_tile_key", &[]))
                            .masked();
                        input.set_text(data.map_tile_key.clone(), cx);
                        input
                    });
                    cx.subscribe(
                        &map_tile_url_input,
                        |state: &mut Siegu, input, event, cx| {
                            if !matches!(event, InputEvent::Changed | InputEvent::Submit)
                                || state.data.map_tile_provider != MapTileProvider::Custom
                            {
                                return;
                            }
                            let url = input.read(cx).text().trim().to_string();
                            if url != state.data.map_tile_url {
                                state.data.map_tile_url = url.clone();
                                persist_setting("map_tile_url", &url);
                                cx.notify();
                            }
                        },
                    )
                    .detach();
                    cx.subscribe(
                        &map_tile_key_input,
                        |state: &mut Siegu, input, event, cx| {
                            if !matches!(event, InputEvent::Changed | InputEvent::Submit) {
                                return;
                            }
                            let key = input.read(cx).text().trim().to_string();
                            if key != state.data.map_tile_key {
                                state.data.map_tile_key = key.clone();
                                persist_setting("map_tile_key", &key);
                                cx.notify();
                            }
                        },
                    )
                    .detach();

                    Siegu {
                        focus,
                        section: Section::Library,
                        search_input,
                        search_query: String::new(),
                        map_tile_url_input,
                        map_tile_key_input,
                        favorites_only: false,
                        videos_only: false,
                        library_window: PhotoWindow::default(),
                        library_items: Vec::new(),
                        library_columns: 1,
                        library_list: ListState::new(0, ListAlignment::Top, px(128.0)),
                        grid_width: cx.new(|_| Pixels::ZERO),
                        drill_down: None,
                        selected_photo: None,
                        viewer_override: opened_media_request(&initial_request)
                            .and_then(|path| photo_entry_from_path(&path)),
                        viewer_metadata: None,
                        onboarding,
                        folders,
                        scan_status: ScanStatus::default(),
                        scan_running: false,
                        scan_abort: None,
                        duplicates: None,
                        duplicate_status: DuplicateStatus::default(),
                        devices: Vec::new(),
                        device_dialog: None,
                        device_error: None,
                        data,
                        _appearance: appearance,
                    }
                })
            })
            .expect("window failed to open");
            cx.activate(true);

            cx.spawn(move |cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    while let Some(request) = request_rx.recv().await {
                        let path = opened_media_request(&request);
                        cx.update(|cx| {
                            for window in cx.windows() {
                                let Some(handle) = window.downcast::<Siegu>() else {
                                    continue;
                                };
                                let _ = handle.update(cx, |state, window, cx| {
                                    window.activate_window();
                                    if let Some(path) = path.clone() {
                                        state.open_media_file(path);
                                    }
                                    cx.notify();
                                });
                                break;
                            }
                        });
                    }
                }
            })
            .detach();
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
            Section::Devices,
            Section::Duplicates,
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
    fn theme_preferences_parse_and_resolve() {
        assert_eq!(
            ThemePreference::from_setting("LIGHT"),
            ThemePreference::Light
        );
        assert_eq!(ThemePreference::from_setting("dark"), ThemePreference::Dark);
        assert_eq!(
            ThemePreference::from_setting("system"),
            ThemePreference::System
        );
        assert_eq!(
            ThemePreference::from_setting("unknown"),
            ThemePreference::System
        );
        assert_eq!(ThemePreference::default(), ThemePreference::System);
        assert_eq!(ThemePreference::System.as_setting(), "system");

        assert_eq!(
            ThemePreference::System.mode(WindowAppearance::Dark),
            Mode::Dark
        );
        assert_eq!(
            ThemePreference::Light.mode(WindowAppearance::Dark),
            Mode::Light
        );
        assert_eq!(
            ThemePreference::Dark.mode(WindowAppearance::Light),
            Mode::Dark
        );
    }

    #[test]
    fn opened_media_arg_accepts_paths_and_file_urls() {
        let path =
            std::env::temp_dir().join(format!("siegu-open-{}-{}.JPG", std::process::id(), "photo"));
        std::fs::write(&path, b"not-an-image").expect("test file");
        let path_arg = path.to_string_lossy().into_owned();
        let url_arg = Url::from_file_path(&path).expect("file url").to_string();
        assert_eq!(
            opened_media_arg(&["--flag".into(), path_arg.clone()]),
            Some(path.clone())
        );
        assert_eq!(opened_media_arg(&[url_arg]), Some(path.clone()));
        assert_eq!(opened_media_arg(&["notes.txt".into()]), None);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn windows_drive_paths_are_not_parsed_as_urls() {
        let path = "C:\\photos\\summer\\image.JPG";
        assert_eq!(path_from_open_arg(path), Some(PathBuf::from(path)));
        assert!(path_from_open_arg("file:///C:/photos/image.JPG").is_some());
        assert!(path_from_open_arg("https://example.com/image.JPG").is_none());
    }

    #[test]
    fn opened_media_request_resolves_relative_arguments() {
        let dir = std::env::temp_dir().join(format!("siegu-open-dir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("test dir");
        let file = dir.join("relative.png");
        std::fs::write(&file, b"not-an-image").expect("test file");
        let request = vec![
            dir.to_string_lossy().into_owned(),
            "relative.png".to_string(),
        ];
        assert_eq!(opened_media_request(&request), Some(file.clone()));
        std::fs::remove_file(file).ok();
        std::fs::remove_dir(dir).ok();
    }

    #[test]
    fn virtual_rows_map_to_absolute_photo_indices() {
        assert_eq!(row_bounds(0, 4, 10), (0, 4));
        assert_eq!(row_bounds(2, 4, 10), (8, 10));
        assert_eq!(row_bounds(3, 4, 10), (12, 10));
    }

    #[test]
    fn library_items_interleave_month_day_headers_and_photo_rows() {
        let groups = vec![
            PhotoDateGroup {
                key: "2026-03-20".into(),
                count: 3,
            },
            PhotoDateGroup {
                key: "2026-03-05".into(),
                count: 2,
            },
            PhotoDateGroup {
                key: "2025-12-31".into(),
                count: 3,
            },
            PhotoDateGroup {
                key: String::new(),
                count: 1,
            },
            PhotoDateGroup {
                key: "2025-11-30".into(),
                count: 0,
            },
        ];
        let items = library_items_for_groups(&groups, 2);
        assert_eq!(items.len(), 13);

        assert_eq!(items[0].kind, LibraryItemKind::Month);
        assert_eq!(
            (items[0].group, items[0].group_start, items[0].group_count),
            (0, 0, 5)
        );
        assert_eq!(items[1].kind, LibraryItemKind::Day);
        assert_eq!((items[1].group, items[1].group_count), (0, 3));
        assert_eq!(items[2].kind, LibraryItemKind::Photos);
        assert_eq!(
            (items[2].row, items[2].group_start, items[2].group_count),
            (0, 0, 3)
        );
        assert_eq!(items[4].kind, LibraryItemKind::Day);
        assert_eq!(
            (items[4].group, items[4].group_start, items[4].group_count),
            (1, 3, 2)
        );
        assert_eq!(items[6].kind, LibraryItemKind::Month);
        assert_eq!(
            (items[6].group, items[6].group_start, items[6].group_count),
            (2, 5, 3)
        );
        assert_eq!(items[7].kind, LibraryItemKind::Day);
        assert_eq!(items[10].kind, LibraryItemKind::Month);
        assert_eq!(
            (
                items[10].group,
                items[10].group_start,
                items[10].group_count
            ),
            (3, 8, 1)
        );
        assert_eq!(items[12].kind, LibraryItemKind::Photos);
        assert_eq!((items[12].row, items[12].group_start), (0, 8));
    }

    #[test]
    fn month_key_extracts_the_month_from_a_day_bucket() {
        assert_eq!(month_key("2026-03-20"), "2026-03");
        assert_eq!(month_key("2026-03"), "2026-03");
        assert_eq!(month_key(""), "");
        assert_eq!(month_key("not-a-date"), "");
    }

    #[test]
    fn month_name_keys_cover_all_twelve_months() {
        let keys: Vec<_> = (1..=12)
            .filter_map(|m| month_name_key(&m.to_string()))
            .collect();
        assert_eq!(keys.len(), 12);
        assert_eq!(keys[0], "january");
        assert_eq!(keys[11], "december");
        assert_eq!(month_name_key("13"), None);
        assert_eq!(month_name_key("03"), Some("march"));
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
    fn viewer_detects_video_files_by_extension() {
        assert!(is_video_path("/photos/clip.MP4"));
        assert!(is_video_path("clip.mov"));
        assert!(!is_video_path("/photos/sunset.jpg"));
        assert!(!is_video_path("/photos/no-extension"));
    }

    #[test]
    fn viewer_exif_dimensions_prefer_pixel_dimensions() {
        let mut props = HashMap::new();
        props.insert("ImageWidth".to_string(), "4000".to_string());
        props.insert("ImageLength".to_string(), "3000".to_string());
        assert_eq!(exif_dimensions(&props).as_deref(), Some("4000 × 3000"));
        props.insert("PixelXDimension".to_string(), "6000".to_string());
        props.insert("PixelYDimension".to_string(), "4000".to_string());
        assert_eq!(exif_dimensions(&props).as_deref(), Some("6000 × 4000"));
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
    fn human_bytes_formats_storage_counters() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1536), "1.5 KB");
        assert_eq!(human_bytes(2 * 1024 * 1024), "2.0 MB");
        assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn device_counts_prefer_remote_values() {
        assert_eq!(effective_count(120, 8), 120);
        assert_eq!(effective_count(0, 8), 8);
    }

    #[test]
    fn device_icons_follow_platform_and_host() {
        assert_eq!(icon_for_os("linux", true), IconName::Monitor);
        assert_eq!(icon_for_os("android", false), IconName::Smartphone);
        assert_eq!(icon_for_os("ios", false), IconName::Smartphone);
        assert_eq!(icon_for_os("linux", false), IconName::Server);
        assert_eq!(icon_for_os("", false), IconName::HardDrive);
        assert_eq!(icon_for_os("windows", false), IconName::Monitor);
    }

    #[test]
    fn storage_color_matches_legacy_thresholds() {
        assert_eq!(storage_severity(74), Severity::Success);
        assert_eq!(storage_severity(75), Severity::Warning);
        assert_eq!(storage_severity(89), Severity::Warning);
        assert_eq!(storage_severity(90), Severity::Danger);
    }

    #[test]
    fn language_from_state_validates_stored_values() {
        let mut state = HashMap::new();
        assert_eq!(language_from_state(&state), "en");
        state.insert("language".to_string(), "pt-BR".to_string());
        assert_eq!(language_from_state(&state), "pt");
        state.insert("language".to_string(), "unknown".to_string());
        assert_eq!(language_from_state(&state), "en");
    }

    #[test]
    fn blank_map_url_uses_the_default_provider() {
        let mut state = HashMap::new();
        assert_eq!(map_tile_url_from_state(&state), DEFAULT_MAP_TILE_URL);
        state.insert("map_tile_url".to_string(), "  ".to_string());
        assert_eq!(map_tile_url_from_state(&state), DEFAULT_MAP_TILE_URL);
        state.insert(
            "map_tile_url".to_string(),
            " https://tiles.example.com/{z}/{x}/{y}.png ".to_string(),
        );
        assert_eq!(
            map_tile_url_from_state(&state),
            "https://tiles.example.com/{z}/{x}/{y}.png"
        );
    }

    #[test]
    fn map_tile_template_is_off_until_a_safe_url_is_available() {
        assert_eq!(
            map_tile_template("", "").as_deref(),
            Some("https://tile.openstreetmap.org/{z}/{x}/{y}.png")
        );
        assert_eq!(
            map_tile_template("ftp://tiles.example.com/{z}/{x}/{y}.png", ""),
            None
        );
        assert_eq!(
            map_tile_template("", "secret key").as_deref(),
            Some(DEFAULT_MAP_TILE_URL)
        );
        assert!(map_tile_template(CARTO_LIGHT_MAP_TILE_URL, "secret key")
            .as_deref()
            .is_some_and(|url| url.contains("key=secret+key")));
    }

    #[test]
    fn default_map_provider_is_selected_without_saved_settings() {
        assert_eq!(MapTileProvider::default(), MapTileProvider::Osm);
        assert_eq!(MapTileProvider::from_url(""), MapTileProvider::Osm);
        assert_eq!(
            MapTileProvider::from_setting("unknown"),
            MapTileProvider::Osm
        );
        assert_eq!(
            MapTileProvider::Osm.label_key(),
            "settings.map_provider_osm"
        );
    }

    #[test]
    fn map_tile_rendering_replaces_provider_placeholders() {
        let url = render_map_tile_url(
            "https://{s}.example.com/{z}/{x}/{y}{r}.png?key=abc",
            Tile { z: 4, x: 3, y: 5 },
        );
        assert_eq!(url, "https://c.example.com/4/3/5.png?key=abc");
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
