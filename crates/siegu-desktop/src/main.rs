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
use siegu_core::database::{
    Album, AlbumKind, DeviceInfo, PersonMatch, PhotoDateGroup, PhotoFilter,
};
use siegu_core::devices::{
    list_devices as list_core_devices, remove_device as remove_core_device,
    rename_device as rename_core_device, storage_percent,
};
use siegu_core::duplicates::{self, DuplicateGroupView, DuplicateStats};
use siegu_core::library::do_get_search_facets;
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
        data_display::Tone,
        feedback::Banner,
        forms::{Choice, InputEvent, MultiSelect, SearchInput, Select, Switch, TextInput},
        layout::{columns_for, ScrollArea},
        maps::{LatLon, MapMarker, MapView, MapViewport, Tile, WorldMap},
        motion::ProgressBar,
        overlays::{ConfirmDialog, Dialog, PromptDialog},
        primitives::{FocusScope, Icon, IconName, Image as ElyImage, Severity},
        theme::{ActiveTheme, IconSize, Mode, Radius, TextSize, Theme},
        typography::{EllipsisTooltip, Heading, Paragraph},
    },
    gpui::{
        canvas, div, list, px, relative, size, AnyElement, App, AppContext, AsyncApp, Bounds,
        Context, Entity, FocusHandle, FontWeight, Image, ImageFormat, ImageSource,
        InteractiveElement, IntoElement, KeyDownEvent, ListAlignment, ListOffset, ListState,
        ParentElement, PathPromptOptions, Pixels, Render, SharedString, StatefulInteractiveElement,
        Styled, Subscription, SystemNotification, TitlebarOptions, WeakEntity, Window,
        WindowAppearance, WindowBounds, WindowOptions,
    },
    gpui_platform, i18n, insight_percent, CollectionCard, InfoCard, MediaViewer, PhotoCard,
    PhotoGrid, SieguAppShell, ViewerField, ViewerInsight, ViewerMetadata, ViewerPerson,
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
        self == Self::Custom
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
    /// Debug rendering of the complete `PhotoFilter`, so any facet change
    /// invalidates cached thumbnails instead of showing rows from the old filter.
    filter_key: String,
    start: usize,
    photos: Vec<PhotoEntry>,
}

impl PhotoWindow {
    fn matches(&self, filter_key: &str) -> bool {
        self.filter_key == filter_key
    }

    fn contains(&self, index: usize) -> bool {
        index >= self.start && index - self.start < self.photos.len()
    }

