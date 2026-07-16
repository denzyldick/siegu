use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use tauri::Emitter;
use tauri::Manager;

use siegu_core::{
    generate_pairing_codes as core_generate_pairing_codes,
    hash_pairing_code as core_hash_pairing_code, PairingCodes as CorePairingCodes,
};

pub use siegu_core::database;

mod file;
mod ml;
#[cfg(test)]
mod test;
mod transport;
mod wallpaper_plugin;

struct WebRtcState {
    active_session: std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    sync_tx:
        Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<transport::SyncMessage>>>>,
}

struct ScanState {
    guard: siegu_core::ScanGuard,
}

struct ShutdownState {
    coordinator: siegu_core::shutdown::ShutdownCoordinator,
}

impl Default for ShutdownState {
    fn default() -> Self {
        Self {
            coordinator: siegu_core::shutdown::ShutdownCoordinator::new(),
        }
    }
}

fn get_config_path(app: &tauri::AppHandle) -> String {
    app.path()
        .app_config_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "".to_string())
}

#[tauri::command]
fn scan_files(app: tauri::AppHandle) {
    let _session = {
        let scan_state = app.state::<ScanState>();
        scan_state.guard.try_start()
    };
    let _session = match _session {
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

    // Shared batcher for all folders in this scan session
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

    // Helper to flush a batch: store in DB, push to UI buffer, send AI jobs
    async fn flush_batch_to_db_and_ui(
        database: &Arc<std::sync::Mutex<database::Database>>,
        app_handle: &tauri::AppHandle,
        ui_buffer: &Arc<tokio::sync::Mutex<Vec<database::Photo>>>,
        batch: Vec<database::Photo>,
    ) {
        // Clone for blocking work (thumbnails + DB insert)
        let db_clone = Arc::clone(database);
        let batch_for_blocking = batch.clone();
        let app_for_blocking = app_handle.clone();
        let app_for_error = app_handle.clone();
        let batch_with_thumbs = tauri::async_runtime::spawn_blocking(move || {
            let mut batch = batch_for_blocking;
            // Generate thumbnails
            for photo in &mut batch {
                if !siegu_core::thumbnail::needs_thumbnail(&photo.encoded) {
                    continue;
                }
                if let Some(data_url) = siegu_core::thumbnail::generate_thumbnail(&photo.location) {
                    photo.encoded = data_url;
                }
            }
            // Store in DB
            if let Ok(mut db) = db_clone.lock() {
                if let Err(e) = db.store_photo_batch(&batch) {
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
            batch
        })
        .await
        .unwrap_or_else(|join_err| {
            emit_log(
                &app_for_error,
                format!("[batch] spawn_blocking JOIN ERROR: {join_err}"),
            );
            batch
        });

        // Push all to UI buffer (with thumbnails)
        {
            let mut buf = ui_buffer.lock().await;
            for p in &batch_with_thumbs {
                buf.push(p.clone());
            }
        }
    }

    tauri::async_runtime::spawn(async move {
        let mut batch_accum: Vec<database::Photo> = Vec::new();
        while let Some(photo) = batch_rx.recv().await {
            batch_accum.push(photo);

            if batch_accum.len() >= 500 {
                let batch = std::mem::take(&mut batch_accum);
                flush_batch_to_db_and_ui(&database, &app_handle_for_batch, &ui_buffer, batch).await;
            }
        }

        if !batch_accum.is_empty() {
            let batch = std::mem::take(&mut batch_accum);
            flush_batch_to_db_and_ui(&database, &app_handle_for_batch, &ui_buffer, batch).await;
        }

        emit_log(
            &app_handle_for_batch,
            "[batch] receiver exited (all senders dropped)".to_string(),
        );
    });

    let abort_flag = Arc::clone(&state.abort);
    let batch_tx_shared = Arc::new(batch_tx);

    std::thread::spawn(move || {
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
        // Check if any AI models are enabled
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

        // Final signal to process everything found in the discovery pass
        if let Some(state) = app.try_state::<ml::MlContext>() {
            let _ = state.tx.send(ml::Job::ProcessAll);
        }
    });
}

#[tauri::command]
async fn check_models(app: tauri::AppHandle) -> Vec<String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Vec::new();
    }
    let models_dir = Path::new(&path).join("models");
    siegu_core::model_manager::check_models_downloaded(&models_dir)
}

#[derive(serde::Serialize, Clone)]
struct DownloadProgress {
    model: String,
    downloaded: u64,
    total: Option<u64>,
}

#[tauri::command]
async fn download_models(
    app: tauri::AppHandle,
    models: Vec<String>,
    state: tauri::State<'_, ml::MlContext>,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;

    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Could not resolve config dir".to_string());
    }
    let models_dir = std::path::PathBuf::from(&path).join("models");
    std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

    let resolved = siegu_core::model_manager::resolve_files_for_models(&models);
    let files_to_download: Vec<(String, String, String, String)> = resolved
        .iter()
        .map(|(entry, _)| {
            (
                entry.model_name.to_string(),
                entry.url.to_string(),
                entry.filename.to_string(),
                entry.sha256.to_string(),
            )
        })
        .collect();

    let tx = state.tx.clone();

    tauri::async_runtime::spawn(async move {
        emit_log(
            &app,
            format!(
                "Download sequence started. Queue size: {}",
                files_to_download.len()
            ),
        );

        let client = match reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36")
            .timeout(std::time::Duration::from_secs(600))
            .connect_timeout(std::time::Duration::from_secs(60))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build() {
                Ok(c) => c,
                Err(e) => {
                    emit_log(&app, format!("ERROR: Failed to create HTTP client: {e}"));
                    return;
                }
            };

        for (model_name, url, filename, expected_hash) in files_to_download {
            let path = models_dir.join(&filename);
            emit_log(&app, format!("Initiating download: {filename}"));
            let mut response = match client.get(&url).send().await {
                Ok(r) => {
                    emit_log(
                        &app,
                        format!("Response received for {}: Status {}", filename, r.status()),
                    );
                    r
                }
                Err(e) => {
                    emit_log(&app, format!("ERROR: Request failed for {filename}: {e}"));
                    continue;
                }
            };

            if !response.status().is_success() {
                emit_log(
                    &app,
                    format!(
                        "ERROR: Download failed for {filename}: Status {}",
                        response.status()
                    ),
                );
                continue;
            }
            let total_size = response.content_length();
            let tmp_path = path.with_extension("tmp");
            let mut file = match tokio::fs::File::create(&tmp_path).await {
                Ok(f) => f,
                Err(e) => {
                    emit_log(
                        &app,
                        format!("ERROR: Failed to create temp file {filename}: {e}"),
                    );
                    continue;
                }
            };
            let mut downloaded: u64 = 0;
            let mut last_emitted: u64 = 0;
            let mut success = true;
            while let Ok(Some(chunk)) = response.chunk().await {
                if (file.write_all(&chunk).await).is_err() {
                    success = false;
                    break;
                }
                downloaded += chunk.len() as u64;

                // Throttle: emit only every 1MB or at 100%
                if downloaded - last_emitted > 1024 * 1024 || Some(downloaded) == total_size {
                    last_emitted = downloaded;
                    let _ = app.emit(
                        "download-progress",
                        DownloadProgress {
                            model: model_name.clone(),
                            downloaded,
                            total: total_size,
                        },
                    );
                }
            }

            if success {
                drop(file);
                if let Err(e) = tokio::fs::rename(&tmp_path, &path).await {
                    emit_log(&app, format!("ERROR: Failed to move {filename}: {e}"));
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                } else {
                    if !expected_hash.is_empty() {
                        match siegu_core::model_manager::verify_sha256(&path, &expected_hash) {
                            Ok(true) => {
                                emit_log(&app, format!("SUCCESS: Finished downloading {filename} (SHA-256 verified)"));
                            }
                            Ok(false) => {
                                emit_log(
                                    &app,
                                    format!("ERROR: SHA-256 mismatch for {filename}, deleting"),
                                );
                                let _ = tokio::fs::remove_file(&path).await;
                            }
                            Err(e) => {
                                emit_log(
                                    &app,
                                    format!("WARNING: Could not verify hash for {filename}: {e}"),
                                );
                            }
                        }
                    } else {
                        emit_log(&app, format!("SUCCESS: Finished downloading {filename}"));
                    }
                }
            } else {
                let _ = tokio::fs::remove_file(&tmp_path).await;
                emit_log(&app, format!("ERROR: Download interrupted for {filename}"));
            }
        }
        // Force engine re-init to load new models
        let _ = tx.send(ml::Job::ProcessAll);
        let _ = app.emit("download-complete", ());
    });

    Ok(())
}

