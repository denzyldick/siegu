use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::database::Photo;

pub const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "heic", "heif", "avif", "gif", "bmp", "tiff", "tif",
];
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "mov", "avi", "webm", "flv", "wmv", "m4v", "3gp",
];
pub const ALL_MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "heic", "heif", "avif", "gif", "bmp", "tiff", "tif", "mp4",
    "mkv", "mov", "avi", "webm", "flv", "wmv", "m4v", "3gp",
];

pub fn is_media_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            let ext = ext.to_lowercase();
            IMAGE_EXTENSIONS.contains(&ext.as_str()) || VIDEO_EXTENSIONS.contains(&ext.as_str())
        })
        .unwrap_or(false)
}

pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            let ext = ext.to_lowercase();
            IMAGE_EXTENSIONS.contains(&ext.as_str())
        })
        .unwrap_or(false)
}

pub fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            let ext = ext.to_lowercase();
            VIDEO_EXTENSIONS.contains(&ext.as_str())
        })
        .unwrap_or(false)
}

pub fn is_heic_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            let ext = ext.to_lowercase();
            ext == "heic" || ext == "heif"
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone, Default)]
pub struct PhotoMetadata {
    pub latitude: f64,
    pub longitude: f64,
    pub created: String,
    pub favorite: bool,
    pub caption: Option<String>,
    pub properties: HashMap<String, String>,
}

/// Metadata restored from a Google Takeout JSON sidecar (`<file>.json` or
/// `<file>.supplemental-metadata.json`). `photoTakenTime` carries the original
/// capture time — including any date the user corrected in Google Photos — so it
/// takes precedence over EXIF. `creationTime` is the upload time and is only used
/// as a fallback for exports that omit `photoTakenTime`.
#[derive(Debug, Clone, Default)]
pub struct SidecarMeta {
    pub created: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub favorite: bool,
    pub caption: Option<String>,
}

/// Normalize a raw date string to the sortable `YYYY-MM-DD HH:MM:SS` format.
///
/// EXIF dates are emitted as `2024:01:15 10:30:00` (colons) while the mtime
/// fallback uses `2024-01-15 10:30:00` (dashes). Lexically `:` (0x3A) sorts
/// after `-` (0x2D), so mixed formats silently corrupt `ORDER BY created`.
/// This rewrites only the leading date portion and leaves times untouched.
pub fn normalize_created(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() < 10 || !raw.is_ascii() {
        return raw.to_string();
    }
    let date_part = &raw[..10];
    if !date_part
        .chars()
        .all(|c| c.is_ascii_digit() || c == ':' || c == '-')
    {
        return raw.to_string();
    }
    let mut normalized = String::with_capacity(raw.len());
    normalized.push_str(&date_part.replace(':', "-"));
    let rest = &raw[10..];
    if let Some(stripped) = rest.strip_prefix('T') {
        normalized.push(' ');
        normalized.push_str(stripped);
    } else {
        normalized.push_str(rest);
    }
    normalized
}

/// Extract a `YYYY-MM-DD HH:MM:SS` capture time from conventional camera
/// filenames (`IMG_20230815_142536.jpg`, `VID_...`, `PXL_...`, and Apple
/// `Screenshot_2023-08-15-14-25-36.png`). Returns `None` when no plausible
/// timestamp is found.
pub fn created_from_filename(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if let Some((date, time)) = find_compact_timestamp(name) {
        return Some(format!("{date} {time}"));
    }
    find_dashed_timestamp(name).map(|(date, time)| format!("{date} {time}"))
}

/// `YYYYMMDD_HHMMSS[millis]` embedded anywhere in the filename.
fn find_compact_timestamp(name: &str) -> Option<(String, String)> {
    let bytes = name.as_bytes();
    let mut i = 0;
    while i + 15 <= bytes.len() {
        if bytes[i..i + 8].iter().all(u8::is_ascii_digit)
            && matches!(bytes.get(i + 8), Some(b'_' | b'-'))
        {
            let start = i + 9;
            let mut j = start;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j - start >= 6 {
                let date = std::str::from_utf8(&bytes[i..i + 8]).ok()?;
                let time = std::str::from_utf8(&bytes[start..start + 6]).ok()?;
                let date_str = format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8]);
                let time_str = format!("{}:{}:{}", &time[0..2], &time[2..4], &time[4..6]);
                if is_valid_date_time(&date_str, &time_str) {
                    return Some((date_str, time_str));
                }
            }
        }
        i += 1;
    }
    None
}

