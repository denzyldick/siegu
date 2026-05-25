use crate::database;
use base64::{engine::general_purpose, Engine as _};
use exif::Reader;
use jwalk::WalkDir;
use notify::event::{CreateKind, ModifyKind};
use notify::{EventKind, RecursiveMode, Watcher};
use rand::{distributions::Alphanumeric, Rng};
use tauri_plugin_notification::NotificationExt;

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::path::Path;

use std::io::BufReader;
use std::io::Cursor;
use std::string::String;

use crate::ml::MlContext;
use tauri::{Emitter, Manager};
use tokio::sync::mpsc::UnboundedSender;

pub async fn start_watcher(app: tauri::AppHandle) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let app_clone = app.clone();

    let mut watcher = match notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    }) {
        Ok(w) => w,
        Err(_) => return,
    };

    let config_path = crate::get_config_path(&app);
    if !config_path.is_empty() {
        let database = crate::database::Database::new(&config_path);
        let folders = database.list_directories();
        for folder in folders {
            if Path::new(&folder).exists() {
                let _ = watcher.watch(Path::new(&folder), RecursiveMode::Recursive);
            }
        }
    }

    tokio::spawn(async move {
        // Keep watcher alive in this task
        let _watcher = watcher;
        let image_extensions = [
            "png", "jpg", "jpeg", "webp", "heic", "avif", "mp4", "mkv", "mov", "avi", "webm",
        ];
        let mut last_scan = tokio::time::Instant::now();

        while let Some(event) = rx.recv().await {
            println!("Watcher received event: {event:?}");
            match event.kind {
                EventKind::Create(CreateKind::File)
                | EventKind::Modify(ModifyKind::Name(_))
                | EventKind::Modify(ModifyKind::Data(_)) => {
                    let mut needs_scan = false;
                    for path in event.paths {
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            if image_extensions.contains(&ext.to_lowercase().as_str()) {
                                needs_scan = true;
                                break;
                            }
                        }
                    }
                    if needs_scan && last_scan.elapsed().as_secs() > 10 {
                        last_scan = tokio::time::Instant::now();

                        let _ = app_clone
                            .notification()
                            .builder()
                            .title("Siegu")
                            .body("New media detected, scanning...")
                            .show();

                        crate::scan_files(app_clone.clone());
                    }
                }
                _ => {}
            }
        }
    });
}

fn emit_log(app: &tauri::AppHandle, message: String) {
    println!("{message}");
    let _ = app.emit("log-message", message);
}

fn decode_heic_image(path: &Path) -> Option<image::DynamicImage> {
    use heic::{DecoderConfig, PixelLayout};
    let data = std::fs::read(path).ok()?;
    let output = DecoderConfig::new()
        .decode(&data, PixelLayout::Rgba8)
        .ok()?;
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(
        output.width,
        output.height,
        output.data,
    )?);
    Some(img)
}

fn build_thumbnail_data_uri(path: &Path, ext: &str) -> String {
    if ["mp4", "mkv", "mov", "avi", "webm"].contains(&ext) {
        return String::new();
    }

    let img = match image::open(path) {
        Ok(img) => img,
        Err(e) => {
            if ext == "heic" || ext == "heif" {
                match decode_heic_image(path) {
                    Some(img) => img,
                    None => {
                        eprintln!(
                            "[siegu] Failed to decode HEIC for thumbnail: {} - {e}",
                            path.display()
                        );
                        return String::new();
                    }
                }
            } else {
                eprintln!(
                    "[siegu] Failed to open image for thumbnail ({}): {} - {e}",
                    ext,
                    path.display()
                );
                return String::new();
            }
        }
    };

    let thumbnail = img.thumbnail(640, 640);
    let mut buffer = Cursor::new(Vec::new());
    if thumbnail
        .write_to(&mut buffer, image::ImageOutputFormat::Jpeg(72))
        .is_ok()
    {
        let encoded = general_purpose::STANDARD.encode(buffer.get_ref());
        format!("data:image/jpeg;base64,{encoded}")
    } else {
        eprintln!(
            "[siegu] Failed to encode thumbnail as JPEG: {}",
            path.display()
        );
        String::new()
    }
}