#[tauri::command]
#[allow(non_snake_case)]
async fn list_files(
    app: tauri::AppHandle,
    offset: usize,
    limit: usize,
    query: String,
    scan: bool,
    favoritesOnly: bool,
    videosOnly: bool,
) -> Result<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Ok("[]".to_string());
    }
    if scan {
        scan_files(app.clone());
    }
    let database = database::Database::new(&path);
    Ok(serde_json::to_string(&database.list_photos(
        &query,
        offset,
        limit,
        favoritesOnly,
        videosOnly,
    ))
    .unwrap_or("[]".to_string()))
}

#[tauri::command]
async fn get_last_scan_time(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "Never".to_string();
    }
    let database = database::Database::new(&path);
    database.get_last_scan_time().unwrap_or("Never".to_string())
}

#[tauri::command]
async fn toggle_favorite(app: tauri::AppHandle, id: String) -> bool {
    let path = get_config_path(&app);
    if path.is_empty() {
        return false;
    }
    let database = database::Database::new(&path);
    database.toggle_favorite(&id)
}

#[tauri::command]
async fn add_directory(app: tauri::AppHandle, path: String) {
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return;
    }
    let database = database::Database::new(&config_path);
    database.add_directory(&path);
}

#[tauri::command]
async fn list_directories(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&database.list_directories()).unwrap_or("[]".to_string())
}

