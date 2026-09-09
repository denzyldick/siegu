use crate::commands;
use crate::common::{emit_log, get_config_path};
use crate::file;
use std::sync::Arc;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;

/// One-time notification so the user knows the app is running in the background.
/// Suppressed when the window is focused (the app is visible then anyway).
pub fn spawn_background_notification(app: &AppHandle) {
    crate::notify::notify_routine(app, "Siegu is running in the background".to_string());
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
///
/// The pass is machine-calibrated: before processing, one of each thumbnail
/// kind (still/HEIC/video) is decoded and timed so the ETA reflects this
/// machine's real decoders, cores and disk — not a hardcoded guess. Progress is
/// surfaced to the scan dialog via `thumb-eta`/`thumb-progress` events.
pub fn spawn_background_thumbnail_warmup(app: &AppHandle) {
    let cp = get_config_path(app);
    if cp.is_empty() {
        return;
    }
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // Let the initial scan and the UI settle before decoding media.
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        run_thumbnail_prep(&app_handle).await;
    });
}

/// Sample up to this many files of each kind for calibration timing.
const CALIBRATION_SAMPLE_SIZE: usize = 3;
/// Thumbnail generation concurrency budget (decoders, not threads).
const THUMB_CONCURRENCY: usize = 3;
/// How long to pause after each batch so the pass yields to scans/AI indexing.
const THUMB_BATCH_PACE_MS: u64 = 150;
/// Ms/photo below which a calibration sample is treated as noise (keeps the
/// estimate from collapsing on tiny/rescaled test libraries).
const CALIBRATION_FLOOR_MS: f64 = 5.0;

async fn run_thumbnail_prep(app: &AppHandle) {
    let cp = get_config_path(app);
    if cp.is_empty() {
        return;
    }

    // Calibrate per-kind decode cost on this machine first. Warm the filesystem
    // cache with the first pass and average a few samples so the base estimate
    // is stable before the ETA is shown.
    let (samples, remaining_by_kind) = {
        let db = crate::database::Database::new(&cp);
        let mut samples: [Vec<String>; 3] = Default::default();
        let mut counts = [0usize; 3];
        for loc in db.missing_thumbnail_locations() {
            let k = siegu_core::thumbnail::classify(&loc) as usize;
            counts[k] += 1;
            if samples[k].len() < CALIBRATION_SAMPLE_SIZE {
                samples[k].push(loc);
            }
        }
        (samples, counts)
    };
    let calibration = calibrate_thumbnail_speeds(&samples);

    // If there is nothing to generate, do not emit any "preparing" state.
    let total_missing: usize = remaining_by_kind.iter().sum();
    if total_missing == 0 {
        crate::common::debug_log("Thumbnail warm-up: nothing to generate.".to_string());
        let _ = app.emit("thumb-status", serde_json::json!({ "status": "idle" }));
        return;
    }

    crate::common::debug_log(format!(
        "Thumbnail warm-up: calibration still={:.1}ms heic={:.1}ms video={:.1}ms",
        calibration[0], calibration[1], calibration[2]
    ));

    // Derive the initial ETA from calibration + the real count per kind.
    let mut remaining_by_kind = remaining_by_kind;
    let estimate_secs = estimate_thumbnail_time_secs(&remaining_by_kind, &calibration);
    let _ = app.emit(
        "thumb-eta",
        serde_json::json!({ "eta": estimate_secs, "active": true }),
    );
    let _ = app.emit(
        "thumb-status",
        serde_json::json!({
            "status": "running",
            "remaining": remaining_by_kind.iter().sum::<usize>(),
        }),
    );
    emit_log(app, "Preparing your photo previews…".to_string());

    let semaphore = Arc::new(tokio::sync::Semaphore::new(THUMB_CONCURRENCY));
    let mut processed_total = 0usize;
    let mut last_logged_total = 0usize;
    let mut last_eta_emit = std::time::Instant::now();
    loop {
        // Yield while a manual scan is running so thumbnails never compete with
        // the discovery pass for disk/CPU. Re-checked every loop iteration.
        let scan_paused = app.state::<crate::ScanState>().guard.is_running();
        if scan_paused {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            continue;
        }
        let batch = {
            let db = crate::database::Database::new(&cp);
            db.photos_missing_thumbnails(200)
        };
        if batch.is_empty() {
            break;
        }
        let mut handles = Vec::with_capacity(batch.len());
        let mut batch_kinds = [0usize; 3];
        for (id, location) in &batch {
            batch_kinds[siegu_core::thumbnail::classify(location) as usize] += 1;
        }
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

        // Track per-kind progress from the batch actually processed so the ETA
        // stays honest as the mix shifts (e.g. all HEIC first, then cheap
        // stills) without re-querying the whole table every batch.
        for (kind, count) in batch_kinds.iter().enumerate() {
            remaining_by_kind[kind] = remaining_by_kind[kind].saturating_sub(*count);
        }

        if last_eta_emit.elapsed().as_millis() >= 500 {
            last_eta_emit = std::time::Instant::now();
            let estimate_secs = estimate_thumbnail_time_secs(&remaining_by_kind, &calibration);
            let _ = app.emit(
                "thumb-eta",
                serde_json::json!({ "eta": estimate_secs, "active": true }),
            );
            let _ = app.emit(
                "thumb-progress",
                serde_json::json!({
                    "remaining": remaining_by_kind.iter().sum::<usize>(),
                }),
            );
        }

        if processed_total - last_logged_total >= 5 {
            last_logged_total = processed_total;
            crate::common::debug_log(format!(
                "Thumbnail warm-up: processed {processed_total} batches..."
            ));
        }

        // Yield between batches so a concurrent scan or model load is not starved.
        tokio::time::sleep(std::time::Duration::from_millis(THUMB_BATCH_PACE_MS)).await;
    }
    if processed_total > 0 {
        emit_log(app, "Your photos are ready.".to_string());
        let _ = app.emit("thumb-status", serde_json::json!({ "status": "complete" }));
        let _ = app.emit("photos-refreshed", ());
    } else {
        let _ = app.emit("thumb-status", serde_json::json!({ "status": "complete" }));
    }
}

