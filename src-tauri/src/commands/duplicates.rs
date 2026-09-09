use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::Emitter;

use crate::common::get_config_path;
use crate::database;

use siegu_core::duplicates::{
    detect_clip_with_backfill, duplicate_stats_from_views, library_overview, DuplicateGroupView,
    DuplicateStats,
};

/// Only one library-wide duplicate scan runs at a time; subsequent starts are
/// ignored (the UI keeps listening to the running scan's events).
static SCANNING: AtomicBool = AtomicBool::new(false);

/// Progress payload emitted on `duplicate-scan-progress`.
#[derive(Debug, Clone, serde::Serialize)]
struct ScanProgress {
    /// Which stage is running: "hashing", "clip-stills", "clip-videos".
    stage: String,
    done: usize,
    total: usize,
}

/// Emit a `duplicate-scan-progress` event while working. Throttled so a huge
/// first scan does not flood the event bus; always fires on stage completion.
fn emit_progress(
    app: &tauri::AppHandle,
    stage: &str,
    done: usize,
    total: usize,
    last: &mut Instant,
) {
    if done == total || last.elapsed().as_millis() >= 150 {
        let _ = app.emit(
            "duplicate-scan-progress",
            ScanProgress {
                stage: stage.to_string(),
                done,
                total,
            },
        );
        *last = Instant::now();
    }
}

/// Kick off a background duplicate scan. Results arrive on
/// `duplicate-scan-done`; progress on `duplicate-scan-progress`. Returns
/// immediately — the heavy work (hashing + optional AI embedding) runs off the
/// main (and command) thread so the UI stays responsive.
///
/// `include_clip` enables the CLIP stage; `include_videos` extends it to
/// videos too. When `include_clip` is on but the CLIP model can't be loaded or
/// no embedding matches exist, the scan still returns exact + perceptual
/// results.
#[tauri::command]
pub fn start_duplicate_scan(
    app: tauri::AppHandle,
    include_clip: Option<bool>,
    include_videos: Option<bool>,
) {
    if SCANNING.swap(true, Ordering::AcqRel) {
        return;
    }

    let include_clip = include_clip.unwrap_or(false);
    let mut include_videos = include_videos.unwrap_or(false);
    std::thread::spawn(move || {
        let mut last = Instant::now();
        let path = get_config_path(&app);
        let (views, stats, overview) = if path.is_empty() {
            (Vec::new(), DuplicateStats::default(), None)
        } else {
            let database = database::Database::new(&path);
            // Hashing stage (SHA-256 + dHash), the same exact/perceptual pass.
            let mut views: Vec<DuplicateGroupView> =
                siegu_core::duplicates::detect_all_view_progress(
                    &database,
                    false, // CLIP handled below so we can load the model once
                    false,
                    &mut |done, total| emit_progress(&app, "hashing", done, total, &mut last),
                );

            // CLIP stage: lazy-embed missing stills/videos and group them.
            if include_clip {
                let config = database.get_state();
                if config.get("dup_scan_videos").is_some_and(|v| v == "false") {
                    include_videos = false;
                }
                if let Some(model) =
                    siegu_core::ml_engine::models::load_clip_visual_only(&path, &config)
                {
                    let (clip_views, computed) = detect_clip_with_backfill(
                        &database,
                        &model,
                        include_videos,
                        &mut |stage, done, total| {
                            emit_progress(&app, stage, done, total, &mut last);
                        },
                    );
                    if computed > 0 {
                        crate::common::debug_log(format!(
                            "Duplicate scan: computed {computed} CLIP embeddings."
                        ));
                    }
                    if !clip_views.is_empty() {
                        views.extend(clip_views);
                    }
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    let _ = app.emit(
                        "duplicate-scan-progress",
                        ScanProgress {
                            stage: "clip-stills".to_string(),
                            done: 0,
                            total: 0,
                        },
                    );
                }
            }

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
    include_videos: Option<bool>,
) -> Vec<DuplicateGroupView> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Vec::new();
    }
    let database = database::Database::new(&path);
    let include_clip = include_clip.unwrap_or(false);
    let include_videos = include_videos.unwrap_or(false);
    let mut views = siegu_core::duplicates::detect_all_view(&database, false, false);
    if include_clip {
        let config = database.get_state();
        if let Some(model) = siegu_core::ml_engine::models::load_clip_visual_only(&path, &config) {
            let (clip_views, _) =
                detect_clip_with_backfill(&database, &model, include_videos, &mut |_, _, _| {});
            views.extend(clip_views);
        }
    }
    views
}

#[tauri::command]
pub async fn duplicate_stats(
    app: tauri::AppHandle,
    include_clip: Option<bool>,
    include_videos: Option<bool>,
) -> DuplicateStats {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Default::default();
    }
    let database = database::Database::new(&path);
    siegu_core::duplicates::duplicate_stats(
        &database,
        include_clip.unwrap_or(false),
        include_videos.unwrap_or(false),
    )
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