/// Recursively discovers supported media files, stores new rows in batches, and queues AI work.
///
/// Discovery intentionally records metadata and thumbnails first. The heavier model inference runs
/// on the ML worker so folder scans stay responsive and progress events remain regular.
pub fn scan_folder(
    app: &tauri::AppHandle,
    directory: String,
    path: &str,
    batch_tx: &UnboundedSender<database::Photo>,
) {
    let db_instance = database::Database::new(path);

    // Load thread config
    let config = db_instance.get_state();
    let num_threads: usize = config
        .get("scan_threads")
        .and_then(|s| s.parse().ok())
        .unwrap_or({
            if cfg!(any(target_os = "android", target_os = "ios")) {
                2
            } else {
                num_cpus::get().min(4)
            }
        });

    emit_log(
        app,
        format!("Starting Discovery Pass with {num_threads} threads in: {directory}"),
    );

    // Collect all valid image and video paths first
    let mut image_paths = Vec::new();
    let video_extensions = ["mp4", "mkv", "mov", "avi", "webm"];
    let image_extensions = ["png", "jpg", "jpeg", "webp", "heic", "avif"];

    use std::sync::atomic::{AtomicBool, Ordering};
    let abort_flag = app
        .try_state::<MlContext>()
        .map(|s| s.abort.clone())
        .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

    for entry in WalkDir::new(directory).follow_links(false) {
        if abort_flag.load(Ordering::SeqCst) {
            return;
        }
        if let Ok(entry) = entry {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if image_extensions.contains(&ext.as_str())
                    || video_extensions.contains(&ext.as_str())
                {
                    if let Ok(path) = fs::canonicalize(path) {
                        image_paths.push(path);
                    }
                }
            }
        }
    }

    use std::sync::{Arc, Mutex};
    let database_arc = Arc::new(Mutex::new(db_instance));

    // 1. Filter out already indexed paths in a single pass
    let all_paths: Vec<String> = image_paths
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    let new_paths_to_process = {
        let db = database_arc.lock().unwrap();
        db.filter_new_paths(&all_paths)
    };

    if new_paths_to_process.is_empty() {
        emit_log(app, "No new photos found.".to_string());
        return;
    }

    let total_files = new_paths_to_process.len();
    emit_log(app, format!("Processing {total_files} new photos..."));

    let app_handle = Arc::new(app.clone());
    let abort_flag_task = Arc::clone(&abort_flag);
    let files_processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let scan_start = Arc::new(std::time::Instant::now());

    // Create a local thread pool for this scan to avoid blocking the global one
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .unwrap();

    use rayon::prelude::*;

    pool.install(|| {
        new_paths_to_process.into_par_iter().for_each(|path_str| {
            if abort_flag_task.load(Ordering::SeqCst) {
                return;
            }
            let path = Path::new(&path_str);
            let id: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(7)
                .map(char::from)
                .collect();

            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let is_video = ["mp4", "mkv", "mov", "avi", "webm"].contains(&ext.as_str());

            let mut latitude = 0.0;
            let mut longitude = 0.0;
            let mut created = String::new();
            let encoded = if is_video {
                String::new()
            } else {
                build_thumbnail_data_uri(path, &ext)
            };

            if let Ok(file) = File::open(path) {
                let mut buff = BufReader::new(&file);

                if let Ok(exif) = Reader::new().read_from_container(&mut buff) {
                    // Extract Created Date
                    if let (Some(date_field), _) = (
                        exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
                            .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY)),
                        (),
                    ) {
                        created = format!("{}", date_field.display_value());
                    }

                    // Extract GPS Coordinates
                    if let (Some(lat_field), Some(lat_ref)) = (
                        exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY),
                        exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY),
                    ) {
                        if let exif::Value::Rational(lat_values) = &lat_field.value {
                            if lat_values.len() == 3 {
                                let lat = lat_values[0].to_f64()
                                    + lat_values[1].to_f64() / 60.0
                                    + lat_values[2].to_f64() / 3600.0;
                                latitude = if format!("{}", lat_ref.display_value()) == "S" {
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
                                longitude = if format!("{}", lon_ref.display_value()) == "W" {
                                    -lon
                                } else {
                                    lon
                                };
                            }
                        }
                    }
                }
            }

            let photo = database::Photo {
                id: id.clone(),
                encoded: encoded.clone(),
                location: path_str.clone(),
                created,
                objects: HashMap::new(),
                properties: HashMap::new(),
                latitude,
                longitude,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: database::AiStatus::default(),
            };

            let processed = files_processed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if processed.is_multiple_of(25) || processed == total_files {
                let filename = Path::new(&path_str)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let elapsed = scan_start.elapsed().as_secs_f64();
                let rate = processed as f64 / elapsed.max(0.001);
                let eta_secs = if rate > 0.0 {
                    ((total_files - processed) as f64 / rate) as u64
                } else {
                    0
                };
                let _ = app_handle.emit(
                    "file-scan-progress",
                    serde_json::json!({
                        "current": processed,
                        "total": total_files,
                        "filename": filename,
                        "eta_secs": eta_secs,
                    }),
                );
            }

            let _ = batch_tx.send(photo);
        });
    });

    emit_log(app, "Done with Discovery Pass".to_string());
}

pub fn read_file_base64(path: String) -> String {
    let canonical = std::fs::canonicalize(&path).unwrap_or_default();
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
    let config = std::env::var("XDG_CONFIG_HOME")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_default();
    let temp = std::env::temp_dir();
    let allowed = [home.as_str(), config.as_str(), temp.to_str().unwrap_or("")];
    let is_allowed = canonical.as_os_str().is_empty()
        || allowed.iter().any(|dir| !dir.is_empty() && canonical.starts_with(dir));
    if !is_allowed {
        eprintln!("Access denied: {path} is outside allowed directories");
        return String::new();
    }
    match fs::read(&path) {
        Ok(bytes) => {
            println!("Reading original file: {} ({} bytes)", path, bytes.len());
            let encoded = general_purpose::STANDARD.encode(bytes);
            let ext = Path::new(&path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_lowercase();
            let mime = match ext.as_str() {
                "png" => "image/png",
                "mp4" => "video/mp4",
                "webm" => "video/webm",
                "mov" => "video/quicktime",
                "avi" => "video/x-msvideo",
                "mkv" => "video/x-matroska",
                _ => "image/jpeg",
            };
            format!("data:{mime};base64,{encoded}")
        }
        Err(e) => {
            eprintln!("Failed to read file {path}: {e}");
            String::new()
        }
    }
}

mod tests {

    #[test]
    fn scan_folder() {
        // Test commented out because scan_folder now requires AppHandle which is hard to mock in unit tests
        /*
        use std::collections::HashMap;
        let mut state = HashMap::new();
        state.insert("path".to_string(), "/home/denzyl".to_string());

        let database = crate::database::Database::new("/home/denzyl");
        database.set_state(state);

        let state = database.get_state();
        let directory = state.get("path").unwrap();
        dbg!(&state);
        let _ = super::scan_folder(directory.to_string(), &directory);
        */
    }
}
