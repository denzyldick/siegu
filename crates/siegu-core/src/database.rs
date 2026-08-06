use std::collections::HashMap;
use std::fs;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Autocomplete suggestion returned by search queries.
#[derive(Debug, Clone, Serialize)]
pub struct SearchSuggestion {
    pub title: String,
    #[serde(rename = "type")]
    pub suggestion_type: String,
}

/// A named person shown in the search dropdown, with the number of photos
/// that contain them and a representative face thumbnail.
#[derive(Debug, Clone, Serialize)]
pub struct SearchPerson {
    pub id: String,
    pub name: String,
    pub representative_crop: Option<String>,
    pub encoded: Option<String>,
    pub photo_count: i64,
}

/// Library-wide counts shown in the search dropdown footer.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchStats {
    pub photos: i64,
    pub videos: i64,
    pub favorites: i64,
    pub ocr_photos: i64,
    pub faces: i64,
    pub named_people: i64,
    /// Photos containing at least one detected face.
    pub face_photos: i64,
    /// Photos classified as NSFW (nsfw score >= 0.8).
    pub nsfw_photos: i64,
}

/// A lightweight photo used by the discovery rails (best shots, favorites,
/// recent). Carries just enough to render a thumbnail and a badge.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchPhotoTile {
    pub id: String,
    pub location: String,
    pub encoded: String,
    pub created: String,
    pub aesthetics_score: Option<f64>,
    pub favorite: bool,
}

/// A resolved location with a representative photo thumbnail so the discovery
/// rail can show a real preview instead of a bare label.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LocationGroup {
    pub name: String,
    pub count: i64,
    pub photo_location: Option<String>,
    pub encoded: Option<String>,
}

/// Photo/video counts for a single calendar day (`YYYY-MM-DD`), powering the
/// date-range picker in the search dropdown.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DayCount {
    pub date: String,
    pub photos: i64,
    pub videos: i64,
}

/// CLIP zero-shot classes that correspond to documents, receipts and
/// screenshots, powering the "Papers & screenshots" section and filter.
pub const PAPER_CLASSES: &[&str] = &[
    "a passport",
    "a driver's license",
    "an id card",
    "a document",
    "a receipt",
    "a screenshot",
    "a meme",
    "a text message",
];

/// Builds a SQL-safe `IN (...)` clause from the paper class list, escaping
/// embedded quotes so labels like "a driver's license" stay valid literals.
fn paper_class_in_clause() -> String {
    PAPER_CLASSES
        .iter()
        .map(|c| format!("'{}'", c.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",")
}

/// How multiple person filters are combined.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PersonMatch {
    /// Photos must contain all selected people together.
    #[default]
    And,
    /// Photos must contain at least one of the selected people.
    Or,
}

/// Optional facet filters combined with AND against the media list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PhotoFilter {
    /// Photos must contain these people, combined per `person_match`.
    #[serde(default)]
    pub person_ids: Vec<String>,
    /// Combine multiple selected people with AND (together) or OR (any).
    #[serde(default)]
    pub person_match: PersonMatch,
    /// Restrict to photos whose only detected faces belong to the selected
    /// people (e.g. "X alone in the frame").
    #[serde(default)]
    pub person_alone: bool,
    pub location: Option<String>,
    pub tag: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    /// Free-text search applied to captions, objects, OCR, people, etc.
    pub query: Option<String>,
    /// Only photos that are videos.
    pub videos: Option<bool>,
    /// Only photos marked as favorites.
    pub favorite: bool,
    /// Only photos containing at least one detected face.
    pub has_faces: bool,
    /// Only photos whose aesthetics score is at least this value.
    pub aesthetics_min: Option<f64>,
    /// Only photos shot with a camera whose Make or Model contains this value.
    pub camera: Option<String>,
    /// Only photos whose object classes are documents/screenshots (CLIP).
    pub papers: bool,
    /// Only show photos classified as NSFW (nsfw score >= 0.8).
    pub nsfw_only: bool,
    /// Random order instead of newest-first (used by "Surprise me").
    pub random: bool,
    /// Sort order: "newest" (default), "oldest", "best" (aesthetics desc), "random".
    pub order_by: Option<String>,
    /// Only photos that belong to this album.
    pub album_id: Option<String>,
}

/// How a non-manual album computes its membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlbumKind {
    /// A plain user-managed album with explicit album_item rows.
    Manual,
    /// A rule-based album whose contents are computed from a stored `PhotoFilter`.
    Smart,
    /// An automatically detected trip (date-bounded collection of photos).
    Trip,
}

impl AlbumKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Smart => "smart",
            Self::Trip => "trip",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "smart" => Self::Smart,
            "trip" => Self::Trip,
            _ => Self::Manual,
        }
    }
}

/// A user-created collection of photos. Albums are local and free for
/// everyone; sharing them is a separate (paid) feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub cover_photo_id: Option<String>,
    pub sort_order: i64,
    /// Number of photos currently in the album.
    pub item_count: i64,
    /// Membership model: manual, smart, or trip.
    pub kind: AlbumKind,
    /// JSON-serialized `PhotoFilter` for smart albums (null otherwise).
    pub rule: Option<String>,
    /// Last modification timestamp (used to order trip albums).
    pub updated_at: Option<String>,
}

/// One tile inside an Albums-view section: a person, a place, or a persisted
/// album (manual / smart / trip).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumSectionItem {
    /// Stable id the frontend uses to open the item (album id, or
    /// `person:<id>` / `place:<name>` for virtual sections).
    pub id: String,
    pub name: String,
    /// Number of photos in this item.
    pub count: i64,
    /// Data-URL thumbnail for the tile (null when no preview is available).
    pub cover_encoded: Option<String>,
    /// Face crop path (people section only).
    pub cover_crop: Option<String>,
    /// Section-level type: "person", "location", "trip", "smart", "manual".
    pub kind: String,
    /// The persisted album when this item is backed by an album row.
    pub album: Option<Album>,
}

/// A titled group of tiles in the Albums view (People, Places, Trips, Smart,
/// and manual albums).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumSection {
    pub id: String,
    pub items: Vec<AlbumSectionItem>,
}

/// Persisted sync session for auto-reconnect on app restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSession {
    pub room_id: String,
    pub signaling_url: String,
    pub port: u16,
    pub is_initiator: bool,
    pub passphrase: String,
}

impl SavedSession {
    /// Encrypt the passphrase using AES-256-GCM with a device-derived key.
    /// Returns a base64-encoded string of nonce + ciphertext.
    pub fn encrypt(&self) -> String {
        use aes_gcm::{
            aead::{Aead, KeyInit, OsRng},
            Aes256Gcm, Nonce,
        };
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        use rand::RngCore;

        let key = Self::derive_key();
        let cipher = Aes256Gcm::new(&key.into());

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = self.passphrase.as_bytes();
        match cipher.encrypt(nonce, plaintext) {
            Ok(ciphertext) => {
                let mut combined = nonce_bytes.to_vec();
                combined.extend_from_slice(&ciphertext);
                B64.encode(combined)
            }
            Err(_) => String::new(),
        }
    }

    /// Decrypt a base64-encoded nonce + ciphertext string back to the passphrase.
    pub fn decrypt(encrypted: &str) -> Result<String, Box<dyn std::error::Error>> {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };
        use base64::{engine::general_purpose::STANDARD as B64, Engine};

        let key = Self::derive_key();
        let cipher = Aes256Gcm::new(&key.into());

        let combined = B64.decode(encrypted)?;
        if combined.len() < 12 {
            return Err("Invalid encrypted data".into());
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = cipher.decrypt(nonce, ciphertext)?;
        Ok(String::from_utf8(plaintext)?)
    }

    /// Derive a 256-bit key from a machine-specific seed.
    /// Uses the hostname + a hardcoded salt for simplicity.
    fn derive_key() -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let hostname = sysinfo::System::name().unwrap_or_else(|| "siegu-default".to_string());
        let mut hasher = Sha256::new();
        hasher.update(format!("siegu-session-key-v1-{hostname}"));
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        key
    }
}

/// SQLite-backed photo, face, and sync metadata store.
pub struct Database {
    pub connection: Connection,
}

use crate::scanner;

/// Build a SQL WHERE clause fragment matching all known video file extensions.
fn video_sql_like() -> String {
    let parts: Vec<String> = scanner::VIDEO_EXTENSIONS
        .iter()
        .map(|ext| format!("location LIKE '%.{ext}'"))
        .collect();
    format!("({})", parts.join(" OR "))
}

/// Build a SQL WHERE clause fragment excluding all known video file extensions.
fn video_sql_not_like() -> String {
    format!(
        "NOT ({})",
        video_sql_like()
            .strip_prefix('(')
            .unwrap_or("")
            .strip_suffix(')')
            .unwrap_or("")
    )
}

/// Whether a file path is one of the known video extensions (case-insensitive).
pub fn is_video_path(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    scanner::VIDEO_EXTENSIONS.contains(&ext.as_str())
}

const MONTH_NAMES: &[(u8, &str, &str)] = &[
    (1, "january", "jan"),
    (2, "february", "feb"),
    (3, "march", "mar"),
    (4, "april", "apr"),
    (5, "may", "may"),
    (6, "june", "jun"),
    (7, "july", "jul"),
    (8, "august", "aug"),
    (9, "september", "sep"),
    (10, "october", "oct"),
    (11, "november", "nov"),
    (12, "december", "dec"),
];

/// Convert a month name (e.g. "january", "mar") to a SQL LIKE pattern for date matching.
fn month_name_to_like(query: &str) -> Option<String> {
    let q_lower = query.to_lowercase();
    for &(num, full, abbr) in MONTH_NAMES {
        if q_lower.contains(full) || q_lower.contains(abbr) {
            return Some(format!("%-{:02}-%", num));
        }
    }
    None
}

/// Days since 1970-01-01 for a date string in either `YYYY-MM-DD` or
/// `YYYY:MM:DD` form (EXIF uses colons). Returns None for unparseable input.
fn date_day_index(value: &str) -> Option<i64> {
    let mut normalized = value.chars().take(10).collect::<String>();
    if normalized.len() < 10 {
        return None;
    }
    if normalized.contains(':') {
        normalized = normalized.replace(':', "-");
    }
    let mut parts = normalized.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: i32 = parts.next()?.parse().ok()?;
    let day: i32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// Inverse of [`date_day_index`] (Howard Hinnant's civil-from-days).
fn day_index_to_date(days: i64) -> Option<String> {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// Howard Hinnant's days-from-civil algorithm (proleptic Gregorian).
fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64;
    let mp = ((month + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146_097 + doe - 719_468
}

/// Photo data for syncing between devices.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PhotoSyncInfo {
    pub id: String,
    pub location: String,
    pub created: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub objects: String, // JSON array of {class, probability}
    pub faces: String,   // JSON array of {face_id, crop_path, encoded, person_id}
    pub caption: Option<String>,
    pub aesthetics_score: Option<f64>,
}

/// A detected object class with its confidence score, used in sync payloads.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SyncObject {
    pub class: String,
    pub probability: String,
}

/// Face crop data for syncing between devices.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SyncFace {
    pub face_id: String,
    pub crop_path: String,
    pub encoded: String,
    pub person_id: Option<String>,
}

/// Borrowed photo data for importing into the database within a transaction.
pub struct ImportedPhoto<'a> {
    pub id: &'a str,
    pub location: &'a str,
    pub created: &'a str,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub objects_json: &'a str,
    pub faces_json: &'a str,
    pub encoded: &'a str,
    pub caption: Option<&'a str>,
    pub aesthetics_score: Option<f64>,
    /// True when the photo was imported from a peer over sync (a backup copy).
    /// Originals scanned from a library folder set this to false.
    pub received: bool,
}

/// A detected face with its associated person name (if any).
#[derive(Debug, Clone, Serialize)]
pub struct FaceWithPerson {
    pub photo_id: String,
    pub face_id: String,
    pub crop_path: String,
    pub encoded: String,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
}

/// Maximum gap (days) between consecutive photos that keeps them in one trip
/// cluster. Larger gaps start a new cluster.
const TRIP_GAP_DAYS: i64 = 3;

/// Minimum photos a trip must contain to be shown.
const TRIP_MIN_PHOTOS: usize = 3;

/// Window (days) within which an adjacent same-country cluster is folded into
/// the current trip, so one journey spanning several cities with quiet gaps
/// in between reads as a single trip.
const TRIP_MERGE_DAYS: i64 = 6;

/// Country part of a `"City, Country"` location name, if any.
fn location_country(location: &str) -> Option<&str> {
    location
        .rsplit_once(", ")
        .map(|(_, country)| country.trim())
        .filter(|c| !c.is_empty())
}

/// Parses `YYYY-MM-DD` into `(year, month, day)`.
fn ymd_parts(value: &str) -> Option<(i32, i32, i32)> {
    let mut parts = value.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: i32 = parts.next()?.parse().ok()?;
    let day: i32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

/// Day-of-year index for `(month, day)` in a fixed non-leap year (2001), so
/// differences between two results equal calendar-day differences.
fn day_of_year_non_leap(month: i32, day: i32) -> i64 {
    let month = month.clamp(1, 12);
    let day = if month == 2 {
        day.clamp(1, 28)
    } else {
        day.clamp(1, 31)
    };
    days_from_civil(2001, month, day)
}

/// Smallest number of days between two day-of-year indexes, wrapping across
/// year boundaries (e.g. Dec 30 and Jan 2 are 3 days apart).
fn circular_year_gap(a: i64, b: i64) -> i64 {
    let diff = (a - b).abs();
    diff.min(365 - diff)
}

/// Seasonal gap (0..=182) between a `YYYY-MM-DD` date and today, ignoring the
/// year so trips from past years surface when they fall around this time of
/// year. `None` when either date cannot be parsed.
fn seasonal_distance_from_today(date: &str) -> Option<i64> {
    let today = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| day_index_to_date((d.as_secs() / 86_400) as i64))?;
    let (_, tm, td) = ymd_parts(&today)?;
    let (_, m, d) = ymd_parts(date)?;
    let today_doy = day_of_year_non_leap(tm, td);
    let date_doy = day_of_year_non_leap(m, d);
    Some(circular_year_gap(today_doy, date_doy))
}

/// Sort key for a trip album id (`trip:YYYY-MM-DD:YYYY-MM-DD`): seasonal gap
/// to today first (closest time of year surfaces), then recency so trips in
/// the same season order newest first.
fn trip_sort_key(album_id: &str) -> (i64, i64) {
    let start = album_id
        .strip_prefix("trip:")
        .and_then(|rest| rest.split(':').next());
    match start {
        Some(date) => (
            seasonal_distance_from_today(date).unwrap_or(i64::MAX),
            -date_day_index(date).unwrap_or(0),
        ),
        None => (i64::MAX, 0),
    }
}

/// Accumulator for one candidate trip: its photos, date span, and resolved
/// locations (`location_name` → photo count). Used to fold same-country
/// clusters into a single trip and to derive a display name.
#[derive(Default)]
struct TripAcc {
    photos: Vec<String>,
    first_day: i64,
    last_day: i64,
    locations: std::collections::HashMap<String, i64>,
}

impl TripAcc {
    fn from_cluster(cluster: &[(String, i64, String)]) -> Self {
        let mut acc = TripAcc {
            first_day: cluster.iter().map(|c| c.1).min().unwrap_or(0),
            last_day: cluster.iter().map(|c| c.1).max().unwrap_or(0),
            ..TripAcc::default()
        };
        for (id, _day, location) in cluster {
            acc.photos.push(id.clone());
            if !location.is_empty() {
                *acc.locations.entry(location.clone()).or_insert(0) += 1;
            }
        }
        acc
    }

    fn merge(&mut self, other: TripAcc) {
        self.first_day = self.first_day.min(other.first_day);
        self.last_day = self.last_day.max(other.last_day);
        self.photos.extend(other.photos);
        for (location, count) in other.locations {
            *self.locations.entry(location).or_insert(0) += count;
        }
    }

    /// Distinct countries visited by this trip's photos.
    fn countries(&self) -> Vec<&str> {
        let mut countries: Vec<&str> = Vec::new();
        for location in self.locations.keys() {
            if let Some(country) = location_country(location) {
                if !countries.contains(&country) {
                    countries.push(country);
                }
            }
        }
        countries
    }

    /// True when both trips resolve to at least one location and share a
    /// country. Trips without location data never merge.
    fn shares_country_with(&self, other: &TripAcc) -> bool {
        let mine = self.countries();
        if mine.is_empty() {
            return false;
        }
        let theirs = other.countries();
        !theirs.is_empty() && theirs.iter().any(|c| mine.contains(c))
    }