/// `Screenshot_YYYY-MM-DD-HH-MM-SS.ext` (Apple convention).
fn find_dashed_timestamp(name: &str) -> Option<(String, String)> {
    let upper = name.to_uppercase();
    let marker = upper.find("SCREENSHOT")?;
    let bytes = name.as_bytes();
    let mut i = marker + "SCREENSHOT".len();
    while i < bytes.len() && matches!(bytes[i], b'_' | b'-' | b' ') {
        i += 1;
    }
    if i + 19 > bytes.len() {
        return None;
    }
    let seg = &bytes[i..i + 19];
    let is_digit = |b: &[u8]| b.iter().all(u8::is_ascii_digit);
    if !(seg[4] == b'-'
        && seg[7] == b'-'
        && seg[10] == b'-'
        && seg[13] == b'-'
        && seg[16] == b'-'
        && is_digit(&seg[0..4])
        && is_digit(&seg[5..7])
        && is_digit(&seg[8..10])
        && is_digit(&seg[11..13])
        && is_digit(&seg[14..16])
        && is_digit(&seg[17..19]))
    {
        return None;
    }
    let date = format!(
        "{}-{}-{}",
        std::str::from_utf8(&seg[0..4]).ok()?,
        std::str::from_utf8(&seg[5..7]).ok()?,
        std::str::from_utf8(&seg[8..10]).ok()?
    );
    let time = format!(
        "{}:{}:{}",
        std::str::from_utf8(&seg[11..13]).ok()?,
        std::str::from_utf8(&seg[14..16]).ok()?,
        std::str::from_utf8(&seg[17..19]).ok()?
    );
    if is_valid_date_time(&date, &time) {
        Some((date, time))
    } else {
        None
    }
}

fn is_valid_date_time(date: &str, time: &str) -> bool {
    let d: Vec<&str> = date.split('-').collect();
    let t: Vec<&str> = time.split(':').collect();
    if d.len() != 3 || t.len() != 3 {
        return false;
    }
    let (Ok(y), Ok(m), Ok(dd)) = (
        d[0].parse::<i32>(),
        d[1].parse::<u32>(),
        d[2].parse::<u32>(),
    ) else {
        return false;
    };
    let (Ok(h), Ok(min), Ok(s)) = (
        t[0].parse::<u32>(),
        t[1].parse::<u32>(),
        t[2].parse::<u32>(),
    ) else {
        return false;
    };
    if !(1970..=2100).contains(&y) || !(1..=12).contains(&m) {
        return false;
    }
    let max_day = match m {
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=max_day).contains(&dd) && h < 24 && min < 60 && s < 60
}

fn value_to_i64(v: &serde_json::Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

fn unix_to_created_string(ts: i64) -> Option<String> {
    use chrono::TimeZone;
    let utc = chrono::Utc.timestamp_opt(ts, 0).single()?;
    Some(
        utc.with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
    )
}

/// Exact sidecar candidates (`<name>.json`, `<name>.supplemental-metadata.json`).
fn sidecar_candidates(media_path: &Path) -> Vec<PathBuf> {
    let parent = media_path.parent().unwrap_or_else(|| Path::new("."));
    let Some(media_name) = media_path.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    vec![
        parent.join(format!("{media_name}.json")),
        parent.join(format!("{media_name}.supplemental-metadata.json")),
    ]
}

/// Cap on directories cached per thread so a scan of many folders never keeps
/// an unbounded listing in memory.
const MAX_DIR_CACHE_ENTRIES: usize = 256;

/// Per-thread cache of `.json` file listings, keyed by directory.
///
/// The read-dir fallback below matches sidecars whose names are truncated by
/// Google's 46-character limit. Without a cache each media file would re-read
/// its entire directory listing, making a scan of a large Takeout library
/// quadratic. Backfill runs use their own cache; this one covers metadata
/// extraction during scans and CLI runs.
fn cached_dir_json_listing(parent: &Path) -> Vec<(String, PathBuf)> {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<PathBuf, Vec<(String, PathBuf)>>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    CACHE.with(|cell| {
        let mut map = cell.borrow_mut();
        if let Some(listing) = map.get(parent) {
            return listing.clone();
        }
        let mut listing = Vec::new();
        if let Ok(rd) = std::fs::read_dir(parent) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
                {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()).map(str::to_string) {
                        listing.push((stem, p));
                    }
                }
            }
        }
        if map.len() >= MAX_DIR_CACHE_ENTRIES {
            map.clear();
        }
        map.insert(parent.to_path_buf(), listing.clone());
        listing
    })
}

