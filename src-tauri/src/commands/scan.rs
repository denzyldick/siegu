use crate::common::get_config_path;
use crate::file;
use crate::ml;

use crate::common::emit_log;
use crate::database;
use std::sync::Arc;
use std::time::SystemTime;
use tauri::Emitter;
use tauri::Manager;

/// Persist a batch of photos to the DB. No thumbnail generation happens here, so
/// discovery results reach the library immediately even for very large libraries.
fn store_batch_to_db(
    db: &mut database::Database,
    batch: &[database::Photo],
) -> Result<usize, String> {
    db.store_photo_batch(batch)?;
    Ok(batch.len())
}

/// Only HEIC/HEIF need a thumbnail generated at scan time: the grid renders JPGs
/// via `convertFileSrc` and videos via a native `<video>` element, and the sync
/// joiner regenerates thumbnails on receipt. Generating poster images for every
/// media file here would hammer the CPU (ffmpeg video decode, multi-MB image
/// decode) without improving the UI.
fn should_dispatch_thumbnail(location: &str) -> bool {
    siegu_core::thumbnail::is_heic_file(location)
}

/// Queue thumbnail generation for a batch in the background with bounded
/// concurrency. Rows are already committed by the time this runs, so thumbnail
/// work can never block the library from showing newly discovered photos.
fn dispatch_thumbnails(
    app: tauri::AppHandle,
    config_path: String,
    batch: Vec<database::Photo>,
    semaphore: Arc<tokio::sync::Semaphore>,
) {
    for photo in batch {
        if !siegu_core::thumbnail::needs_thumbnail(&photo.encoded)
            || !should_dispatch_thumbnail(&photo.location)
        {
            continue;
        }
        let app = app.clone();
        let path = config_path.clone();
        let id = photo.id.clone();
        let location = photo.location.clone();
        let permit = Arc::clone(&semaphore);
        tauri::async_runtime::spawn(async move {
            let _permit = permit.acquire_owned().await;
            tauri::async_runtime::spawn_blocking(move || {
                let db = database::Database::new(&path);
                if db.has_thumbnail(&id) {
                    return;
                }
                let Some(data_url) = siegu_core::thumbnail::generate_thumbnail(&location) else {
                    return;
                };
                if db.update_photo_thumbnail(&id, &data_url)
                    && siegu_core::thumbnail::is_heic_file(&location)
                {
                    let _ = app.emit("photo-analysis-result", serde_json::json!({ "id": id }));
                }
            });
        });
    }
}