#[tauri::command]
async fn remove_directory(app: tauri::AppHandle, path: String) {
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return;
    }
    let database = database::Database::new(&config_path);
    database.remove_directory(path);
}

#[tauri::command]
async fn read_file_base64(app: tauri::AppHandle, path: String) -> String {
    file::read_file_base64(&app, path)
}

#[tauri::command]
async fn get_raw_photo(app: tauri::AppHandle, path: String) -> String {
    file::read_file_base64(&app, path)
}

#[tauri::command]
async fn get_people(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&database.get_people()).unwrap_or("[]".to_string())
}

#[tauri::command]
async fn get_unnamed_faces(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&database.get_anonymous_people_groups()).unwrap_or("[]".to_string())
}

#[tauri::command]
fn assign_name_to_face(
    app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
    face_id: String,
    name: String,
) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "".to_string();
    }
    let database = database::Database::new(&path);
    let id = database.assign_name_to_face(&face_id, &name);

    let _ = state.tx.send(ml::Job::ProcessAll);
    id
}

#[tauri::command]
async fn get_person_photos(app: tauri::AppHandle, person_id: String) -> Result<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Ok("[]".to_string());
    }
    let database = database::Database::new(&path);
    Ok(
        serde_json::to_string(&database.get_photos_for_person(&person_id))
            .unwrap_or("[]".to_string()),
    )
}

#[tauri::command]
async fn is_initialized(app: tauri::AppHandle) -> bool {
    let path = get_config_path(&app);
    if path.is_empty() {
        return false;
    }
    let database = database::Database::new(&path);
    !database.list_directories().is_empty()
}

#[tauri::command]
async fn get_person_faces(app: tauri::AppHandle, person_id: String) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&database.get_person_faces(&person_id)).unwrap_or("[]".to_string())
}

#[tauri::command]
async fn get_faces_for_photo(app: tauri::AppHandle, photo_id: String) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&database.get_faces_for_photo(&photo_id)).unwrap_or("[]".to_string())
}

#[tauri::command]
async fn delete_face(app: tauri::AppHandle, face_id: String) {
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let database = database::Database::new(&path);
    let _ = database
        .connection
        .execute("DELETE FROM faces WHERE face_id = ?1", [&face_id]);
}