    /// Compact display name: `"City, Country"` for a single location,
    /// `"Country"` when one country spans several cities, else the two most
    /// photographed locations. Falls back to `"Trip"` without location data.
    fn display_name(&self) -> String {
        let mut ranked: Vec<(&str, i64)> = self
            .locations
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        if ranked.is_empty() {
            return "Trip".to_string();
        }
        let countries = self.countries();
        if countries.len() == 1 && ranked.len() > 1 {
            return countries[0].to_string();
        }
        ranked.truncate(2);
        ranked
            .into_iter()
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl Database {
    /// Get all photos (with their detected objects and faces) for device sync.
    /// Includes every scanned photo so sync works immediately after a scan, plus
    /// photos received over sync (stored under a 'siegu' folder). The manifest is
    /// "what this device has", so it must include received files: otherwise a
    /// sync-only device reports an empty library and re-requests everything it
    /// already received on every reconnect.
    pub fn get_photo_sync_info(&self) -> Vec<PhotoSyncInfo> {
        let mut results = Vec::new();
        let sql = "SELECT id, location, created, latitude, longitude, caption, aesthetics_score FROM photo p";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            let iter = stmt.query_map([], |row| {
                let id: String = row.get(0)?;

                // Fetch objects for this photo
                let mut objects = Vec::new();
                if let Ok(mut obj_stmt) = self
                    .connection
                    .prepare("SELECT class, probability FROM object WHERE photo_id = ?1")
                {
                    if let Ok(obj_rows) = obj_stmt.query_map([&id], |r| {
                        Ok(SyncObject {
                            class: r.get(0)?,
                            probability: r.get(1)?,
                        })
                    }) {
                        for obj in obj_rows.flatten() {
                            objects.push(obj);
                        }
                    }
                }

                // Fetch faces for this photo
                let mut faces = Vec::new();
                if let Ok(mut face_stmt) = self.connection.prepare(
                    "SELECT face_id, crop_path, encoded, person_id FROM faces WHERE photo_id = ?1",
                ) {
                    if let Ok(face_rows) = face_stmt.query_map([&id], |r| {
                        Ok(SyncFace {
                            face_id: r.get(0)?,
                            crop_path: r.get(1)?,
                            encoded: r.get(2)?,
                            person_id: r.get(3)?,
                        })
                    }) {
                        for face in face_rows.flatten() {
                            faces.push(face);
                        }
                    }
                }

                Ok(PhotoSyncInfo {
                    id,
                    location: row.get(1)?,
                    created: row.get(2).unwrap_or_default(),
                    latitude: row.get(3).ok(),
                    longitude: row.get(4).ok(),
                    objects: serde_json::to_string(&objects).unwrap_or("[]".to_string()),
                    faces: serde_json::to_string(&faces).unwrap_or("[]".to_string()),
                    caption: row.get(5).ok(),
                    aesthetics_score: row.get(6).ok(),
                })
            });
            if let Ok(iter) = iter {
                for p in iter.flatten() {
                    results.push(p);
                }
            }
        }
        results
    }

    /// Get sync info for a single photo by its ID.
    pub fn get_photo_sync_info_by_id(&self, photo_id: &str) -> Result<PhotoSyncInfo, String> {
        let sql = "SELECT id, location, created, latitude, longitude, caption, aesthetics_score FROM photo WHERE id = ?1";
        self.connection
            .query_row(sql, [photo_id], |row| {
                let id: String = row.get(0)?;

                // Fetch objects for this photo
                let mut objects = Vec::new();
                if let Ok(mut obj_stmt) = self
                    .connection
                    .prepare("SELECT class, probability FROM object WHERE photo_id = ?1")
                {
                    if let Ok(obj_rows) = obj_stmt.query_map([&id], |r| {
                        Ok(SyncObject {
                            class: r.get(0)?,
                            probability: r.get(1)?,
                        })
                    }) {
                        for obj in obj_rows.flatten() {
                            objects.push(obj);
                        }
                    }
                }

                // Fetch faces for this photo
                let mut faces = Vec::new();
                if let Ok(mut face_stmt) = self.connection.prepare(
                    "SELECT face_id, crop_path, encoded, person_id FROM faces WHERE photo_id = ?1",
                ) {
                    if let Ok(face_rows) = face_stmt.query_map([&id], |r| {
                        Ok(SyncFace {
                            face_id: r.get(0)?,
                            crop_path: r.get(1)?,
                            encoded: r.get(2)?,
                            person_id: r.get(3)?,
                        })
                    }) {
                        for face in face_rows.flatten() {
                            faces.push(face);
                        }
                    }
                }

                Ok(PhotoSyncInfo {
                    id,
                    location: row.get(1)?,
                    created: row.get(2).unwrap_or_default(),
                    latitude: row.get(3).ok(),
                    longitude: row.get(4).ok(),
                    objects: serde_json::to_string(&objects).unwrap_or("[]".to_string()),
                    faces: serde_json::to_string(&faces).unwrap_or("[]".to_string()),
                    caption: row.get(5).ok(),
                    aesthetics_score: row.get(6).ok(),
                })
            })
            .map_err(|e| e.to_string())
    }

    /// Open or create the database at the given config directory, running schema migrations.
    pub fn new(config_path: &str) -> Self {
        let path = format!("{config_path}/siegu.db");
        let _ = fs::create_dir_all(config_path);
        // Fatal: the app cannot function without a usable database, and `new` cannot signal failure.
        #[allow(clippy::expect_used)]
        let conn = Connection::open(&path)
            .expect("failed to open the Siegu database; the app cannot function without it");

        // Enable WAL mode for better concurrency and set a busy timeout
        let _ = conn.execute("PRAGMA journal_mode=WAL;", ());
        let _ = conn.busy_timeout(std::time::Duration::from_secs(5));
        // Write/read tuning: with WAL, NORMAL only fsyncs on checkpoint, and a
        // larger page cache plus mmap avoids re-reading thumbnails/embeddings.
        let _ = conn.execute("PRAGMA synchronous=NORMAL;", ());
        let _ = conn.execute("PRAGMA cache_size=-32768;", ());
        let _ = conn.execute("PRAGMA mmap_size=268435456;", ());

        let _ = conn.execute("CREATE TABLE IF NOT EXISTS photo (id STRING PRIMARY KEY, location STRING, encoded STRING, created DATE_TIME, latitude REAL, longitude REAL, indexed INTEGER DEFAULT 0, caption TEXT, aesthetics_score REAL);", ());
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS ai_status (photo_id STRING PRIMARY KEY, clip INTEGER DEFAULT 0, face INTEGER DEFAULT 0, ocr INTEGER DEFAULT 0, nsfw INTEGER DEFAULT 0, aesthetics INTEGER DEFAULT 0, yolo INTEGER DEFAULT 0, blip INTEGER DEFAULT 0, arcface INTEGER DEFAULT 0, midas INTEGER DEFAULT 0, whisper INTEGER DEFAULT 0, sam INTEGER DEFAULT 0, superres INTEGER DEFAULT 0);", ());
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS ocr (photo_id STRING, text TEXT);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_ocr_photo_id ON ocr(photo_id);",
            (),
        );

        // Simple migration: try to add columns if they don't exist (ignore errors if they do)
        let _ = conn.execute("ALTER TABLE photo ADD COLUMN latitude REAL;", ());
        let _ = conn.execute("ALTER TABLE photo ADD COLUMN longitude REAL;", ());
        let _ = conn.execute("ALTER TABLE photo ADD COLUMN created DATE_TIME;", ());
        let _ = conn.execute("ALTER TABLE photo ADD COLUMN caption TEXT;", ());
        let _ = conn.execute("ALTER TABLE photo ADD COLUMN aesthetics_score REAL;", ());
        let _ = conn.execute(
            "ALTER TABLE photo ADD COLUMN sync_needed INTEGER DEFAULT 0;",
            (),
        );
        let _ = conn.execute(
            "ALTER TABLE photo ADD COLUMN indexed INTEGER DEFAULT 0;",
            (),
        );
        let _ = conn.execute(
            "ALTER TABLE photo ADD COLUMN received INTEGER DEFAULT 0;",
            (),
        );

        // Virtual generated column mirroring video_sql_like(): lets the
        // videos-only filter use an index instead of a 14-way LIKE scan.
        // VIRTUAL (not STORED) because ALTER TABLE ADD COLUMN rejects STORED.
        let is_video_expr = scanner::VIDEO_EXTENSIONS
            .iter()
            .map(|ext| format!("location LIKE '%.{ext}'"))
            .collect::<Vec<_>>()
            .join(" OR ");
        let _ = conn.execute(
            &format!(
                "ALTER TABLE photo ADD COLUMN is_video INTEGER GENERATED ALWAYS AS ({is_video_expr}) VIRTUAL"
            ),
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_is_video ON photo(is_video);",
            (),
        );

        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_location ON photo(location);",
            (),
        );