/// Locate the Takeout sidecar for a media file.
///
/// Google exports either `<file>.json` (legacy) or
/// `<file>.supplemental-metadata.json` (2024+). Filenames are truncated at 46
/// characters, so the `supplemental-metadata` suffix is often cut short; the
/// read-dir fallback matches any `.json` sidecar whose stem starts with the
/// media filename.
pub fn find_takeout_sidecar(media_path: &Path) -> Option<PathBuf> {
    if let Some(p) = sidecar_candidates(media_path)
        .into_iter()
        .find(|c| c.is_file())
    {
        return Some(p);
    }
    let parent = media_path.parent()?;
    let media_name = media_path.file_name()?.to_str()?.to_string();
    let media_stem = media_name.split('.').next().unwrap_or(&media_name);
    for (stem, p) in cached_dir_json_listing(parent) {
        if stem == media_name
            || stem == media_stem
            || (stem.len() > media_name.len() && stem.starts_with(&media_name))
        {
            return Some(p);
        }
    }
    None
}

/// Parse a Google Takeout sidecar JSON into `SidecarMeta`. Returns `None` for
/// files that carry no useful media metadata.
pub fn parse_sidecar(path: &Path) -> Option<SidecarMeta> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut meta = SidecarMeta::default();
    let taken = v
        .pointer("/photoTakenTime/timestamp")
        .and_then(value_to_i64);
    let created = v.pointer("/creationTime/timestamp").and_then(value_to_i64);
    if let Some(ts) = taken.or(created) {
        meta.created = unix_to_created_string(ts);
    }
    meta.latitude = v.pointer("/geoData/latitude").and_then(|x| x.as_f64());
    meta.longitude = v.pointer("/geoData/longitude").and_then(|x| x.as_f64());
    meta.favorite = v
        .get("favorited")
        .and_then(|b| b.as_bool())
        .unwrap_or(false);
    meta.caption = v
        .get("description")
        .and_then(|s| s.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if meta.created.is_none()
        && meta.latitude.is_none()
        && meta.longitude.is_none()
        && !meta.favorite
        && meta.caption.is_none()
    {
        return None;
    }
    Some(meta)
}

/// Find and parse the Takeout sidecar for a media file, if any.
pub fn sidecar_meta_for(media_path: &Path) -> Option<SidecarMeta> {
    let sidecar = find_takeout_sidecar(media_path)?;
    parse_sidecar(&sidecar)
}