#[tauri::command]
async fn get_top_tags(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    let mut suggestions: Vec<database::SearchSuggestion> = Vec::new();

    if let Ok(mut stmt) = database
        .connection
        .prepare("SELECT class FROM object GROUP BY class ORDER BY COUNT(*) DESC LIMIT 5")
    {
        if let Ok(iter) = stmt.query_map([], |row| {
            Ok(database::SearchSuggestion {
                title: row.get(0)?,
                suggestion_type: "tag".to_string(),
            })
        }) {
            for item in iter.flatten() {
                suggestions.push(item);
            }
        }
    }

    if let Ok(mut stmt) = database
        .connection
        .prepare("SELECT value FROM properties WHERE key = 'location_name' GROUP BY value ORDER BY COUNT(*) DESC LIMIT 5")
    {
        if let Ok(iter) = stmt.query_map([], |row| {
            Ok(database::SearchSuggestion {
                title: row.get(0)?,
                suggestion_type: "location".to_string(),
            })
        }) {
            for item in iter.flatten() {
                suggestions.push(item);
            }
        }
    }

    if let Ok(mut stmt) = database
        .connection
        .prepare("SELECT name FROM people WHERE name IS NOT NULL GROUP BY name ORDER BY COUNT(*) DESC LIMIT 5")
    {
        if let Ok(iter) = stmt.query_map([], |row| {
            Ok(database::SearchSuggestion {
                title: row.get(0)?,
                suggestion_type: "person".to_string(),
            })
        }) {
            for item in iter.flatten() {
                suggestions.push(item);
            }
        }
    }

    serde_json::to_string(&suggestions).unwrap_or("[]".to_string())
}

#[tauri::command]
fn merge_people(
    app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
    from_id: String,
    to_id: String,
) {
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let db = database::Database::new(&path);
    db.merge_people(&from_id, &to_id);

    let _ = state.tx.send(ml::Job::ProcessAll);
}

#[tauri::command]
async fn rename_person(app: tauri::AppHandle, id: String, new_name: String) {
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let db = database::Database::new(&path);
    db.rename_person(&id, &new_name);
}

#[tauri::command]
async fn cleanup_database(app: tauri::AppHandle, confirm: bool) {
    if !confirm {
        return;
    }
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let db_path = std::path::Path::new(&path).join("siegu.db");
    if db_path.exists() {
        let _ = std::fs::remove_file(db_path);
    }
}

#[tauri::command]
async fn remove_directory_full(app: tauri::AppHandle, path: String) {
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return;
    }
    let mut db = database::Database::new(&config_path);
    db.remove_directory_full(&path);
}

#[tauri::command]
#[allow(non_snake_case)]
async fn start_webrtc_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, WebRtcState>,
    roomId: String,
    isInitiator: bool,
    signalingUrl: String,
) -> Result<(), String> {
    let app_handle = app.clone();
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return Err("Config error".to_string());
    }

    // Check tier: free tier cannot use remote signaling servers
    let db = database::Database::new(&config_path);
    let config = db.get_state();
    let tier = config.get("tier").map(|s| s.as_str()).unwrap_or("free");
    if tier == "free" && !signalingUrl.contains("127.0.0.1") && !signalingUrl.contains("localhost")
    {
        return Err("Free tier does not support remote sync. Use LAN sync instead.".to_string());
    }

    // Abort existing session if any
    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            emit_log(&app, "Aborting previous WebRTC session".to_string());
            handle.abort();
        }

        let sync_tx_inner = Arc::clone(&state.sync_tx);

        let handle = tauri::async_runtime::spawn(async move {
            let client = transport::WebRtcClient {
                room_id: roomId,
                is_initiator: isInitiator,
                signaling_url: signalingUrl,
                app_handle: Some(app_handle),
                config_path,
                sync_tx: sync_tx_inner,
            };
            let _ = client.start().await;
        });

        *session = Some(handle);
    }

    Ok(())
}

