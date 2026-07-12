use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
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

#[derive(Debug, Clone, Default)]
pub struct PhotoMetadata {
    pub latitude: f64,
    pub longitude: f64,
    pub created: String,
    pub properties: HashMap<String, String>,
}

pub fn extract_photo_metadata(path: &Path) -> PhotoMetadata {
    let mut meta = PhotoMetadata::default();

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return meta,
    };
    let mut buff = BufReader::new(&file);
    let exif = match exif::Reader::new().read_from_container(&mut buff) {
        Ok(e) => e,
        Err(_) => return meta,
    };

    if let Some(date_field) = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))
    {
        meta.created = format!("{}", date_field.display_value());
    }

    if let (Some(lat_field), Some(lat_ref)) = (
        exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY),
        exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY),
    ) {
        if let exif::Value::Rational(lat_values) = &lat_field.value {
            if lat_values.len() == 3 {
                let lat = lat_values[0].to_f64()
                    + lat_values[1].to_f64() / 60.0
                    + lat_values[2].to_f64() / 3600.0;
                meta.latitude = if format!("{}", lat_ref.display_value()) == "S" {
                    -lat
                } else {
                    lat
                };
            }
        }
    }

    if let (Some(lon_field), Some(lon_ref)) = (
        exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY),
        exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY),
    ) {
        if let exif::Value::Rational(lon_values) = &lon_field.value {
            if lon_values.len() == 3 {
                let lon = lon_values[0].to_f64()
                    + lon_values[1].to_f64() / 60.0
                    + lon_values[2].to_f64() / 3600.0;
                meta.longitude = if format!("{}", lon_ref.display_value()) == "W" {
                    -lon
                } else {
                    lon
                };
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
            let value = format!("{}", field.display_value());
            meta.properties.insert(format!("{tag}"), value);
        }
    }

    meta
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
        favorite: false,
        indexed: 0,
        caption: None,
        aesthetics_score: None,
        ai_status: crate::database::AiStatus::default(),
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
    let mut stmt = match db.connection.prepare("SELECT location FROM photo") {
        Ok(s) => s,
        Err(_) => return std::collections::HashSet::new(),
    };
    stmt.query_map([], |row| row.get::<_, String>(0))
        .map(|iter| iter.flatten().collect())
        .unwrap_or_default()
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
    fn test_extract_photo_metadata_non_exif_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("plain.txt");
        fs::write(&file_path, "hello world").unwrap();

        let meta = extract_photo_metadata(&file_path);
        assert_eq!(meta.latitude, 0.0);
        assert_eq!(meta.longitude, 0.0);
        assert!(meta.created.is_empty());
    }

    #[test]
    fn test_photo_from_metadata() {
        let meta = PhotoMetadata {
            latitude: 40.7128,
            longitude: -74.0060,
            created: "2024:01:15 10:30:00".to_string(),
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
        assert!(!photo.favorite);
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
    fn test_load_existing_paths_empty_db() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let paths = load_existing_paths(db_path);
        assert!(paths.is_empty());
    }
}