fn created_from_mtime(path: &Path) -> String {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return String::new(),
    };
    let modified = match meta.modified() {
        Ok(t) => t,
        Err(_) => return String::new(),
    };
    let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) else {
        return String::new();
    };
    let secs = duration.as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let month_days: &[i64] = if leap {
        &[31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 1u32;
    for &md in month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        m += 1;
    }
    let d = remaining + 1;

    format!("{y:04}-{m:02}-{d:02} {hours:02}:{minutes:02}:{seconds:02}")
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Best-effort metadata extraction, in priority order:
///
/// 1. Google Takeout JSON sidecar (`photoTakenTime` → `creationTime`). This is
///    the most authoritative source: it records the original capture time,
///    including any corrections the user made in Google Photos.
/// 2. Filename conventions (`IMG_/VID_/PXL_YYYYMMDD_HHMMSS`, ...).
/// 3. EXIF `DateTimeOriginal`/`DateTime` for image files.
/// 4. File mtime as a last resort (videos and other files without EXIF).
pub fn extract_photo_metadata(path: &Path) -> PhotoMetadata {
    let mut meta = PhotoMetadata::default();

    if let Some(sidecar) = sidecar_meta_for(path) {
        meta.created = sidecar.created.unwrap_or_default();
        meta.latitude = sidecar.latitude.unwrap_or(0.0);
        meta.longitude = sidecar.longitude.unwrap_or(0.0);
        meta.favorite = sidecar.favorite;
        meta.caption = sidecar.caption;
    }

    if meta.created.is_empty() {
        meta.created = created_from_filename(path).unwrap_or_default();
    }

    let prefer_exif_created = meta.created.is_empty();
    fill_exif(&mut meta, path, prefer_exif_created);

    if meta.created.is_empty() {
        meta.created = created_from_mtime(path);
    }

    meta
}

/// Read EXIF metadata into `meta`. `prefer_exif_created` controls whether a
/// DateTime field may set `created`; it is only true when no sidecar or
/// filename date was found. GPS and properties are always attempted, but GPS
/// does not overwrite sidecar coordinates.
fn fill_exif(meta: &mut PhotoMetadata, path: &Path, prefer_exif_created: bool) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut buff = BufReader::new(&file);
    let exif = match exif::Reader::new().read_from_container(&mut buff) {
        Ok(e) => e,
        Err(_) => return,
    };

    if prefer_exif_created {
        if let Some(date_field) = exif
            .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
            .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))
        {
            meta.created = normalize_created(&format!("{}", date_field.display_value()));
        }
    }

    if meta.latitude == 0.0 {
        if let (Some(lat_field), Some(lat_ref)) = (
            exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY),
            exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY),
        ) {
            if let exif::Value::Rational(lat_values) = &lat_field.value {
                if lat_values.len() == 3 {
                    let lat = lat_values[0].to_f64()
                        + lat_values[1].to_f64() / 60.0
                        + lat_values[2].to_f64() / 3600.0;
                    meta.latitude = if clean_field_value(lat_ref).eq_ignore_ascii_case("S") {
                        -lat
                    } else {
                        lat
                    };
                }
            }
        }
    }

    if meta.longitude == 0.0 {
        if let (Some(lon_field), Some(lon_ref)) = (
            exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY),
            exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY),
        ) {
            if let exif::Value::Rational(lon_values) = &lon_field.value {
                if lon_values.len() == 3 {
                    let lon = lon_values[0].to_f64()
                        + lon_values[1].to_f64() / 60.0
                        + lon_values[2].to_f64() / 3600.0;
                    meta.longitude = if clean_field_value(lon_ref).eq_ignore_ascii_case("W") {
                        -lon
                    } else {
                        lon
                    };
                }
            }
        }
    }

    let exif_tags = [
        exif::Tag::Orientation,
        exif::Tag::Make,
        exif::Tag::Model,
        exif::Tag::LensModel,
        exif::Tag::LensMake,
        exif::Tag::FocalLength,
        exif::Tag::FocalLengthIn35mmFilm,
        exif::Tag::PixelXDimension,
        exif::Tag::PixelYDimension,
        exif::Tag::ImageWidth,
        exif::Tag::ImageLength,
        exif::Tag::PhotographicSensitivity,
        exif::Tag::ExposureTime,
        exif::Tag::FNumber,
        exif::Tag::ExposureProgram,
        exif::Tag::Flash,
        exif::Tag::WhiteBalance,
        exif::Tag::MeteringMode,
        exif::Tag::SceneCaptureType,
        exif::Tag::Software,
        exif::Tag::DateTimeOriginal,
        exif::Tag::DateTime,
    ];

    for tag in &exif_tags {
        if let Some(field) = exif.get_field(*tag, exif::In::PRIMARY) {
            meta.properties
                .insert(format!("{tag}"), clean_field_value(field));
        }
    }
}

/// Render an EXIF field as a clean, normalized string.
///
/// `display_value()` wraps ASCII values in double quotes (kamadak-exif quotes
/// `Ascii` strings), which then leak into stored properties and camera facets.
/// String tags are read from the raw `Ascii` bytes instead; other tags fall back
/// to `display_value()`.
fn clean_field_value(field: &exif::Field) -> String {
    match &field.value {
        exif::Value::Ascii(parts) => parts
            .iter()
            .filter_map(|bytes| String::from_utf8(bytes.clone()).ok())
            .map(|s| s.trim().replace('"', ""))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        _ => format!("{}", field.display_value()).trim().to_string(),
    }
}