#[tauri::command]
async fn start_lan_host(
    app: tauri::AppHandle,
    state: tauri::State<'_, WebRtcState>,
    room_id: String,
    is_initiator: bool,
) -> Result<(), String> {
    let app_handle = app.clone();
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return Err("Config error".to_string());
    }

    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            emit_log(&app, "Aborting previous WebRTC session".to_string());
            handle.abort();
        }

        let sync_tx_inner = Arc::clone(&state.sync_tx);

        let handle = tauri::async_runtime::spawn(async move {
            let client = transport::WebRtcClient {
                room_id,
                is_initiator,
                signaling_url: String::new(),
                app_handle: Some(app_handle),
                config_path,
                sync_tx: sync_tx_inner,
            };
            let _ = client.start_lan(0).await;
        });

        *session = Some(handle);
    }

    Ok(())
}

#[tauri::command]
async fn discover_lan_devices(
    app: tauri::AppHandle,
    timeout_secs: u64,
) -> Result<Vec<siegu_core::mdns::DiscoveredHost>, String> {
    let daemon = siegu_core::mdns::create_daemon().map_err(|e| e.to_string())?;
    let hosts =
        siegu_core::mdns::discover_hosts(&daemon, timeout_secs).map_err(|e| e.to_string())?;
    emit_log(&app, format!("Discovered {} LAN device(s)", hosts.len()));
    Ok(hosts)
}

#[tauri::command]
async fn stop_webrtc_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, WebRtcState>,
) -> Result<(), String> {
    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            emit_log(&app, "Stopping WebRTC session".to_string());
            handle.abort();
        }
    }
    {
        let mut tx = state.sync_tx.lock().await;
        *tx = None;
    }
    Ok(())
}

#[tauri::command]
fn get_indexing_status(state: tauri::State<'_, ml::MlContext>) -> usize {
    let count = state
        .pending_count
        .load(std::sync::atomic::Ordering::SeqCst);
    if count > 1_000_000 {
        0
    } else {
        count
    }
}

#[tauri::command]
fn get_unindexed_count(app: tauri::AppHandle) -> usize {
    let path = get_config_path(&app);
    if path.is_empty() {
        return 0;
    }
    let db = database::Database::new(&path);
    let count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM photo WHERE indexed < 2", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    count as usize
}

#[tauri::command]
async fn join_network(app: tauri::AppHandle, ip: String, name: String) {
    emit_log(&app, format!("Adding new device: {name} at {ip}"));
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let db = database::Database::new(&path);
    let _ = db.connection.execute(
        "INSERT OR REPLACE INTO device(ip, name) VALUES(?1, ?2)",
        (ip, name),
    );
}

#[tauri::command]
async fn remove_device(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Config error".to_string());
    }
    let db = database::Database::new(&path);
    db.connection
        .execute("DELETE FROM device WHERE name = ?1", [name])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn list_devices(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let db = database::Database::new(&path);
    let mut devices = db.list_devices();

    // Add current host
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let (photo_count, video_count) = db.get_media_counts();

    devices.insert(
        0,
        database::DeviceInfo {
            id: "host".to_string(),
            title: format!("Siegu ({hostname})"),
            icon: "mdi-laptop".to_string(),
            up_to_date: true,
            host: true,
            photo_count,
            video_count,
            os: std::env::consts::OS.to_string(),
        },
    );

    serde_json::to_string(&devices).unwrap_or("[]".to_string())
}

#[tauri::command]
async fn list_objects(app: tauri::AppHandle, query: String) -> Result<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Ok("[]".to_string());
    }
    let db = database::Database::new(&path);
    Ok(serde_json::to_string(&db.list_objects(&query)).unwrap_or("[]".to_string()))
}

#[tauri::command]
fn process_video_frames(
    _app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
    id: String,
    frames: Vec<String>,
) {
    if frames.len() > 1000 || id.len() > 64 {
        return;
    }
    if frames.iter().any(|f| f.len() > 512) {
        return;
    }
    let _ = state.tx.send(ml::Job::AnalyzeSingle(id));
}

#[tauri::command]
async fn get_media_server_port(app: tauri::AppHandle) -> u16 {
    app.state::<transport::MediaServerState>().port
}

#[tauri::command]
async fn index_faces(
    app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
) -> Result<(), String> {
    emit_log(&app, "Face indexing requested...".to_string());
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Config error".to_string());
    }
    let db = database::Database::new(&path);

    // Force indexing_mode to immediate so the worker actually processes the items
    let mut state_map = std::collections::HashMap::new();
    state_map.insert("indexing_mode".to_string(), "immediate".to_string());
    db.set_state(state_map);

    let _ = state
        .tx
        .send(ml::Job::ProcessModel("ultraface".to_string()));
    Ok(())
}

