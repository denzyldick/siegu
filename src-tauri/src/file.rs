use crate::common::{emit_log, get_config_path};
use crate::database;
use base64::{engine::general_purpose, Engine as _};
use rand::{distributions::Alphanumeric, Rng};

use notify::event::{CreateKind, ModifyKind};
use notify::{EventKind, RecursiveMode, Watcher};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ml::MlContext;
use tauri::Emitter;
use tauri::Manager;
use tokio::sync::mpsc::Sender;

/// Watcher-triggered rescans wait at least this long after the previous one so
/// bulk imports (thousands of files landing over minutes) don't restart a full
/// walk after every 10-second lull.
const WATCHER_RESCAN_MIN_INTERVAL_MS: u64 = 60_000;

/// Entries are extracted and handed to the batch writer in bounded chunks so
/// peak memory stays flat regardless of library size.
const SCAN_CHUNK_SIZE: usize = 2048;

/// Minimum interval between discovery progress events.
const DISCOVERY_PROGRESS_INTERVAL_MS: u64 = 500;

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Timestamp (unix ms) of the last watcher-initiated rescan.
static LAST_WATCHER_SCAN_MS: AtomicU64 = AtomicU64::new(0);

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

    let config_path = get_config_path(&app);
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

        while let Some(event) = rx.recv().await {
            match event.kind {
                EventKind::Create(CreateKind::File)
                | EventKind::Modify(ModifyKind::Name(_))
                | EventKind::Modify(ModifyKind::Data(_)) => {
                    let needs_scan = event
                        .paths
                        .iter()
                        .any(|p| siegu_core::scanner::is_media_file(p));
                    if needs_scan {
                        let now_ms = unix_now_ms();
                        let last = LAST_WATCHER_SCAN_MS.load(Ordering::Relaxed);
                        if now_ms.saturating_sub(last) < WATCHER_RESCAN_MIN_INTERVAL_MS {
                            continue;
                        }
                        if LAST_WATCHER_SCAN_MS
                            .compare_exchange(last, now_ms, Ordering::SeqCst, Ordering::Relaxed)
                            .is_err()
                        {
                            continue;
                        }

                        use tauri_plugin_notification::NotificationExt;
                        let _ = app_clone
                            .notification()
                            .builder()
                            .title("Siegu")
                            .body("New media detected, scanning...")
                            .show();

                        crate::commands::scan::scan_files(app_clone.clone());
                    }
                }
                _ => {}
            }
        }
    });
}

/// Emit a discovery progress event, throttled so a fast scan doesn't flood the
/// webview. Uses CAS on the last-emitted timestamp so concurrent extraction
/// threads can't emit more than one event per window.
fn emit_discovery_progress(
    app: &tauri::AppHandle,
    last_emit_ms: &AtomicU64,
    directory: &str,
    found: usize,
    force: bool,
) {
    let now_ms = unix_now_ms();
    let last = last_emit_ms.load(Ordering::Relaxed);
    if !force && now_ms.saturating_sub(last) < DISCOVERY_PROGRESS_INTERVAL_MS {
        return;
    }
    if last_emit_ms
        .compare_exchange(last, now_ms, Ordering::SeqCst, Ordering::Relaxed)
        .is_ok()
    {
        let _ = app.emit(
            "scan-progress",
            serde_json::json!({
                "status": "discovering",
                "files_found": found,
                "current_directory": directory,
            }),
        );
    }
}