        // Deduplicate existing rows by location, then enforce uniqueness
        let _ = conn.execute(
            "DELETE FROM photo WHERE rowid NOT IN (SELECT MIN(rowid) FROM photo GROUP BY location)",
            (),
        );
        let _ = conn.execute("DROP INDEX IF EXISTS idx_photo_location;", ());
        let _ = conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_photo_location_unique ON photo(location);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_created ON photo(created);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_aesthetics ON photo(aesthetics_score);",
            (),
        );
        // Matches "best" ordering (aesthetics DESC NULLS LAST) so SQLite scans
        // the index instead of sorting the whole table for each page.
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_aesthetics_desc ON photo(aesthetics_score DESC);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_indexed ON photo(indexed);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_coords ON photo(latitude, longitude);",
            (),
        );
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS directory (name TEXT);", ());
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS object(photo_id TEXT, class TEXT, probability TEXT);",
            (),
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS properties (photo_id TEXT, key TEXT, value TEXT);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_object_photo_id ON object(photo_id);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_object_class ON object(class);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_properties_photo_id ON properties(photo_id);",
            (),
        );
        // Backs tag/location/non-camera facet lookups (key + value equality).
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_properties_key_value ON properties(key, value);",
            (),
        );
        // Backs the ocr/text portion of full-text search.
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_ocr_text ON ocr(text);", ());
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS peer_device(\
             device_id TEXT PRIMARY KEY, name TEXT, ip TEXT, port INTEGER DEFAULT 0, \
             device_type TEXT DEFAULT '', os TEXT DEFAULT '', \
             models_enabled TEXT DEFAULT '[]', protocol_version INTEGER DEFAULT 1, \
             storage_used INTEGER DEFAULT 0, storage_capacity INTEGER DEFAULT 0, \
             last_seen TEXT DEFAULT (datetime('now'))\
             );",
            (),
        );
        // Migrate legacy device table → peer_device, then drop it.
        let _ = conn.execute(
            "INSERT OR IGNORE INTO peer_device(device_id, name, ip, port, last_seen) \
             SELECT lower(hex(randomblob(16))), name, ip, 0, datetime('now') FROM device",
            (),
        );
        let _ = conn.execute("DROP TABLE IF EXISTS device", ());
        let _ = conn.execute(
            "ALTER TABLE peer_device ADD COLUMN photo_count INTEGER DEFAULT 0;",
            (),
        );
        let _ = conn.execute(
            "ALTER TABLE peer_device ADD COLUMN video_count INTEGER DEFAULT 0;",
            (),
        );
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS faces (photo_id STRING, face_id STRING PRIMARY KEY, crop_path STRING, encoded STRING, embedding BLOB, person_id STRING);", ());
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS people (id STRING PRIMARY KEY, name STRING, embedding BLOB);", ());
        // Must run after the faces/people tables exist: created here (not above)
        // so fresh databases get these indexes too — previously the CREATE
        // INDEX statements ran before the tables and silently failed, leaving
        // fresh installs without index coverage for person/face queries.
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_faces_person_id ON faces(person_id);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_faces_person_photo ON faces(person_id, photo_id);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_faces_photo_id ON faces(photo_id);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_people_name ON people(name);",
            (),
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS config(key TEXT, value TEXT);",
            (),
        );
        // Deduplicate config rows (old schema had no UNIQUE constraint)
        let _ = conn.execute(
            "DELETE FROM config WHERE rowid NOT IN (SELECT MIN(rowid) FROM config GROUP BY key)",
            (),
        );
        // Logs are no longer stored in SQLite (unbounded disk growth). The app
        // keeps a bounded in-memory ring buffer for the log viewer and a size-
        // rotated siegu_debug.log on disk. Drop any legacy table to reclaim space.
        let _ = conn.execute("DROP TABLE IF EXISTS logs", ());

        // Albums (local, free tier): user-created collections of photos.
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS album(\
             id TEXT PRIMARY KEY, name TEXT NOT NULL, \
             created_at TEXT DEFAULT (datetime('now')), \
             cover_photo_id TEXT, sort_order INTEGER DEFAULT 0\
             );",
            (),
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS album_item(\
             album_id TEXT NOT NULL, photo_id TEXT NOT NULL, \
             added_at TEXT DEFAULT (datetime('now')), \
             position INTEGER DEFAULT 0, \
             PRIMARY KEY (album_id, photo_id)\
             );",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_album_item_album ON album_item(album_id, position);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_album_item_photo ON album_item(photo_id);",
            (),
        );
        let _ = conn.execute(
            "ALTER TABLE album ADD COLUMN kind TEXT NOT NULL DEFAULT 'manual';",
            (),
        );
        let _ = conn.execute("ALTER TABLE album ADD COLUMN rule TEXT;", ());
        let _ = conn.execute(
            "ALTER TABLE album ADD COLUMN updated_at TEXT DEFAULT (datetime('now'));",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_album_kind ON album(kind);",
            (),
        );

        // Dismissed-trip signatures persist so a deleted trip does not
        // reappear on the next sync.
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS dismissed_trip(id TEXT PRIMARY KEY);",
            (),
        );

        Self { connection: conn }
    }

    /// Run `PRAGMA integrity_check` and return true if the database is healthy.
    pub fn check_integrity(&self) -> crate::Result<bool> {
        let result: String = self
            .connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if result == "ok" {
            Ok(true)
        } else {
            tracing::error!(result = %result, "Database integrity check failed");
            Ok(false)
        }
    }

    /// Read all config key-value pairs from the config table.
    pub fn get_state(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Ok(mut stmt) = self.connection.prepare("SELECT key, value FROM config") {
            if let Ok(rows) = stmt.query_map([], |row| {
                let key = Self::sql_value_to_string(row.get::<_, rusqlite::types::Value>(0)?);
                let value = Self::sql_value_to_string(row.get::<_, rusqlite::types::Value>(1)?);
                Ok((key, value))
            }) {
                for row in rows.flatten() {
                    map.insert(row.0, row.1);
                }
            }
        }
        map
    }

    /// Convert any SQLite storage class to its string representation.
    fn sql_value_to_string(value: rusqlite::types::Value) -> String {
        match value {
            rusqlite::types::Value::Null => String::new(),
            rusqlite::types::Value::Integer(i) => i.to_string(),
            rusqlite::types::Value::Real(f) => f.to_string(),
            rusqlite::types::Value::Text(s) => s,
            rusqlite::types::Value::Blob(b) => String::from_utf8_lossy(&b).into_owned(),
        }
    }

    /// Write config key-value pairs, replacing any existing keys.
    pub fn set_state(&self, state: HashMap<String, String>) {
        for (key, value) in state {
            let _ = self
                .connection
                .execute("DELETE FROM config WHERE key = ?1", [&key]);
            let _ = self.connection.execute(
                "INSERT INTO config (key, value) VALUES(?1, ?2)",
                (&key, &value),
            );
        }
    }

    /// Get the timestamp of the last directory scan.
    pub fn get_last_scan_time(&self) -> Option<String> {
        self.get_state().get("last_scan_time").cloned()
    }

    /// Set the timestamp of the last directory scan.
    pub fn set_last_scan_time(&self, timestamp: String) {
        let _ = self.connection.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES('last_scan_time', ?1)",
            [&timestamp],
        );
    }

    /// Save a sync session for auto-reconnect on next startup.
    /// The passphrase is encrypted with AES-256-GCM using a device-derived key.
    pub fn save_session(&self, session: &SavedSession) {
        let encrypted = session.encrypt();
        let mut state = self.get_state();
        state.insert("session_room_id".to_string(), session.room_id.clone());
        state.insert(
            "session_signaling_url".to_string(),
            session.signaling_url.clone(),
        );
        state.insert("session_port".to_string(), session.port.to_string());
        state.insert(
            "session_is_initiator".to_string(),
            session.is_initiator.to_string(),
        );
        state.insert("session_encrypted_passphrase".to_string(), encrypted);
        self.set_state(state);
    }

    /// Load the saved session, if any. Returns None if no session exists or decryption fails.
    pub fn load_session(&self) -> Option<SavedSession> {
        let state = self.get_state();
        let room_id = state.get("session_room_id")?.clone();
        let signaling_url = state.get("session_signaling_url")?.clone();
        let port = state
            .get("session_port")
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(0);
        let is_initiator = state
            .get("session_is_initiator")
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);
        let encrypted = state.get("session_encrypted_passphrase")?.clone();
        let passphrase = SavedSession::decrypt(&encrypted).ok()?;
        Some(SavedSession {
            room_id,
            signaling_url,
            port,
            is_initiator,
            passphrase,
        })
    }

    /// Clear any saved session.
    pub fn clear_session(&self) {
        let mut state = self.get_state();
        state.remove("session_room_id");
        state.remove("session_signaling_url");
        state.remove("session_port");
        state.remove("session_is_initiator");
        state.remove("session_encrypted_passphrase");
        self.set_state(state);
    }

    /// Search for object tags, location names, people, and month suggestions matching the query.
    pub fn list_objects(&self, query: &str) -> Vec<SearchSuggestion> {
        let mut objects = Vec::new();
        let sql = "SELECT class, 'tag' FROM object WHERE class LIKE ?1 \
            UNION SELECT value, 'location' FROM properties WHERE key = 'location_name' AND value LIKE ?1 \
            UNION SELECT name, 'person' FROM people WHERE name IS NOT NULL AND name LIKE ?1 \
            LIMIT 20";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([format!("%{query}%")], |row| {
                Ok(SearchSuggestion {
                    title: row.get(0)?,
                    suggestion_type: row.get(1)?,
                })
            }) {
                for item in iter.flatten() {
                    objects.push(item);
                }
            }
        }

        let q = query.trim();
        if q.len() >= 2 {
            let q_lower = q.to_lowercase();
            for &(num, full, abbr) in MONTH_NAMES {
                if full.starts_with(&q_lower) || abbr.starts_with(&q_lower) {
                    objects.push(SearchSuggestion {
                        title: full.to_string(),
                        suggestion_type: "date".to_string(),
                    });
                }

                if q_lower.contains(full) || q_lower.contains(abbr) {
                    let ym_sql = "SELECT DISTINCT substr(created, 1, 7) FROM photo WHERE created LIKE ?1 ORDER BY 1 DESC LIMIT 5";
                    if let Ok(mut stmt) = self.connection.prepare(ym_sql) {
                        let pat = format!("%-{:02}-%", num);
                        if let Ok(iter) = stmt.query_map([&pat], |row| row.get::<_, String>(0)) {
                            for ym in iter.flatten() {
                                if let Some((year, _)) = ym.split_once('-') {
                                    objects.push(SearchSuggestion {
                                        title: format!("{} {}", full, year),
                                        suggestion_type: "date".to_string(),
                                    });
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }

        objects
    }

    fn enrich_objects(&self, photos: &mut [Photo]) {
        if photos.is_empty() {
            return;
        }
        let index: std::collections::HashMap<String, usize> = photos
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.clone(), i))
            .collect();
        let placeholders: Vec<String> = photos
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT photo_id, class, probability FROM object WHERE photo_id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = photos
            .iter()
            .map(|p| &p.id as &dyn rusqlite::types::ToSql)
            .collect();
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(params.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)
                        .ok()
                        .or_else(|| row.get::<_, String>(2).ok().and_then(|s| s.parse().ok()))
                        .unwrap_or(0.0),
                ))
            }) {
                for row in iter.flatten() {
                    if let Some(&i) = index.get(&row.0) {
                        photos[i].objects.insert(row.1, row.2);
                    }
                }
            }
        }
    }

    fn enrich_properties(&self, photos: &mut [Photo]) {
        if photos.is_empty() {
            return;
        }
        let index: std::collections::HashMap<String, usize> = photos
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.clone(), i))
            .collect();
        let placeholders: Vec<String> = photos
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT photo_id, key, value FROM properties WHERE photo_id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = photos
            .iter()
            .map(|p| &p.id as &dyn rusqlite::types::ToSql)
            .collect();
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(params.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)
                        .ok()
                        .or_else(|| row.get::<_, f64>(2).ok().map(|v| v.to_string()))
                        .or_else(|| row.get::<_, i64>(2).ok().map(|v| v.to_string()))
                        .unwrap_or_default(),
                ))
            }) {
                for row in iter.flatten() {
                    if let Some(&i) = index.get(&row.0) {
                        photos[i].properties.insert(row.1, row.2);
                    }
                }
            }
        }
    }

    /// List photos with search, pagination, and optional favorite/video filters.
    pub fn list_photos(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
        favorites_only: bool,
        videos_only: bool,
    ) -> Vec<Photo> {
        self.list_photos_filtered(
            query,
            offset,
            limit,
            favorites_only,
            videos_only,
            &PhotoFilter::default(),
            true,
        )
    }

    /// Build the "WHERE 1=1 ..." clause and bound parameters for a filtered
    /// photo query. Params begin with ?1 = offset, ?2 = limit so the same
    /// parameter vector works for both paged listings and COUNT(*) queries
    /// (callers pass offset 0 / limit -1 for an unbounded count).
    fn photo_where_clause(
        &self,
        query: &str,
        favorites_only: bool,
        videos_only: bool,
        filter: &PhotoFilter,
        offset: usize,
        limit: usize,
    ) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
        let fav_filter = if favorites_only {
            "AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite')"
        } else {
            ""
        };
        let video_filter = if videos_only {
            "AND p.is_video = 1"
        } else {
            ""
        };

        let is_uuid = query.len() == 36 && query.chars().all(|c| c.is_alphanumeric() || c == '-');

        let month_like = month_name_to_like(query);
        let month_param = month_like.as_ref().map(|p| p.to_string());

        let q_filter = if !query.is_empty() {
            if is_uuid {
                "AND (p.id = ?3 OR EXISTS(SELECT 1 FROM faces WHERE photo_id=p.id AND person_id = ?3))".to_string()
            } else if month_param.is_some() {
                "AND (p.location LIKE ?3 OR p.id LIKE ?3 OR p.caption LIKE ?3 \
                    OR EXISTS(SELECT 1 FROM object WHERE photo_id=p.id AND class LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM ocr WHERE photo_id=p.id AND text LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM faces f JOIN people p_name ON f.person_id = p_name.id WHERE f.photo_id=p.id AND p_name.name LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='location_name' AND value LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='transcript' AND value LIKE ?3) \
                    OR p.created LIKE ?3 OR p.created LIKE ?4)".to_string()
            } else {
                "AND (p.location LIKE ?3 OR p.id LIKE ?3 OR p.caption LIKE ?3 \
                    OR EXISTS(SELECT 1 FROM object WHERE photo_id=p.id AND class LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM ocr WHERE photo_id=p.id AND text LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM faces f JOIN people p_name ON f.person_id = p_name.id WHERE f.photo_id=p.id AND p_name.name LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='location_name' AND value LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='transcript' AND value LIKE ?3) \
                    OR p.created LIKE ?3)".to_string()
            }
        } else {
            String::new()
        };

        // Build facet filters using the next available parameter slots.
        let mut facet_filters = String::new();
        let mut extra_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut slot = 2usize; // ?1 offset, ?2 limit
        if !query.is_empty() {
            slot += 1; // ?3 query
            if month_param.is_some() {
                slot += 1; // ?4 month
            }
        }
        if !filter.person_ids.is_empty() {
            let n = filter.person_ids.len();
            let placeholders: Vec<String> = (1..=n).map(|i| format!("?{}", slot + i)).collect();
            let joined = placeholders.join(",");
            if filter.person_alone {
                facet_filters.push_str(&format!(
                    " AND p.id IN (SELECT f.photo_id FROM faces f WHERE f.person_id IN ({joined}) \
                     GROUP BY f.photo_id HAVING COUNT(DISTINCT f.person_id) = {n}) \
                     AND (SELECT COUNT(*) FROM faces f WHERE f.photo_id = p.id) = {n}"
                ));
            } else if filter.person_match == PersonMatch::Or {
                facet_filters.push_str(&format!(
                    " AND EXISTS(SELECT 1 FROM faces f WHERE f.photo_id = p.id AND f.person_id IN ({joined}))"
                ));
            } else {
                facet_filters.push_str(&format!(
                    " AND p.id IN (SELECT f.photo_id FROM faces f WHERE f.person_id IN ({joined}) \
                     GROUP BY f.photo_id HAVING COUNT(DISTINCT f.person_id) = {n})"
                ));
            }
            for id in &filter.person_ids {
                extra_params.push(Box::new(id.clone()));
            }
            slot += n;
        }
        if let Some(ref location) = filter.location {
            slot += 1;
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='location_name' AND value = ?{slot})"
            ));
            extra_params.push(Box::new(location.clone()));
        }
        if let Some(ref tag) = filter.tag {
            slot += 1;
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM object WHERE photo_id=p.id AND class = ?{slot})"
            ));
            extra_params.push(Box::new(tag.clone()));
        }
        if let Some(ref date_from) = filter.date_from {
            slot += 1;
            facet_filters.push_str(&format!(" AND p.created >= ?{slot}"));
            extra_params.push(Box::new(date_from.clone()));
        }
        if let Some(ref date_to) = filter.date_to {
            slot += 1;
            facet_filters.push_str(&format!(" AND p.created <= ?{slot}"));
            extra_params.push(Box::new(date_to.clone()));
        }
        if filter.favorite {
            facet_filters.push_str(
                " AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite')",
            );
        }
        if filter.has_faces {
            facet_filters.push_str(" AND EXISTS(SELECT 1 FROM faces WHERE photo_id=p.id)");
        }
        if let Some(ref aesthetics_min) = filter.aesthetics_min {
            slot += 1;
            facet_filters.push_str(&format!(" AND p.aesthetics_score >= ?{slot}"));
            extra_params.push(Box::new(*aesthetics_min));
        }
        if let Some(ref camera) = filter.camera {
            slot += 1;
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key IN ('Make','Model') AND value LIKE ?{slot})"
            ));
            extra_params.push(Box::new(format!("%{camera}%")));
        }
        if filter.papers {
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM object WHERE photo_id=p.id AND class IN ({paper_in}))",
                paper_in = paper_class_in_clause()
            ));
        }
        if filter.nsfw_only {
            facet_filters.push_str(
                " AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='nsfw' AND CAST(value AS REAL) >= 0.8)",
            );
        }
        if let Some(ref album_id) = filter.album_id {
            slot += 1;
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM album_item WHERE album_id=?{slot} AND photo_id=p.id)"
            ));
            extra_params.push(Box::new(album_id.clone()));
        }

        let where_clause =
            format!("WHERE 1=1 {fav_filter} {video_filter} {q_filter} {facet_filters}");
        let q_param = if is_uuid {
            query.to_string()
        } else {
            format!("%{query}%")
        };
        let mut params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(offset as i64), Box::new(limit as i64)];
        if !query.is_empty() {
            params.push(Box::new(q_param));
            if let Some(ref mp) = month_param {
                params.push(Box::new(mp.clone()));
            }
        }
        params.extend(extra_params);
        (where_clause, params)
    }

    /// Count photos matching a search + facet filter without loading any rows.
    /// Used for smart-album membership and section totals.
    pub fn count_photos_filtered(
        &self,
        query: &str,
        favorites_only: bool,
        videos_only: bool,
        filter: &PhotoFilter,
    ) -> i64 {
        let (where_clause, params) =
            self.photo_where_clause(query, favorites_only, videos_only, filter, 0, usize::MAX);
        let sql = format!("SELECT COUNT(*) FROM photo p {where_clause}");
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        self.connection
            .prepare(&sql)
            .ok()
            .and_then(|mut stmt| stmt.query_row(param_refs.as_slice(), |row| row.get(0)).ok())
            .unwrap_or(0)
    }

    /// List photos with search, pagination, favorite/video filters, and facet
    /// filters (person, location, tag, date range) combined with AND.
    #[allow(clippy::too_many_arguments)]
    pub fn list_photos_filtered(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
        favorites_only: bool,
        videos_only: bool,
        filter: &PhotoFilter,
        include_encoded: bool,
    ) -> Vec<Photo> {
        let mut photos = Vec::new();
        let (where_clause, params) =
            self.photo_where_clause(query, favorites_only, videos_only, filter, offset, limit);

        let order_by = if let Some(order) = filter.order_by.as_deref() {
            match order {
                "oldest" => "ORDER BY p.created ASC",
                "best" => "ORDER BY p.aesthetics_score DESC NULLS LAST, p.created DESC",
                "random" => "ORDER BY RANDOM()",
                _ => "ORDER BY p.created DESC",
            }
        } else if filter.random {
            "ORDER BY RANDOM()"
        } else {
            "ORDER BY p.created DESC"
        };

        let encoded_col = if include_encoded {
            "p.encoded"
        } else {
            "NULL AS encoded"
        };
        let sql = format!("SELECT p.id, p.location, {encoded_col}, p.latitude, p.longitude, p.created, EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, p.caption, p.aesthetics_score, 
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres, p.sync_needed, p.received 
            FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id {where_clause} {order_by} LIMIT ?1, ?2");
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            if let Ok(iter) = stmt.query_map(param_refs.as_slice(), |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row
                        .get::<_, Option<String>>(2)
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
                    created: row.get(5).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: row.get(3).unwrap_or(0.0),
                    longitude: row.get(4).unwrap_or(0.0),
                    favorite: row.get(6).unwrap_or(false),
                    indexed: row.get(7).unwrap_or(0),
                    caption: row.get(8).ok(),
                    aesthetics_score: row.get(9).ok(),
                    ai_status: AiStatus {
                        clip: row.get(10).unwrap_or(0),
                        face: row.get(11).unwrap_or(0),
                        ocr: row.get(12).unwrap_or(0),
                        nsfw: row.get(13).unwrap_or(0),
                        aesthetics: row.get(14).unwrap_or(0),
                        yolo: row.get(15).unwrap_or(0),
                        blip: row.get(16).unwrap_or(0),
                        arcface: row.get(17).unwrap_or(0),
                        midas: row.get(18).unwrap_or(0),
                        whisper: row.get(19).unwrap_or(0),
                        sam: row.get(20).unwrap_or(0),
                        superres: row.get(21).unwrap_or(0),
                    },
                    sync_needed: row.get(22).unwrap_or(false),
                    received: row.get(23).unwrap_or(false),
                })
            }) {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        self.enrich_objects(&mut photos);
        self.enrich_properties(&mut photos);
        photos
    }

    /// Named people with a representative face thumbnail, ordered by the number
    /// of distinct photos that contain them.
    pub fn get_search_people(&self, limit: i64) -> Vec<SearchPerson> {
        let mut people = Vec::new();
        let sql = "SELECT p.id, p.name, f.crop_path, f.encoded, \
            (SELECT COUNT(DISTINCT photo_id) FROM faces WHERE person_id = p.id) \
            FROM people p LEFT JOIN faces f ON p.id = f.person_id \
            WHERE p.name IS NOT NULL AND TRIM(p.name) != '' \
            GROUP BY p.id ORDER BY 5 DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok(SearchPerson {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    representative_crop: row.get(2).ok(),
                    encoded: row.get(3).ok(),
                    photo_count: row.get(4)?,
                })
            }) {
                for p in iter.flatten() {
                    people.push(p);
                }
            }
        }
        people
    }

    /// Distinct resolved location names with photo counts, most common first.
    pub fn get_location_counts(&self, limit: i64) -> Vec<(String, i64)> {
        let mut counts = Vec::new();
        let sql = "SELECT value, COUNT(*) FROM properties WHERE key = 'location_name' \
            GROUP BY value ORDER BY COUNT(*) DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            }) {
                for c in iter.flatten() {
                    counts.push(c);
                }
            }
        }
        counts
    }

    /// Detected object classes with photo counts, most common first.
    pub fn get_tag_counts(&self, limit: i64) -> Vec<(String, i64)> {
        let mut counts = Vec::new();
        let sql =
            "SELECT class, COUNT(*) FROM object GROUP BY class ORDER BY COUNT(*) DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            }) {
                for c in iter.flatten() {
                    counts.push(c);
                }
            }
        }
        counts
    }

    /// Year-month buckets ("2026-03") with photo counts, newest first.
    pub fn get_month_counts(&self, limit: i64) -> Vec<(String, i64)> {
        let mut counts = Vec::new();
        let sql = "SELECT substr(created, 1, 7) AS ym, COUNT(*) FROM photo \
            WHERE created IS NOT NULL AND created != '' GROUP BY ym ORDER BY ym DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            }) {
                for c in iter.flatten() {
                    counts.push(c);
                }
            }
        }
        counts
    }

    /// Library-wide counts used by the search dropdown footer.
    pub fn get_search_stats(&self) -> SearchStats {
        let query_count = |sql: &str| -> i64 {
            self.connection
                .query_row(sql, [], |r| r.get(0))
                .unwrap_or(0)
        };
        let video_like = video_sql_like();
        SearchStats {
            photos: query_count("SELECT COUNT(*) FROM photo"),
            videos: query_count(&format!("SELECT COUNT(*) FROM photo WHERE {video_like}")),
            favorites: query_count("SELECT COUNT(*) FROM properties WHERE key = 'favorite'"),
            ocr_photos: query_count("SELECT COUNT(DISTINCT photo_id) FROM ocr"),
            faces: query_count("SELECT COUNT(*) FROM faces"),
            named_people: query_count("SELECT COUNT(*) FROM people WHERE name IS NOT NULL"),
            face_photos: query_count("SELECT COUNT(DISTINCT photo_id) FROM faces"),
            nsfw_photos: query_count(
                "SELECT COUNT(DISTINCT photo_id) FROM properties WHERE key = 'nsfw' AND CAST(value AS REAL) >= 0.8",
            ),
        }
    }

    /// Highest-rated photos by aesthetics score, for the "Best of your library" rail.
    /// Best-scored photo of each day, most recent day first, for the Best Shots rail.
    pub fn get_best_photos(&self, limit: i64) -> Vec<SearchPhotoTile> {
        let mut tiles = Vec::new();
        let sql = "SELECT id, location, encoded, created, aesthetics_score, favorite FROM ( \
            SELECT p.id, p.location, p.encoded, p.created, p.aesthetics_score, \
                EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite') AS favorite, \
                ROW_NUMBER() OVER ( \
                    PARTITION BY substr(p.created, 1, 10) \
                    ORDER BY p.aesthetics_score DESC, p.created ASC \
                ) AS rn \
            FROM photo p \
            WHERE p.aesthetics_score IS NOT NULL AND p.aesthetics_score > 0 \
        ) WHERE rn = 1 \
        ORDER BY created DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok(SearchPhotoTile {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2).unwrap_or_default(),
                    created: row.get(3).unwrap_or_default(),
                    aesthetics_score: row.get(4).ok(),
                    favorite: row.get(5).unwrap_or(false),
                })
            }) {
                for t in iter.flatten() {
                    tiles.push(t);
                }
            }
        }
        tiles
    }

    /// Favorited photos, newest first, for the Favorites rail.
    pub fn get_favorite_photos(&self, limit: i64) -> Vec<SearchPhotoTile> {
        let mut tiles = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.created, p.aesthetics_score, 1 \
            FROM photo p WHERE EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite') \
            ORDER BY p.created DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok(SearchPhotoTile {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2).unwrap_or_default(),
                    created: row.get(3).unwrap_or_default(),
                    aesthetics_score: row.get(4).ok(),
                    favorite: row.get(5).unwrap_or(false),
                })
            }) {
                for t in iter.flatten() {
                    tiles.push(t);
                }
            }
        }
        tiles
    }

    /// Most recently added photos for the Recent rail.
    pub fn get_recent_photos(&self, limit: i64) -> Vec<SearchPhotoTile> {
        let mut tiles = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.created, p.aesthetics_score, \
            EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite') \
            FROM photo p ORDER BY p.created DESC, p.id DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok(SearchPhotoTile {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2).unwrap_or_default(),
                    created: row.get(3).unwrap_or_default(),
                    aesthetics_score: row.get(4).ok(),
                    favorite: row.get(5).unwrap_or(false),
                })
            }) {
                for t in iter.flatten() {
                    tiles.push(t);
                }
            }
        }
        tiles
    }

    /// Document/screenshot classes (CLIP zero-shot) with photo counts.
    pub fn get_paper_counts(&self, limit: i64) -> Vec<(String, i64)> {
        let mut counts = Vec::new();
        let sql = format!(
            "SELECT class, COUNT(*) FROM object WHERE class IN ({paper_in}) \
        GROUP BY class ORDER BY COUNT(*) DESC LIMIT ?1",
            paper_in = paper_class_in_clause()
        );
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            }) {
                for c in iter.flatten() {
                    counts.push(c);
                }
            }
        }
        counts
    }

    /// Camera identifiers (Make + Model from EXIF) with photo counts, most common first.
    pub fn get_camera_counts(&self, limit: i64) -> Vec<(String, i64)> {
        let mut by_photo: std::collections::HashMap<String, (Option<String>, Option<String>)> =
            std::collections::HashMap::new();
        let sql = "SELECT photo_id, key, value FROM properties WHERE key IN ('Make','Model')";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            }) {
                for r in iter.flatten() {
                    let (pid, key, value) = r;
                    let entry = by_photo.entry(pid).or_default();
                    if key == "Make" {
                        entry.0 = Some(value);
                    } else {
                        entry.1 = Some(value);
                    }
                }
            }
        }
        let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for (_, (make, model)) in by_photo {
            let brand = match (make, model) {
                (Some(m), _) if !m.trim().is_empty() => m.trim().to_lowercase(),
                (_, Some(md)) if !md.trim().is_empty() => {
                    md.split_whitespace().next().unwrap_or("").to_lowercase()
                }
                _ => continue,
            };
            if brand.is_empty() {
                continue;
            }
            *counts.entry(brand).or_insert(0) += 1;
        }
        let mut sorted: Vec<(String, i64)> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        sorted.truncate(limit as usize);
        sorted
    }

    /// Locations with counts and a representative photo thumbnail for the rail.
    /// Ordered by most photos (used for search facets).
    pub fn get_location_groups(&self, limit: i64) -> Vec<LocationGroup> {
        let mut groups = Vec::new();
        let sql = "SELECT pr.value AS name, COUNT(*) AS cnt, \
            (SELECT p2.location FROM photo p2 JOIN properties pr2 ON pr2.photo_id=p2.id \
                WHERE pr2.key='location_name' AND pr2.value=pr.value \
                ORDER BY (p2.encoded != '') DESC, p2.created DESC LIMIT 1) AS rep_loc, \
            (SELECT p2.encoded FROM photo p2 JOIN properties pr2 ON pr2.photo_id=p2.id \
                WHERE pr2.key='location_name' AND pr2.value=pr.value \
                ORDER BY (p2.encoded != '') DESC, p2.created DESC LIMIT 1) AS rep_enc \
            FROM properties pr WHERE pr.key='location_name' \
            GROUP BY pr.value ORDER BY cnt DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok(LocationGroup {
                    name: row.get(0)?,
                    count: row.get(1)?,
                    photo_location: row.get(2).ok(),
                    encoded: row.get(3).ok(),
                })
            }) {
                for g in iter.flatten() {
                    groups.push(g);
                }
            }
        }
        groups
    }

    /// Day-level photo/video counts within a date range (inclusive, `YYYY-MM-DD`).
    pub fn get_day_counts(&self, from: &str, to: &str) -> Vec<DayCount> {
        let mut counts = Vec::new();
        let sql = format!(
            "SELECT substr(created, 1, 10) AS day, COUNT(*) AS cnt, \
             SUM(CASE WHEN {} THEN 1 ELSE 0 END) AS videos \
             FROM photo WHERE substr(created, 1, 10) BETWEEN ?1 AND ?2 \
             GROUP BY day ORDER BY day",
            video_sql_like()
        );
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map([from, to], |row| {
                Ok(DayCount {
                    date: row.get(0)?,
                    photos: row.get(1)?,
                    videos: row.get(2).unwrap_or(0),
                })
            }) {
                for d in iter.flatten() {
                    counts.push(d);
                }
            }
        }
        counts
    }

    /// Toggle the favorite status of a photo. Returns true if now favorited.
    pub fn toggle_favorite(&self, photo_id: &str) -> bool {
        let exists = self
            .connection
            .query_row(
                "SELECT 1 FROM properties WHERE photo_id = ?1 AND key = 'favorite'",
                [photo_id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if exists {
            let _ = self.connection.execute(
                "DELETE FROM properties WHERE photo_id = ?1 AND key = 'favorite'",
                [photo_id],
            );
            false
        } else {
            let _ = self.connection.execute(
                "INSERT INTO properties (photo_id, key, value) VALUES(?1, 'favorite', 'true')",
                [photo_id],
            );
            true
        }
    }

    /// Set the favorite flag for many photos in a single transaction.
    /// Returns the number of photos affected.
    pub fn set_favorites(&self, photo_ids: &[String], favorite: bool) -> usize {
        if photo_ids.is_empty() {
            return 0;
        }
        let placeholders: Vec<String> = (1..=photo_ids.len()).map(|i| format!("?{i}")).collect();
        let in_clause = placeholders.join(", ");
        let params: Vec<&dyn rusqlite::ToSql> = photo_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();
        let affected = if favorite {
            let sql = format!(
                "INSERT INTO properties (photo_id, key, value) \
                 SELECT id, 'favorite', 'true' FROM photo \
                 WHERE id IN ({in_clause}) \
                 AND NOT EXISTS (SELECT 1 FROM properties p2 \
                     WHERE p2.photo_id = photo.id AND p2.key = 'favorite')"
            );
            self.connection
                .execute(&sql, params.as_slice())
                .unwrap_or(0)
        } else {
            let sql = format!(
                "DELETE FROM properties WHERE key = 'favorite' AND photo_id IN ({in_clause})"
            );
            self.connection
                .execute(&sql, params.as_slice())
                .unwrap_or(0)
        };
        affected as usize
    }

    /// Get all photos that have non-zero GPS coordinates, for map heatmap display.
    pub fn get_heatmap_points(&self) -> Vec<MapPoint> {
        let mut points = Vec::new();
        let sql = "SELECT id, latitude, longitude, location FROM photo \
            WHERE (latitude != 0.0 OR longitude != 0.0)";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], |row| {
                Ok(MapPoint {
                    id: row.get(0)?,
                    latitude: row.get(1).unwrap_or(0.0),
                    longitude: row.get(2).unwrap_or(0.0),
                    location: row.get(3).unwrap_or_default(),
                })
            }) {
                for p in iter.flatten() {
                    points.push(p);
                }
            }
        }
        points
    }

    /// Fetch multiple photos by their IDs in a single query.
    pub fn get_photos_by_ids(&self, photo_ids: &[String]) -> Vec<Photo> {
        if photo_ids.is_empty() {
            return Vec::new();
        }
        let mut photos = Vec::new();
        let placeholders: Vec<String> = photo_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, p.caption, p.aesthetics_score, \
             s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres, p.sync_needed, p.received \
             FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = photo_ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(params.as_slice(), |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2)?,
                    created: row.get(5).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: row.get(3).unwrap_or(0.0),
                    longitude: row.get(4).unwrap_or(0.0),
                    favorite: row.get(6).unwrap_or(false),
                    indexed: row.get(7).unwrap_or(0),
                    caption: row.get(8).ok(),
                    aesthetics_score: row.get(9).ok(),
                    ai_status: AiStatus {
                        clip: row.get(10).unwrap_or(0),
                        face: row.get(11).unwrap_or(0),
                        ocr: row.get(12).unwrap_or(0),
                        nsfw: row.get(13).unwrap_or(0),
                        aesthetics: row.get(14).unwrap_or(0),
                        yolo: row.get(15).unwrap_or(0),
                        blip: row.get(16).unwrap_or(0),
                        arcface: row.get(17).unwrap_or(0),
                        midas: row.get(18).unwrap_or(0),
                        whisper: row.get(19).unwrap_or(0),
                        sam: row.get(20).unwrap_or(0),
                        superres: row.get(21).unwrap_or(0),
                    },
                    sync_needed: row.get(22).unwrap_or(false),
                    received: row.get(23).unwrap_or(false),
                })
            }) {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        self.enrich_objects(&mut photos);
        self.enrich_properties(&mut photos);
        photos
    }

    /// Fetch a single photo by its ID, with objects and properties populated.
    /// Concatenated OCR text for a photo (all detected text rows).
    pub fn get_photo_ocr(&self, photo_id: &str) -> String {
        let mut parts = Vec::new();
        if let Ok(mut stmt) = self
            .connection
            .prepare("SELECT text FROM ocr WHERE photo_id = ?1 ORDER BY rowid")
        {
            if let Ok(rows) = stmt.query_map([photo_id], |r| r.get::<_, String>(0)) {
                for row in rows.flatten() {
                    let t = row.trim();
                    if !t.is_empty() {
                        parts.push(t.to_string());
                    }
                }
            }
        }
        parts.join(" ")
    }

    pub fn get_photo_by_id(&self, photo_id: &str) -> Option<Photo> {
        let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, p.caption, p.aesthetics_score, \
             s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres, p.sync_needed, p.received \
             FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.id = ?1";
        let mut stmt = self.connection.prepare(sql).ok()?;
        let mut rows = stmt
            .query_map([photo_id], |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2)?,
                    created: row.get(5).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: row.get(3).unwrap_or(0.0),
                    longitude: row.get(4).unwrap_or(0.0),
                    favorite: row.get(6).unwrap_or(false),
                    indexed: row.get(7).unwrap_or(0),
                    caption: row.get(8).ok(),
                    aesthetics_score: row.get(9).ok(),
                    ai_status: AiStatus {
                        clip: row.get(10).unwrap_or(0),
                        face: row.get(11).unwrap_or(0),
                        ocr: row.get(12).unwrap_or(0),
                        nsfw: row.get(13).unwrap_or(0),
                        aesthetics: row.get(14).unwrap_or(0),
                        yolo: row.get(15).unwrap_or(0),
                        blip: row.get(16).unwrap_or(0),
                        arcface: row.get(17).unwrap_or(0),
                        midas: row.get(18).unwrap_or(0),
                        whisper: row.get(19).unwrap_or(0),
                        sam: row.get(20).unwrap_or(0),
                        superres: row.get(21).unwrap_or(0),
                    },
                    sync_needed: row.get(22).unwrap_or(false),
                    received: row.get(23).unwrap_or(false),
                })
            })
            .ok()?;
        let mut photo = rows.next()?.ok()?;
        self.enrich_objects(std::slice::from_mut(&mut photo));
        self.enrich_properties(std::slice::from_mut(&mut photo));
        Some(photo)
    }

    /// Slim photo row for the ML worker: only the fields inference needs
    /// (id, location, ai_status). Avoids loading base64 thumbnails, captions,
    /// aesthetics, objects and properties that the indexing path never reads.
    pub fn get_photo_for_indexing(&self, photo_id: &str) -> Option<Photo> {
        let sql = "SELECT p.id, p.location, p.created, \
             s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, \
             s.whisper, s.sam, s.superres \
             FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.id = ?1";
        let mut stmt = self.connection.prepare(sql).ok()?;
        let mut rows = stmt
            .query_map([photo_id], |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: String::new(),
                    created: row.get(2).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: 0.0,
                    longitude: 0.0,
                    favorite: false,
                    indexed: 0,
                    caption: None,
                    aesthetics_score: None,
                    ai_status: AiStatus {
                        clip: row.get(3).unwrap_or(0),
                        face: row.get(4).unwrap_or(0),
                        ocr: row.get(5).unwrap_or(0),
                        nsfw: row.get(6).unwrap_or(0),
                        aesthetics: row.get(7).unwrap_or(0),
                        yolo: row.get(8).unwrap_or(0),
                        blip: row.get(9).unwrap_or(0),
                        arcface: row.get(10).unwrap_or(0),
                        midas: row.get(11).unwrap_or(0),
                        whisper: row.get(12).unwrap_or(0),
                        sam: row.get(13).unwrap_or(0),
                        superres: row.get(14).unwrap_or(0),
                    },
                    sync_needed: false,
                    received: false,
                })
            })
            .ok()?;
        rows.next()?.ok()
    }

    /// Get base64-encoded thumbnails for a batch of photo IDs.
    pub fn get_photo_encoded_batch(&self, photo_ids: &[String]) -> HashMap<String, String> {
        let mut result = HashMap::new();
        if photo_ids.is_empty() {
            return result;
        }
        let placeholders: Vec<String> = photo_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT id, encoded FROM photo WHERE id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = photo_ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(params.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }) {
                for row in iter.flatten() {
                    result.insert(row.0, row.1);
                }
            }
        }
        result
    }

    /// Store or replace a detected face with its embedding and optional person assignment.
    pub fn store_face(&self, face: Face) {
        let embedding_bytes: Vec<u8> = face
            .embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        if let Err(e) = self.connection.execute("INSERT OR REPLACE INTO faces(photo_id, face_id, crop_path, encoded, embedding, person_id) VALUES(?1, ?2, ?3, ?4, ?5, ?6)", (&face.photo_id, &face.face_id, &face.crop_path, &face.encoded, &embedding_bytes, &face.person_id)) {
            tracing::warn!("store_face: failed to store face {}: {e}", face.face_id);
        }
    }

    /// List all named people with a representative face and face count.
    pub fn get_people(&self) -> Vec<PersonWithFace> {
        let mut people = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare("SELECT p.id, p.name, f.crop_path, f.face_id, f.encoded, p.embedding, (SELECT COUNT(*) FROM faces WHERE person_id = p.id) FROM people p LEFT JOIN faces f ON p.id = f.person_id WHERE p.name IS NOT NULL GROUP BY p.id") {
            if let Ok(iter) = stmt.query_map([], |row| {
                let embedding: Option<Vec<f32>> = row.get::<_, Option<Vec<u8>>>(5).ok().flatten().map(|bytes| bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
                Ok(PersonWithFace { id: row.get(0)?, name: row.get(1)?, representative_crop: row.get(2).ok(), representative_face_id: row.get(3).ok(), encoded: row.get(4).ok(), embedding, face_count: row.get(6)? })
            }) {
                for p in iter.flatten() { people.push(p); }
            }
        }
        people
    }

    /// Assign a name to a face, creating or merging people as needed. Returns the person ID.
    pub fn assign_name_to_face(&self, face_id: &str, name: &str) -> String {
        let existing_id: Option<String> = self
            .connection
            .query_row("SELECT id FROM people WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .ok();
        let current_person_id: Option<String> = self
            .connection
            .query_row(
                "SELECT person_id FROM faces WHERE face_id = ?1",
                [face_id],
                |row| row.get(0),
            )
            .ok();

        let target_id = match existing_id {
            Some(id) => {
                if let Some(anon_id) = current_person_id {
                    if anon_id != id {
                        let _ = self.connection.execute(
                            "UPDATE faces SET person_id = ?1 WHERE person_id = ?2",
                            (&id, &anon_id),
                        );
                        let _ = self
                            .connection
                            .execute("DELETE FROM people WHERE id = ?1", [&anon_id]);
                    }
                } else {
                    let _ = self.connection.execute(
                        "UPDATE faces SET person_id = ?1 WHERE face_id = ?2",
                        (&id, face_id),
                    );
                }
                id
            }
            None => {
                if let Some(id) = current_person_id {
                    let _ = self
                        .connection
                        .execute("UPDATE people SET name = ?1 WHERE id = ?2", (name, &id));
                    id
                } else {
                    let new_id = uuid::Uuid::new_v4().to_string();
                    let _ = self.connection.execute(
                        "INSERT INTO people (id, name) VALUES (?1, ?2)",
                        (&new_id, name),
                    );
                    let _ = self.connection.execute(
                        "UPDATE faces SET person_id = ?1 WHERE face_id = ?2",
                        (&new_id, face_id),
                    );
                    new_id
                }
            }
        };

        self.update_person_centroid(&target_id);
        target_id
    }

    /// Create an unnamed person record from a face embedding. Returns the new person ID.
    pub fn create_anonymous_person(&self, embedding: &[f32]) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let embedding_bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        let _ = self.connection.execute(
            "INSERT INTO people (id, name, embedding) VALUES (?1, NULL, ?2)",
            (&id, &embedding_bytes),
        );
        id
    }

    /// Recompute the average face embedding (centroid) for a person.
    pub fn update_person_centroid(&self, person_id: &str) {
        let mut embeddings = Vec::new();
        if let Ok(mut stmt) = self
            .connection
            .prepare("SELECT embedding FROM faces WHERE person_id = ?1")
        {
            if let Ok(rows) = stmt.query_map([person_id], |row| row.get::<_, Vec<u8>>(0)) {
                for row in rows.flatten() {
                    let emb: Vec<f32> = row
                        .chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    if emb.len() == 512 {
                        embeddings.push(emb);
                    }
                }
            }
        }

        if embeddings.is_empty() {
            return;
        }

        let count = embeddings.len() as f32;
        let mut centroid = vec![0.0f32; 512];
        for emb in embeddings {
            for (sum, value) in centroid.iter_mut().zip(emb.iter()).take(512) {
                *sum += value;
            }
        }
        for value in centroid.iter_mut().take(512) {
            *value /= count;
        }

        let norm: f32 = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in centroid.iter_mut() {
                *v /= norm;
            }
        }

        let centroid_bytes: Vec<u8> = centroid.iter().flat_map(|f| f.to_le_bytes()).collect();
        let _ = self.connection.execute(
            "UPDATE people SET embedding = ?1 WHERE id = ?2",
            (centroid_bytes, person_id),
        );
    }

    /// List all unnamed people grouped by face similarity, ordered by face count.
    pub fn get_anonymous_people_groups(&self) -> Vec<PersonWithFace> {
        let mut results = Vec::new();
        let sql = "SELECT p.id, f.crop_path, f.face_id, f.encoded, p.embedding, COUNT(*) \
            FROM people p JOIN faces f ON p.id = f.person_id \
            WHERE p.name IS NULL GROUP BY p.id ORDER BY COUNT(*) DESC";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], |row| {
                let embedding: Option<Vec<f32>> = row
                    .get::<_, Option<Vec<u8>>>(4)
                    .ok()
                    .flatten()
                    .map(|bytes| {
                        bytes
                            .chunks_exact(4)
                            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                            .collect()
                    });
                Ok(PersonWithFace {
                    id: row.get(0)?,
                    name: "Unnamed Person".to_string(),
                    representative_crop: row.get(1).ok(),
                    representative_face_id: row.get(2).ok(),
                    encoded: row.get(3).ok(),
                    embedding,
                    face_count: row.get(5)?,
                })
            }) {
                for p in iter.flatten() {
                    results.push(p);
                }
            }
        }
        results
    }

    /// Get all detected faces in a photo, with person names if assigned.
    pub fn get_faces_for_photo(&self, photo_id: &str) -> Vec<FaceWithPerson> {
        let mut faces = Vec::new();
        let sql = "SELECT f.photo_id, f.face_id, f.crop_path, f.encoded, f.person_id, p.name FROM faces f LEFT JOIN people p ON f.person_id = p.id WHERE f.photo_id = ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([photo_id], |row| {
                Ok(FaceWithPerson {
                    photo_id: row.get(0)?,
                    face_id: row.get(1)?,
                    crop_path: row.get(2)?,
                    encoded: row.get(3)?,
                    person_id: row.get(4)?,
                    person_name: row.get(5)?,
                })
            }) {
                for f in iter.flatten() {
                    faces.push(f);
                }
            }
        }
        faces
    }

    /// Get all face records belonging to a person.
    pub fn get_person_faces(&self, person_id: &str) -> Vec<Face> {
        let mut faces = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare("SELECT photo_id, face_id, crop_path, encoded, person_id FROM faces WHERE person_id = ?1") {
            if let Ok(iter) = stmt.query_map([person_id], |row| {
                Ok(Face {
                    photo_id: row.get(0)?,
                    face_id: row.get(1)?,
                    crop_path: row.get(2)?,
                    encoded: row.get(3)?,
                    embedding: Vec::new(), // Not needed for UI
                    person_id: row.get(4)?,
                })
            }) {
                for f in iter.flatten() { faces.push(f); }
            }
        }
        faces
    }

    /// Get all photos that contain a given person.
    pub fn get_photos_for_person(
        &self,
        person_id: &str,
        offset: usize,
        limit: usize,
    ) -> Vec<Photo> {
        let mut photos = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, p.caption, p.aesthetics_score, 
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres 
            FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id JOIN faces f ON p.id = f.photo_id WHERE f.person_id = ?1 GROUP BY p.id ORDER BY p.created DESC LIMIT ?2 OFFSET ?3";
        let params: [&dyn rusqlite::types::ToSql; 3] =
            [&person_id, &(limit as i64), &(offset as i64)];
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map(params.as_slice(), |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2)?,
                    created: row.get(5).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: row.get(3).unwrap_or(0.0),
                    longitude: row.get(4).unwrap_or(0.0),
                    favorite: row.get(6).unwrap_or(false),
                    indexed: row.get(7).unwrap_or(2),
                    caption: row.get(8).ok(),
                    aesthetics_score: row.get(9).ok(),
                    ai_status: AiStatus {
                        clip: row.get(10).unwrap_or(0),
                        face: row.get(11).unwrap_or(0),
                        ocr: row.get(12).unwrap_or(0),
                        nsfw: row.get(13).unwrap_or(0),
                        aesthetics: row.get(14).unwrap_or(0),
                        yolo: row.get(15).unwrap_or(0),
                        blip: row.get(16).unwrap_or(0),
                        arcface: row.get(17).unwrap_or(0),
                        midas: row.get(18).unwrap_or(0),
                        whisper: row.get(19).unwrap_or(0),
                        sam: row.get(20).unwrap_or(0),
                        superres: row.get(21).unwrap_or(0),
                    },
                    sync_needed: false,
                    received: false,
                })
            }) {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        photos
    }

    /// List all monitored directory paths.
    pub fn list_directories(&self) -> Vec<String> {
        let mut results = Vec::new();
        if let Ok(mut stm) = self.connection.prepare("SELECT name FROM directory") {
            if let Ok(iter) = stm.query_map((), |row| row.get(0)) {
                for val in iter.flatten() {
                    results.push(val);
                }
            }
        }
        results
    }

    /// Remove a monitored directory path from the config.
    pub fn remove_directory(&self, path: String) {
        if let Err(e) = self
            .connection
            .execute("DELETE FROM directory WHERE name = ?1", [&path])
        {
            tracing::warn!("remove_directory: failed to remove '{path}': {e}");
        }
    }

    /// Add a directory path to the monitored directories list.
    pub fn add_directory(&self, path: &str) {
        if let Err(e) = self
            .connection
            .execute("INSERT INTO directory (name) VALUES(?1)", [&path])
        {
            tracing::warn!("add_directory: failed to add '{path}': {e}");
        }
    }

    /// Mark onboarding as complete so the app does not show first-run setup again
    /// even when no scan directory was configured (e.g. an Android device that only
    /// receives photos over sync).
    pub fn set_onboarding_complete(&self) {
        let mut state = std::collections::HashMap::new();
        state.insert("onboarding_complete".to_string(), "1".to_string());
        self.set_state(state);
    }

    /// Whether onboarding has been completed.
    pub fn is_onboarding_complete(&self) -> bool {
        self.get_state()
            .get("onboarding_complete")
            .map(|v| v == "1")
            .unwrap_or(false)
    }

    /// Whether any photo exists in the library at all.
    pub fn has_any_photos(&self) -> bool {
        self.connection
            .query_row("SELECT EXISTS(SELECT 1 FROM photo)", [], |r| r.get(0))
            .unwrap_or(false)
    }

    /// Merge all faces from `from_id` into `to_id`, then delete the source person.
    pub fn merge_people(&self, from_id: &str, to_id: &str) {
        if let Err(e) = self.connection.execute(
            "UPDATE faces SET person_id = ?1 WHERE person_id = ?2",
            (to_id, from_id),
        ) {
            tracing::warn!("merge_people: failed to update faces from {from_id} to {to_id}: {e}");
            return;
        }
        if let Err(e) = self
            .connection
            .execute("DELETE FROM people WHERE id = ?1", [from_id])
        {
            tracing::warn!("merge_people: failed to delete {from_id}: {e}");
            return;
        }
        self.update_person_centroid(to_id);
    }

    /// Rename a person record.
    pub fn rename_person(&self, id: &str, new_name: &str) {
        if let Err(e) = self
            .connection
            .execute("UPDATE people SET name = ?1 WHERE id = ?2", (new_name, id))
        {
            tracing::warn!("rename_person: failed to rename {id} to {new_name}: {e}");
        }
    }

    /// Remove a directory and all its photos, objects, faces, and properties in a transaction.
    pub fn remove_directory_full(&mut self, path: &str) {
        let tx = match self.connection.transaction() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("remove_directory_full: failed to start transaction: {e}");
                return;
            }
        };

        let mut photo_ids = Vec::new();
        if let Ok(mut stmt) = tx.prepare("SELECT id FROM photo WHERE location LIKE ?1") {
            if let Ok(rows) = stmt.query_map([format!("{path}%")], |row| row.get::<_, String>(0)) {
                for id in rows.flatten() {
                    photo_ids.push(id);
                }
            }
        }
        for id in &photo_ids {
            let _ = tx.execute("DELETE FROM object WHERE photo_id = ?1", [&id]);
            let _ = tx.execute("DELETE FROM faces WHERE photo_id = ?1", [&id]);
            let _ = tx.execute("DELETE FROM properties WHERE photo_id = ?1", [&id]);
            let _ = tx.execute("DELETE FROM photo WHERE id = ?1", [&id]);
        }
        let _ = tx.execute("DELETE FROM directory WHERE name = ?1", [path]);
        if let Err(e) = tx.commit() {
            tracing::warn!("remove_directory_full: failed to commit: {e}");
        }
    }

    /// Delete rows whose media file no longer exists on disk (immediate prune).
    /// Only rows rooted at `dir` are considered. Removing the row drops the file
    /// from the sync manifest, so the next manifest exchange re-requests and
    /// restores it from any peer that still holds a copy. Returns the number of
    /// rows removed.
    pub fn prune_missing_files(&mut self, dir: &str) -> usize {
        let mut gone: Vec<String> = Vec::new();
        let prefix = format!("{dir}{}", std::path::MAIN_SEPARATOR);
        if let Ok(mut stmt) = self
            .connection
            .prepare("SELECT id, location FROM photo WHERE location = ?1 OR location LIKE ?2")
        {
            if let Ok(rows) = stmt.query_map(rusqlite::params![dir, format!("{prefix}%")], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }) {
                for r in rows.flatten() {
                    if !std::path::Path::new(&r.1).exists() {
                        gone.push(r.0);
                    }
                }
            }
        }
        if gone.is_empty() {
            return 0;
        }
        let tx = match self.connection.transaction() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("prune_missing_files: failed to start transaction: {e}");
                return 0;
            }
        };
        let mut removed = 0;
        for id in &gone {
            let _ = tx.execute("DELETE FROM object WHERE photo_id = ?1", [id]);
            let _ = tx.execute("DELETE FROM faces WHERE photo_id = ?1", [id]);
            let _ = tx.execute("DELETE FROM properties WHERE photo_id = ?1", [id]);
            if tx
                .execute("DELETE FROM photo WHERE id = ?1", [id])
                .map(|n| n > 0)
                .unwrap_or(false)
            {
                removed += 1;
            }
        }
        if let Err(e) = tx.commit() {
            tracing::warn!("prune_missing_files: failed to commit: {e}");
        }
        removed
    }

    /// Import a photo with its objects and faces within a transaction (for device sync).
    pub fn import_photo(&mut self, photo: ImportedPhoto<'_>) {
        let tx = match self.connection.transaction() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("import_photo: failed to start transaction: {e}");
                return;
            }
        };

        if let Err(e) = tx.execute(
            "INSERT OR REPLACE INTO photo (id, location, created, latitude, longitude, encoded, caption, aesthetics_score, received) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (photo.id, photo.location, photo.created, photo.latitude, photo.longitude, photo.encoded, photo.caption, photo.aesthetics_score, photo.received),
        ) {
            tracing::warn!("import_photo: failed to upsert photo {}: {e}", photo.id);
            return;
        }

        if let Err(e) = tx.execute("DELETE FROM object WHERE photo_id = ?1", [photo.id]) {
            tracing::warn!(
                "import_photo: failed to clear objects for {}: {e}",
                photo.id
            );
        }
        if let Err(e) = tx.execute("DELETE FROM faces WHERE photo_id = ?1", [photo.id]) {
            tracing::warn!("import_photo: failed to clear faces for {}: {e}", photo.id);
        }

        if let Ok(objects) = serde_json::from_str::<Vec<SyncObject>>(photo.objects_json) {
            for obj in &objects {
                if let Err(e) = tx.execute(
                    "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                    (photo.id, &obj.class, &obj.probability),
                ) {
                    tracing::warn!(
                        "import_photo: failed to insert object for {}: {e}",
                        photo.id
                    );
                }
            }
        }

        if let Ok(faces) = serde_json::from_str::<Vec<SyncFace>>(photo.faces_json) {
            for face in &faces {
                if let Err(e) = tx.execute(
                    "INSERT OR REPLACE INTO faces (photo_id, face_id, crop_path, encoded, person_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (photo.id, &face.face_id, &face.crop_path, &face.encoded, &face.person_id),
                ) {
                    tracing::warn!("import_photo: failed to insert face for {}: {e}", photo.id);
                }
            }
        }

        if let Err(e) = tx.commit() {
            tracing::warn!(
                "import_photo: failed to commit transaction for {}: {e}",
                photo.id
            );
        }
    }

    /// Return (photo_count, video_count) based on file extensions.
    pub fn get_media_counts(&self) -> (i64, i64) {
        let not_video = video_sql_not_like();
        let is_video = video_sql_like();
        let photo_count: i64 = self
            .connection
            .query_row(
                &format!("SELECT COUNT(*) FROM photo WHERE {not_video}"),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let video_count: i64 = self
            .connection
            .query_row(
                &format!("SELECT COUNT(*) FROM photo WHERE {is_video}"),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        (photo_count, video_count)
    }

    /// Return (pending_photo_count, pending_video_count) of items that still need
    /// to be backed up to another device (originals not yet received by a peer).
    pub fn get_pending_sync_counts(&self) -> (i64, i64) {
        let not_video = video_sql_not_like();
        let is_video = video_sql_like();
        let photo_count: i64 = self
            .connection
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM photo WHERE {not_video} AND sync_needed = 1 AND received = 0"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let video_count: i64 = self
            .connection
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM photo WHERE {is_video} AND sync_needed = 1 AND received = 0"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        (photo_count, video_count)
    }
    pub fn get_all_people_with_embeddings(&self) -> Vec<(String, Vec<f32>)> {
        let mut results = Vec::new();
        if let Ok(mut stmt) = self
            .connection
            .prepare("SELECT id, embedding FROM people WHERE embedding IS NOT NULL")
        {
            if let Ok(iter) = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let bytes: Vec<u8> = row.get(1)?;
                let emb: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                Ok((id, emb))
            }) {
                for p in iter.flatten() {
                    results.push(p);
                }
            }
        }
        results
    }

    /// Batch-insert photos with their properties, ignoring duplicates.
    pub fn store_photo_batch(&mut self, photos: &[Photo]) -> Result<(), String> {
        let tx = self.connection.transaction().map_err(|e| e.to_string())?;
        {
            let mut stmt = tx.prepare_cached("INSERT OR IGNORE INTO photo(id, location, encoded, created, latitude, longitude, indexed, sync_needed) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 1, 1)").map_err(|e| e.to_string())?;
            for p in photos {
                let _ = stmt.execute((
                    &p.id,
                    &p.location,
                    &p.encoded,
                    &p.created,
                    &p.latitude,
                    &p.longitude,
                ));
            }
        }
        {
            let mut prop_stmt = tx
                .prepare_cached(
                    "INSERT OR REPLACE INTO properties(photo_id, key, value) VALUES(?1, ?2, ?3)",
                )
                .map_err(|e| e.to_string())?;
            for p in photos {
                for (key, value) in &p.properties {
                    let _ = prop_stmt.execute((&p.id, key, value));
                }
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }

    /// Set the indexed level for a photo (0=new, 1=metadata, 2=fully processed).
    pub fn update_photo_indexed(&self, id: &str, indexed: i32) {
        if let Err(e) = self
            .connection
            .execute("UPDATE photo SET indexed = ?1 WHERE id = ?2", (indexed, id))
        {
            tracing::warn!("update_photo_indexed: failed for {id}: {e}");
        }
    }

    /// Whether the photo already has a stored thumbnail (non-empty `encoded`).
    pub fn has_thumbnail(&self, id: &str) -> bool {
        self.connection
            .query_row("SELECT encoded FROM photo WHERE id = ?1", [id], |r| {
                r.get::<_, String>(0)
            })
            .map(|encoded| !encoded.is_empty())
            .unwrap_or(false)
    }

    /// Store a generated thumbnail for a photo. Only writes when the photo has no
    /// thumbnail yet. Returns true if the thumbnail was actually stored.
    pub fn update_photo_thumbnail(&self, id: &str, encoded: &str) -> bool {
        self.connection
            .execute(
                "UPDATE photo SET encoded = ?1 WHERE id = ?2 AND (encoded IS NULL OR encoded = '')",
                (encoded, id),
            )
            .map(|affected| affected > 0)
            .unwrap_or(false)
    }

    /// Update caption, aesthetics_score, and indexed level for a photo.
    pub fn clear_sync_needed(&self, id: &str) {
        if let Err(e) = self
            .connection
            .execute("UPDATE photo SET sync_needed = 0 WHERE id = ?1", [id])
        {
            tracing::warn!("clear_sync_needed: failed for {id}: {e}");
        }
    }

    pub fn update_photo_metadata(
        &self,
        id: &str,
        caption: Option<&str>,
        aesthetics_score: Option<f64>,
        indexed: i32,
    ) {
        if let Err(e) = self.connection.execute(
            "UPDATE photo SET caption = ?1, aesthetics_score = ?2, indexed = ?3 WHERE id = ?4",
            (caption, aesthetics_score, indexed, id),
        ) {
            tracing::warn!("update_photo_metadata: failed for {id}: {e}");
        }
    }

    /// Record that a specific ML model has processed a photo.
    pub fn update_ai_status(&self, photo_id: &str, model: &str, status: i32) {
        match model {
            "clip" | "face" | "ocr" | "nsfw" | "aesthetics" | "yolo" | "blip" | "arcface"
            | "midas" | "whisper" | "sam" | "superres" => {}
            _ => {
                tracing::warn!("Invalid model name: {model}");
                return;
            }
        }
        let sql = format!("INSERT INTO ai_status (photo_id, {model}) VALUES (?1, ?2) ON CONFLICT(photo_id) DO UPDATE SET {model} = ?2");
        let _ = self.connection.execute(&sql, (photo_id, status));
    }

    /// Get up to 50 photos that have not yet been fully AI-processed.
    pub fn get_unindexed_photos(&self) -> Vec<Photo> {
        let mut photos = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, p.indexed, p.caption, p.aesthetics_score, 
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres 
            FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.indexed < 2 LIMIT 50";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2)?,
                    created: row.get(5).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: row.get(3).unwrap_or(0.0),
                    longitude: row.get(4).unwrap_or(0.0),
                    favorite: false,
                    indexed: row.get(6).unwrap_or(0),
                    caption: row.get(7).ok(),
                    aesthetics_score: row.get(8).ok(),
                    ai_status: AiStatus {
                        clip: row.get(9).unwrap_or(0),
                        face: row.get(10).unwrap_or(0),
                        ocr: row.get(11).unwrap_or(0),
                        nsfw: row.get(12).unwrap_or(0),
                        aesthetics: row.get(13).unwrap_or(0),
                        yolo: row.get(14).unwrap_or(0),
                        blip: row.get(15).unwrap_or(0),
                        arcface: row.get(16).unwrap_or(0),
                        midas: row.get(17).unwrap_or(0),
                        whisper: row.get(18).unwrap_or(0),
                        sam: row.get(19).unwrap_or(0),
                        superres: row.get(20).unwrap_or(0),
                    },
                    sync_needed: false,
                    received: false,
                })
            }) {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        photos
    }

    /// Get a paginated batch of unindexed photos.
    pub fn get_unindexed_photos_batch(&self, offset: usize, limit: usize) -> Vec<Photo> {
        let mut photos = Vec::new();
        let sql = "SELECT p.id, p.location, p.latitude, p.longitude, p.created, p.indexed, 
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres 
            FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.indexed < 2 LIMIT ?1 OFFSET ?2";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) =
                stmt.query_map(rusqlite::params![limit as i64, offset as i64], |row| {
                    Ok(Photo {
                        id: row.get(0)?,
                        location: row.get(1)?,
                        encoded: String::new(),
                        created: row.get(4).unwrap_or_default(),
                        objects: HashMap::new(),
                        properties: HashMap::new(),
                        latitude: row.get(2).unwrap_or(0.0),
                        longitude: row.get(3).unwrap_or(0.0),
                        favorite: false,
                        indexed: row.get(5).unwrap_or(0),
                        caption: None,
                        aesthetics_score: None,
                        ai_status: AiStatus {
                            clip: row.get(6).unwrap_or(0),
                            face: row.get(7).unwrap_or(0),
                            ocr: row.get(8).unwrap_or(0),
                            nsfw: row.get(9).unwrap_or(0),
                            aesthetics: row.get(10).unwrap_or(0),
                            yolo: row.get(11).unwrap_or(0),
                            blip: row.get(12).unwrap_or(0),
                            arcface: row.get(13).unwrap_or(0),
                            midas: row.get(14).unwrap_or(0),
                            whisper: row.get(15).unwrap_or(0),
                            sam: row.get(16).unwrap_or(0),
                            superres: row.get(17).unwrap_or(0),
                        },
                        sync_needed: false,
                        received: false,
                    })
                })
            {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        photos
    }

    /// Get photo IDs that have not been processed by the given model.
    pub fn get_photos_missing_model(&self, model: &str) -> Vec<String> {
        let mut ids = Vec::new();
        match model {
            "clip" | "face" | "ocr" | "nsfw" | "aesthetics" | "yolo" | "blip" | "arcface"
            | "midas" | "whisper" | "sam" | "superres" => {}
            _ => {
                tracing::warn!("Invalid model name: {model}");
                return ids;
            }
        }
        let sql = format!("SELECT p.id FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE s.{model} = 0 OR s.{model} IS NULL");
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for id in iter.flatten() {
                    ids.push(id);
                }
            }
        }
        ids
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
pub struct AiStatus {
    pub clip: i32,
    pub face: i32,
    pub ocr: i32,
    pub nsfw: i32,
    pub aesthetics: i32,
    pub yolo: i32,
    pub blip: i32,
    pub arcface: i32,
    pub midas: i32,
    pub whisper: i32,
    pub sam: i32,
    pub superres: i32,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Photo {
    pub id: String,
    pub location: String,
    pub encoded: String,
    pub created: String,
    pub objects: HashMap<String, f64>,
    pub properties: HashMap<String, String>,
    pub latitude: f64,
    pub longitude: f64,
    pub favorite: bool,
    pub indexed: i32, // 0: new, 1: metadata only, 2: fully processed
    pub caption: Option<String>,
    pub aesthetics_score: Option<f64>,
    pub ai_status: AiStatus,
    /// True until any peer has received the file (drives the "not backed up" badge).
    #[serde(default)]
    pub sync_needed: bool,
    /// True when this row was imported from a peer (a backup copy).
    #[serde(default)]
    pub received: bool,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct MapPoint {
    pub id: String,
    pub latitude: f64,
    pub longitude: f64,
    pub location: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Face {
    pub photo_id: String,
    pub face_id: String,
    pub crop_path: String,
    pub encoded: String,
    pub embedding: Vec<f32>,
    pub person_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonWithFace {
    pub id: String,
    pub name: String,
    pub representative_crop: Option<String>,
    pub representative_face_id: Option<String>,
    pub encoded: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub face_count: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub up_to_date: bool,
    pub host: bool,
    pub photo_count: i64,
    pub video_count: i64,
    pub os: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDevice {
    pub device_id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub device_type: String,
    pub os: String,
    pub models_enabled: Vec<String>,
    pub protocol_version: u8,
    pub storage_used: i64,
    pub storage_capacity: i64,
    pub last_seen: String,
    /// Media received from this peer (running total).
    pub photo_count: i64,
    pub video_count: i64,
}

impl Database {
    pub fn upsert_peer_device(&self, device: &PeerDevice) {
        let _ = self.connection.execute(
            "INSERT INTO peer_device(device_id, name, ip, port, device_type, os, models_enabled, protocol_version, storage_used, storage_capacity, last_seen, photo_count, video_count) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'), ?11, ?12) \
             ON CONFLICT(device_id) DO UPDATE SET \
             name=excluded.name, ip=excluded.ip, port=excluded.port, \
             device_type=excluded.device_type, os=excluded.os, \
             models_enabled=excluded.models_enabled, protocol_version=excluded.protocol_version, \
             storage_used=excluded.storage_used, storage_capacity=excluded.storage_capacity, \
             photo_count=excluded.photo_count, video_count=excluded.video_count, \
             last_seen=datetime('now')",
            rusqlite::params![
                device.device_id, device.name, device.ip, device.port as i32,
                device.device_type, device.os,
                serde_json::to_string(&device.models_enabled).unwrap_or_else(|_| "[]".to_string()),
                device.protocol_version as i32, device.storage_used, device.storage_capacity,
                device.photo_count, device.video_count,
            ],
        );
    }

    pub fn update_peer_device_seen(&self, device_id: &str) {
        let _ = self.connection.execute(
            "UPDATE peer_device SET last_seen = datetime('now') WHERE device_id = ?1",
            rusqlite::params![device_id],
        );
    }

    pub fn update_peer_device_storage(&self, device_id: &str, used: i64, capacity: i64) {
        let _ = self.connection.execute(
            "UPDATE peer_device SET storage_used = ?2, storage_capacity = ?3 WHERE device_id = ?1",
            rusqlite::params![device_id, used, capacity],
        );
    }

    pub fn list_peer_devices(&self) -> Vec<PeerDevice> {
        let mut results = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare(
            "SELECT device_id, name, ip, port, device_type, os, models_enabled, protocol_version, storage_used, storage_capacity, last_seen, photo_count, video_count \
             FROM peer_device ORDER BY last_seen DESC"
        ) {
            if let Ok(iter) = stmt.query_map([], |row| {
                let models_str: String = row.get(6).unwrap_or_else(|_| "[]".to_string());
                let models: Vec<String> = serde_json::from_str(&models_str).unwrap_or_default();
                Ok(PeerDevice {
                    device_id: row.get(0)?,
                    name: row.get(1)?,
                    ip: row.get(2)?,
                    port: row.get::<_, i32>(3).unwrap_or(0) as u16,
                    device_type: row.get(4)?,
                    os: row.get(5)?,
                    models_enabled: models,
                    protocol_version: row.get::<_, i32>(7).unwrap_or(1) as u8,
                    storage_used: row.get(8)?,
                    storage_capacity: row.get(9)?,
                    last_seen: row.get(10)?,
                    photo_count: row.get(11)?,
                    video_count: row.get(12)?,
                })
            }) {
                for d in iter.flatten() {
                    results.push(d);
                }
            }
        }
        results
    }

    pub fn get_peer_device(&self, device_id: &str) -> Option<PeerDevice> {
        self.connection.query_row(
            "SELECT device_id, name, ip, port, device_type, os, models_enabled, protocol_version, storage_used, storage_capacity, last_seen, photo_count, video_count \
             FROM peer_device WHERE device_id = ?1",
            rusqlite::params![device_id],
            |row| {
                let models_str: String = row.get(6).unwrap_or_else(|_| "[]".to_string());
                let models: Vec<String> = serde_json::from_str(&models_str).unwrap_or_default();
                Ok(PeerDevice {
                    device_id: row.get(0)?,
                    name: row.get(1)?,
                    ip: row.get(2)?,
                    port: row.get::<_, i32>(3).unwrap_or(0) as u16,
                    device_type: row.get(4)?,
                    os: row.get(5)?,
                    models_enabled: models,
                    protocol_version: row.get::<_, i32>(7).unwrap_or(1) as u8,
                    storage_used: row.get(8)?,
                    storage_capacity: row.get(9)?,
                    last_seen: row.get(10)?,
                    photo_count: row.get(11)?,
                    video_count: row.get(12)?,
                })
            },
        ).ok()
    }

    /// Rename a peer device by its id. Leaves every other field untouched.
    pub fn rename_peer_device(&self, device_id: &str, new_name: &str) {
        let _ = self.connection.execute(
            "UPDATE peer_device SET name = ?2 WHERE device_id = ?1",
            rusqlite::params![device_id, new_name],
        );
    }

    /// Add to a peer's running photo/video totals.
    pub fn increment_peer_device_counts(&self, device_id: &str, photos: i64, videos: i64) {
        let _ = self.connection.execute(
            "UPDATE peer_device SET photo_count = photo_count + ?2, video_count = video_count + ?3 WHERE device_id = ?1",
            rusqlite::params![device_id, photos, videos],
        );
    }

    pub fn remove_peer_device(&self, device_id: &str) {
        let _ = self.connection.execute(
            "DELETE FROM peer_device WHERE device_id = ?1",
            rusqlite::params![device_id],
        );
    }

    // ---------- Albums (local, free tier) ----------

    /// Create a new empty album and return the persisted row.
    pub fn create_album(&self, name: &str) -> Result<Album, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Album name cannot be empty".to_string());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let sort_order: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM album",
                [],
                |r| r.get(0),
            )
            .unwrap_or(1);
        self.connection
            .execute(
                "INSERT INTO album(id, name, sort_order) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, name, sort_order],
            )
            .map_err(|e| e.to_string())?;
        self.get_album(&id)
            .ok_or_else(|| "Failed to create album".to_string())
    }

    /// Rename an existing album. Rejects blank names.
    pub fn rename_album(&self, album_id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Album name cannot be empty".to_string());
        }
        self.connection
            .execute(
                "UPDATE album SET name = ?1 WHERE id = ?2",
                rusqlite::params![name, album_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Delete an album and all of its items. Deleting an auto-detected trip
    /// dismisses it so it does not reappear on the next trip sync.
    pub fn delete_album(&self, album_id: &str) -> Result<(), String> {
        if let Ok(kind) = self.connection.query_row(
            "SELECT kind FROM album WHERE id = ?1",
            rusqlite::params![album_id],
            |r| r.get::<_, String>(0),
        ) {
            if AlbumKind::parse(&kind) == AlbumKind::Trip {
                let _ = self.connection.execute(
                    "INSERT OR IGNORE INTO dismissed_trip(id) VALUES (?1)",
                    rusqlite::params![album_id],
                );
            }
        }
        self.connection
            .execute(
                "DELETE FROM album_item WHERE album_id = ?1",
                rusqlite::params![album_id],
            )
            .map_err(|e| e.to_string())?;
        self.connection
            .execute(
                "DELETE FROM album WHERE id = ?1",
                rusqlite::params![album_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Forget every dismissed trip so previously deleted trips are re-detected
    /// on the next sync. Returns the number of dismissals removed.
    pub fn clear_dismissed_trips(&self) -> i64 {
        self.connection
            .execute("DELETE FROM dismissed_trip", ())
            .map(|n| n as i64)
            .unwrap_or(0)
    }

    fn album_from_row(row: &rusqlite::Row) -> rusqlite::Result<Album> {
        Ok(Album {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            cover_photo_id: row.get(3)?,
            sort_order: row.get(4)?,
            item_count: row.get(5)?,
            kind: AlbumKind::parse(&row.get::<_, String>(6).unwrap_or_default()),
            rule: row.get(7)?,
            updated_at: row.get(8)?,
        })
    }

    /// Parse a stored smart-album rule into a filter, or None if it is unset
    /// or malformed.
    fn album_rule_filter(album: &Album) -> Option<PhotoFilter> {
        if album.kind == AlbumKind::Manual {
            return None;
        }
        let rule = album.rule.as_deref()?;
        serde_json::from_str(rule).ok()
    }

    /// Fill in live counts and covers for smart/trip albums (which are computed
    /// from their rule rather than stored album_item rows).
    fn resolve_album_metrics(&self, albums: &mut [Album]) {
        for album in albums {
            if album.kind == AlbumKind::Manual {
                continue;
            }
            if let Some(filter) = Self::album_rule_filter(album) {
                let query = filter.query.as_deref().unwrap_or("");
                let videos = filter.videos.unwrap_or(false);
                album.item_count = self.count_photos_filtered(query, false, videos, &filter);
                if album.cover_photo_id.is_none() {
                    let mut cover_filter = filter.clone();
                    cover_filter.order_by = Some("best".to_string());
                    let first =
                        self.list_photos_filtered(query, 0, 1, false, videos, &cover_filter, false);
                    if let Some(photo) = first.first() {
                        album.cover_photo_id = Some(photo.id.clone());
                    }
                }
            }
        }
    }

    /// List all albums ordered by sort_order, with live item counts.
    pub fn list_albums(&self) -> Vec<Album> {
        let sql = "SELECT a.id, a.name, a.created_at, a.cover_photo_id, a.sort_order, \
            CASE WHEN a.kind IN ('smart','trip') THEN 0 \
            ELSE (SELECT COUNT(*) FROM album_item WHERE album_id = a.id) END AS item_count, \
            a.kind, a.rule, a.updated_at \
            FROM album a ORDER BY a.sort_order ASC, a.created_at ASC";
        let mut albums = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], Self::album_from_row) {
                for album in iter.flatten() {
                    albums.push(album);
                }
            }
        }
        self.resolve_album_metrics(&mut albums);
        albums
    }

    /// Fetch a single album by id, or None if it does not exist.
    pub fn get_album(&self, album_id: &str) -> Option<Album> {
        let sql = "SELECT a.id, a.name, a.created_at, a.cover_photo_id, a.sort_order, \
            CASE WHEN a.kind IN ('smart','trip') THEN 0 \
            ELSE (SELECT COUNT(*) FROM album_item WHERE album_id = a.id) END AS item_count, \
            a.kind, a.rule, a.updated_at \
            FROM album a WHERE a.id = ?1";
        let mut album = self
            .connection
            .query_row(sql, rusqlite::params![album_id], Self::album_from_row)
            .ok();
        if let Some(ref mut album) = album {
            let slice = std::slice::from_mut(album);
            self.resolve_album_metrics(slice);
        }
        album
    }

    /// Create a rule-based album. `kind` must be Smart or Trip (Manual albums
    /// go through [`Self::create_album`]). The rule is stored as JSON and its
    /// membership is computed on demand.
    pub fn create_smart_album(
        &self,
        name: &str,
        rule: &PhotoFilter,
        kind: AlbumKind,
    ) -> Result<Album, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Album name cannot be empty".to_string());
        }
        if kind == AlbumKind::Manual {
            return self.create_album(name);
        }
        let rule_json = serde_json::to_string(rule).map_err(|e| e.to_string())?;
        let id = uuid::Uuid::new_v4().to_string();
        let sort_order: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM album",
                [],
                |r| r.get(0),
            )
            .unwrap_or(1);
        self.connection
            .execute(
                "INSERT INTO album(id, name, sort_order, kind, rule, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
                rusqlite::params![id, name, sort_order, kind.as_str(), rule_json],
            )
            .map_err(|e| e.to_string())?;
        self.get_album(&id)
            .ok_or_else(|| "Failed to create album".to_string())
    }

    /// Replace a smart/trip album's rule, bumping its updated_at.
    pub fn update_smart_album_rule(
        &self,
        album_id: &str,
        rule: &PhotoFilter,
    ) -> Result<(), String> {
        let rule_json = serde_json::to_string(rule).map_err(|e| e.to_string())?;
        self.connection
            .execute(
                "UPDATE album SET rule = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![rule_json, album_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_albums_by_kind(&self, kind: AlbumKind) -> Vec<Album> {
        let sql = "SELECT a.id, a.name, a.created_at, a.cover_photo_id, a.sort_order, \
            CASE WHEN a.kind IN ('smart','trip') THEN 0 \
            ELSE (SELECT COUNT(*) FROM album_item WHERE album_id = a.id) END AS item_count, \
            a.kind, a.rule, a.updated_at \
            FROM album a WHERE a.kind = ?1 \
            ORDER BY a.sort_order ASC, a.created_at ASC";
        let mut albums = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map(rusqlite::params![kind.as_str()], Self::album_from_row)
            {
                for album in iter.flatten() {
                    albums.push(album);
                }
            }
        }
        self.resolve_album_metrics(&mut albums);
        albums
    }

    /// Data-URL thumbnail for an album's cover photo, if any.
    fn album_cover_encoded(&self, album: &Album) -> Option<String> {
        let cover = album.cover_photo_id.as_deref()?;
        self.connection
            .query_row(
                "SELECT encoded FROM photo WHERE id = ?1",
                rusqlite::params![cover],
                |r| r.get::<_, String>(0),
            )
            .ok()
    }

    /// Turn one time-clustered group of photos into a trip: a display name, a
    /// `from|to` date range, and the photo ids. Returns None for clusters that
    /// are too small (fewer than `TRIP_MIN_PHOTOS`).
    fn finalize_trip(&self, acc: &TripAcc) -> Option<(String, String, Vec<String>)> {
        if acc.photos.len() < TRIP_MIN_PHOTOS {
            return None;
        }
        let date_from = day_index_to_date(acc.first_day)?;
        let date_to = day_index_to_date(acc.last_day)?;
        let year = &date_from[..4];
        let name = format!("{} · {year}", acc.display_name());
        Some((name, format!("{date_from}|{date_to}"), acc.photos.clone()))
    }

    /// Recompute trips from photo timestamps and upsert them as `trip` albums
    /// under stable signatures (so re-scanning never duplicates a trip and
    /// manual edits to a trip's name survive). Returns the number of trips
    /// synced. Gaps of more than `TRIP_GAP_DAYS` between consecutive photos
    /// split trips; adjacent clusters that revisit the same country within
    /// `TRIP_MERGE_DAYS` are then folded into a single trip (Rome → Florence
    /// → Venice shows as one "Italy" trip). Clusters need at least
    /// `TRIP_MIN_PHOTOS` photos (even on a single day, e.g. a day trip).
    pub fn sync_trips(&self) -> i64 {
        let mut rows: Vec<(String, i64, String)> = Vec::new();
        let sql = "SELECT p.id, p.created, COALESCE( \
            (SELECT value FROM properties WHERE photo_id = p.id AND key = 'location_name' LIMIT 1), '') \
            FROM photo p WHERE p.created IS NOT NULL AND p.created != ''";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], |r| {
                let created: String = r.get(1)?;
                Ok((
                    r.get::<_, String>(0)?,
                    date_day_index(&created).unwrap_or(i64::MIN),
                    r.get::<_, String>(2).unwrap_or_default(),
                ))
            }) {
                for row in iter.flatten() {
                    if row.1 != i64::MIN {
                        rows.push(row);
                    }
                }
            }
        }
        rows.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