#[tauri::command]
async fn analyze_photo(state: tauri::State<'_, ml::MlContext>, id: String) -> Result<(), String> {
    state.abort.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = state.tx.send(ml::Job::AnalyzeSingle(id));
    Ok(())
}

#[tauri::command]
async fn analyze_photo_model(
    state: tauri::State<'_, ml::MlContext>,
    id: String,
    model_id: String,
) -> Result<(), String> {
    state.abort.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = state.tx.send(ml::Job::AnalyzeSingleWithModel(id, model_id));
    Ok(())
}

#[tauri::command]
async fn analyze_model(
    state: tauri::State<'_, ml::MlContext>,
    model_id: String,
) -> Result<(), String> {
    let _ = state.tx.send(ml::Job::ProcessModel(model_id));
    Ok(())
}

#[tauri::command]
async fn abort_indexing(state: tauri::State<'_, ml::MlContext>) -> Result<(), String> {
    state.abort.store(true, std::sync::atomic::Ordering::SeqCst);
    state
        .pending_count
        .store(0, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
async fn get_heatmap_data(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    let points = database.get_heatmap_points();
    emit_log(
        &app,
        format!("DEBUG: Found {} photos with GPS for heatmap", points.len()),
    );
    serde_json::to_string(&points).unwrap_or("[]".to_string())
}

#[tauri::command]
async fn get_photo_encoded_batch(
    app: tauri::AppHandle,
    ids: Vec<String>,
) -> std::collections::HashMap<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() || ids.is_empty() {
        return std::collections::HashMap::new();
    }
    let database = database::Database::new(&path);
    database.get_photo_encoded_batch(&ids)
}

#[tauri::command]
async fn get_photo_by_id(app: tauri::AppHandle, id: String) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "null".to_string();
    }
    let database = database::Database::new(&path);
    match database.get_photo_by_id(&id) {
        Some(photo) => serde_json::to_string(&photo).unwrap_or("null".to_string()),
        None => "null".to_string(),
    }
}

#[tauri::command]
async fn get_photos_for_map_click(app: tauri::AppHandle, ids: Vec<String>) -> String {
    let path = get_config_path(&app);
    if path.is_empty() || ids.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    let photos = database.get_photos_by_ids(&ids);
    serde_json::to_string(&photos).unwrap_or("[]".to_string())
}

#[tauri::command]
async fn get_os() -> String {
    std::env::consts::OS.to_string()
}

