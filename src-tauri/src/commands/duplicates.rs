use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::Emitter;

use crate::common::get_config_path;
use crate::database;

use siegu_core::duplicates::{
    duplicate_stats_from_views, library_overview, DuplicateGroupView, DuplicateStats,
};

/// Only one library-wide duplicate scan runs at a time; subsequent starts are
/// ignored (the UI keeps listening to the running scan's events).
static SCANNING: AtomicBool = AtomicBool::new(false);

/// Progress payload emitted on `duplicate-scan-progress`.
#[derive(Debug, Clone, serde::Serialize)]
struct ScanProgress {
    done: usize,
    total: usize,
}

/// Emit a `duplicate-scan-progress` event while hashing. Throttled so a huge
/// first scan does not flood the event bus; always fires on completion.
fn emit_progress(app: &tauri::AppHandle, done: usize, total: usize, last: &mut Instant) {
    if done == total || last.elapsed().as_millis() >= 150 {
        let _ = app.emit("duplicate-scan-progress", ScanProgress { done, total });
        *last = Instant::now();
    }
}

/// Kick off a background duplicate scan. Results arrive on
/// `duplicate-scan-done`; progress on `duplicate-scan-progress`. Returns
/// immediately — the heavy work (hashing every un-hashed photo) runs off the
/// main (and command) thread so the UI stays responsive.
#[tauri::command]
pub fn start_duplicate_scan(app: tauri::AppHandle, include_clip: Option<bool>) {
    if SCANNING.swap(true, Ordering::AcqRel) {
        return;
    }

    let include_clip = include_clip.unwrap_or(false);
    std::thread::spawn(move || {
        let mut last = Instant::now();
        let path = get_config_path(&app);
        let (views, stats, overview) = if path.is_empty() {
            (Vec::new(), DuplicateStats::default(), None)
        } else {
            let database = database::Database::new(&path);
            let views: Vec<DuplicateGroupView> = siegu_core::duplicates::detect_all_view_progress(
                &database,
                include_clip,
                &mut |done, total| emit_progress(&app, done, total, &mut last),
            );
            let stats: DuplicateStats = duplicate_stats_from_views(&views);
            let overview = library_overview(&database);
            (views, stats, Some(overview))
        };
        let _ = app.emit(
            "duplicate-scan-done",
            serde_json::json!({
                "groups": views,
                "stats": stats,
                "library_bytes": overview.as_ref().map(|o| o.library_bytes).unwrap_or(0),
                "photo_count": overview.as_ref().map(|o| o.photo_count).unwrap_or(0),
                "video_count": overview.as_ref().map(|o| o.video_count).unwrap_or(0),
            }),
        );
        SCANNING.store(false, Ordering::Release);
    });
}

#[tauri::command]
pub async fn find_duplicates(
    app: tauri::AppHandle,
    include_clip: Option<bool>,
) -> Vec<DuplicateGroupView> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Vec::new();
    }
    let database = database::Database::new(&path);
    siegu_core::duplicates::detect_all_view(&database, include_clip.unwrap_or(false))
}

#[tauri::command]
pub async fn duplicate_stats(app: tauri::AppHandle, include_clip: Option<bool>) -> DuplicateStats {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Default::default();
    }
    let database = database::Database::new(&path);
    siegu_core::duplicates::duplicate_stats(&database, include_clip.unwrap_or(false))
}

#[tauri::command]
pub async fn trash_duplicate_members(app: tauri::AppHandle, ids: Vec<String>) -> usize {
    let path = get_config_path(&app);
    if path.is_empty() {
        return 0;
    }
    let database = database::Database::new(&path);
    let mut trashed = 0usize;
    for id in &ids {
        if database.trash_photo(id).is_ok() {
            trashed += 1;
        }
    }
    trashed
}