pub fn photo_from_metadata(path_str: &str, meta: &PhotoMetadata) -> Photo {
    use rand::distributions::Alphanumeric;
    use rand::Rng;

    let id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();

    Photo {
        id,
        encoded: String::new(),
        location: path_str.to_string(),
        created: meta.created.clone(),
        objects: HashMap::new(),
        properties: meta.properties.clone(),
        latitude: meta.latitude,
        longitude: meta.longitude,
        favorite: meta.favorite,
        indexed: 0,
        caption: meta.caption.clone(),
        aesthetics_score: None,
        ai_status: crate::database::AiStatus::default(),
        sync_needed: true,
        received: false,
    }
}

#[derive(Debug)]
pub struct ScanGuard {
    running: Arc<AtomicBool>,
}

impl ScanGuard {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn try_start(&self) -> Option<ScanSession> {
        if self
            .running
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(ScanSession {
                guard: self.running.clone(),
            })
        } else {
            None
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Default for ScanGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct ScanSession {
    guard: Arc<AtomicBool>,
}

impl Drop for ScanSession {
    fn drop(&mut self) {
        self.guard.store(false, Ordering::Release);
    }
}

pub fn load_existing_paths(db_path: &str) -> std::collections::HashSet<String> {
    let db = crate::database::Database::new(db_path);
    db.existing_locations()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_media_file() {
        assert!(is_media_file(Path::new("photo.jpg")));
        assert!(is_media_file(Path::new("photo.JPEG")));
        assert!(is_media_file(Path::new("video.mp4")));
        assert!(is_media_file(Path::new("image.heic")));
        assert!(!is_media_file(Path::new("doc.pdf")));
        assert!(!is_media_file(Path::new("script.rs")));
        assert!(!is_media_file(Path::new("noext")));
    }

    #[test]
    fn test_is_image_file() {
        assert!(is_image_file(Path::new("photo.jpg")));
        assert!(is_image_file(Path::new("photo.avif")));
        assert!(!is_image_file(Path::new("video.mp4")));
    }

    #[test]
    fn test_is_video_file() {
        assert!(is_video_file(Path::new("video.mp4")));
        assert!(is_video_file(Path::new("video.mkv")));
        assert!(!is_video_file(Path::new("photo.jpg")));
    }

    #[test]
    fn test_extract_photo_metadata_missing_file() {
        let meta = extract_photo_metadata(Path::new("/nonexistent/file.jpg"));
        assert_eq!(meta.latitude, 0.0);
        assert_eq!(meta.longitude, 0.0);
        assert!(meta.created.is_empty());
        assert!(meta.properties.is_empty());
    }

    #[test]
    fn test_extract_photo_metadata_non_exif_file_uses_mtime() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("plain.txt");
        fs::write(&file_path, "hello world").unwrap();

        let meta = extract_photo_metadata(&file_path);
        assert_eq!(meta.latitude, 0.0);
        assert_eq!(meta.longitude, 0.0);
        assert!(
            !meta.created.is_empty(),
            "non-exif file should get created from mtime"
        );
        assert!(
            meta.created.starts_with("20"),
            "created should be a date string"
        );
    }

    #[test]
    fn test_photo_from_metadata() {
        let meta = PhotoMetadata {
            latitude: 40.7128,
            longitude: -74.0060,
            created: "2024:01:15 10:30:00".to_string(),
            favorite: true,
            caption: Some("Test".to_string()),
            properties: {
                let mut m = HashMap::new();
                m.insert("Make".to_string(), "Apple".to_string());
                m.insert("Model".to_string(), "iPhone 15".to_string());
                m
            },
        };
        let photo = photo_from_metadata("/tmp/test.jpg", &meta);
        assert_eq!(photo.location, "/tmp/test.jpg");
        assert_eq!(photo.latitude, 40.7128);
        assert_eq!(photo.longitude, -74.0060);
        assert_eq!(photo.created, "2024:01:15 10:30:00");
        assert_eq!(photo.properties.get("Make").unwrap(), "Apple");
        assert_eq!(photo.id.len(), 7);
        assert!(photo.favorite);
        assert_eq!(photo.caption.as_deref(), Some("Test"));
        assert_eq!(photo.indexed, 0);
    }

    #[test]
    fn test_scan_guard_concurrent() {
        let guard = ScanGuard::new();
        let s1 = guard.try_start();
        assert!(s1.is_some());
        assert!(guard.is_running());

        let s2 = guard.try_start();
        assert!(s2.is_none());

        drop(s1);
        assert!(!guard.is_running());

        let s3 = guard.try_start();
        assert!(s3.is_some());
    }

    #[test]
    fn test_scan_guard_default() {
        let guard = ScanGuard::default();
        assert!(!guard.is_running());
    }

    #[test]
    fn test_clean_field_value_strips_ascii_quotes() {
        let field = exif::Field {
            tag: exif::Tag::Make,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![b"\"OnePlus\"".to_vec()]),
        };
        assert_eq!(clean_field_value(&field), "OnePlus");

        let field = exif::Field {
            tag: exif::Tag::Model,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![b"  iPhone 15 Pro  ".to_vec()]),
        };
        assert_eq!(clean_field_value(&field), "iPhone 15 Pro");
    }

    #[test]
    fn test_load_existing_paths_empty_db() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let paths = load_existing_paths(db_path);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_normalize_created() {
        assert_eq!(
            normalize_created("2024:01:15 10:30:00"),
            "2024-01-15 10:30:00"
        );
        assert_eq!(
            normalize_created("2024-01-15 10:30:00"),
            "2024-01-15 10:30:00"
        );
        assert_eq!(
            normalize_created(" 2024:01:15 10:30:00 "),
            "2024-01-15 10:30:00"
        );
        assert_eq!(normalize_created(""), "");
        assert_eq!(normalize_created("no-date"), "no-date");
    }

    #[test]
    fn test_created_from_filename_patterns() {
        assert_eq!(
            created_from_filename(Path::new("IMG_20230815_142536.jpg")),
            Some("2023-08-15 14:25:36".to_string())
        );
        assert_eq!(
            created_from_filename(Path::new("VID_20230815_142536.mp4")),
            Some("2023-08-15 14:25:36".to_string())
        );
        assert_eq!(
            created_from_filename(Path::new("PXL_20230815_142536789.jpg")),
            Some("2023-08-15 14:25:36".to_string())
        );
        assert_eq!(
            created_from_filename(Path::new("Screenshot_2023-08-15-14-25-36.png")),
            Some("2023-08-15 14:25:36".to_string())
        );
        assert_eq!(created_from_filename(Path::new("vacation.jpg")), None);
        assert_eq!(
            created_from_filename(Path::new("IMG_99999999_999999.mp4")),
            None
        );
    }

    #[test]
    fn test_parse_sidecar_photo_taken_priority() {
        let dir = TempDir::new().unwrap();
        let sidecar = dir.path().join("photo.jpg.json");
        std::fs::write(
            &sidecar,
            r#"{
                "photoTakenTime": { "timestamp": "1692113136" },
                "creationTime": { "timestamp": "1692120000" },
                "geoData": { "latitude": -33.8, "longitude": 151.2 },
                "favorited": true,
                "description": "Sydney"
            }"#,
        )
        .unwrap();
        let meta = parse_sidecar(&sidecar).unwrap();
        assert_eq!(
            meta.created.unwrap(),
            unix_to_created_string(1692113136).unwrap()
        );
        assert_eq!(meta.latitude, Some(-33.8));
        assert_eq!(meta.longitude, Some(151.2));
        assert!(meta.favorite);
        assert_eq!(meta.caption.as_deref(), Some("Sydney"));
    }

    #[test]
    fn test_extract_photo_metadata_prefers_sidecar_for_video() {
        let dir = TempDir::new().unwrap();
        let media = dir.path().join("VID_20230101_120000.mp4");
        std::fs::write(&media, "fake video bytes").unwrap();
        std::fs::write(
            dir.path().join("VID_20230101_120000.mp4.json"),
            r#"{"photoTakenTime": { "timestamp": "1672574400" }}"#,
        )
        .unwrap();
        let meta = extract_photo_metadata(&media);
        assert_eq!(meta.created, unix_to_created_string(1672574400).unwrap());
    }

    #[test]
    fn test_extract_photo_metadata_falls_back_to_filename_then_mtime() {
        let dir = TempDir::new().unwrap();
        let media = dir.path().join("VID_20230101_120000.mp4");
        std::fs::write(&media, "fake video bytes").unwrap();
        let meta = extract_photo_metadata(&media);
        assert_eq!(meta.created, "2023-01-01 12:00:00");
    }
}