#[tauri::command]
async fn set_wallpaper(app: tauri::AppHandle, path: String) -> Result<(), String> {
    set_wallpaper_impl(&app, &path)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn set_wallpaper_impl(_app: &tauri::AppHandle, path: &str) -> Result<(), String> {
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        if desktop.contains("COSMIC") {
            return set_cosmic_wallpaper(path);
        }
    }

    if let Err(e) = wallpaper::set_from_path(path) {
        let uri = format!("\"file://{}\"", path);
        let output = std::process::Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.desktop.background")
            .arg("picture-uri")
            .arg(&uri)
            .output()
            .map_err(|e| format!("Failed to run gsettings: {}", e))?;

        if output.status.success() {
            return Ok(());
        }

        return Err(format!(
            "wallpaper crate: {}; gsettings: {}",
            e,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn set_cosmic_wallpaper(path: &str) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let config_dir = std::path::Path::new(&home)
        .join(".config")
        .join("cosmic")
        .join("com.system76.CosmicBackground")
        .join("v1");

    let content = format!(
        "(\n    output: \"all\",\n    source: Path(\"{}\"),\n    filter_by_theme: true,\n    rotation_frequency: 300,\n    filter_method: Lanczos,\n    scaling_mode: Zoom,\n    sampling_method: Alphanumeric,\n)",
        path
    );

    std::fs::write(config_dir.join("all"), &content)
        .map_err(|e| format!("Failed to write COSMIC background config: {}", e))?;

    std::fs::write(config_dir.join("same-on-all"), "true")
        .map_err(|e| format!("Failed to write COSMIC same-on-all config: {}", e))?;

    Ok(())
}

#[cfg(target_os = "android")]
fn set_wallpaper_impl(app: &tauri::AppHandle, path: &str) -> Result<(), String> {
    app.run_mobile_plugin(
        "wallpaper",
        "setWallpaper",
        serde_json::json!({"path": path}),
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[cfg(target_os = "ios")]
fn set_wallpaper_impl(_app: &tauri::AppHandle, _path: &str) -> Result<(), String> {
    Err("Setting wallpaper is not supported on this platform".to_string())
}

#[tauri::command]
async fn resolve_photo_locations(app: tauri::AppHandle) -> Result<(), String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Config path empty".to_string());
    }
    let db = database::Database::new(&path);
    let sql = "SELECT id, latitude, longitude FROM photo WHERE (latitude != 0.0 OR longitude != 0.0) AND id NOT IN (SELECT photo_id FROM properties WHERE key = 'location_name')";
    let mut resolved = 0usize;
    if let Ok(mut stmt) = db.connection.prepare(sql) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0),
            ))
        }) {
            for row in rows.flatten() {
                let (id, lat, lon) = row;
                if let Some((city, country)) = siegu_core::geocode::find_nearest_city(lat, lon) {
                    let location_name = format!("{}, {}", city, country);
                    let _ = db.connection.execute(
                        "INSERT OR REPLACE INTO properties (photo_id, key, value) VALUES (?1, 'location_name', ?2)",
                        rusqlite::params![id, location_name],
                    );
                    resolved += 1;
                }
            }
        }
    }
    emit_log(&app, format!("Resolved {} photo locations", resolved));
    Ok(())
}

#[tauri::command]
async fn get_location_names(app: tauri::AppHandle) -> Vec<String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Vec::new();
    }
    let db = database::Database::new(&path);
    let mut names = Vec::new();
    if let Ok(mut stmt) = db
        .connection
        .prepare("SELECT DISTINCT value FROM properties WHERE key = 'location_name' ORDER BY value")
    {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                names.push(row);
            }
        }
    }
    names
}

#[tauri::command]
async fn initialize_sync_folder(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let target = std::path::PathBuf::from(&path).join("siegu");
    if let Err(e) = std::fs::create_dir_all(&target) {
        return Err(format!("Failed to create folder at {target:?}: {e}"));
    }
    // Also add it to authorized directories
    let path_clone = path.clone();
    add_directory(app, path_clone).await;
    Ok(())
}

#[tauri::command]
async fn save_config(app: tauri::AppHandle, key: String, value: String) {
    if let Err(e) = siegu_core::config::validate_config_value(&key, &value) {
        emit_log(&app, format!("Invalid config: {e}"));
        return;
    }
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let db = database::Database::new(&path);
    let mut state = HashMap::new();
    state.insert(key, value);
    db.set_state(state);
}

#[tauri::command]
async fn get_config(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "{}".to_string();
    }
    let db = database::Database::new(&path);
    serde_json::to_string(&db.get_state()).unwrap_or("{}".to_string())
}

#[tauri::command]
async fn get_logs(app: tauri::AppHandle, limit: usize) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&database.get_logs(limit)).unwrap_or("[]".to_string())
}

#[tauri::command]
async fn clear_logs(app: tauri::AppHandle) {
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let database = database::Database::new(&path);
    database.clear_logs();
}

pub fn emit_log(app: &tauri::AppHandle, message: String) {
    tracing::info!("{}", message);
    let _ = app.emit("log-message", message.clone());
    let path = get_config_path(app);
    if !path.is_empty() {
        let database = database::Database::new(&path);
        let upper = message.to_uppercase();
        let level = if upper.contains("ERROR") || upper.contains("FATAL") {
            "error"
        } else if upper.contains("WARN") || upper.contains("WARNING") {
            "warn"
        } else if upper.contains("DEBUG") {
            "debug"
        } else {
            "info"
        };
        database.store_log(level, &message);
    }
}