        // Stage 1: cluster photos into time runs separated by large gaps.
        let mut time_clusters: Vec<Vec<(String, i64, String)>> = Vec::new();
        let mut current: Vec<(String, i64, String)> = Vec::new();
        let mut prev_day: Option<i64> = None;
        for row in rows {
            if let Some(prev) = prev_day {
                if row.1 - prev > TRIP_GAP_DAYS {
                    time_clusters.push(std::mem::take(&mut current));
                }
            }
            prev_day = Some(row.1);
            current.push(row);
        }
        if !current.is_empty() {
            time_clusters.push(current);
        }

        // Stage 2: fold adjacent clusters back together when they revisit the
        // same country within a short window (one trip spanning several cities).
        let mut merged: Vec<TripAcc> = Vec::new();
        for cluster in time_clusters {
            let acc = TripAcc::from_cluster(&cluster);
            if let Some(prev) = merged.last_mut() {
                if acc.first_day - prev.last_day <= TRIP_MERGE_DAYS
                    && prev.shares_country_with(&acc)
                {
                    prev.merge(acc);
                    continue;
                }
            }
            merged.push(acc);
        }

        let mut trip_ids: Vec<String> = Vec::new();
        for acc in merged {
            let Some((name, range, _photo_ids)) = self.finalize_trip(&acc) else {
                continue;
            };
            let (date_from, date_to) = match range.split_once('|') {
                Some((f, t)) => (f.to_string(), t.to_string()),
                _ => continue,
            };
            let id = format!("trip:{date_from}:{date_to}");
            // Skip trips the user dismissed.
            let dismissed: bool = self
                .connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM dismissed_trip WHERE id = ?1)",
                    rusqlite::params![id],
                    |r| r.get(0),
                )
                .unwrap_or(false);
            if dismissed {
                continue;
            }
            trip_ids.push(id.clone());
            let rule = PhotoFilter {
                date_from: Some(date_from),
                // Inclusive end: `p.created <= ?` compares the full timestamp,
                // so append a late time to cover the whole final day.
                date_to: Some(format!("{date_to} 23:59:59")),
                order_by: Some("oldest".to_string()),
                ..PhotoFilter::default()
            };
            let rule_json = serde_json::to_string(&rule).unwrap_or_default();
            let _ = self.connection.execute(
                // `name` is only set on insert so a user rename survives resyncs.
                "INSERT INTO album(id, name, sort_order, kind, rule, created_at, updated_at) \
                 VALUES (?1, ?2, 1000000, 'trip', ?3, datetime('now'), datetime('now')) \
                 ON CONFLICT(id) DO UPDATE SET \
                 rule = excluded.rule, updated_at = datetime('now')",
                rusqlite::params![id, name, rule_json],
            );
        }

        // Drop trip rows whose signature no longer matches a current cluster.
        if trip_ids.is_empty() {
            let _ = self
                .connection
                .execute("DELETE FROM album WHERE kind = 'trip'", ());
        } else {
            let placeholders: Vec<String> = (1..=trip_ids.len()).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "DELETE FROM album WHERE kind = 'trip' AND id NOT IN ({})",
                placeholders.join(",")
            );
            let _ = self
                .connection
                .execute(&sql, rusqlite::params_from_iter(trip_ids.iter()));
        }
        trip_ids.len() as i64
    }

    /// Full data for the Albums view: People, Places, Trips, Smart albums, and
    /// manual albums, in that order.
    pub fn get_album_sections(&self) -> Vec<AlbumSection> {
        let mut sections = Vec::new();

        let people: Vec<AlbumSectionItem> = self
            .get_search_people(100)
            .into_iter()
            .map(|p| AlbumSectionItem {
                id: format!("person:{}", p.id),
                name: p.name,
                count: p.photo_count,
                cover_encoded: p.encoded,
                cover_crop: p.representative_crop,
                kind: "person".to_string(),
                album: None,
            })
            .collect();
        if !people.is_empty() {
            sections.push(AlbumSection {
                id: "people".to_string(),
                items: people,
            });
        }

        self.sync_trips();
        let mut trips: Vec<AlbumSectionItem> = self
            .list_albums_by_kind(AlbumKind::Trip)
            .into_iter()
            .map(|album| AlbumSectionItem {
                id: album.id.clone(),
                name: album.name.clone(),
                count: album.item_count,
                cover_encoded: self.album_cover_encoded(&album),
                cover_crop: None,
                kind: "trip".to_string(),
                album: Some(album),
            })
            .collect();
        // Surface trips that fall around this time of year (newest first on
        // ties) instead of always showing the oldest first.
        trips.sort_by_key(|item| trip_sort_key(&item.id));
        if !trips.is_empty() {
            sections.push(AlbumSection {
                id: "trips".to_string(),
                items: trips,
            });
        }

        let smart: Vec<AlbumSectionItem> = self
            .list_albums_by_kind(AlbumKind::Smart)
            .into_iter()
            .map(|album| AlbumSectionItem {
                id: album.id.clone(),
                name: album.name.clone(),
                count: album.item_count,
                cover_encoded: self.album_cover_encoded(&album),
                cover_crop: None,
                kind: "smart".to_string(),
                album: Some(album),
            })
            .collect();
        if !smart.is_empty() {
            sections.push(AlbumSection {
                id: "smart".to_string(),
                items: smart,
            });
        }

        let manual: Vec<AlbumSectionItem> = self
            .list_albums_by_kind(AlbumKind::Manual)
            .into_iter()
            .map(|album| AlbumSectionItem {
                id: album.id.clone(),
                name: album.name.clone(),
                count: album.item_count,
                cover_encoded: self.album_cover_encoded(&album),
                cover_crop: None,
                kind: "manual".to_string(),
                album: Some(album),
            })
            .collect();
        if !manual.is_empty() {
            sections.push(AlbumSection {
                id: "albums".to_string(),
                items: manual,
            });
        }

        sections
    }

    /// Add photos to an album, appending them in position order. Duplicates are
    /// ignored (album_item PK is (album_id, photo_id)). The cover is updated to
    /// the last photo actually added.
    pub fn add_album_items(&self, album_id: &str, photo_ids: &[String]) -> Result<(), String> {
        let album_exists: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM album WHERE id = ?1)",
                rusqlite::params![album_id],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if !album_exists {
            return Err(format!("Album '{album_id}' does not exist"));
        }
        if let Ok(kind) = self.connection.query_row(
            "SELECT kind FROM album WHERE id = ?1",
            rusqlite::params![album_id],
            |r| r.get::<_, String>(0),
        ) {
            if AlbumKind::parse(&kind) != AlbumKind::Manual {
                return Err("Smart and trip albums are managed automatically".to_string());
            }
        }
        let mut max_position: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(position), 0) FROM album_item WHERE album_id = ?1",
                rusqlite::params![album_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let mut last_added: Option<String> = None;
        for photo_id in photo_ids {
            let inserted = self
                .connection
                .execute(
                    "INSERT OR IGNORE INTO album_item(album_id, photo_id, position) VALUES (?1, ?2, ?3)",
                    rusqlite::params![album_id, photo_id, max_position],
                )
                .map_err(|e| e.to_string())?;
            if inserted > 0 {
                max_position += 1;
                last_added = Some(photo_id.clone());
            }
        }
        if let Some(ref cover) = last_added {
            let _ = self.connection.execute(
                "UPDATE album SET cover_photo_id = ?1 WHERE id = ?2",
                rusqlite::params![cover, album_id],
            );
        }
        Ok(())
    }

    /// Remove photos from an album. If the current cover photo is removed, the
    /// most recently added remaining photo becomes the new cover.
    pub fn remove_album_items(&self, album_id: &str, photo_ids: &[String]) -> Result<(), String> {
        for photo_id in photo_ids {
            self.connection
                .execute(
                    "DELETE FROM album_item WHERE album_id = ?1 AND photo_id = ?2",
                    rusqlite::params![album_id, photo_id],
                )
                .map_err(|e| e.to_string())?;
        }
        let current_cover: Option<String> = self
            .connection
            .query_row(
                "SELECT cover_photo_id FROM album WHERE id = ?1",
                rusqlite::params![album_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        if let Some(ref cover) = current_cover {
            if photo_ids.contains(cover) {
                let new_cover: Option<String> = self
                    .connection
                    .query_row(
                        "SELECT photo_id FROM album_item WHERE album_id = ?1 \
                         ORDER BY position DESC, added_at DESC LIMIT 1",
                        rusqlite::params![album_id],
                        |r| r.get(0),
                    )
                    .ok()
                    .flatten();
                let _ = self.connection.execute(
                    "UPDATE album SET cover_photo_id = ?1 WHERE id = ?2",
                    rusqlite::params![new_cover, album_id],
                );
            }
        }
        Ok(())
    }

    /// Reorder photos within an album. `ordered_ids` should contain the album's
    /// photo ids in the new order; ids not present in the album are ignored.
    pub fn reorder_album(&self, album_id: &str, ordered_ids: &[String]) -> Result<(), String> {
        for (position, photo_id) in ordered_ids.iter().enumerate() {
            self.connection
                .execute(
                    "UPDATE album_item SET position = ?1 WHERE album_id = ?2 AND photo_id = ?3",
                    rusqlite::params![position as i64, album_id, photo_id],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Paginated album contents. Manual albums use their stored item order;
    /// smart/trip albums compute membership from their rule on the fly.
    pub fn get_album_contents(&self, album_id: &str, offset: usize, limit: usize) -> Vec<Photo> {
        let Some(album) = self.get_album(album_id) else {
            return Vec::new();
        };
        if album.kind != AlbumKind::Manual {
            if let Some(filter) = Self::album_rule_filter(&album) {
                let query = filter.query.as_deref().unwrap_or("");
                let videos = filter.videos.unwrap_or(false);
                return self
                    .list_photos_filtered(query, offset, limit, false, videos, &filter, true);
            }
            return Vec::new();
        }
        let mut photos = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, \
            EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, \
            p.caption, p.aesthetics_score, \
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, \
            s.whisper, s.sam, s.superres, p.sync_needed, p.received \
            FROM photo p \
            JOIN album_item ai ON ai.photo_id = p.id \
            LEFT JOIN ai_status s ON p.id = s.photo_id \
            WHERE ai.album_id = ?3 \
            ORDER BY ai.position ASC, ai.added_at DESC \
            LIMIT ?1, ?2";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map(
                rusqlite::params![offset as i64, limit as i64, album_id],
                |row| {
                    Ok(Photo {
                        id: row.get(0)?,
                        location: row.get(1)?,
                        encoded: row.get(2)?,
                        created: row.get(5).unwrap_or_default(),
                        objects: HashMap::new(),
                        properties: HashMap::new(),
                        latitude: row.get(3).unwrap_or(0.0),
                        longitude: row.get(4).unwrap_or(0.0),
                        favorite: row.get(6).unwrap_or(false),
                        indexed: row.get(7).unwrap_or(0),
                        caption: row.get(8).ok(),
                        aesthetics_score: row.get(9).ok(),
                        ai_status: AiStatus {
                            clip: row.get(10).unwrap_or(0),
                            face: row.get(11).unwrap_or(0),
                            ocr: row.get(12).unwrap_or(0),
                            nsfw: row.get(13).unwrap_or(0),
                            aesthetics: row.get(14).unwrap_or(0),
                            yolo: row.get(15).unwrap_or(0),
                            blip: row.get(16).unwrap_or(0),
                            arcface: row.get(17).unwrap_or(0),
                            midas: row.get(18).unwrap_or(0),
                            whisper: row.get(19).unwrap_or(0),
                            sam: row.get(20).unwrap_or(0),
                            superres: row.get(21).unwrap_or(0),
                        },
                        sync_needed: row.get(22).unwrap_or(false),
                        received: row.get(23).unwrap_or(false),
                    })
                },
            ) {
                for photo in iter.flatten() {
                    photos.push(photo);
                }
            }
        }
        self.enrich_objects(&mut photos);
        self.enrich_properties(&mut photos);
        photos
    }

    /// Number of items currently in an album (0 if the album does not exist).
    pub fn album_item_count(&self, album_id: &str) -> i64 {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM album_item WHERE album_id = ?1",
                rusqlite::params![album_id],
                |r| r.get(0),
            )
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};
    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_db() -> Database {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("siegu_test_{}_{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        Database::new(&dir.display().to_string())
    }

    #[test]
    fn test_database_creation() {
        let db = test_db();
        let (photos, videos) = db.get_media_counts();
        assert!(photos >= 0);
        assert!(videos >= 0);
    }

    #[test]
    fn test_sync_trips_merges_same_country_clusters() {
        let db = test_db();
        let insert = |id: &str, created: &str, location: &str| {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, '')",
                    (id, format!("/{id}.jpg"), created),
                )
                .unwrap();
            db.connection
                .execute(
                    "INSERT INTO properties (photo_id, key, value) VALUES (?1, 'location_name', ?2)",
                    (id, location),
                )
                .unwrap();
        };
        // Rome June 1–3, then Florence June 8–10: 5-day gap, same country.
        for (i, day) in [1, 2, 3].iter().enumerate() {
            insert(
                &format!("r{i}"),
                &format!("2023-06-{day:02} 10:00:00"),
                "Rome, Italy",
            );
        }
        for (i, day) in [8, 9, 10].iter().enumerate() {
            insert(
                &format!("f{i}"),
                &format!("2023-06-{day:02} 10:00:00"),
                "Florence, Italy",
            );
        }
        // Tokyo months later: a separate trip.
        for (i, day) in [1, 2, 3].iter().enumerate() {
            insert(
                &format!("t{i}"),
                &format!("2023-09-{day:02} 10:00:00"),
                "Tokyo, Japan",
            );
        }

        assert_eq!(db.sync_trips(), 2);
        let trips = db.list_albums_by_kind(AlbumKind::Trip);
        let mut names: Vec<&str> = trips.iter().map(|a| a.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["Italy · 2023", "Tokyo, Japan · 2023"]);
        let italy = trips.iter().find(|a| a.name == "Italy · 2023").unwrap();
        assert_eq!(italy.item_count, 6);
    }

    #[test]
    fn test_sync_trips_keeps_different_countries_separate() {
        let db = test_db();
        let insert = |id: &str, created: &str, location: &str| {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, '')",
                    (id, format!("/{id}.jpg"), created),
                )
                .unwrap();
            db.connection
                .execute(
                    "INSERT INTO properties (photo_id, key, value) VALUES (?1, 'location_name', ?2)",
                    (id, location),
                )
                .unwrap();
        };
        for (i, day) in [1, 2, 3].iter().enumerate() {
            insert(
                &format!("r{i}"),
                &format!("2023-06-{day:02} 10:00:00"),
                "Rome, Italy",
            );
        }
        // Same window, different country: must stay a separate trip.
        for (i, day) in [8, 9, 10].iter().enumerate() {
            insert(
                &format!("p{i}"),
                &format!("2023-06-{day:02} 10:00:00"),
                "Paris, France",
            );
        }

        assert_eq!(db.sync_trips(), 2);
        let trips = db.list_albums_by_kind(AlbumKind::Trip);
        let mut names: Vec<&str> = trips.iter().map(|a| a.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["Paris, France · 2023", "Rome, Italy · 2023"]);
    }

    #[test]
    fn test_trip_cover_prefers_highest_aesthetics() {
        let db = test_db();
        for (id, aesthetics) in [("p1", 0.2f64), ("p2", 0.5), ("p3", 0.9)] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded, aesthetics_score) \
                     VALUES (?1, ?2, ?3, '', ?4)",
                    (id, format!("/{id}.jpg"), "2023-06-01 10:00:00", aesthetics),
                )
                .unwrap();
            db.connection
                .execute(
                    "INSERT INTO properties (photo_id, key, value) VALUES (?1, 'location_name', ?2)",
                    (id, "Rome, Italy"),
                )
                .unwrap();
        }

        assert_eq!(db.sync_trips(), 1);
        let trips = db.list_albums_by_kind(AlbumKind::Trip);
        assert_eq!(trips.len(), 1);
        assert_eq!(trips[0].cover_photo_id.as_deref(), Some("p3"));
    }

    #[test]
    fn test_sync_trips_dismissed_trip_is_skipped() {
        let db = test_db();
        for (id, day) in [("d1", 1), ("d2", 2), ("d3", 3)] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, '')",
                    (
                        id,
                        format!("/{id}.jpg"),
                        format!("2023-06-{day:02} 10:00:00"),
                    ),
                )
                .unwrap();
        }

        assert_eq!(db.sync_trips(), 1);
        let id = db.list_albums_by_kind(AlbumKind::Trip)[0].id.clone();
        db.connection
            .execute("INSERT INTO dismissed_trip (id) VALUES (?1)", [&id])
            .unwrap();
        assert_eq!(db.sync_trips(), 0);
        assert!(db.list_albums_by_kind(AlbumKind::Trip).is_empty());
    }

    #[test]
    fn test_sync_trips_names_trip_without_location() {
        let db = test_db();
        for (id, day) in [("n1", 1), ("n2", 2), ("n3", 3)] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, '')",
                    (
                        id,
                        format!("/{id}.jpg"),
                        format!("2023-06-{day:02} 10:00:00"),
                    ),
                )
                .unwrap();
        }

        assert_eq!(db.sync_trips(), 1);
        let trips = db.list_albums_by_kind(AlbumKind::Trip);
        assert_eq!(trips.len(), 1);
        assert_eq!(trips[0].name, "Trip · 2023");
    }

    #[test]
    fn test_state_set_get() {
        let db = test_db();
        let mut state = HashMap::new();
        state.insert("theme".to_string(), "dark".to_string());
        state.insert("tier".to_string(), "paid".to_string());
        db.set_state(state);

        let config = db.get_state();
        assert_eq!(config.get("theme").unwrap(), "dark");
        assert_eq!(config.get("tier").unwrap(), "paid");
    }

    #[test]
    fn test_state_overwrite() {
        let db = test_db();
        let mut state = HashMap::new();
        state.insert("theme".to_string(), "dark".to_string());
        db.set_state(state);

        let mut state2 = HashMap::new();
        state2.insert("theme".to_string(), "light".to_string());
        db.set_state(state2);

        let config = db.get_state();
        assert_eq!(config.get("theme").unwrap(), "light");
    }

    #[test]
    fn test_add_list_remove_directory() {
        let db = test_db();
        db.add_directory("/home/test/photos");
        db.add_directory("/home/test/videos");

        let dirs = db.list_directories();
        assert!(dirs.contains(&"/home/test/photos".to_string()));
        assert!(dirs.contains(&"/home/test/videos".to_string()));

        db.remove_directory("/home/test/photos".to_string());
        let dirs = db.list_directories();
        assert!(!dirs.contains(&"/home/test/photos".to_string()));
        assert!(dirs.contains(&"/home/test/videos".to_string()));
    }

    #[test]
    fn test_check_integrity() {
        let db = test_db();
        assert!(db.check_integrity().unwrap_or(false));
    }

    #[test]
    fn test_toggle_favorite() {
        let mut db = test_db();
        let photo = Photo {
            id: "test_fav_1".to_string(),
            location: "/tmp/test_fav.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[photo]);

        let result = db.toggle_favorite("test_fav_1");
        assert!(result);
        let p = db.get_photo_by_id("test_fav_1").unwrap();
        assert!(p.favorite);

        let result = db.toggle_favorite("test_fav_1");
        assert!(!result);
        let p = db.get_photo_by_id("test_fav_1").unwrap();
        assert!(!p.favorite);
    }

    #[test]
    fn test_store_and_get_photo_properties() {
        let mut db = test_db();
        let mut props = HashMap::new();
        props.insert("Make".to_string(), "Apple".to_string());
        props.insert("Model".to_string(), "iPhone 15".to_string());

        let photo = Photo {
            id: "test_props_1".to_string(),
            location: "/tmp/test_props.jpg".to_string(),
            encoded: String::new(),
            created: "2024-06-01".to_string(),
            objects: HashMap::new(),
            properties: props,
            latitude: 40.7128,
            longitude: -74.0060,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[photo]);

        let p = db.get_photo_by_id("test_props_1").unwrap();
        assert_eq!(p.latitude, 40.7128);
        assert_eq!(p.longitude, -74.0060);
    }

    #[test]
    fn test_get_heatmap_points() {
        let mut db = test_db();
        let photo = Photo {
            id: "test_heat_1".to_string(),
            location: "/tmp/test_heat.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 52.3676,
            longitude: 4.9041,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[photo]);
        let points = db.get_heatmap_points();
        assert!(points.iter().any(|p| p.id == "test_heat_1"));
    }

    #[test]
    fn test_peer_device_upsert_and_list() {
        let db = test_db();
        db.upsert_peer_device(&PeerDevice {
            device_id: "test-id".to_string(),
            name: "test-device".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 0,
            device_type: String::new(),
            os: "android".to_string(),
            models_enabled: vec![],
            protocol_version: 1,
            storage_used: 0,
            storage_capacity: 0,
            last_seen: String::new(),
            photo_count: 0,
            video_count: 0,
        });
        let peers = db.list_peer_devices();
        assert!(peers.iter().any(|p| p.name == "test-device"));
    }

    #[test]
    fn test_peer_device_rename_and_counts() {
        let db = test_db();
        db.upsert_peer_device(&PeerDevice {
            device_id: "test-id".to_string(),
            name: "old-name".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 0,
            device_type: String::new(),
            os: "android".to_string(),
            models_enabled: vec![],
            protocol_version: 1,
            storage_used: 0,
            storage_capacity: 0,
            last_seen: String::new(),
            photo_count: 0,
            video_count: 0,
        });

        db.rename_peer_device("test-id", "new-name");
        let peer = db.get_peer_device("test-id").expect("peer exists");
        assert_eq!(peer.name, "new-name");
        assert_eq!(peer.os, "android");

        db.increment_peer_device_counts("test-id", 3, 2);
        let peer = db.get_peer_device("test-id").expect("peer exists");
        assert_eq!(peer.photo_count, 3);
        assert_eq!(peer.video_count, 2);

        db.increment_peer_device_counts("test-id", 1, 0);
        let peer = db.get_peer_device("test-id").expect("peer exists");
        assert_eq!(peer.photo_count, 4);
        assert_eq!(peer.video_count, 2);
    }

    #[test]
    fn test_is_video_path() {
        assert!(super::is_video_path("/DCIM/video.mp4"));
        assert!(super::is_video_path("/clip.MOV"));
        assert!(!super::is_video_path("/DCIM/photo.jpg"));
        assert!(!super::is_video_path("/photo.JPEG"));
    }

    #[test]
    fn test_last_scan_time() {
        let db = test_db();
        assert!(db.get_last_scan_time().is_none());

        db.set_last_scan_time("2024-06-01T12:00:00Z".to_string());
        assert_eq!(db.get_last_scan_time().unwrap(), "2024-06-01T12:00:00Z");
    }

    #[test]
    fn test_update_photo_indexed() {
        let mut db = test_db();
        let photo = Photo {
            id: "idx_1".to_string(),
            location: "/tmp/test_idx.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[photo]);

        db.update_photo_indexed("idx_1", 2);
        let loaded = db.get_photo_by_id("idx_1").expect("exists");
        assert_eq!(loaded.indexed, 2);
    }

    #[test]
    fn test_update_ai_status_and_missing_model() {
        let mut db = test_db();
        let photo = Photo {
            id: "ai_1".to_string(),
            location: "/tmp/test_ai.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[photo]);

        let missing = db.get_photos_missing_model("clip");
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "ai_1");

        db.update_ai_status("ai_1", "clip", 1);
        let missing = db.get_photos_missing_model("clip");
        assert!(missing.is_empty());
    }

    #[test]
    fn test_get_unindexed_photos() {
        let mut db = test_db();
        let p1 = Photo {
            id: "unidx_1".to_string(),
            location: "/tmp/test_unidx1.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let p2 = Photo {
            id: "unidx_2".to_string(),
            location: "/tmp/test_unidx2.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 2,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[p1, p2]);

        db.update_photo_indexed("unidx_1", 0);
        db.update_photo_indexed("unidx_2", 2);

        let unindexed = db.get_unindexed_photos();
        assert_eq!(unindexed.len(), 1);
        assert_eq!(unindexed[0].id, "unidx_1");
    }

    #[test]
    fn test_get_unindexed_photos_batch() {
        let mut db = test_db();
        let photos: Vec<Photo> = (0..10)
            .map(|i| Photo {
                id: format!("batch_{i}"),
                location: format!("/tmp/test_batch_{i}.jpg"),
                encoded: String::new(),
                created: "2024-01-01".to_string(),
                objects: HashMap::new(),
                properties: HashMap::new(),
                latitude: 0.0,
                longitude: 0.0,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: AiStatus::default(),
                sync_needed: true,
                received: false,
            })
            .collect();
        let _ = db.store_photo_batch(&photos);

        let batch1 = db.get_unindexed_photos_batch(0, 3);
        assert_eq!(batch1.len(), 3);

        let batch2 = db.get_unindexed_photos_batch(3, 3);
        assert_eq!(batch2.len(), 3);

        let batch_end = db.get_unindexed_photos_batch(9, 3);
        assert_eq!(batch_end.len(), 1);

        let batch_past = db.get_unindexed_photos_batch(20, 3);
        assert!(batch_past.is_empty());
    }

    #[test]
    fn test_update_ai_status_invalid_model() {
        let db = test_db();
        db.update_ai_status("photo_1", "invalid_model", 1);
    }

    #[test]
    fn test_video_sql_like_contains_all_extensions() {
        let sql = super::video_sql_like();
        for ext in super::scanner::VIDEO_EXTENSIONS {
            assert!(sql.contains(ext), "SQL should contain extension: {ext}");
        }
    }

    #[test]
    fn test_video_sql_not_like() {
        let sql = super::video_sql_not_like();
        assert!(sql.starts_with("NOT ("), "should start with NOT (");
        assert!(sql.ends_with(')'), "should end with )");
    }

    #[test]
    fn test_get_media_counts_includes_all_video_extensions() {
        let mut db = test_db();
        let extensions = [
            "mp4", "mkv", "mov", "avi", "webm", "flv", "wmv", "m4v", "3gp",
        ];
        let mut photos: Vec<Photo> = extensions
            .iter()
            .enumerate()
            .map(|(i, ext)| Photo {
                id: format!("vid_{i}"),
                location: format!("/tmp/video_{i}.{ext}"),
                encoded: String::new(),
                created: String::new(),
                objects: HashMap::new(),
                properties: HashMap::new(),
                latitude: 0.0,
                longitude: 0.0,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: AiStatus::default(),
                sync_needed: true,
                received: false,
            })
            .collect();
        photos.push(Photo {
            id: "img_1".to_string(),
            location: "/tmp/photo.jpg".to_string(),
            encoded: String::new(),
            created: String::new(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        });
        let _ = db.store_photo_batch(&photos);

        let (photo_count, video_count) = db.get_media_counts();
        assert_eq!(
            video_count, 9,
            "should count all 9 video extensions, got {video_count}"
        );
        assert_eq!(photo_count, 1, "should count 1 photo, got {photo_count}");
    }

    #[test]
    fn test_get_pending_sync_counts_filters_received() {
        let mut db = test_db();
        let photos: Vec<Photo> = (0..6)
            .map(|i| Photo {
                id: format!("pending_{i}"),
                location: if i % 2 == 0 {
                    format!("/tmp/p_{i}.jpg")
                } else {
                    format!("/tmp/p_{i}.mp4")
                },
                encoded: String::new(),
                created: String::new(),
                objects: HashMap::new(),
                properties: HashMap::new(),
                latitude: 0.0,
                longitude: 0.0,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: AiStatus::default(),
                sync_needed: true,
                received: false,
            })
            .collect();
        let _ = db.store_photo_batch(&photos);
        db.import_photo(ImportedPhoto {
            id: "pending_0",
            location: "/tmp/p_0.jpg",
            created: "2024-01-01",
            latitude: None,
            longitude: None,
            objects_json: "[]",
            faces_json: "[]",
            encoded: "",
            caption: None,
            aesthetics_score: None,
            received: true,
        });
        db.clear_sync_needed("pending_1");

        let (pending_photos, pending_videos) = db.get_pending_sync_counts();
        assert_eq!(pending_photos, 2, "should count 2 pending photos");
        assert_eq!(pending_videos, 2, "should count 2 pending videos");
    }

    #[test]
    fn test_list_photos_videos_only() {
        let mut db = test_db();
        let photos: Vec<Photo> = (0..5)
            .map(|i| {
                let ext = if i % 2 == 0 { "mp4" } else { "jpg" };
                Photo {
                    id: format!("media_{i}"),
                    location: format!("/tmp/file_{i}.{ext}"),
                    encoded: String::new(),
                    created: String::new(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: 0.0,
                    longitude: 0.0,
                    favorite: false,
                    indexed: 0,
                    caption: None,
                    aesthetics_score: None,
                    ai_status: AiStatus::default(),
                    sync_needed: true,
                    received: false,
                }
            })
            .collect();
        let _ = db.store_photo_batch(&photos);

        let all = db.list_photos("", 0, 100, false, false);
        assert_eq!(all.len(), 5);

        let videos = db.list_photos("", 0, 100, false, true);
        assert_eq!(videos.len(), 3, "should return only mp4 files");
        for v in &videos {
            assert!(v.location.ends_with(".mp4"), "expected mp4: {}", v.location);
        }
    }

    #[test]
    fn test_list_photos_facet_filters() {
        let db = test_db();
        db.connection
            .execute(
                "INSERT INTO people (id, name) VALUES ('person-1', 'Alice')",
                (),
            )
            .unwrap();
        for (id, created) in [
            ("p1", "2026-03-05"),
            ("p2", "2026-02-10"),
            ("p3", "2025-12-20"),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, '')",
                    (id, format!("/{id}.jpg"), created),
                )
                .unwrap();
        }
        db.connection
            .execute(
                "INSERT INTO faces (photo_id, face_id, crop_path, encoded, person_id) VALUES ('p1', 'f1', '', 'enc', 'person-1')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p2', 'beach', '0.9')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'location_name', 'Paris, France')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p3', 'location_name', 'Tokyo, Japan')",
                (),
            )
            .unwrap();

        let filter = |f: PhotoFilter| db.list_photos_filtered("", 0, 100, false, false, &f, true);

        let by_person = filter(PhotoFilter {
            person_ids: vec!["person-1".into()],
            ..Default::default()
        });
        assert_eq!(by_person.len(), 1);
        assert_eq!(by_person[0].id, "p1");

        let by_location = filter(PhotoFilter {
            location: Some("Tokyo, Japan".into()),
            ..Default::default()
        });
        assert_eq!(by_location.len(), 1);
        assert_eq!(by_location[0].id, "p3");

        let by_tag = filter(PhotoFilter {
            tag: Some("beach".into()),
            ..Default::default()
        });
        assert_eq!(by_tag.len(), 1);
        assert_eq!(by_tag[0].id, "p2");

        let by_date = filter(PhotoFilter {
            date_from: Some("2026-01-01".into()),
            date_to: Some("2026-03-31".into()),
            ..Default::default()
        });
        assert_eq!(by_date.len(), 2);

        let combined = filter(PhotoFilter {
            person_ids: vec!["person-1".into()],
            location: Some("Paris, France".into()),
            ..Default::default()
        });
        assert_eq!(combined.len(), 1);

        let empty = filter(PhotoFilter {
            person_ids: vec!["person-1".into()],
            tag: Some("beach".into()),
            ..Default::default()
        });
        assert!(empty.is_empty());
    }

    #[test]
    fn test_search_facet_counts() {
        let db = test_db();
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES ('p1', '/a.jpg', '2026-03-01', '')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES ('p2', '/b.jpg', '2026-03-15', '')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES ('p3', '/c.mp4', '2026-02-01', '')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'location_name', 'Paris, France')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p2', 'location_name', 'Paris, France')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'favorite', 'true')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p1', 'cat', '0.9')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p2', 'cat', '0.8')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO ocr (photo_id, text) VALUES ('p2', 'hello')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO people (id, name) VALUES ('person-1', 'Alice')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO faces (photo_id, face_id, person_id) VALUES ('p1', 'f1', 'person-1')",
                (),
            )
            .unwrap();

        let locations = db.get_location_counts(10);
        assert_eq!(locations, vec![("Paris, France".to_string(), 2)]);

        let tags = db.get_tag_counts(10);
        assert_eq!(tags, vec![("cat".to_string(), 2)]);

        let months = db.get_month_counts(10);
        assert_eq!(
            months,
            vec![("2026-03".to_string(), 2), ("2026-02".to_string(), 1)]
        );

        let people = db.get_search_people(10);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].name, "Alice");
        assert_eq!(people[0].photo_count, 1);

        let stats = db.get_search_stats();
        assert_eq!(stats.photos, 3);
        assert_eq!(stats.videos, 1);
        assert_eq!(stats.favorites, 1);
        assert_eq!(stats.ocr_photos, 1);
        assert_eq!(stats.faces, 1);
        assert_eq!(stats.named_people, 1);
        assert_eq!(stats.face_photos, 1);
    }

    #[test]
    fn test_get_photo_ocr_concatenates_rows() {
        let db = test_db();
        let _ = db.connection.execute(
            "INSERT INTO photo (id, location, created, encoded) VALUES ('p1', '/a.jpg', '2026-01-01', '')",
            (),
        );
        let _ = db.connection.execute(
            "INSERT INTO ocr (photo_id, text) VALUES ('p1', 'hello'), ('p1', 'world')",
            (),
        );
        let result = db.get_photo_ocr("p1");
        assert_eq!(result, "hello world");
        assert_eq!(db.get_photo_ocr("missing"), "");
    }

    #[test]
    fn test_discovery_photo_rails_and_groups() {
        let db = test_db();
        for (id, created, aesthetics) in [
            ("p1", "2026-03-01", Some(0.9)),
            ("p2", "2026-02-10", Some(0.5)),
            ("p3", "2026-01-05", None),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded, aesthetics_score) VALUES (?1, ?2, ?3, '', ?4)",
                    (id, format!("/{id}.jpg"), created, aesthetics),
                )
                .unwrap();
        }
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p2', 'favorite', 'true')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'location_name', 'Paris, France')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'Make', 'Apple')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'Model', 'iPhone 15 Pro')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p1', 'a receipt', '0.9')",
                (),
            )
            .unwrap();

        let best = db.get_best_photos(10);
        assert_eq!(best.len(), 2);
        assert_eq!(best[0].id, "p1");
        assert_eq!(best[0].aesthetics_score, Some(0.9));

        let favs = db.get_favorite_photos(10);
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].id, "p2");
        assert!(favs[0].favorite);

        let recent = db.get_recent_photos(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].id, "p1");

        let papers = db.get_paper_counts(10);
        assert_eq!(papers, vec![("a receipt".to_string(), 1)]);

        let cameras = db.get_camera_counts(10);
        assert_eq!(cameras, vec![("apple".to_string(), 1)]);

        let groups = db.get_location_groups(10);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Paris, France");
        assert_eq!(groups[0].count, 1);
    }

    #[test]
    fn test_get_best_photos_one_per_day() {
        let db = test_db();
        for (id, created, aesthetics) in [
            ("d1a", "2026-03-10 09:00:00", Some(0.7)),
            ("d1b", "2026-03-10 18:00:00", Some(0.95)),
            ("d1c", "2026-03-10 12:00:00", Some(0.8)),
            ("d2", "2026-03-09", Some(0.5)),
            ("d3", "2026-03-08", None),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded, aesthetics_score) VALUES (?1, ?2, ?3, '', ?4)",
                    (id, format!("/{id}.jpg"), created, aesthetics),
                )
                .unwrap();
        }

        let best = db.get_best_photos(10);
        assert_eq!(
            best.len(),
            2,
            "one best shot per day, skipping days without scores"
        );
        assert_eq!(
            best[0].id, "d1b",
            "best-scored photo of the most recent day"
        );
        assert_eq!(best[0].aesthetics_score, Some(0.95));
        assert_eq!(best[1].id, "d2");
    }

    #[test]
    fn test_get_day_counts() {
        let db = test_db();
        for (id, created, location) in [
            ("p1", "2026-03-01T10:00:00", "/a.jpg"),
            ("p2", "2026-03-01T18:00:00", "/b.jpg"),
            ("v1", "2026-03-02T09:00:00", "/c.MP4"),
            ("p3", "2026-04-10T09:00:00", "/d.jpg"),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, '')",
                    (id, location, created),
                )
                .unwrap();
        }

        let counts = db.get_day_counts("2026-03-01", "2026-03-31");
        assert_eq!(counts.len(), 2);
        assert_eq!(counts[0].date, "2026-03-01");
        assert_eq!(counts[0].photos, 2);
        assert_eq!(counts[0].videos, 0);
        assert_eq!(counts[1].date, "2026-03-02");
        assert_eq!(counts[1].photos, 1);
        assert_eq!(counts[1].videos, 1);

        let empty = db.get_day_counts("2025-01-01", "2025-01-31");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_list_photos_new_facet_filters() {
        let db = test_db();
        for (id, created, aesthetics) in [
            ("p1", "2026-03-01", Some(0.9)),
            ("p2", "2026-02-10", Some(0.4)),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded, aesthetics_score) VALUES (?1, ?2, ?3, '', ?4)",
                    (id, format!("/{id}.jpg"), created, aesthetics),
                )
                .unwrap();
        }
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'favorite', 'true')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'Make', 'Sony')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO faces (photo_id, face_id) VALUES ('p1', 'f1')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p1', 'a screenshot', '0.9')",
                (),
            )
            .unwrap();

        let filter = |f: PhotoFilter| db.list_photos_filtered("", 0, 100, false, false, &f, true);

        let favs = filter(PhotoFilter {
            favorite: true,
            ..Default::default()
        });
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].id, "p1");

        let faces = filter(PhotoFilter {
            has_faces: true,
            ..Default::default()
        });
        assert_eq!(faces.len(), 1);

        let quality = filter(PhotoFilter {
            aesthetics_min: Some(0.6),
            ..Default::default()
        });
        assert_eq!(quality.len(), 1);
        assert_eq!(quality[0].id, "p1");

        let sony = filter(PhotoFilter {
            camera: Some("Sony".into()),
            ..Default::default()
        });
        assert_eq!(sony.len(), 1);

        let papers = filter(PhotoFilter {
            papers: true,
            ..Default::default()
        });
        assert_eq!(papers.len(), 1);

        let random = filter(PhotoFilter {
            random: true,
            ..Default::default()
        });
        assert_eq!(random.len(), 2);
    }

    fn make_photo(id: &str, location: &str) -> Photo {
        Photo {
            id: id.to_string(),
            location: location.to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        }
    }

    #[test]
    fn test_has_thumbnail_and_update_photo_thumbnail() {
        let mut db = test_db();
        let _ = db.store_photo_batch(&[make_photo("thumb_1", "/tmp/thumb.jpg")]);

        assert!(!db.has_thumbnail("thumb_1"), "fresh photo has no thumbnail");
        assert!(
            db.update_photo_thumbnail("thumb_1", "data:image/jpeg;base64,abc"),
            "first thumbnail write should succeed"
        );
        assert!(db.has_thumbnail("thumb_1"));

        let photo = db.get_photo_by_id("thumb_1").unwrap();
        assert_eq!(photo.encoded, "data:image/jpeg;base64,abc");

        assert!(
            !db.update_photo_thumbnail("thumb_1", "data:image/jpeg;base64,overwrite"),
            "existing thumbnail must not be overwritten"
        );
        let photo = db.get_photo_by_id("thumb_1").unwrap();
        assert_eq!(photo.encoded, "data:image/jpeg;base64,abc");
    }

    #[test]
    fn test_get_photo_sync_info_includes_unindexed_and_received() {
        let mut db = test_db();
        let _ = db.store_photo_batch(&[
            make_photo("sync_plain", "/home/test/plain.jpg"),
            make_photo("sync_indexed", "/home/test/indexed.jpg"),
            make_photo("sync_siegu", "/home/test/siegu/received.jpg"),
        ]);
        db.import_photo(ImportedPhoto {
            id: "sync_indexed",
            location: "/home/test/indexed.jpg",
            created: "2024-01-01",
            latitude: None,
            longitude: None,
            objects_json: r#"[{"class":"person","probability":0.9}]"#,
            faces_json: "[]",
            encoded: "",
            caption: None,
            aesthetics_score: None,
            received: true,
        });

        let info = db.get_photo_sync_info();
        let ids: Vec<&str> = info.iter().map(|p| p.id.as_str()).collect();

        assert!(
            ids.contains(&"sync_plain"),
            "unindexed photo must appear in sync info immediately"
        );
        assert!(ids.contains(&"sync_indexed"));
        assert!(
            ids.contains(&"sync_siegu"),
            "received files under /siegu/ are part of this device's library and must be in the manifest"
        );
    }

    #[test]
    fn test_prune_missing_files_removes_only_gone_rows() {
        let mut db = test_db();
        let dir = std::env::temp_dir().join(format!(
            "siegu_prune_{}_{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        let existing = dir.join("still_here.jpg");
        fs::write(&existing, b"jpeg").expect("create existing file");

        let existing_path = existing.display().to_string();
        let gone_path = dir.join("deleted.jpg").display().to_string();
        let _ = db.store_photo_batch(&[
            make_photo("prune_existing", &existing_path),
            make_photo("prune_gone", &gone_path),
        ]);

        let removed = db.prune_missing_files(&dir.display().to_string());
        assert_eq!(removed, 1, "only the deleted file's row should be pruned");
        assert!(db.get_photo_by_id("prune_existing").is_some());
        assert!(db.get_photo_by_id("prune_gone").is_none());
    }

    #[test]
    fn test_ingest_marks_sync_needed_and_import_marks_received() {
        let mut db = test_db();
        let _ = db.store_photo_batch(&[make_photo("orig_1", "/tmp/orig.jpg")]);
        db.import_photo(ImportedPhoto {
            id: "recv_1",
            location: "/tmp/siegu/recv.jpg",
            created: "2024-01-01",
            latitude: None,
            longitude: None,
            objects_json: "[]",
            faces_json: "[]",
            encoded: "",
            caption: None,
            aesthetics_score: None,
            received: true,
        });

        let pending: i64 = db
            .connection
            .query_row(
                "SELECT COUNT(*) FROM photo WHERE sync_needed = 1 AND received = 0",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(pending, 1, "only the scanned original awaits sync");

        let received: i64 = db
            .connection
            .query_row("SELECT COUNT(*) FROM photo WHERE received = 1", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        assert_eq!(received, 1, "imported photo must be flagged as received");
    }

    #[test]
    fn test_onboarding_complete_flag() {
        let db = test_db();
        assert!(!db.is_onboarding_complete(), "fresh DB is not initialized");

        db.set_onboarding_complete();
        assert!(db.is_onboarding_complete());
    }

    #[test]
    fn test_legacy_config_numeric_value_reads_as_string() {
        let db = test_db();
        db.connection.execute("DROP TABLE config", ()).unwrap();
        db.connection
            .execute("CREATE TABLE config(key STRING, value STRING)", ())
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO config (key, value) VALUES('session_port', '35225')",
                (),
            )
            .unwrap();
        assert_eq!(
            db.get_state().get("session_port").map(String::as_str),
            Some("35225")
        );
    }

    #[test]
    fn test_has_any_photos() {
        let mut db = test_db();
        assert!(!db.has_any_photos(), "fresh DB has no photos");

        let _ = db.store_photo_batch(&[make_photo("p_any", "/tmp/any.jpg")]);
        assert!(db.has_any_photos());
    }

    #[test]
    fn test_hot_queries_use_indexes() {
        let db = test_db();

        // The faces/people indexes were previously created before their tables
        // and silently failed on fresh databases. Guard the schema directly so
        // that ordering bug cannot regress.
        let mut stmt = db
            .connection
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut indexes = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            indexes.push(row.get::<_, String>(0).unwrap());
        }
        for expected in [
            "idx_photo_indexed",
            "idx_photo_created",
            "idx_faces_person_id",
            "idx_faces_person_photo",
            "idx_faces_photo_id",
            "idx_people_name",
            "idx_object_photo_id",
            "idx_properties_photo_id",
        ] {
            assert!(
                indexes.iter().any(|i| i == expected),
                "missing schema index {expected}; have: {indexes:?}"
            );
        }

        // And the hot paths must actually reach them, so accidental SQL changes
        // that fall back to full scans fail loudly on CI.
        let cases: [(&str, &str, &str); 8] = [
            (
                "scan batch (WHERE indexed<2)",
                "SELECT p.id FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.indexed < 2 LIMIT ?1 OFFSET ?2",
                "idx_photo_indexed",
            ),
            (
                "photo by id",
                "SELECT id, location FROM photo WHERE id = ?1",
                "sqlite_autoindex_photo_1",
            ),
            (
                "photo grid (ORDER BY created)",
                "SELECT p.id FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE 1=1 ORDER BY p.created DESC LIMIT ?1, ?2",
                "idx_photo_created",
            ),
            (
                "person filter (EXISTS faces)",
                "SELECT p.id FROM photo p WHERE EXISTS(SELECT 1 FROM faces WHERE photo_id=p.id AND person_id = ?1)",
                "idx_faces_person_photo",
            ),
            (
                "person photo count",
                "SELECT COUNT(DISTINCT photo_id) FROM faces WHERE person_id = ?1",
                "idx_faces_person_photo",
            ),
            (
                "person embeddings",
                "SELECT embedding FROM faces WHERE person_id = ?1",
                "idx_faces_person_photo",
            ),
            (
                "objects by photo",
                "SELECT class, probability FROM object WHERE photo_id = ?1",
                "idx_object_photo_id",
            ),
            (
                "properties by photo",
                "SELECT * FROM properties WHERE photo_id = ?1",
                "idx_properties_photo_id",
            ),
        ];
        for (name, sql, index) in cases {
            let plan = db_explain(&db.connection, sql);
            assert!(
                plan.contains(index),
                "{name} should reach index {index} but plan was:\n{plan}"
            );
        }
    }

    fn db_explain(connection: &rusqlite::Connection, sql: &str) -> String {
        let mut stmt = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap();
        let param_count = sql.matches('?').count();
        let params: Vec<i64> = (0..param_count).map(|_| 0).collect();
        let mut rows = stmt.query(rusqlite::params_from_iter(params)).unwrap();
        let mut out = String::new();
        while let Ok(Some(row)) = rows.next() {
            let detail: String = row.get(3).unwrap();
            out.push_str(&detail);
            out.push('\n');
        }
        out
    }
}
