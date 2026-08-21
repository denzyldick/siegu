use crate::commands;
use crate::common::{emit_log, get_config_path};
use crate::file;
use std::sync::Arc;
use tauri::AppHandle;
use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;

/// One-time notification so the user knows the app is running in the background.
pub fn spawn_background_notification(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = app_handle
            .notification()
            .builder()
            .title("Siegu")
            .body("Siegu is running in the background")
            .show();
    });
}

/// Clean up stale temp files on startup.
pub fn spawn_startup_temp_cleanup(app: &AppHandle) {
    let cp = get_config_path(app);
    if cp.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        siegu_core::mesh::MeshManager::cleanup_temp_files(&cp).await;
    });
}

/// Generate and store thumbnails for every media file that lacks one, in the
/// background with bounded concurrency, so syncs and UI loads never block on an
/// on-the-fly decode. The grid normally renders JPGs/videos directly, so this
/// fills the DB `encoded` column for anything left empty at scan time (non-HEIC
/// files). Idempotent: photos that already have a thumbnail are skipped, and
/// processed rows drop out of the query on the next pass.
pub fn spawn_background_thumbnail_warmup(app: &AppHandle) {
    let cp = get_config_path(app);
    if cp.is_empty() {
        return;
    }
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // Let the initial scan and the UI settle before decoding media.
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let semaphore = Arc::new(tokio::sync::Semaphore::new(3));
        let mut processed_total = 0usize;
        let mut last_logged_total = 0usize;
        loop {
            let batch = {
                let db = crate::database::Database::new(&cp);
                db.photos_missing_thumbnails(200)
            };
            if batch.is_empty() {
                break;
            }
            let mut handles = Vec::with_capacity(batch.len());
            for (id, location) in batch {
                let permit = Arc::clone(&semaphore);
                let cp_clone = cp.clone();
                handles.push(tauri::async_runtime::spawn(async move {
                    let _permit = match permit.acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => return,
                    };
                    tauri::async_runtime::spawn_blocking(move || {
                        if let Some(thumb) = siegu_core::thumbnail::generate_thumbnail(&location) {
                            let db = crate::database::Database::new(&cp_clone);
                            let _ = db.update_photo_thumbnail(&id, &thumb);
                        }
                    })
                    .await
                    .ok();
                }));
            }
            for handle in handles {
                let _ = handle.await;
            }
            processed_total += 1;
            if processed_total - last_logged_total >= 5 {
                last_logged_total = processed_total;
                crate::common::debug_log(format!(
                    "Thumbnail warm-up: processed {processed_total} batches..."
                ));
            }
        }
        if processed_total > 0 {
            emit_log(&app_handle, "Getting your photos ready…".to_string());
            let _ = app_handle.emit("photos-refreshed", ());
        }
    });
}

/// Re-scan the library hourly so long-running sources (mobile uploads, a peer's
/// backup drive) are reflected without a manual scan.
pub fn spawn_interval_rescan(app: &AppHandle) {
    let app_handle_for_interval = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        // `tokio::time::interval` fires its first tick immediately, which
        // would trigger a full library rescan on every app launch and slow
        // startup for large libraries. Consume that tick so the first scan
        // only runs after a full hour; the file watcher and the manual
        // "Scan" button still cover live changes.
        interval.tick().await;
        loop {
            interval.tick().await;
            crate::common::debug_log("Interval tick: checking for media updates...".to_string());
            commands::scan::scan_files(app_handle_for_interval.clone());
        }
    });
}

/// Periodic temp file cleanup every 30 minutes.
pub fn spawn_periodic_temp_cleanup(app: &AppHandle) {
    let app_handle_for_cleanup = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1800));
        loop {
            interval.tick().await;
            let cp = get_config_path(&app_handle_for_cleanup);
            if !cp.is_empty() {
                siegu_core::mesh::MeshManager::cleanup_temp_files(&cp).await;
            }
        }
    });
}

/// Watch configured folders and rescan when new media files appear.
pub fn spawn_file_watcher(app: &AppHandle) {
    let app_handle_for_watcher = app.clone();
    tauri::async_runtime::spawn(async move {
        file::start_watcher(app_handle_for_watcher).await;
    });
}