#[tauri::command]
pub fn scan_files(app: tauri::AppHandle) {
    let session = {
        let scan_state = app.state::<crate::ScanState>();
        scan_state.guard.try_start()
    };
    let session = match session {
        Some(s) => s,
        None => {
            emit_log(&app, "Scan already in progress, skipping.".to_string());
            return;
        }
    };

    emit_log(&app, "Starting media scan...".to_string());
    let path = get_config_path(&app);

    if path.is_empty() {
        emit_log(
            &app,
            "Error: Config path is empty, cannot scan.".to_string(),
        );
        return;
    }
    let database = database::Database::new(&path);
    let folders = database.list_directories();
    emit_log(
        &app,
        format!("Found {} folders to scan in database.", folders.len()),
    );

    if !folders.is_empty() {
        use tauri_plugin_notification::NotificationExt;
        let _ = app
            .notification()
            .builder()
            .title("Siegu")
            .body(format!("Started scanning {} folder(s)...", folders.len()))
            .show();
    }

    let state = app.state::<ml::MlContext>();
    state
        .abort
        .store(false, std::sync::atomic::Ordering::SeqCst);

    let (batch_tx, mut batch_rx) = tokio::sync::mpsc::unbounded_channel::<database::Photo>();
    let app_handle_for_batch = app.clone();
    let path_for_batch = path.clone();

    let database = Arc::new(std::sync::Mutex::new(database::Database::new(
        &path_for_batch,
    )));
    let ui_buffer = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    {
        let app = app_handle_for_batch.clone();
        let ui = Arc::clone(&ui_buffer);
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(2000));
            loop {
                interval.tick().await;
                let batch = {
                    let mut buf = ui.lock().await;
                    let b = buf.clone();
                    buf.clear();
                    b
                };
                if !batch.is_empty() {
                    emit_log(&app, format!("[ui-buffer] emitting {} photos", batch.len()));
                    if let Err(e) = app.emit("photos-discovered", &batch) {
                        emit_log(&app, format!("[ui-buffer] ERROR emitting photos: {e}"));
                    }
                }
            }
        });
    }

    async fn flush_batch_to_db_and_ui(
        database: &Arc<std::sync::Mutex<database::Database>>,
        app_handle: &tauri::AppHandle,
        ui_buffer: &Arc<tokio::sync::Mutex<Vec<database::Photo>>>,
        batch: &[database::Photo],
    ) {
        let db_clone = Arc::clone(database);
        let batch_for_blocking = batch.to_vec();
        let app_for_blocking = app_handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Ok(mut db) = db_clone.lock() {
                if let Err(e) = store_batch_to_db(&mut db, &batch_for_blocking) {
                    emit_log(
                        &app_for_blocking,
                        format!("[batch] ERROR storing photo batch: {e}"),
                    );
                }
            } else {
                emit_log(
                    &app_for_blocking,
                    "[batch] ERROR: could not lock DB mutex".to_string(),
                );
            }
        })
        .await;

        {
            let mut buf = ui_buffer.lock().await;
            buf.extend_from_slice(batch);
        }
    }

    let thumb_semaphore = Arc::new(tokio::sync::Semaphore::new(2));

    tauri::async_runtime::spawn(async move {
        let mut batch_accum: Vec<database::Photo> = Vec::new();
        while let Some(photo) = batch_rx.recv().await {
            batch_accum.push(photo);

            if batch_accum.len() >= 500 {
                let batch = std::mem::take(&mut batch_accum);
                flush_batch_to_db_and_ui(&database, &app_handle_for_batch, &ui_buffer, &batch)
                    .await;
                dispatch_thumbnails(
                    app_handle_for_batch.clone(),
                    path_for_batch.clone(),
                    batch,
                    Arc::clone(&thumb_semaphore),
                );
            }
        }

        if !batch_accum.is_empty() {
            let batch = std::mem::take(&mut batch_accum);
            flush_batch_to_db_and_ui(&database, &app_handle_for_batch, &ui_buffer, &batch).await;
            dispatch_thumbnails(
                app_handle_for_batch.clone(),
                path_for_batch.clone(),
                batch,
                Arc::clone(&thumb_semaphore),
            );
        }

        emit_log(
            &app_handle_for_batch,
            "[batch] receiver exited (all senders dropped)".to_string(),
        );
    });

    let abort_flag = Arc::clone(&state.abort);
    let batch_tx_shared = Arc::new(batch_tx);

    std::thread::spawn(move || {
        let _session = session;
        let total = folders.len();
        if total == 0 {
            emit_log(
                &app,
                "No folders to scan. Skipping scan thread.".to_string(),
            );
            return;
        }

        for (i, folder) in folders.iter().enumerate() {
            if abort_flag.load(std::sync::atomic::Ordering::SeqCst) {
                emit_log(&app, "Scan aborted by user.".to_string());
                return;
            }
            let progress = (i as f32 / total as f32 * 100.0) as u32;
            let _ = app.emit("scan-progress", serde_json::json!({ "status": "discovering", "progress": progress, "current": i + 1, "total": total, "current_directory": folder }));
            emit_log(
                &app,
                format!("Scanning folder {} of {}: {}", i + 1, total, folder),
            );
            emit_log(
                &app,
                format!("[scan_files] Calling scan_folder for: {folder}"),
            );
            file::scan_folder(&app, folder.clone(), &path, &batch_tx_shared);
            emit_log(
                &app,
                format!("[scan_files] scan_folder completed for: {folder}"),
            );
        }

        emit_log(
            &app,
            "Finished scanning all folders. Updating last scan time...".to_string(),
        );
        let db_check = database::Database::new(&path);
        let config = db_check.get_state();
        let any_model_enabled = [
            "clip",
            "face",
            "ocr",
            "nsfw",
            "aesthetics",
            "yolo",
            "blip",
            "arcface",
            "midas",
            "whisper",
        ]
        .iter()
        .any(|m| {
            config
                .get(&format!("model_enabled_{}", m))
                .is_some_and(|v| v == "true")
        });

        if any_model_enabled {
            let _ = app.emit(
                "scan-progress",
                serde_json::json!({ "status": "indexing", "progress": 100, "message": "Analyzing photos with AI..." }),
            );
            use tauri_plugin_notification::NotificationExt;
            let _ = app
                .notification()
                .builder()
                .title("Siegu")
                .body("Files discovered, analyzing with AI...")
                .show();
        } else {
            let _ = app.emit(
                "scan-progress",
                serde_json::json!({ "status": "complete", "progress": 100, "message": "Scan complete" }),
            );
        }

        let database = database::Database::new(&path);
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();
        database.set_last_scan_time(timestamp);

        if let Some(state) = app.try_state::<ml::MlContext>() {
            let _ = state.tx.send(ml::Job::ProcessAll);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{make_photo, test_db};

    #[test]
    fn test_store_batch_to_db_persists_without_thumbnails() {
        let (mut db, _dir) = test_db();
        let batch = vec![
            make_photo("p1", "/tmp/p1.jpg"),
            make_photo("p2", "/tmp/p2.jpg"),
        ];

        let count = store_batch_to_db(&mut db, &batch).expect("batch should persist");

        assert_eq!(count, 2);
        let photos = db.list_photos("", 0, 10, false, false);
        assert_eq!(photos.len(), 2);
        assert!(
            photos.iter().all(|p| p.encoded.is_empty()),
            "rows must be committed without generating thumbnails"
        );
    }

    #[test]
    fn test_store_batch_to_db_is_idempotent() {
        let (mut db, _dir) = test_db();
        let batch = vec![make_photo("p1", "/tmp/p1.jpg")];

        assert_eq!(store_batch_to_db(&mut db, &batch).unwrap(), 1);
        assert_eq!(store_batch_to_db(&mut db, &batch).unwrap(), 1);

        let photos = db.list_photos("", 0, 10, false, false);
        assert_eq!(photos.len(), 1);
    }

    #[test]
    fn test_should_dispatch_thumbnail_only_heic() {
        assert!(should_dispatch_thumbnail("/photos/img.heic"));
        assert!(should_dispatch_thumbnail("/photos/img.HEIF"));
        assert!(!should_dispatch_thumbnail("/photos/img.jpg"));
        assert!(!should_dispatch_thumbnail("/photos/img.jpeg"));
        assert!(!should_dispatch_thumbnail("/photos/img.png"));
        assert!(!should_dispatch_thumbnail("/photos/video.mp4"));
        assert!(!should_dispatch_thumbnail("/photos/video.mov"));
    }
}