#[tauri::command]
async fn request_start_sync(state: tauri::State<'_, WebRtcState>) -> Result<(), String> {
    let mut tx_lock = state.sync_tx.lock().await;
    if let Some(tx) = tx_lock.as_mut() {
        tx.send(transport::SyncMessage::StartSync)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(desktop)]
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

#[tauri::command]
async fn generate_pairing_codes() -> Result<CorePairingCodes, String> {
    core_generate_pairing_codes()
}

#[tauri::command]
async fn hash_pairing_code(input: String) -> Result<String, String> {
    core_hash_pairing_code(input)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(wallpaper_plugin::init())
        .setup(|app| {
            if let Err(e) = ffmpeg_next::init() {
                emit_log(
                    app.handle(),
                    format!("WARNING: ffmpeg init failed (thumbnails for videos disabled): {e}"),
                );
            }

            #[cfg(desktop)]
            {
                let show_i = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
                let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

                let _tray = TrayIconBuilder::new()
                    .menu(&menu)
                    .icon(app.default_window_icon().unwrap().clone())
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "quit" => {
                            if let Some(state) = app.try_state::<ShutdownState>() {
                                state.coordinator.signal();
                            }
                            app.exit(0);
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app)?;

                #[cfg(target_os = "linux")]
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.with_webview(|webview| {
                        use webkit2gtk::{WebContextExt, WebViewExt};
                        let wv = webview.inner();
                        if let Some(ctx) = wv.context() {
                            ctx.set_spell_checking_enabled(false);
                        }
                    });
                }
            }

            emit_log(
                app.handle(),
                "App is setting up background tasks...".to_string(),
            );
            use tauri_plugin_notification::NotificationExt;
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = app_handle
                    .notification()
                    .builder()
                    .title("Siegu")
                    .body("Siegu is running in the background")
                    .show();
            });

            let config_path = get_config_path(app.handle());
            let ml_context = ml::start_background_worker(app.handle(), config_path.clone());
            app.manage(ml_context);

            let media_server_port = transport::start_media_server(config_path);
            app.manage(transport::MediaServerState {
                port: media_server_port,
            });

            app.manage(WebRtcState {
                active_session: std::sync::Mutex::new(None),
                sync_tx: Arc::new(tokio::sync::Mutex::new(None)),
            });

            app.manage(ScanState {
                guard: siegu_core::ScanGuard::new(),
            });

            app.manage(ShutdownState::default());

            // Start periodic background scan
            let app_handle_for_interval = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 1 hour
                loop {
                    emit_log(
                        &app_handle_for_interval,
                        "Interval tick: checking for media updates...".to_string(),
                    );
                    interval.tick().await;
                    scan_files(app_handle_for_interval.clone());
                }
            });

            // Start real-time filesystem watcher
            let app_handle_for_watcher = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                file::start_watcher(app_handle_for_watcher).await;
            });

            Ok(())
        })
        .on_window_event(|_window, event| match event {
            #[cfg(desktop)]
            tauri::WindowEvent::CloseRequested { api, .. } => {
                _window.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            scan_files,
            check_models,
            download_models,
            get_logs,
            clear_logs,
            list_files,
            get_last_scan_time,
            toggle_favorite,
            add_directory,
            list_directories,
            remove_directory,
            read_file_base64,
            get_raw_photo,
            get_people,
            get_unnamed_faces,
            assign_name_to_face,
            get_person_photos,
            rename_person,
            merge_people,
            is_initialized,
            get_top_tags,
            get_person_faces,
            get_faces_for_photo,
            delete_face,
            join_network,
            remove_device,
            list_devices,
            list_objects,
            generate_pairing_codes,
            hash_pairing_code,
            start_webrtc_session,
            start_lan_host,
            discover_lan_devices,
            stop_webrtc_session,
            request_start_sync,
            process_video_frames,
            merge_people,
            rename_person,
            cleanup_database,
            remove_directory_full,
            get_media_server_port,
            index_faces,
            abort_indexing,
            get_os,
            set_wallpaper,
            save_config,
            get_config,
            get_indexing_status,
            get_unindexed_count,
            get_heatmap_data,
            get_photo_by_id,
            get_photo_encoded_batch,
            get_photos_for_map_click,
            initialize_sync_folder,
            analyze_photo,
            analyze_photo_model,
            analyze_model,
            resolve_photo_locations,
            get_location_names,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