/// Recursively discovers supported media files, stores new rows in batches, and queues AI work.
///
/// Discovery intentionally records metadata first. The heavier model inference and thumbnail
/// generation run on the ML worker so folder scans stay responsive and progress events remain regular.
///
/// The walk runs in two phases: jwalk collects media paths on its own dedicated rayon pool (so
/// directory traversal is never starved by extraction saturating the shared global pool), then
/// metadata extraction proceeds over bounded chunks of that list. Chunking keeps peak memory flat
/// and avoids the old streaming design where `par_bridge` + `blocking_send` could park every
/// worker when the batch channel filled, stalling the whole scan.
pub fn scan_folder(
    app: &tauri::AppHandle,
    directory: String,
    database: &Arc<std::sync::Mutex<database::Database>>,
    batch_tx: &Sender<database::Photo>,
) {
    let existing = {
        let db = database.lock().unwrap_or_else(|e| e.into_inner());
        db.existing_locations()
    };
    crate::common::debug_log(format!(
        "[scan_folder] Loaded {} existing paths from DB",
        existing.len()
    ));

    emit_log(app, "Looking for new photos…".to_string());

    let abort_flag = app
        .try_state::<MlContext>()
        .map(|s| s.abort.clone())
        .unwrap_or_else(|| Arc::new(std::sync::atomic::AtomicBool::new(false)));

    let abort_flag_task = Arc::clone(&abort_flag);

    use rayon::prelude::*;

    let total_new = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let total_new_clone = Arc::clone(&total_new);
    let last_emit_ms = AtomicU64::new(0);

    // Phase 1 — collect media paths on jwalk's dedicated pool.
    let media_paths: Vec<PathBuf> = jwalk::WalkDir::new(&directory)
        .follow_links(false)
        .parallelism(jwalk::Parallelism::RayonNewPool(4))
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| siegu_core::scanner::is_media_file(p))
        .collect();

    // Phase 2 — extract metadata chunk by chunk on the global pool, handing each
    // finished chunk to the batch writer before moving on.
    let mut result: Result<(), ()> = Ok(());
    for chunk in media_paths.chunks(SCAN_CHUNK_SIZE) {
        if abort_flag_task.load(Ordering::SeqCst) {
            result = Err(());
            break;
        }
        let extracted: Vec<Option<database::Photo>> = chunk
            .par_iter()
            .map(|file_path| {
                if abort_flag_task.load(Ordering::SeqCst) {
                    return None;
                }
                let path_str = file_path.display().to_string();
                if existing.contains(&path_str) {
                    return None;
                }

                let found = total_new_clone.fetch_add(1, Ordering::SeqCst) + 1;
                emit_discovery_progress(app, &last_emit_ms, &directory, found, false);

                let meta = siegu_core::scanner::extract_photo_metadata(file_path);
                let id: String = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(7)
                    .map(char::from)
                    .collect();

                Some(database::Photo {
                    id,
                    encoded: String::new(),
                    location: path_str,
                    created: meta.created,
                    objects: std::collections::HashMap::new(),
                    properties: meta.properties,
                    latitude: meta.latitude,
                    longitude: meta.longitude,
                    favorite: meta.favorite,
                    indexed: 0,
                    caption: meta.caption,
                    aesthetics_score: None,
                    ai_status: database::AiStatus::default(),
                    sync_needed: true,
                    received: false,
                })
            })
            .collect();

        for photo in extracted.into_iter().flatten() {
            if batch_tx.blocking_send(photo).is_err() {
                result = Err(());
                break;
            }
        }
        if result.is_err() {
            break;
        }
    }

    let total = total_new.load(std::sync::atomic::Ordering::SeqCst);
    if result.is_err() {
        emit_log(app, "Scan stopped.".to_string());
        crate::common::debug_log(format!("[scan_folder] Scan aborted for: {directory}"));
        return;
    }
    if total == 0 {
        emit_log(app, "No new photos found.".to_string());
        return;
    }
    // Final exact count so the UI counter never lags behind the throttled events.
    emit_discovery_progress(app, &last_emit_ms, &directory, total, true);
    emit_log(app, format!("Found {total} new photos."));
    crate::common::debug_log(format!(
        "[scan_folder] Sent {} photos to batch channel for: {directory}",
        total
    ));
}

#[tauri::command]
pub async fn read_file_base64(app: tauri::AppHandle, path: String) -> String {
    read_file_base64_inner(&app, path)
}

fn read_file_base64_inner(app: &tauri::AppHandle, path: String) -> String {
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
        crate::common::debug_log(format!(
            "Access denied: {path} is outside allowed directories"
        ));
        return String::new();
    }
    match std::fs::read(&path) {
        Ok(bytes) => {
            crate::common::debug_log(format!(
                "Reading original file: {} ({} bytes)",
                path,
                bytes.len()
            ));
            let encoded = general_purpose::STANDARD.encode(bytes);
            let ext = Path::new(&path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_lowercase();
            let mime = match ext.as_str() {
                "png" => "image/png",
                "heic" | "heif" => "image/heic",
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
            emit_log(app, format!("Couldn't read file {path}: {e}"));
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