    fn get(&self, index: usize) -> Option<&PhotoEntry> {
        self.contains(index)
            .then(|| &self.photos[index - self.start])
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

/// Facet values offered by the library filter panel. They come from the local
/// SQLite indexes; no network or second device is involved.
#[derive(Clone, Default)]
struct SearchFacetsData {
    /// `(person id, display name)`; an empty name is an unnamed face group.
    people: Vec<(String, String)>,
    locations: Vec<String>,
    tags: Vec<String>,
    cameras: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SearchSort {
    #[default]
    Newest,
    Best,
    Random,
}

impl SearchSort {
    /// The three orders the legacy toolbar offers.
    fn values() -> [Self; 3] {
        [Self::Newest, Self::Best, Self::Random]
    }

    fn from_setting(value: &str) -> Self {
        match value {
            "best" => Self::Best,
            "random" => Self::Random,
            _ => Self::Newest,
        }
    }

    fn as_setting(self) -> &'static str {
        match self {
            Self::Newest => "newest",
            Self::Best => "best",
            Self::Random => "random",
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Self::Newest => "search.sort_newest",
            Self::Best => "search.sort_best",
            Self::Random => "search.sort_random",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SearchStorageFilter {
    #[default]
    Any,
    Stored,
    NotStored,
}

impl SearchStorageFilter {
    fn from_setting(value: &str) -> Self {
        match value {
            "stored" => Self::Stored,
            "not_stored" => Self::NotStored,
            _ => Self::Any,
        }
    }

    fn as_setting(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Stored => "stored",
            Self::NotStored => "not_stored",
        }
    }
}

/// Desktop-side search state. The query, favorites and videos switches remain
/// separate for the existing toolbar; everything here maps to `PhotoFilter`
/// fields that the core database already knows how to apply.
#[derive(Clone, Debug, Default)]
struct SearchFilterState {
    person_ids: Vec<String>,
    person_match: PersonMatch,
    person_alone: bool,
    location: Option<String>,
    tag: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    camera: Option<String>,
    aesthetics_min: Option<f64>,
    has_faces: bool,
    papers: bool,
    nsfw_only: bool,
    storage: SearchStorageFilter,
    sort: SearchSort,
}

impl SearchFilterState {
    fn to_photo_filter(&self) -> PhotoFilter {
        let (random, order_by) = match self.sort {
            SearchSort::Random => (true, None),
            sort => (false, Some(sort.as_setting().to_string())),
        };
        PhotoFilter {
            person_ids: self.person_ids.clone(),
            person_match: self.person_match,
            person_alone: self.person_alone,
            location: self.location.clone(),
            tag: self.tag.clone(),
            date_from: self.date_from.clone(),
            date_to: self.date_to.clone(),
            query: None,
            videos: None,
            favorite: false,
            has_faces: self.has_faces,
            aesthetics_min: self.aesthetics_min,
            camera: self.camera.clone(),
            papers: self.papers,
            nsfw_only: self.nsfw_only,
            stored_only: self.storage == SearchStorageFilter::Stored,
            not_stored_only: self.storage == SearchStorageFilter::NotStored,
            random,
            order_by,
            album_id: None,
        }
    }

    fn is_active(&self) -> bool {
        !self.person_ids.is_empty()
            || self.person_alone
            || self.location.is_some()
            || self.tag.is_some()
            || self.date_from.is_some()
            || self.date_to.is_some()
            || self.camera.is_some()
            || self.aesthetics_min.is_some()
            || self.has_faces
            || self.papers
            || self.nsfw_only
            || self.storage != SearchStorageFilter::Any
            || self.sort != SearchSort::Newest
    }

    fn clear_chip(&mut self, id: &str) {
        match id {
            "people" => {
                self.person_ids.clear();
                self.person_match = PersonMatch::And;
            }
            "person_alone" => self.person_alone = false,
            "location" => self.location = None,
            "tag" => self.tag = None,
            "date_range" => {
                self.date_from = None;
                self.date_to = None;
            }
            "camera" => self.camera = None,
            "quality" => self.aesthetics_min = None,
            "faces" => self.has_faces = false,
            "papers" => self.papers = false,
            "nsfw" => self.nsfw_only = false,
            "storage" => self.storage = SearchStorageFilter::Any,
            "sort" => self.sort = SearchSort::Newest,
            "all" => *self = Self::default(),
            _ => {}
        }
    }
}

/// Everything the desktop shell reads from the local library once at startup.
#[derive(Default)]
struct LibraryData {
    total_photos: usize,
    date_groups: Vec<PhotoDateGroup>,
    people: Vec<PersonEntry>,
    facets: SearchFacetsData,
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
    sidebar_collapsed: bool,
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

/// A pending person management dialog. The rename field is an entity so its
/// text and selection survive repaints while the dialog is open.
#[derive(Clone)]
enum PersonDialog {
    Rename {
        id: String,
        field: Entity<TextInput>,
    },
    Merge {
        id: String,
        target_id: Option<String>,
    },
}

/// A pending collection dialog. Text inputs are entities so typed values
/// survive repaints while the dialog is open.
#[derive(Clone)]
enum AlbumDialog {
    Create {
        field: Entity<TextInput>,
    },
    Rename {
        id: String,
        field: Entity<TextInput>,
    },
    Delete {
        id: String,
        name: String,
    },
    AddTo {
        photo_id: String,
        field: Entity<TextInput>,
    },
}

/// The desktop shell. Reads the local library and renders one section at a time.
struct Siegu {
    focus: FocusHandle,
    section: Section,
    sidebar_collapsed: bool,
    search_input: Entity<TextInput>,
    search_query: String,
    recent_searches: Vec<String>,
    date_from_input: Entity<TextInput>,
    date_to_input: Entity<TextInput>,
    camera_input: Entity<TextInput>,
    map_tile_url_input: Entity<TextInput>,
    map_tile_key_input: Entity<TextInput>,
    favorites_only: bool,
    videos_only: bool,
    search_filters: SearchFilterState,
    filters_open: bool,
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
    person_dialog: Option<PersonDialog>,
    album_dialog: Option<AlbumDialog>,
    album_error: Option<String>,
    album_notice: Option<String>,
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

    fn current_photo_filter(&self) -> PhotoFilter {
        self.search_filters.to_photo_filter()
    }

    fn filter_signature(&self) -> String {
        photo_filter_signature(
            &self.search_query,
            self.favorites_only,
            self.videos_only,
            &self.current_photo_filter(),
        )
    }

    /// Keep the text inputs in sync after a chip removal or a full reset.
    /// Their subscriptions see the same value and do not re-run the query.
    fn sync_filter_inputs(&mut self, cx: &mut Context<Self>) {
        let date_from = self.search_filters.date_from.clone().unwrap_or_default();
        let date_to = self.search_filters.date_to.clone().unwrap_or_default();
        let camera = self.search_filters.camera.clone().unwrap_or_default();
        let query = self.search_query.clone();
        self.date_from_input
            .update(cx, |input, cx| input.set_text(date_from, cx));
        self.date_to_input
            .update(cx, |input, cx| input.set_text(date_to, cx));
        self.camera_input
            .update(cx, |input, cx| input.set_text(camera, cx));
        self.search_input
            .update(cx, |input, cx| input.set_text(query, cx));
    }

    fn clear_filter(&mut self, id: &str, cx: &mut Context<Self>) {
        if id == "all" {
            self.search_query.clear();
            self.favorites_only = false;
            self.videos_only = false;
            self.search_filters = SearchFilterState::default();
        } else {
            match id {
                "query" => {
                    self.search_query.clear();
                    self.search_input
                        .update(cx, |input, cx| input.set_text(String::new(), cx));
                }
                "favorites" => self.favorites_only = false,
                "videos" => self.videos_only = false,
                other => self.search_filters.clear_chip(other),
            }
        }
        self.sync_filter_inputs(cx);
        self.reload_library_photos();
        cx.notify();
    }

    fn filter_chips(&self, cx: &App) -> Vec<(String, SharedString)> {
        let mut chips = Vec::new();
        if !self.search_query.is_empty() {
            chips.push((
                "query".to_string(),
                i18n::text(
                    cx,
                    "search.results_for",
                    &[("query", clip(&self.search_query, 28).as_str())],
                ),
            ));
        }
        if self.favorites_only {
            chips.push((
                "favorites".to_string(),
                i18n::text(cx, "search.magic.favorites", &[]),
            ));
        }
        if self.videos_only {
            chips.push((
                "videos".to_string(),
                i18n::text(cx, "search.magic.videos", &[]),
            ));
        }
        if !self.search_filters.person_ids.is_empty() {
            let label = i18n::text(cx, "search.people", &[]);
            chips.push((
                "people".to_string(),
                format!("{} ({})", label, self.search_filters.person_ids.len()).into(),
            ));
        }
        if self.search_filters.person_alone {
            chips.push((
                "person_alone".to_string(),
                i18n::text(cx, "search.person_alone", &[]),
            ));
        }
        if let Some(value) = &self.search_filters.location {
            chips.push(("location".to_string(), value.clone().into()));
        }
        if let Some(value) = &self.search_filters.tag {
            chips.push(("tag".to_string(), value.clone().into()));
        }
        if self.search_filters.date_from.is_some() || self.search_filters.date_to.is_some() {
            let from = self.search_filters.date_from.as_deref().unwrap_or("…");
            let to = self.search_filters.date_to.as_deref().unwrap_or("…");
            chips.push(("date_range".to_string(), format!("{from} – {to}").into()));
        }
        if let Some(value) = &self.search_filters.camera {
            chips.push(("camera".to_string(), value.clone().into()));
        }
        if let Some(value) = self.search_filters.aesthetics_min {
            chips.push((
                "quality".to_string(),
                format!(
                    "{} {}",
                    i18n::text(cx, "search.advanced.quality", &[]),
                    value
                )
                .into(),
            ));
        }
        if self.search_filters.has_faces {
            chips.push((
                "faces".to_string(),
                i18n::text(cx, "search.magic.faces", &[]),
            ));
        }
        if self.search_filters.papers {
            chips.push((
                "papers".to_string(),
                i18n::text(cx, "search.magic.papers", &[]),
            ));
        }
        if self.search_filters.nsfw_only {
            chips.push(("nsfw".to_string(), i18n::text(cx, "search.magic.nsfw", &[])));
        }
        if self.search_filters.storage != SearchStorageFilter::Any {
            let key = match self.search_filters.storage {
                SearchStorageFilter::Stored => "search.magic.stored",
                SearchStorageFilter::NotStored => "search.magic.not_stored",
                SearchStorageFilter::Any => "filters.all",
            };
            chips.push(("storage".to_string(), i18n::text(cx, key, &[])));
        }
        if self.search_filters.sort != SearchSort::Newest {
            let label = i18n::text(cx, self.search_filters.sort.label_key(), &[]);
            chips.push(("sort".to_string(), label));
        }
        chips
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
        let weak_album = cx.entity().downgrade();
        let photo_id = entry.photo.id.clone();
        let album_photo_id = photo_id.clone();
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
        })
        .on_add_to_album(move |window, cx| {
            if let Some(entity) = weak_album.upgrade() {
                let photo_id = album_photo_id.clone();
                entity.update(cx, |state, cx| {
                    let field = cx.new(|cx| {
                        TextInput::new(window, cx).placeholder(i18n::text(
                            cx,
                            "albums.new_album_placeholder",
                            &[],
                        ))
                    });
                    state.album_error = None;
                    state.album_notice = None;
                    state.album_dialog = Some(AlbumDialog::AddTo { photo_id, field });
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
        let filter = self.current_photo_filter();
        let filter_key = self.filter_signature();
        if self.library_window.matches(&filter_key)
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
            &filter,
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
            filter_key,
            start,
            photos,
        };
    }

    fn reset_library_grid(&mut self) {
        self.library_window = PhotoWindow {
            filter_key: self.filter_signature(),
            ..Default::default()
        };
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
            let filter = self.current_photo_filter();
            self.data.total_photos = db
                .count_photos_filtered(
                    &self.search_query,
                    self.favorites_only,
                    self.videos_only,
                    &filter,
                )
                .max(0) as usize;
            self.data.date_groups = db.list_photo_date_groups(
                &self.search_query,
                self.favorites_only,
                self.videos_only,
                &filter,
            );
        }
        self.reset_library_grid();
    }

    fn refresh_data(&mut self) {
        self.data = load_library();
        self.library_window = PhotoWindow {
            filter_key: self.filter_signature(),
            ..Default::default()
        };
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

    /// Local advanced filters. Every control writes straight into
    /// `SearchFilterState` and reloads the SQLite-backed library; facets are
    /// read from the local indexes when the library loads.
    fn advanced_filters_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let weak = cx.entity().downgrade();

        let people_choices: Vec<Choice> = self
            .data
            .facets
            .people
            .iter()
            .map(|(id, name)| {
                let label = if name.is_empty() {
                    i18n::text(cx, "search.unnamed_faces", &[])
                } else {
                    name.clone().into()
                };
                Choice::new(id.clone(), label)
            })
            .collect();
        let people_field: AnyElement = if people_choices.is_empty() {
            Paragraph::new(i18n::text(cx, "people.no_faces", &[])).into_any_element()
        } else {
            let weak_people = weak.clone();
            MultiSelect::new("filter-people", people_choices)
                .label(i18n::text(cx, "search.people", &[]))
                .placeholder(i18n::text(cx, "filters.all", &[]))
                .selected(self.search_filters.person_ids.clone())
                .on_change(move |values, _, app| {
                    if let Some(entity) = weak_people.upgrade() {
                        entity.update(app, |state, cx| {
                            state.search_filters.person_ids =
                                values.iter().map(|value| value.to_string()).collect();
                            state.reload_library_photos();
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        };

        let any_label = i18n::text(cx, "filters.all", &[]);
        let location_choices = facet_select_choices(&self.data.facets.locations, any_label.clone());
        let tag_choices = facet_select_choices(&self.data.facets.tags, any_label.clone());
        let camera_choices = facet_select_choices(&self.data.facets.cameras, any_label.clone());
        let quality_choices = [
            Choice::new("", any_label.clone()),
            Choice::new("0.5", "0.5+"),
            Choice::new("0.7", "0.7+"),
            Choice::new("0.85", "0.85+"),
        ];
        let sort_choices = SearchSort::values()
            .map(|sort| Choice::new(sort.as_setting(), i18n::text(cx, sort.label_key(), &[])));
        let date_range_field = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(Paragraph::new(i18n::text(cx, "filters.date_range", &[])))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.date_from_input.clone())
                    .child(self.date_to_input.clone()),
            );

        let weak_location = weak.clone();
        let weak_tag = weak.clone();
        let weak_camera = weak.clone();
        let weak_quality = weak.clone();
        let weak_person_match = weak.clone();
        let weak_alone = weak.clone();
        let weak_faces = weak.clone();
        let weak_papers = weak.clone();
        let weak_nsfw = weak.clone();
        let weak_storage = weak.clone();
        let weak_sort = weak.clone();
        let weak_clear_all = weak.clone();

        let chips = self.filter_chips(cx);
        let chip_buttons = chips
            .into_iter()
            .enumerate()
            .map(|(index, (id, label))| {
                let weak_chip = weak.clone();
                Button::new(("clear-filter", index), label)
                    .icon(IconName::X)
                    .variant(ButtonVariant::Subtle)
                    .on_click(move |_, _, app| {
                        if let Some(entity) = weak_chip.upgrade() {
                            let id = id.clone();
                            entity.update(app, |state, cx| state.clear_filter(&id, cx));
                        }
                    })
            })
            .collect::<Vec<_>>();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .rounded(px(12.0))
            .border_1()
            .border_color(theme.colors.border)
            .bg(theme.colors.surface)
            .child(Heading::h4(i18n::text(cx, "search.advanced.title", &[])))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(people_field)
                    .child(
                        Select::new(
                            "filter-person-match",
                            [
                                Choice::new("and", i18n::text(cx, "search.person_match_and", &[])),
                                Choice::new("or", i18n::text(cx, "search.person_match_or", &[])),
                            ],
                        )
                        .label(i18n::text(cx, "search.people", &[]))
                        .selected(match self.search_filters.person_match {
                            PersonMatch::And => "and",
                            PersonMatch::Or => "or",
                        })
                        .on_change(move |value, _, app| {
                            if let Some(entity) = weak_person_match.upgrade() {
                                let match_mode = if value.as_ref() == "or" {
                                    PersonMatch::Or
                                } else {
                                    PersonMatch::And
                                };
                                entity.update(app, |state, cx| {
                                    state.search_filters.person_match = match_mode;
                                    state.reload_library_photos();
                                    cx.notify();
                                });
                            }
                        }),
                    )
                    .child(
                        Switch::new("filter-person-alone", self.search_filters.person_alone)
                            .label(i18n::text(cx, "search.person_alone", &[]))
                            .on_change(move |on, _, app| {
                                if let Some(entity) = weak_alone.upgrade() {
                                    entity.update(app, |state, cx| {
                                        state.search_filters.person_alone = on;
                                        state.reload_library_photos();
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        Select::new("filter-location", location_choices)
                            .label(i18n::text(cx, "search.locations", &[]))
                            .selected(self.search_filters.location.clone().unwrap_or_default())
                            .on_change(move |value, _, app| {
                                if let Some(entity) = weak_location.upgrade() {
                                    let value = value.to_string();
                                    entity.update(app, |state, cx| {
                                        state.search_filters.location =
                                            (!value.is_empty()).then_some(value);
                                        state.reload_library_photos();
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        Select::new("filter-tag", tag_choices)
                            .label(i18n::text(cx, "search.tags", &[]))
                            .selected(self.search_filters.tag.clone().unwrap_or_default())
                            .on_change(move |value, _, app| {
                                if let Some(entity) = weak_tag.upgrade() {
                                    let value = value.to_string();
                                    entity.update(app, |state, cx| {
                                        state.search_filters.tag =
                                            (!value.is_empty()).then_some(value);
                                        state.reload_library_photos();
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        Select::new("filter-camera", camera_choices)
                            .label(i18n::text(cx, "search.cameras", &[]))
                            .selected(self.search_filters.camera.clone().unwrap_or_default())
                            .on_change(move |value, _, app| {
                                if let Some(entity) = weak_camera.upgrade() {
                                    let value = value.to_string();
                                    entity.update(app, |state, cx| {
                                        state.search_filters.camera =
                                            (!value.is_empty()).then_some(value);
                                        state.reload_library_photos();
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(date_range_field)
                    .child(
                        Select::new("filter-quality", quality_choices)
                            .label(i18n::text(cx, "search.advanced.quality", &[]))
                            .selected(
                                self.search_filters
                                    .aesthetics_min
                                    .map(|value| value.to_string())
                                    .unwrap_or_default(),
                            )
                            .on_change(move |value, _, app| {
                                if let Some(entity) = weak_quality.upgrade() {
                                    let value = value.parse::<f64>().ok();
                                    entity.update(app, |state, cx| {
                                        state.search_filters.aesthetics_min = value;
                                        state.reload_library_photos();
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(
                        Switch::new("filter-faces", self.search_filters.has_faces)
                            .label(i18n::text(cx, "search.magic.faces", &[]))
                            .on_change(move |on, _, app| {
                                if let Some(entity) = weak_faces.upgrade() {
                                    entity.update(app, |state, cx| {
                                        state.search_filters.has_faces = on;
                                        state.reload_library_photos();
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        Switch::new("filter-papers", self.search_filters.papers)
                            .label(i18n::text(cx, "search.magic.papers", &[]))
                            .on_change(move |on, _, app| {
                                if let Some(entity) = weak_papers.upgrade() {
                                    entity.update(app, |state, cx| {
                                        state.search_filters.papers = on;
                                        state.reload_library_photos();
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        Switch::new("filter-nsfw", self.search_filters.nsfw_only)
                            .label(i18n::text(cx, "search.magic.nsfw", &[]))
                            .on_change(move |on, _, app| {
                                if let Some(entity) = weak_nsfw.upgrade() {
                                    entity.update(app, |state, cx| {
                                        state.search_filters.nsfw_only = on;
                                        state.reload_library_photos();
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        Select::new(
                            "filter-storage",
                            [
                                Choice::new("any", i18n::text(cx, "filters.all", &[])),
                                Choice::new("stored", i18n::text(cx, "search.magic.stored", &[])),
                                Choice::new(
                                    "not_stored",
                                    i18n::text(cx, "search.magic.not_stored", &[]),
                                ),
                            ],
                        )
                        .label(i18n::text(cx, "search.magic.storage", &[]))
                        .selected(self.search_filters.storage.as_setting())
                        .on_change(move |value, _, app| {
                            if let Some(entity) = weak_storage.upgrade() {
                                let storage = SearchStorageFilter::from_setting(value.as_ref());
                                entity.update(app, |state, cx| {
                                    state.search_filters.storage = storage;
                                    state.reload_library_photos();
                                    cx.notify();
                                });
                            }
                        }),
                    )
                    .child(
                        Select::new("filter-sort", sort_choices)
                            .label(i18n::text(cx, "search.sort_label", &[]))
                            .selected(self.search_filters.sort.as_setting())
                            .on_change(move |value, _, app| {
                                if let Some(entity) = weak_sort.upgrade() {
                                    let sort = SearchSort::from_setting(value.as_ref());
                                    entity.update(app, |state, cx| {
                                        state.search_filters.sort = sort;
                                        state.reload_library_photos();
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .children(chip_buttons)
                    .child(
                        Button::new("filter-clear-all", i18n::text(cx, "search.clear_all", &[]))
                            .icon(IconName::X)
                            .variant(ButtonVariant::Ghost)
                            .on_click(move |_, _, app| {
                                if let Some(entity) = weak_clear_all.upgrade() {
                                    entity.update(app, |state, cx| state.clear_filter("all", cx));
                                }
                            }),
                    ),
            )
            .into_any_element()
    }

    fn library_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let weak_fav = cx.entity().downgrade();
        let weak_vid = cx.entity().downgrade();
        let weak_add = cx.entity().downgrade();
        let weak_scan = cx.entity().downgrade();
        let weak_filters = cx.entity().downgrade();

        let has_filters = !self.search_query.is_empty()
            || self.favorites_only
            || self.videos_only
            || self.search_filters.is_active();
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
                div().flex_1().min_w(px(220.0)).max_w(px(420.0)).child(
                    SearchInput::new("library-search", &self.search_input)
                        .history(self.recent_searches.clone()),
                ),
            )
            .child(
                Button::new(
                    "library-filters",
                    i18n::text(cx, "search.advanced.title", &[]),
                )
                .icon(IconName::Filter)
                .variant(if self.filters_open {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Secondary
                })
                .on_click(move |_, _, app| {
                    if let Some(entity) = weak_filters.upgrade() {
                        entity.update(app, |state, cx| {
                            state.filters_open = !state.filters_open;
                            cx.notify();
                        });
                    }
                }),
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

        let chips = self.filter_chips(cx);
        let chip_row = (!chips.is_empty()).then(|| {
            let mut row = div().flex().flex_wrap().items_center().gap_2();
            for (index, (id, label)) in chips.into_iter().enumerate() {
                let weak_chip = cx.entity().downgrade();
                row = row.child(
                    Button::new(("active-filter", index), label)
                        .icon(IconName::X)
                        .variant(ButtonVariant::Subtle)
                        .on_click(move |_, _, app| {
                            if let Some(entity) = weak_chip.upgrade() {
                                let id = id.clone();
                                entity.update(app, |state, cx| state.clear_filter(&id, cx));
                            }
                        }),
                );
            }
            row
        });
        let filter_panel = self.filters_open.then(|| self.advanced_filters_view(cx));

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
            .children(chip_row)
            .children(filter_panel)
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

    fn people_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.data.people.is_empty() {
            return empty_state(
                &i18n::text(cx, "people.no_faces", &[]),
                &i18n::text(cx, "people.no_faces_desc", &[]),
            );
        }
        let mut grid = PhotoGrid::new();
        for (idx, entry) in self.data.people.iter().enumerate() {
            let weak = cx.entity().downgrade();
            let person_id = entry.person.id.clone();
            let name = entry.person.name.clone();
            let unnamed = i18n::text(cx, "search.unnamed_faces", &[]);
            let display_name = person_display_name(&name, &unnamed);
            let detail = i18n::text(
                cx,
                "search.faces_count",
                &[("count", &entry.person.face_count.to_string())],
            );
            let mut card = InfoCard::new(("person", idx), IconName::Users, display_name.clone())
                .detail(detail.clone())
                .on_click(move |_, cx| {
                    if let Some(entity) = weak.upgrade() {
                        let id = person_id.clone();
                        let title = display_name.clone();
                        let subtitle = detail.clone();
                        entity.update(cx, |state, cx| {
                            state.open_drill_down(title, subtitle.to_string(), |db| {
                                db.get_photos_for_person(&id, 0, 60)
                            });
                            cx.notify();
                        });
                    }
                });
            if let Some(thumb) = &entry.thumbnail {
                card = card.source(thumb.clone());
            }

            let weak_rename = cx.entity().downgrade();
            let rename_id = entry.person.id.clone();
            let rename_name = entry.person.name.clone();
            let rename = Button::new(("person-rename", idx), i18n::text(cx, "people.rename", &[]))
                .icon(IconName::Pencil)
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, window, app| {
                    if let Some(entity) = weak_rename.upgrade() {
                        let id = rename_id.clone();
                        let name = rename_name.clone();
                        entity.update(app, |state, cx| {
                            let field = cx.new(|cx| TextInput::new(window, cx));
                            field.update(cx, |input, cx| input.set_text(name, cx));
                            state.person_dialog = Some(PersonDialog::Rename { id, field });
                            cx.notify();
                        });
                    }
                });

            let weak_merge = cx.entity().downgrade();
            let merge_id = entry.person.id.clone();
            let merge = Button::new(("person-merge", idx), i18n::text(cx, "people.merge", &[]))
                .icon(IconName::Merge)
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, _, app| {
                    if let Some(entity) = weak_merge.upgrade() {
                        let id = merge_id.clone();
                        entity.update(app, |state, cx| {
                            state.person_dialog = Some(PersonDialog::Merge {
                                id,
                                target_id: None,
                            });
                            cx.notify();
                        });
                    }
                });

            grid = grid.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(card)
                    .child(div().flex().gap_2().child(rename).child(merge)),
            );
        }
        grid.into_any_element()
    }

    fn collections_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let create_label = i18n::text(cx, "albums.new_album", &[]);

        let weak_create = cx.entity().downgrade();
        let create_click = move |_: &_, window: &mut Window, app: &mut App| {
            if let Some(entity) = weak_create.upgrade() {
                entity.update(app, |state, cx| {
                    let field = cx.new(|cx| {
                        TextInput::new(window, cx).placeholder(i18n::text(
                            cx,
                            "albums.new_album_placeholder",
                            &[],
                        ))
                    });
                    state.album_error = None;
                    state.album_notice = None;
                    state.album_dialog = Some(AlbumDialog::Create { field });
                    cx.notify();
                });
            }
        };

        let toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div().flex().flex_col().gap_1().child(
                    Paragraph::new(i18n::text(
                        cx,
                        "albums.summary",
                        &[("count", &self.data.albums.len().to_string())],
                    ))
                    .text_size(theme.text_size(TextSize::Sm))
                    .text_color(theme.colors.fg_muted),
                ),
            )
            .child(
                Button::new("album-create", create_label.clone())
                    .icon(IconName::FolderPlus)
                    .variant(ButtonVariant::Primary)
                    .on_click(create_click),
            );

        let feedback = self
            .album_error
            .as_ref()
            .map(|message| ("albums-error", Severity::Danger, message.clone(), true))
            .or_else(|| {
                self.album_notice
                    .as_ref()
                    .map(|message| ("albums-notice", Severity::Success, message.clone(), false))
            })
            .map(|(id, severity, message, is_error)| {
                let weak_dismiss = cx.entity().downgrade();
                Banner::new(id, severity, message)
                    .action(i18n::text(cx, "common.close", &[]), move |_, cx| {
                        if let Some(entity) = weak_dismiss.upgrade() {
                            entity.update(cx, |state, cx| {
                                if is_error {
                                    state.album_error = None;
                                } else {
                                    state.album_notice = None;
                                }
                                cx.notify();
                            });
                        }
                    })
                    .into_any_element()
            });

        let body = if self.data.albums.is_empty() {
            let weak_empty = cx.entity().downgrade();
            div()
                .flex()
                .flex_1()
                .min_h(px(300.0))
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(64.0))
                        .rounded_full()
                        .bg(theme.colors.hover)
                        .text_color(theme.colors.fg_muted)
                        .child(Icon::new(IconName::FolderPlus).size(IconSize::Xl)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .child(
                            Paragraph::new(i18n::text(cx, "albums.no_albums", &[]))
                                .font_weight(FontWeight::SEMIBOLD),
                        )
                        .child(
                            Paragraph::new(i18n::text(cx, "albums.no_albums_hint", &[]))
                                .text_size(theme.text_size(TextSize::Sm))
                                .text_color(theme.colors.fg_muted),
                        ),
                )
                .child(
                    Button::new("album-empty-create", create_label)
                        .icon(IconName::FolderPlus)
                        .variant(ButtonVariant::Primary)
                        .on_click(move |_, window, app| {
                            if let Some(entity) = weak_empty.upgrade() {
                                entity.update(app, |state, cx| {
                                    let field = cx.new(|cx| {
                                        TextInput::new(window, cx).placeholder(i18n::text(
                                            cx,
                                            "albums.new_album_placeholder",
                                            &[],
                                        ))
                                    });
                                    state.album_error = None;
                                    state.album_notice = None;
                                    state.album_dialog = Some(AlbumDialog::Create { field });
                                    cx.notify();
                                });
                            }
                        }),
                )
                .into_any_element()
        } else {
            let mut grid = PhotoGrid::new().gap(px(8.0));
            for idx in 0..self.data.albums.len() {
                let album = self.data.albums[idx].album.clone();
                let thumbnail = self.data.albums[idx].thumbnail.clone();
                let detail = album_item_count_label(album.item_count, cx);
                let (kind_label, kind_tone) = album_kind_badge(&album.kind, cx);

                let weak_open = cx.entity().downgrade();
                let open_id = album.id.clone();
                let open_title = album.name.clone();
                let open_subtitle = detail.to_string();
                let weak_rename = cx.entity().downgrade();
                let rename_id = album.id.clone();
                let rename_name = album.name.clone();
                let weak_delete = cx.entity().downgrade();
                let delete_id = album.id.clone();
                let delete_name = album.name.clone();

                let mut card = CollectionCard::new(("album", idx), album.name.clone())
                    .detail(detail)
                    .badge(kind_label, kind_tone)
                    .action_labels(
                        i18n::text(cx, "albums.rename_album", &[]),
                        i18n::text(cx, "albums.delete_album", &[]),
                    )
                    .on_open(move |_, cx| {
                        if let Some(entity) = weak_open.upgrade() {
                            let id = open_id.clone();
                            let title = open_title.clone();
                            let subtitle = open_subtitle.clone();
                            entity.update(cx, |state, cx| {
                                state.open_drill_down(title, subtitle, |db| {
                                    db.get_album_contents(&id, 0, 60)
                                });
                                cx.notify();
                            });
                        }
                    })
                    .on_rename(move |window, app| {
                        if let Some(entity) = weak_rename.upgrade() {
                            let id = rename_id.clone();
                            let name = rename_name.clone();
                            entity.update(app, |state, cx| {
                                let field = cx.new(|cx| {
                                    TextInput::new(window, cx).placeholder(i18n::text(
                                        cx,
                                        "albums.rename_placeholder",
                                        &[],
                                    ))
                                });
                                field.update(cx, |input, cx| input.set_text(name, cx));
                                state.album_error = None;
                                state.album_notice = None;
                                state.album_dialog = Some(AlbumDialog::Rename { id, field });
                                cx.notify();
                            });
                        }
                    })
                    .on_delete(move |_, app| {
                        if let Some(entity) = weak_delete.upgrade() {
                            let id = delete_id.clone();
                            let name = delete_name.clone();
                            entity.update(app, |state, cx| {
                                state.album_error = None;
                                state.album_notice = None;
                                state.album_dialog = Some(AlbumDialog::Delete { id, name });
                                cx.notify();
                            });
                        }
                    });
                if let Some(thumb) = thumbnail {
                    card = card.source(thumb);
                }
                grid = grid.child(card);
            }
            grid.into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .gap_3()
            .child(toolbar)
            .children(feedback)
            .child(body)
            .into_any_element()
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

    /// Shared cleanup after a person rename or merge: reload every local view
    /// that can hold the old id and drop any open drill-down built from it.
    fn finish_person_change(&mut self, cx: &mut Context<Self>) {
        self.person_dialog = None;
        self.drill_down = None;
        self.selected_photo = None;
        self.viewer_override = None;
        self.refresh_data();
        cx.notify();
    }

    fn rename_person(&mut self, id: &str, name: &str, cx: &mut Context<Self>) {
        let dir = default_config_dir();
        if dir.join("siegu.db").exists() {
            let db = Database::new(&dir.display().to_string());
            db.rename_person(id, name.trim());
            self.finish_person_change(cx);
        }
    }

    fn merge_person(&mut self, from_id: &str, to_id: &str, cx: &mut Context<Self>) {
        if from_id == to_id {
            return;
        }
        let dir = default_config_dir();
        if dir.join("siegu.db").exists() {
            let db = Database::new(&dir.display().to_string());
            db.merge_people(from_id, to_id);
            self.finish_person_change(cx);
        }
    }

    /// Apply a local collection write. `close_dialog` is false for the add
    /// picker so a photo can be filed into several collections in one pass;
    /// `reset_views` drops drill-downs that may still hold the old identity.
    fn apply_album_result(
        &mut self,
        result: Result<(), String>,
        notice: Option<String>,
        cx: &mut Context<Self>,
        close_dialog: bool,
        reset_views: bool,
    ) {
        match result {
            Ok(()) => {
                if reset_views {
                    self.drill_down = None;
                    self.selected_photo = None;
                    self.viewer_override = None;
                }
                if close_dialog {
                    self.album_dialog = None;
                }
                self.album_error = None;
                self.album_notice = notice;
                self.refresh_data();
            }
            Err(error) => {
                self.album_error = Some(error);
                self.album_notice = None;
            }
        }
        cx.notify();
    }

    fn create_album(&mut self, name: &str, cx: &mut Context<Self>) {
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let result = db.create_album(name).map(|_| ());
        let notice = Some(i18n::text(cx, "albums.created", &[]).to_string());
        self.apply_album_result(result, notice, cx, true, true);
    }

    fn rename_album(&mut self, id: &str, name: &str, cx: &mut Context<Self>) {
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let result = db.rename_album(id, name);
        let notice = Some(i18n::text(cx, "albums.renamed", &[]).to_string());
        self.apply_album_result(result, notice, cx, true, true);
    }

    fn delete_album(&mut self, id: &str, cx: &mut Context<Self>) {
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let result = db.delete_album(id);
        let notice = Some(i18n::text(cx, "albums.deleted", &[]).to_string());
        self.apply_album_result(result, notice, cx, true, true);
    }

    fn add_photo_to_album(
        &mut self,
        album_id: &str,
        album_name: &str,
        photo_id: &str,
        cx: &mut Context<Self>,
    ) {
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let result = db.add_album_items(album_id, &[photo_id.to_string()]);
        let notice =
            Some(i18n::text(cx, "albums.added_to_album", &[("album", album_name)]).to_string());
        self.apply_album_result(result, notice, cx, false, false);
    }

    fn create_album_and_add_photo(&mut self, name: &str, photo_id: &str, cx: &mut Context<Self>) {
        let dir = default_config_dir();
        if !dir.join("siegu.db").exists() {
            return;
        }
        let db = Database::new(&dir.display().to_string());
        let result = db.create_album(name).and_then(|album| {
            db.add_album_items(&album.id, &[photo_id.to_string()])?;
            Ok(())
        });
        let notice = Some(i18n::text(cx, "albums.added_to_album", &[("album", name)]).to_string());
        self.apply_album_result(result, notice, cx, true, false);
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

    fn person_dialogs(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(dialog) = self.person_dialog.clone() else {
            return Vec::new();
        };
        let weak_close = cx.entity().downgrade();
        let close = move |_: &mut Window, cx: &mut App| {
            if let Some(entity) = weak_close.upgrade() {
                entity.update(cx, |state, cx| {
                    state.person_dialog = None;
                    cx.notify();
                });
            }
        };

        match dialog {
            PersonDialog::Rename { id, field } => {
                let name = self
                    .data
                    .people
                    .iter()
                    .find(|entry| entry.person.id == id)
                    .map(|entry| entry.person.name.clone())
                    .unwrap_or_default();
                let unnamed = i18n::text(cx, "search.unnamed_faces", &[]);
                let display_name = person_display_name(&name, &unnamed);
                let weak_submit = cx.entity().downgrade();
                let submit_id = id.clone();
                vec![PromptDialog::new(
                    format!("person-rename-{id}"),
                    i18n::text(cx, "people.rename", &[]),
                    &field,
                    close,
                )
                .label(i18n::text(
                    cx,
                    "people.new_name_for",
                    &[("name", display_name.as_str())],
                ))
                .submit(i18n::text(cx, "people.update_name", &[]))
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
                        let name = text.trim().to_string();
                        entity.update(app, |state, cx| state.rename_person(&id, &name, cx));
                    }
                })
                .into_any_element()]
            }
            PersonDialog::Merge { id, target_id } => {
                let name = self
                    .data
                    .people
                    .iter()
                    .find(|entry| entry.person.id == id)
                    .map(|entry| entry.person.name.clone())
                    .unwrap_or_default();
                let unnamed = i18n::text(cx, "search.unnamed_faces", &[]);
                let display_name = person_display_name(&name, &unnamed);
                let choices = person_option_rows(
                    self.data.people.iter().map(|entry| &entry.person),
                    &id,
                    &unnamed,
                )
                .into_iter()
                .map(|(value, label)| Choice::new(value, label))
                .collect::<Vec<_>>();
                let weak_target = cx.entity().downgrade();
                let select = Select::new("person-merge-target", choices)
                    .label(i18n::text(cx, "people.target_profile", &[]))
                    .selected(target_id.clone().unwrap_or_default())
                    .on_change(move |value, _, app| {
                        let target = (!value.is_empty()).then(|| value.to_string());
                        if let Some(entity) = weak_target.upgrade() {
                            entity.update(app, |state, cx| {
                                if let Some(PersonDialog::Merge { target_id, .. }) =
                                    &mut state.person_dialog
                                {
                                    *target_id = target;
                                    cx.notify();
                                }
                            });
                        }
                    });

                let weak_confirm = cx.entity().downgrade();
                let from_id = id.clone();
                let confirm_target = target_id.clone();
                let cancel_label = i18n::text(cx, "common.cancel", &[]);
                let confirm_label = i18n::text(cx, "people.confirm_merge", &[]);
                vec![Dialog::new(
                    format!("person-merge-{id}"),
                    i18n::text(cx, "people.merge", &[]),
                    close,
                )
                .detail(i18n::text(
                    cx,
                    "people.merge_desc",
                    &[("name", display_name.as_str())],
                ))
                .child(select)
                .action(move |close| {
                    let label = cancel_label.clone();
                    Button::new("person-merge-cancel", label)
                        .variant(ButtonVariant::Ghost)
                        .on_click(move |_, window, app| close(window, app))
                })
                .action(move |close| {
                    Button::new("person-merge-confirm", confirm_label)
                        .disabled(confirm_target.is_none())
                        .on_click(move |_, window, app| {
                            close(window, app);
                            let Some(to_id) = confirm_target.clone() else {
                                return;
                            };
                            if let Some(entity) = weak_confirm.upgrade() {
                                let from_id = from_id.clone();
                                entity.update(app, |state, cx| {
                                    state.merge_person(&from_id, &to_id, cx);
                                });
                            }
                        })
                })
                .into_any_element()]
            }
        }
    }

    fn album_dialogs(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme();
        let Some(dialog) = self.album_dialog.clone() else {
            return Vec::new();
        };
        let weak_close = cx.entity().downgrade();
        let close = move |_: &mut Window, cx: &mut App| {
            if let Some(entity) = weak_close.upgrade() {
                entity.update(cx, |state, cx| {
                    state.album_dialog = None;
                    state.album_error = None;
                    state.album_notice = None;
                    cx.notify();
                });
            }
        };

        match dialog {
            AlbumDialog::Create { field } => {
                let weak_submit = cx.entity().downgrade();
                let name_required = i18n::text(cx, "albums.name_required", &[]);
                vec![PromptDialog::new(
                    "album-create",
                    i18n::text(cx, "albums.new_album", &[]),
                    &field,
                    close,
                )
                .label(i18n::text(cx, "albums.create_hint", &[]))
                .submit(i18n::text(cx, "common.create", &[]))
                .check(move |text| {
                    if text.trim().is_empty() {
                        Err(name_required.clone())
                    } else {
                        Ok(())
                    }
                })
                .on_submit(move |text, _, app| {
                    if let Some(entity) = weak_submit.upgrade() {
                        let name = text.trim().to_string();
                        entity.update(app, |state, cx| state.create_album(&name, cx));
                    }
                })
                .into_any_element()]
            }
            AlbumDialog::Rename { id, field } => {
                let weak_submit = cx.entity().downgrade();
                let submit_id = id.clone();
                let name_required = i18n::text(cx, "albums.name_required", &[]);
                vec![PromptDialog::new(
                    format!("album-rename-{id}"),
                    i18n::text(cx, "albums.rename_album", &[]),
                    &field,
                    close,
                )
                .label(i18n::text(cx, "albums.rename_hint", &[]))
                .submit(i18n::text(cx, "common.save", &[]))
                .check(move |text| {
                    if text.trim().is_empty() {
                        Err(name_required.clone())
                    } else {
                        Ok(())
                    }
                })
                .on_submit(move |text, _, app| {
                    if let Some(entity) = weak_submit.upgrade() {
                        let id = submit_id.clone();
                        let name = text.trim().to_string();
                        entity.update(app, |state, cx| state.rename_album(&id, &name, cx));
                    }
                })
                .into_any_element()]
            }
            AlbumDialog::Delete { id, name } => {
                let weak_confirm = cx.entity().downgrade();
                let confirm_id = id.clone();
                let message = i18n::text(
                    cx,
                    "albums.delete_confirm_named",
                    &[("name", name.as_str())],
                );
                vec![ConfirmDialog::new(
                    format!("album-delete-{id}"),
                    i18n::text(cx, "albums.delete_confirm_title", &[]),
                    message,
                    close,
                )
                .confirm(i18n::text(cx, "common.delete", &[]))
                .destructive()
                .on_confirm(move |_, app| {
                    if let Some(entity) = weak_confirm.upgrade() {
                        let id = confirm_id.clone();
                        entity.update(app, |state, cx| state.delete_album(&id, cx));
                    }
                })
                .into_any_element()]
            }
            AlbumDialog::AddTo { photo_id, field } => {
                let name_ready = !field.read(cx).text().trim().is_empty();
                let weak_create = cx.entity().downgrade();
                let create_field = field.clone();
                let create_photo_id = photo_id.clone();
                let create = Button::new("album-add-create", i18n::text(cx, "common.create", &[]))
                    .icon(IconName::FolderPlus)
                    .variant(ButtonVariant::Primary)
                    .disabled(!name_ready)
                    .on_click(move |_, _, app| {
                        let name = create_field.read(app).text().trim().to_string();
                        if name.is_empty() {
                            return;
                        }
                        if let Some(entity) = weak_create.upgrade() {
                            let photo_id = create_photo_id.clone();
                            entity.update(app, |state, cx| {
                                state.create_album_and_add_photo(&name, &photo_id, cx);
                            });
                        }
                    });

                let manual: Vec<_> = self
                    .data
                    .albums
                    .iter()
                    .filter(|entry| entry.album.kind == AlbumKind::Manual)
                    .collect();
                let mut album_list = div().flex().flex_col().gap_1();
                if manual.is_empty() {
                    album_list = album_list.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .min_h(px(88.0))
                            .child(
                                Paragraph::new(i18n::text(cx, "albums.no_albums_yet", &[]))
                                    .text_size(theme.text_size(TextSize::Sm))
                                    .text_color(theme.colors.fg_muted),
                            ),
                    );
                } else {
                    for (idx, entry) in manual.into_iter().enumerate() {
                        let weak_add = cx.entity().downgrade();
                        let add_id = entry.album.id.clone();
                        let add_name = entry.album.name.clone();
                        let add_photo_id = photo_id.clone();
                        let count = album_item_count_label(entry.album.item_count, cx);
                        album_list = album_list.child(
                            div()
                                .id(("album-add-item", idx))
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .px_3()
                                .py_2()
                                .rounded(theme.radius(Radius::Md))
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.colors.hover))
                                .on_click(move |_, _, app| {
                                    if let Some(entity) = weak_add.upgrade() {
                                        let album_id = add_id.clone();
                                        let album_name = add_name.clone();
                                        let photo_id = add_photo_id.clone();
                                        entity.update(app, |state, cx| {
                                            state.add_photo_to_album(
                                                &album_id,
                                                &album_name,
                                                &photo_id,
                                                cx,
                                            );
                                        });
                                    }
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_none()
                                                .items_center()
                                                .justify_center()
                                                .size(px(34.0))
                                                .rounded(theme.radius(Radius::Md))
                                                .bg(theme.colors.hover)
                                                .text_color(theme.colors.fg_muted)
                                                .child(
                                                    Icon::new(IconName::Layers).size(IconSize::Sm),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .min_w_0()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .min_w_0()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme.colors.fg)
                                                        .child(EllipsisTooltip::new(
                                                            ("album-add-name", idx),
                                                            entry.album.name.clone(),
                                                        )),
                                                )
                                                .child(
                                                    Paragraph::new(count)
                                                        .text_size(theme.text_size(TextSize::Sm))
                                                        .text_color(theme.colors.fg_muted),
                                                ),
                                        ),
                                )
                                .child(
                                    Icon::new(IconName::ArrowUpRight)
                                        .size(IconSize::Sm)
                                        .color(theme.colors.fg_subtle),
                                ),
                        );
                    }
                }

                let feedback = self
                    .album_error
                    .as_ref()
                    .map(|message| ("album-add-error", Severity::Danger, message.clone()))
                    .or_else(|| {
                        self.album_notice
                            .as_ref()
                            .map(|message| ("album-add-notice", Severity::Success, message.clone()))
                    })
                    .map(|(id, severity, message)| {
                        let weak_dismiss = cx.entity().downgrade();
                        Banner::new(id, severity, message)
                            .action(i18n::text(cx, "common.close", &[]), move |_, cx| {
                                if let Some(entity) = weak_dismiss.upgrade() {
                                    entity.update(cx, |state, cx| {
                                        state.album_error = None;
                                        state.album_notice = None;
                                        cx.notify();
                                    });
                                }
                            })
                            .into_any_element()
                    });
                let close_label = i18n::text(cx, "common.close", &[]);
                vec![Dialog::new(
                    "album-add-to",
                    i18n::text(cx, "media_viewer.add_to_album", &[]),
                    close,
                )
                .detail(i18n::text(cx, "albums.add_hint", &[]))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            Paragraph::new(i18n::text(cx, "albums.create_new", &[]))
                                .font_weight(FontWeight::SEMIBOLD),
                        )
                        .child(field.clone())
                        .child(div().flex().justify_end().child(create))
                        .child(div().h(px(1.0)).w_full().bg(theme.colors.border))
                        .child(
                            Paragraph::new(i18n::text(cx, "albums.choose_existing", &[]))
                                .font_weight(FontWeight::SEMIBOLD),
                        )
                        .children(feedback)
                        .child(
                            ScrollArea::new("album-add-list")
                                .vertical()
                                .max_h(px(260.0))
                                .child(album_list),
                        ),
                )
                .action(move |close| {
                    let label = close_label.clone();
                    Button::new("album-add-close", label)
                        .variant(ButtonVariant::Primary)
                        .on_click(move |_, window, app| close(window, app))
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
    let facet_rows = do_get_search_facets(&db);
    let facets = SearchFacetsData {
        people: facet_rows
            .people
            .into_iter()
            .map(|person| (person.id, person.name.unwrap_or_default()))
            .chain(
                facet_rows
                    .unnamed_faces
                    .into_iter()
                    .map(|group| (group.id, String::new())),
            )
            .collect(),
        locations: facet_rows.locations.into_iter().map(|l| l.name).collect(),
        tags: facet_rows.tags.into_iter().map(|tag| tag.name).collect(),
        cameras: facet_rows
            .cameras
            .into_iter()
            .map(|camera| camera.name)
            .collect(),
    };
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
    let sidebar_collapsed = state
        .get("sidebar_collapsed")
        .map(|value| is_truthy(value))
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
        facets,
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
        sidebar_collapsed,
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
    // OpenStreetMap and CARTO are keyless; only a custom provider uses the
    // saved API key, and appending one to a built-in URL breaks the request.
    let key = if MapTileProvider::from_url(custom).accepts_key() {
        key
    } else {
        ""
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

/// Cache key for the virtual grid. The free-text query and the two quick
/// switches are separate arguments to SQLite, so they belong here too.
fn photo_filter_signature(
    query: &str,
    favorites_only: bool,
    videos_only: bool,
    filter: &PhotoFilter,
) -> String {
    format!("{query}|{favorites_only}|{videos_only}|{filter:?}")
}

/// Localized display name for a person, falling back to the unnamed-group
/// label for face clusters the user has not named yet.
fn person_display_name(name: &str, unnamed: &str) -> String {
    if name.trim().is_empty() {
        unnamed.to_string()
    } else {
        name.to_string()
    }
}

/// Albums that accept explicit membership. Smart and trip albums compute their
/// contents from rules, so they are hidden from the add-to-collection picker.
#[cfg(test)]
fn manual_album_rows<'a>(albums: impl IntoIterator<Item = &'a Album>) -> Vec<(String, String)> {
    albums
        .into_iter()
        .filter(|album| album.kind == AlbumKind::Manual)
        .map(|album| (album.id.clone(), album.name.clone()))
        .collect()
}

/// The localized kind marker shown on a collection card.
fn album_kind_badge(kind: &AlbumKind, cx: &App) -> (SharedString, Tone) {
    match kind {
        AlbumKind::Manual => (i18n::text(cx, "albums.kind_manual", &[]), Tone::Neutral),
        AlbumKind::Smart => (i18n::text(cx, "albums.kind_smart", &[]), Tone::Accent),
        AlbumKind::Trip => (i18n::text(cx, "albums.kind_trip", &[]), Tone::Info),
    }
}

/// Singular and plural item counts are separate locale strings so cards never
/// read "1 items".
fn album_item_count_label(count: i64, cx: &App) -> SharedString {
    let key = if count == 1 {
        "albums.item_count_one"
    } else {
        "albums.item_count_other"
    };
    i18n::text(cx, key, &[("count", &count.to_string())])
}

/// Candidate rows for the person merge dialog: every person except the one
/// being replaced, with unnamed groups given a localized display name.
fn person_option_rows<'a>(
    people: impl IntoIterator<Item = &'a PersonWithFace>,
    from_id: &str,
    unnamed: &str,
) -> Vec<(String, String)> {
    people
        .into_iter()
        .filter(|person| person.id != from_id)
        .map(|person| {
            let name = if person.name.trim().is_empty() {
                unnamed.to_string()
            } else {
                person.name.clone()
            };
            (person.id.clone(), name)
        })
        .collect()
}

/// Turn a local facet list into a select with an explicit "All" option.
fn facet_select_choices(values: &[String], all_label: SharedString) -> Vec<Choice> {
    let mut choices = vec![Choice::new("", all_label)];
    choices.extend(
        values
            .iter()
            .filter(|value| !value.trim().is_empty())
            .map(|value| Choice::new(value.clone(), value.clone())),
    );
    choices
}

/// Text inputs hold ISO dates; the end of a day needs the last second so a
/// `YYYY-MM-DD` range includes photos taken that day.
fn date_filter_value(raw: &str, end_of_day: bool) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if end_of_day && value.len() == 10 && value.bytes().filter(|b| *b == b'-').count() == 2 {
        Some(format!("{value} 23:59:59"))
    } else {
        Some(value.to_string())
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
                    state.person_dialog = None;
                    state.album_dialog = None;
                    state.album_error = None;
                    state.album_notice = None;
                    if state.section == Section::Devices {
                        state.reload_devices(cx);
                    }
                    cx.notify();
                });
            }
        });

        let weak_menu = cx.entity().downgrade();
        let on_toggle_menu: Rc<dyn Fn(&mut Window, &mut App)> = Rc::new(move |_, app| {
            if let Some(entity) = weak_menu.upgrade() {
                entity.update(app, |state, cx| {
                    state.sidebar_collapsed = !state.sidebar_collapsed;
                    persist_setting(
                        "sidebar_collapsed",
                        if state.sidebar_collapsed {
                            "true"
                        } else {
                            "false"
                        },
                    );
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
        let person_dialogs = if self.person_dialog.is_some() {
            self.person_dialogs(cx)
        } else {
            Vec::new()
        };
        let album_dialogs = if self.album_dialog.is_some() {
            self.album_dialogs(cx)
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
                                .collapsed(self.sidebar_collapsed)
                                .on_nav(on_nav)
                                .on_toggle_menu(on_toggle_menu)
                                .child(shell_body),
                        ),
                    ),
            )
            .children(viewer)
            .children(onboarding)
            .children(device_dialogs)
            .children(person_dialogs)
            .children(album_dialogs)
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
                            if matches!(event, InputEvent::Submit) {
                                let recent = query.trim();
                                if !recent.is_empty() {
                                    state.recent_searches.retain(|item| item != recent);
                                    state.recent_searches.insert(0, recent.to_string());
                                    state.recent_searches.truncate(8);
                                }
                            }
                            if query != state.search_query {
                                state.search_query = query;
                                state.reload_library_photos();
                                cx.notify();
                            }
                        }
                    })
                    .detach();

                    let date_from_input =
                        cx.new(|cx| TextInput::new(window, cx).placeholder("YYYY-MM-DD"));
                    cx.subscribe(&date_from_input, |state: &mut Siegu, input, event, cx| {
                        if !matches!(event, InputEvent::Changed | InputEvent::Submit) {
                            return;
                        }
                        let value = date_filter_value(&input.read(cx).text(), false);
                        if value != state.search_filters.date_from {
                            state.search_filters.date_from = value;
                            state.reload_library_photos();
                            cx.notify();
                        }
                    })
                    .detach();
                    let date_to_input =
                        cx.new(|cx| TextInput::new(window, cx).placeholder("YYYY-MM-DD"));
                    cx.subscribe(&date_to_input, |state: &mut Siegu, input, event, cx| {
                        if !matches!(event, InputEvent::Changed | InputEvent::Submit) {
                            return;
                        }
                        let value = date_filter_value(&input.read(cx).text(), true);
                        if value != state.search_filters.date_to {
                            state.search_filters.date_to = value;
                            state.reload_library_photos();
                            cx.notify();
                        }
                    })
                    .detach();
                    let camera_input = cx.new(|cx| {
                        TextInput::new(window, cx)
                            .label(i18n::text(cx, "search.cameras", &[]))
                            .placeholder("Canon")
                    });
                    cx.subscribe(&camera_input, |state: &mut Siegu, input, event, cx| {
                        if !matches!(event, InputEvent::Changed | InputEvent::Submit) {
                            return;
                        }
                        let value = {
                            let value = input.read(cx).text().trim().to_string();
                            (!value.is_empty()).then_some(value)
                        };
                        if value != state.search_filters.camera {
                            state.search_filters.camera = value;
                            state.reload_library_photos();
                            cx.notify();
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
                        sidebar_collapsed: data.sidebar_collapsed,
                        search_input,
                        search_query: String::new(),
                        recent_searches: Vec::new(),
                        date_from_input,
                        date_to_input,
                        camera_input,
                        map_tile_url_input,
                        map_tile_key_input,
                        favorites_only: false,
                        videos_only: false,
                        search_filters: SearchFilterState::default(),
                        filters_open: false,
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
                        person_dialog: None,
                        album_dialog: None,
                        album_error: None,
                        album_notice: None,
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
        assert_eq!(
            map_tile_template(CARTO_LIGHT_MAP_TILE_URL, "secret key").as_deref(),
            Some(CARTO_LIGHT_MAP_TILE_URL)
        );
        assert!(
            map_tile_template("https://tiles.example.com/{z}/{x}/{y}.png", "secret key")
                .as_deref()
                .is_some_and(|url| url.contains("key=secret+key"))
        );
    }

    #[test]
    fn only_custom_tile_provider_accepts_a_key() {
        assert!(!MapTileProvider::Osm.accepts_key());
        assert!(!MapTileProvider::CartoLight.accepts_key());
        assert!(!MapTileProvider::CartoVoyager.accepts_key());
        assert!(MapTileProvider::Custom.accepts_key());
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

    #[test]
    fn search_filters_map_to_core_photo_filter() {
        let mut filters = SearchFilterState::default();
        filters.person_ids = vec!["p1".into(), "p2".into()];
        filters.person_match = PersonMatch::Or;
        filters.person_alone = true;
        filters.location = Some("Paris".into());
        filters.tag = Some("beach".into());
        filters.date_from = Some("2026-01-01".into());
        filters.date_to = Some("2026-01-31 23:59:59".into());
        filters.aesthetics_min = Some(0.7);
        filters.camera = Some("Canon".into());
        filters.has_faces = true;
        filters.papers = true;
        filters.nsfw_only = true;
        filters.storage = SearchStorageFilter::NotStored;
        filters.sort = SearchSort::Best;

        let filter = filters.to_photo_filter();
        assert_eq!(filter.person_ids, vec!["p1".to_string(), "p2".to_string()]);
        assert_eq!(filter.person_match, PersonMatch::Or);
        assert!(filter.person_alone);
        assert_eq!(filter.location.as_deref(), Some("Paris"));
        assert_eq!(filter.tag.as_deref(), Some("beach"));
        assert_eq!(filter.date_from.as_deref(), Some("2026-01-01"));
        assert_eq!(filter.date_to.as_deref(), Some("2026-01-31 23:59:59"));
        assert_eq!(filter.aesthetics_min, Some(0.7));
        assert_eq!(filter.camera.as_deref(), Some("Canon"));
        assert!(filter.has_faces);
        assert!(filter.papers);
        assert!(filter.nsfw_only);
        assert!(!filter.stored_only);
        assert!(filter.not_stored_only);
        assert!(!filter.random);
        assert_eq!(filter.order_by.as_deref(), Some("best"));
        assert!(filters.is_active());
    }

    #[test]
    fn random_sort_uses_the_core_random_flag() {
        let mut filters = SearchFilterState::default();
        assert_eq!(
            filters.to_photo_filter().order_by.as_deref(),
            Some("newest")
        );

        filters.sort = SearchSort::Random;
        let filter = filters.to_photo_filter();
        assert!(filter.random);
        assert_eq!(filter.order_by, None);
    }

    #[test]
    fn cache_key_changes_with_query_switches_and_facets() {
        let base = PhotoFilter::default();
        let empty = photo_filter_signature("", false, false, &base);
        let query = photo_filter_signature("dog", false, false, &base);
        let favorites = photo_filter_signature("", true, false, &base);
        let videos = photo_filter_signature("", false, true, &base);
        let mut advanced = base.clone();
        advanced.has_faces = true;
        let advanced_key = photo_filter_signature("", false, false, &advanced);

        assert_ne!(empty, query);
        assert_ne!(empty, favorites);
        assert_ne!(empty, videos);
        assert_ne!(empty, advanced_key);

        let window = PhotoWindow {
            filter_key: empty.clone(),
            ..Default::default()
        };
        assert!(window.matches(&empty));
        assert!(!window.matches(&query));
    }

    #[test]
    fn date_filter_value_is_inclusive_for_a_plain_end_date() {
        assert_eq!(
            date_filter_value(" 2026-03-20 ", false).as_deref(),
            Some("2026-03-20")
        );
        assert_eq!(
            date_filter_value("2026-03-20", true).as_deref(),
            Some("2026-03-20 23:59:59")
        );
        assert_eq!(
            date_filter_value("2026-03-20 10:00", true).as_deref(),
            Some("2026-03-20 10:00")
        );
        assert_eq!(date_filter_value("  ", true), None);
    }

    #[test]
    fn clearing_a_chip_resets_its_related_values() {
        let mut filters = SearchFilterState::default();
        filters.person_ids = vec!["p1".into()];
        filters.person_match = PersonMatch::Or;
        filters.person_alone = true;
        filters.date_from = Some("2026-01-01".into());
        filters.date_to = Some("2026-01-31".into());
        filters.sort = SearchSort::Best;

        filters.clear_chip("people");
        assert!(filters.person_ids.is_empty());
        assert_eq!(filters.person_match, PersonMatch::And);
        filters.clear_chip("person_alone");
        filters.clear_chip("date_range");
        assert_eq!(filters.date_from, None);
        assert_eq!(filters.date_to, None);
        filters.clear_chip("sort");
        assert_eq!(filters.sort, SearchSort::Newest);
        assert!(!filters.is_active());
    }

    #[test]
    fn filtered_date_groups_keep_month_and_day_headers() {
        let groups = vec![
            PhotoDateGroup {
                key: "2026-04-02".into(),
                count: 2,
            },
            PhotoDateGroup {
                key: "2026-03-31".into(),
                count: 1,
            },
            PhotoDateGroup {
                key: "2025-12-24".into(),
                count: 3,
            },
        ];
        let items = library_items_for_groups(&groups, 2);
        let kinds: Vec<_> = items.iter().map(|item| item.kind).collect();
        assert_eq!(
            kinds,
            vec![
                LibraryItemKind::Month,
                LibraryItemKind::Day,
                LibraryItemKind::Photos,
                LibraryItemKind::Month,
                LibraryItemKind::Day,
                LibraryItemKind::Photos,
                LibraryItemKind::Month,
                LibraryItemKind::Day,
                LibraryItemKind::Photos,
                LibraryItemKind::Photos,
            ]
        );
        assert_eq!((items[0].group_start, items[0].group_count), (0, 2));
        assert_eq!((items[3].group_start, items[3].group_count), (2, 1));
        assert_eq!((items[6].group_start, items[6].group_count), (3, 3));
    }

    fn test_person(id: &str, name: &str) -> PersonWithFace {
        PersonWithFace {
            id: id.to_string(),
            name: name.to_string(),
            representative_crop: None,
            representative_face_id: None,
            encoded: None,
            embedding: None,
            face_count: 1,
        }
    }

    #[test]
    fn person_display_name_falls_back_for_unnamed_groups() {
        assert_eq!(person_display_name("Ada", "Other faces"), "Ada");
        assert_eq!(person_display_name("  ", "Other faces"), "Other faces");
    }

    #[test]
    fn merge_targets_exclude_the_source_and_label_unnamed_people() {
        let people = vec![
            test_person("a", ""),
            test_person("b", "Ada"),
            test_person("c", "Grace"),
        ];

        assert_eq!(
            person_option_rows(&people, "a", "Other faces"),
            vec![
                ("b".to_string(), "Ada".to_string()),
                ("c".to_string(), "Grace".to_string()),
            ]
        );
        assert_eq!(
            person_option_rows(&people, "b", "Other faces"),
            vec![
                ("a".to_string(), "Other faces".to_string()),
                ("c".to_string(), "Grace".to_string()),
            ]
        );
    }

    fn test_album(id: &str, name: &str, kind: AlbumKind) -> Album {
        Album {
            id: id.to_string(),
            name: name.to_string(),
            created_at: String::new(),
            cover_photo_id: None,
            sort_order: 0,
            item_count: 0,
            kind,
            rule: None,
            updated_at: None,
            share_count: 0,
        }
    }

    #[test]
    fn add_to_collection_only_offers_manual_albums() {
        let albums = vec![
            test_album("a", "Holiday", AlbumKind::Manual),
            test_album("b", "Best shots", AlbumKind::Smart),
            test_album("c", "Trip to Rome", AlbumKind::Trip),
            test_album("d", "Family", AlbumKind::Manual),
        ];

        assert_eq!(
            manual_album_rows(albums.iter()),
            vec![
                ("a".to_string(), "Holiday".to_string()),
                ("d".to_string(), "Family".to_string()),
            ]
        );
    }
}
