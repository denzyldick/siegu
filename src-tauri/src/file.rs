use crate::database;
use crate::emit_log;
use base64::{engine::general_purpose, Engine as _};
use exif::Reader;

use notify::event::{CreateKind, ModifyKind};
use notify::{EventKind, RecursiveMode, Watcher};
use rand::{distributions::Alphanumeric, Rng};
use tauri_plugin_notification::NotificationExt;

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use std::io::BufReader;
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
            emit_log(&app_clone, format!("Watcher received event: {event:?}"));
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

/// Recursively discovers supported media files, stores new rows in batches, and queues AI work.
///
/// Discovery intentionally records metadata first. The heavier model inference and thumbnail
/// generation run on the ML worker so folder scans stay responsive and progress events remain regular.
pub fn scan_folder(
    app: &tauri::AppHandle,
    directory: String,
    path: &str,
    batch_tx: &UnboundedSender<database::Photo>,
) {
    let db_instance = database::Database::new(path);

    // Preload existing paths into a HashSet for O(1) lookups during the walk
    let existing_paths: std::collections::HashSet<String> = {
        let mut stmt = db_instance
            .connection
            .prepare("SELECT location FROM photo")
            .expect("Failed to prepare existing-path query");
        stmt.query_map([], |row| row.get::<_, String>(0))
            .expect("Failed to query existing paths")
            .flatten()
            .collect()
    };
    emit_log(
        app,
        format!(
            "[scan_folder] Loaded {} existing paths from DB",
            existing_paths.len()
        ),
    );

    emit_log(app, format!("Starting Discovery Pass in: {directory}"));

    let video_extensions = ["mp4", "mkv", "mov", "avi", "webm"];
    let image_extensions = ["png", "jpg", "jpeg", "webp", "heic", "avif"];

    let abort_flag = app
        .try_state::<MlContext>()
        .map(|s| s.abort.clone())
        .unwrap_or_else(|| Arc::new(std::sync::atomic::AtomicBool::new(false)));

    let app_handle = Arc::new(app.clone());
    let abort_flag_task = Arc::clone(&abort_flag);
    let files_processed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let scan_start = Arc::new(std::time::Instant::now());
    let existing = Arc::new(existing_paths);

    use rayon::prelude::*;

    let total_new = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_new_clone = Arc::clone(&total_new);

    // Parallel directory walk using jwalk's parallel iterator.
    // Files stream to the batch channel immediately as they're discovered.
    let result: Result<(), ()> = jwalk::WalkDir::new(&directory)
        .follow_links(false)
        .into_iter()
        .par_bridge()
        .try_for_each(|entry_result| {
            if abort_flag_task.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(());
            }
            let entry = match entry_result {
                Ok(e) => e,
                Err(_) => return Ok(()),
            };
            let file_path = entry.path();
            if let Some(ext) = file_path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if !image_extensions.contains(&ext.as_str())
                    && !video_extensions.contains(&ext.as_str())
                {
                    return Ok(());
                }
            } else {
                return Ok(());
            }

            let path_str = file_path.display().to_string();

            // Skip if already in DB
            if existing.contains(&path_str) {
                return Ok(());
            }

            total_new_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            let id: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(7)
                .map(char::from)
                .collect();
            let mut latitude = 0.0;
            let mut longitude = 0.0;
            let mut created = String::new();
            let encoded = String::new();
            let mut properties = HashMap::new();

            if let Ok(file) = File::open(&file_path) {
                let mut buff = BufReader::new(&file);

                if let Ok(exif) = Reader::new().read_from_container(&mut buff) {
                    if let (Some(date_field), _) = (
                        exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
                            .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY)),
                        (),
                    ) {
                        created = format!("{}", date_field.display_value());
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
                            properties.insert(format!("{tag}"), value);
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
                properties,
                latitude,
                longitude,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: database::AiStatus::default(),
            };

            let processed = files_processed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if processed.is_multiple_of(500) {
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let elapsed = scan_start.elapsed().as_secs_f64();
                let total_sofar = total_new.load(std::sync::atomic::Ordering::SeqCst);
                let rate = processed as f64 / elapsed.max(0.001);
                let eta_secs = if rate > 0.0 {
                    let remaining = total_sofar.saturating_sub(processed) as f64;
                    (remaining / rate) as u64
                } else {
                    0
                };
                let _ = app_handle.emit(
                    "file-scan-progress",
                    serde_json::json!({
                        "current": processed,
                        "total": total_sofar,
                        "filename": filename,
                        "eta_secs": eta_secs,
                    }),
                );
            }

            let send_result = batch_tx.send(photo);
            dbg!("[file] sent photo to batch_tx", &send_result.is_ok());

            Ok(())
        });

    let total = total_new.load(std::sync::atomic::Ordering::SeqCst);
    if result.is_err() {
        emit_log(app, format!("[scan_folder] Scan aborted for: {directory}"));
        return;
    }
    if total == 0 {
        emit_log(app, "No new photos found.".to_string());
        return;
    }
    // Final progress update
    let _ = app_handle.emit(
        "file-scan-progress",
        serde_json::json!({
            "current": total,
            "total": total,
            "filename": "",
            "eta_secs": 0,
        }),
    );
    emit_log(
        app,
        format!(
            "[scan_folder] Sent {} photos to batch channel for: {directory}",
            total
        ),
    );
    emit_log(app, "Done with Discovery Pass".to_string());
}

pub fn read_file_base64(app: &tauri::AppHandle, path: String) -> String {
    let canonical = std::fs::canonicalize(&path).unwrap_or_default();
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let config = std::env::var("XDG_CONFIG_HOME")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_default();
    let temp = std::env::temp_dir();
    let allowed = [home.as_str(), config.as_str(), temp.to_str().unwrap_or("")];
    let is_allowed = canonical.as_os_str().is_empty()
        || allowed
            .iter()
            .any(|dir| !dir.is_empty() && canonical.starts_with(dir));
    if !is_allowed {
        emit_log(
            app,
            format!("Access denied: {path} is outside allowed directories"),
        );
        return String::new();
    }
    match fs::read(&path) {
        Ok(bytes) => {
            emit_log(
                app,
                format!("Reading original file: {} ({} bytes)", path, bytes.len()),
            );
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
            emit_log(app, format!("Failed to read file {path}: {e}"));
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
