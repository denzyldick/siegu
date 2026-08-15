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
/// Thumbnails are written to the DB only; when the last dispatched thumbnail
/// finishes the grid is nudged with a single `photos-refreshed` (one event per
/// scan, not one per photo) so HEIC tiles and the full-screen viewer get the
/// generated thumbnail in a single reload.
fn dispatch_thumbnails(
    config_path: String,
    batch: Vec<database::Photo>,
    semaphore: Arc<tokio::sync::Semaphore>,
    pending_thumbs: Arc<std::sync::atomic::AtomicUsize>,
    app: tauri::AppHandle,
) {
    for photo in batch {
        if !siegu_core::thumbnail::needs_thumbnail(&photo.encoded)
            || !should_dispatch_thumbnail(&photo.location)
        {
            continue;
        }
        pending_thumbs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let path = config_path.clone();
        let id = photo.id.clone();
        let location = photo.location.clone();
        let permit = Arc::clone(&semaphore);
        let pending = Arc::clone(&pending_thumbs);
        let app_for_thumb = app.clone();
        tauri::async_runtime::spawn(async move {
            let _permit = permit.acquire_owned().await;
            tauri::async_runtime::spawn_blocking(move || {
                let db = database::Database::new(&path);
                if !db.has_thumbnail(&id) {
                    if let Some(data_url) = siegu_core::thumbnail::generate_thumbnail(&location) {
                        let _ = db.update_photo_thumbnail(&id, &data_url);
                    }
                }
                // fetch_sub returns the previous value: ==1 means this was the
                // last pending thumbnail, so refresh the library exactly once.
                if pending.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                    let _ = app_for_thumb.emit("photos-refreshed", ());
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

    let (batch_tx, mut batch_rx) = tokio::sync::mpsc::channel::<database::Photo>(512);
    let app_handle_for_batch = app.clone();
    let path_for_batch = path.clone();

    let database = Arc::new(std::sync::Mutex::new(database::Database::new(
        &path_for_batch,
    )));
    let database_for_scan = Arc::clone(&database);

    async fn flush_batch_to_db(
        database: &Arc<std::sync::Mutex<database::Database>>,
        app_handle: &tauri::AppHandle,
        batch: &[database::Photo],
    ) -> usize {
        let db_clone = Arc::clone(database);
        let batch_for_blocking = batch.to_vec();
        let app_for_blocking = app_handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let Ok(mut db) = db_clone.lock() else {
                emit_log(
                    &app_for_blocking,
                    "[batch] ERROR: could not lock DB mutex".to_string(),
                );
                return 0;
            };
            match store_batch_to_db(&mut db, &batch_for_blocking) {
                Ok(stored) => stored,
                Err(e) => {
                    emit_log(
                        &app_for_blocking,
                        format!("[batch] ERROR storing photo batch: {e}"),
                    );
                    0
                }
            }
        })
        .await
        .unwrap_or(0)
    }

    let thumb_semaphore = Arc::new(tokio::sync::Semaphore::new(2));
    let pending_thumbs = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    tauri::async_runtime::spawn(async move {
        let mut batch_accum: Vec<database::Photo> = Vec::new();
        let mut total_stored: usize = 0;
        while let Some(photo) = batch_rx.recv().await {
            batch_accum.push(photo);

            if batch_accum.len() >= 500 {
                let batch = std::mem::take(&mut batch_accum);
                total_stored += flush_batch_to_db(&database, &app_handle_for_batch, &batch).await;
                dispatch_thumbnails(
                    path_for_batch.clone(),
                    batch,
                    Arc::clone(&thumb_semaphore),
                    Arc::clone(&pending_thumbs),
                    app_handle_for_batch.clone(),
                );
            }
        }

        if !batch_accum.is_empty() {
            let batch = std::mem::take(&mut batch_accum);
            total_stored += flush_batch_to_db(&database, &app_handle_for_batch, &batch).await;
            dispatch_thumbnails(
                path_for_batch.clone(),
                batch,
                Arc::clone(&thumb_semaphore),
                Arc::clone(&pending_thumbs),
                app_handle_for_batch.clone(),
            );
        }

        if total_stored > 0 {
            emit_log(
                &app_handle_for_batch,
                format!(
                    "[batch] scan finished; {total_stored} photo(s) stored, refreshing library"
                ),
            );
            // Single event for the whole scan: the grid reloads page one and
            // infinite scroll pulls the rest. No per-photo streaming.
            let _ = app_handle_for_batch.emit("photos-refreshed", ());
        } else {
            emit_log(
                &app_handle_for_batch,
                "[batch] receiver exited (all senders dropped)".to_string(),
            );
        }
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
            file::scan_folder(&app, folder.clone(), &database_for_scan, &batch_tx_shared);
            emit_log(
                &app,
                format!("[scan_files] scan_folder completed for: {folder}"),
            );

            // Immediate prune: drop rows whose media file no longer exists on
            // disk. Once removed from the manifest, the next manifest exchange
            // re-requests and restores the file from any peer holding a copy.
            let mut db_prune = database::Database::new(&path);
            let pruned = db_prune.prune_missing_files(folder);
            if pruned > 0 {
                emit_log(
                    &app,
                    format!("[scan_files] Pruned {pruned} missing files from: {folder}"),
                );
            }
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
            let _ = state.tx.blocking_send(ml::Job::ProcessAll);
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