/// Time `generate_thumbnail` on a representative sample of each kind. Samples
/// are uploaded per kind; a kind with no samples keeps an "unknown" marker so
/// the estimate degrades instead of guessing.
fn calibrate_thumbnail_speeds(samples: &[Vec<String>; 3]) -> [f64; 3] {
    let mut ms_per_kind = [f64::NAN; 3];
    for (kind, paths) in samples.iter().enumerate() {
        let mut durations = Vec::with_capacity(paths.len());
        for path in paths {
            let start = std::time::Instant::now();
            let _ = siegu_core::thumbnail::generate_thumbnail_bytes(path);
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            if elapsed >= CALIBRATION_FLOOR_MS {
                durations.push(elapsed);
            }
        }
        if !durations.is_empty() {
            ms_per_kind[kind] = durations.iter().sum::<f64>() / durations.len() as f64;
        }
    }
    ms_per_kind
}

/// Total expected seconds for the remaining thumbnails at the current
/// concurrency, using the per-kind calibration (or a conservative fallback
/// when a kind was never sampled — e.g. videos on a machine without ffmpeg).
fn estimate_thumbnail_time_secs(remaining_by_kind: &[usize; 3], calibration: &[f64; 3]) -> f64 {
    const FALLBACK_MS: [f64; 3] = [250.0, 400.0, 500.0];
    let mut total_ms = 0.0;
    for (kind, count) in remaining_by_kind.iter().enumerate() {
        let per = calibration[kind].min(30_000.0);
        let per = if per.is_finite() && per > 0.0 {
            per
        } else {
            FALLBACK_MS[kind]
        };
        let per = per.max(CALIBRATION_FLOOR_MS);
        total_ms += per * *count as f64;
    }
    total_ms / THUMB_CONCURRENCY as f64 / 1000.0
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
