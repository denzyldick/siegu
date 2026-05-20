use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use tauri::Emitter;
use tauri::Manager;

mod database;
mod face_detector;
mod file;
mod geocode;
mod ml;
mod server;
#[cfg(test)]
mod test;
mod transport;

struct WebRtcState {
    active_session: std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    sync_tx:
        Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<transport::SyncMessage>>>>,
}

fn get_config_path(app: &tauri::AppHandle) -> String {
    app.path()
        .app_config_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "".to_string())
}

#[tauri::command]
fn scan_files(app: tauri::AppHandle) {
    println!("Starting media scan...");
    let path = get_config_path(&app);
    if path.is_empty() {
        println!("Error: Config path is empty, cannot scan.");
        return;
    }
    let database = database::Database::new(&path);
    let folders = database.list_directories();
    println!("Found {} folders to scan in database.", folders.len());

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

    // Initial signal to process any leftovers from previous runs
    let _ = state.tx.send(ml::Job::ProcessAll);

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
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                interval.tick().await;
                let batch = {
                    let mut buf = ui.lock().await;
                    let b = buf.clone();
                    buf.clear();
                    b
                };
                if !batch.is_empty() {
                    eprintln!("[ui-buffer] emitting {} photos", batch.len());
                    let _ = app.emit("photos-discovered", &batch);
                }
            }
        });
    }

    tauri::async_runtime::spawn(async move {
        while let Some(photo) = batch_rx.recv().await {
            let db_arc = Arc::clone(&database);
            let app_clone = app_handle_for_batch.clone();
            let ui = Arc::clone(&ui_buffer);
            let photo_for_db = photo.clone();
            let photo_id = photo.id.clone();

            eprintln!("[batch] saving photo {} to DB", photo_id);
            let _ = tauri::async_runtime::spawn_blocking(move || {
                if let Ok(mut db) = db_arc.lock() {
                    let _ = db.store_photo_batch(&[photo_for_db]);
                }
            }).await;

            eprintln!("[batch] pushing {} to UI buffer (size={})", photo_id, {
                let buf = ui.lock().await;
                buf.len()
            });
            ui.lock().await.push(photo);

            if let Some(state) = app_clone.try_state::<ml::MlContext>() {
                eprintln!("[batch] sending AnalyzeSingle({})", photo_id);
                let _ = state.tx.send(ml::Job::AutoAnalyzeSingle(photo_id));
            } else {
                eprintln!("[batch] WARN: MlContext state not available");
            }
        }
        eprintln!("[batch] receiver exited (all senders dropped)");
    });

    let abort_flag = Arc::clone(&state.abort);
    let batch_tx_shared = Arc::new(batch_tx);

    std::thread::spawn(move || {
        let total = folders.len();
        if total == 0 {
            println!("No folders to scan. Skipping scan thread.");
            return;
        }

        for (i, folder) in folders.iter().enumerate() {
            if abort_flag.load(std::sync::atomic::Ordering::SeqCst) {
                println!("Scan aborted by user.");
                return;
            }
            let progress = (i as f32 / total as f32 * 100.0) as u32;
            let _ = app.emit("scan-progress", serde_json::json!({ "status": "discovering", "progress": progress, "current": i + 1, "total": total, "current_directory": folder }));
            println!("Scanning folder {} of {}: {}", i + 1, total, folder);
            file::scan_folder(&app, folder.clone(), &path, &batch_tx_shared);
        }

        println!("Finished scanning all folders. Updating last scan time...");
        let _ = app.emit(
            "scan-progress",
            serde_json::json!({ "status": "indexing", "progress": 100, "message": "Analyzing photos with AI..." }),
        );

        let database = database::Database::new(&path);
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();
        database.set_last_scan_time(timestamp);

        use tauri_plugin_notification::NotificationExt;
        let _ = app
            .notification()
            .builder()
            .title("Siegu")
            .body("Files discovered, analyzing with AI...")
            .show();

        // Final signal to process everything found in the discovery pass
        if let Some(state) = app.try_state::<ml::MlContext>() {
            let _ = state.tx.send(ml::Job::ProcessAll);
        }
    });
}

#[tauri::command]
async fn check_models(app: tauri::AppHandle) -> Vec<String> {
    let path = get_config_path(&app);
    let mut downloaded = Vec::new();
    if path.is_empty() {
        return downloaded;
    }
    let models_dir = Path::new(&path).join("models");

    let clip_files = [
        "clip-vit-base-patch32-visual.onnx",
        "clip-vit-base-patch32-text.onnx",
        "tokenizer.json",
    ];
    let mut clip_ok = true;
    for name in clip_files {
        let p = models_dir.join(name);
        let min_size = match name {
            "clip-vit-base-patch32-visual.onnx" => 150 * 1024 * 1024,
            "clip-vit-base-patch32-text.onnx" => 40 * 1024 * 1024,
            _ => 1024, // tokenizer.json
        };

        if !p.exists() || p.metadata().map(|m| m.len()).unwrap_or(0) < min_size {
            clip_ok = false;
            break;
        }
    }
    if clip_ok {
        downloaded.push("clip".to_string());
    }

    let ultraface_path = models_dir.join("version-RFB-320.onnx");
    if ultraface_path.exists()
        && ultraface_path.metadata().map(|m| m.len()).unwrap_or(0) > 1024 * 1024
    {
        downloaded.push("ultraface".to_string());
    }

    let ocr_files = ["ocr_det.onnx", "ocr_rec.onnx"];
    let mut ocr_ok = true;
    for name in ocr_files {
        let p = models_dir.join(name);
        if !p.exists() || p.metadata().map(|m| m.len()).unwrap_or(0) < 1024 {
            ocr_ok = false;
            break;
        }
    }
    if ocr_ok {
        let dict_path = models_dir.join("en_dict.txt");
        if !dict_path.exists() {
            ocr_ok = false;
        }
    }
    if ocr_ok {
        downloaded.push("ocr".to_string());
    }

    if models_dir.join("nsfw.onnx").exists() {
        downloaded.push("nsfw".to_string());
    }
    if models_dir.join("aesthetics.onnx").exists() {
        downloaded.push("aesthetics".to_string());
    }
    if models_dir.join("yolov8.onnx").exists() {
        downloaded.push("yolo".to_string());
    }
    if models_dir.join("blip.onnx").exists() {
        downloaded.push("blip".to_string());
    }
    if models_dir.join("arcface.onnx").exists() {
        downloaded.push("arcface".to_string());
    }
    if models_dir.join("midas.onnx").exists() {
        downloaded.push("midas".to_string());
    }
    if models_dir.join("whisper.onnx").exists() {
        downloaded.push("whisper".to_string());
    }

    downloaded
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

    let mut files_to_download: Vec<(String, String, String)> = Vec::new();
    for model in &models {
        let m = model.to_lowercase();
        if m == "clip" {
            files_to_download.push(("clip-visual".to_string(), "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model.onnx".to_string(), "clip-vit-base-patch32-visual.onnx".to_string()));
            files_to_download.push(("clip-text".to_string(), "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/text_model.onnx".to_string(), "clip-vit-base-patch32-text.onnx".to_string()));
            files_to_download.push((
                "clip-tokenizer".to_string(),
                "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/tokenizer.json"
                    .to_string(),
                "tokenizer.json".to_string(),
            ));
        } else if m == "ultraface" {
            files_to_download.push(("ultraface".to_string(), "https://raw.githubusercontent.com/Linzaer/Ultra-Light-Fast-Generic-Face-Detector-1MB/master/models/onnx/version-RFB-320.onnx".to_string(), "version-RFB-320.onnx".to_string()));
        } else if m == "ocr" {
            files_to_download.push(("ocr-det".to_string(), "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/en_PP-OCRv3_det_infer.onnx".to_string(), "ocr_det.onnx".to_string()));
            files_to_download.push(("ocr-rec".to_string(), "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv3/en_PP-OCRv3_rec_infer.onnx".to_string(), "ocr_rec.onnx".to_string()));
            files_to_download.push(("ocr-dict".to_string(), "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.6/ppocr/utils/en_dict.txt".to_string(), "en_dict.txt".to_string()));
        } else if m == "nsfw" {
            files_to_download.push(("nsfw".to_string(), "https://huggingface.co/onnx-community/nsfw_image_detection-ONNX/resolve/main/onnx/model.onnx".to_string(), "nsfw.onnx".to_string()));
        } else if m == "aesthetics" {
            files_to_download.push(("aesthetics".to_string(), "https://huggingface.co/fsw/aesthetic-predictor-v2-5_onnx/resolve/main/aesthetic_predictor_v2_5.onnx".to_string(), "aesthetics.onnx".to_string()));
        } else if m == "yolo" {
            files_to_download.push((
                "yolo".to_string(),
                "https://huggingface.co/webml/yolov8n/resolve/main/onnx/yolov8n.onnx".to_string(),
                "yolov8.onnx".to_string(),
            ));
        } else if m == "blip" {
            files_to_download.push(("blip".to_string(), "https://huggingface.co/onnx-community/Salesforce_blip-image-captioning-base/resolve/main/split_0.onnx".to_string(), "blip.onnx".to_string()));
        } else if m == "arcface" {
            files_to_download.push((
                "arcface".to_string(),
                "https://huggingface.co/crj/dl-ws/resolve/main/arcface_w600k_r50.onnx".to_string(),
                "arcface.onnx".to_string(),
            ));
        } else if m == "midas" {
            files_to_download.push((
                "midas".to_string(),
                "https://huggingface.co/Xenova/dpt-hybrid-midas/resolve/main/onnx/model.onnx"
                    .to_string(),
                "midas.onnx".to_string(),
            ));
        } else if m == "whisper" {
            files_to_download.push(("whisper".to_string(), "https://huggingface.co/Xenova/whisper-tiny.en/resolve/main/onnx/encoder_model.onnx".to_string(), "whisper.onnx".to_string()));
        }
    }

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

        for (model_name, url, filename) in files_to_download {
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
                    emit_log(&app, format!("SUCCESS: Finished downloading {filename}"));
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
async fn read_file_base64(path: String) -> String {
    file::read_file_base64(path)
}

#[tauri::command]
async fn get_raw_photo(path: String) -> String {
    file::read_file_base64(path)
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
async fn cleanup_database(app: tauri::AppHandle) {
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
    let db = database::Database::new(&config_path);
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

    // Abort existing session if any
    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            println!("Aborting previous WebRTC session");
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
async fn stop_webrtc_session(state: tauri::State<'_, WebRtcState>) -> Result<(), String> {
    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            println!("Stopping WebRTC session");
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
async fn join_network(app: tauri::AppHandle, ip: String, name: String) {
    println!("Adding new device: {name} at {ip}");
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
    let mut payload = format!("__VIDEO_FRAMES__:{id}");
    for frame in frames {
        payload.push_str("|||");
        payload.push_str(&frame);
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
    println!("Face indexing requested...");
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
    println!("DEBUG: Found {} photos with GPS for heatmap", points.len());
    serde_json::to_string(&points).unwrap_or("[]".to_string())
}

#[tauri::command]
async fn get_photo_encoded_batch(app: tauri::AppHandle, ids: Vec<String>) -> std::collections::HashMap<String, String> {
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
                if let Some((city, country)) = geocode::find_nearest_city(lat, lon) {
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
    println!("Resolved {} photo locations", resolved);
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
    println!("{message}");
    let _ = app.emit("log-message", message.clone());
    let path = get_config_path(app);
    if !path.is_empty() {
        let database = database::Database::new(&path);
        let level = if message.to_lowercase().contains("error") {
            "error"
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
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
            }

            println!("App is setting up background tasks...");
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
            let (tx, pending_count, abort) =
                ml::start_background_worker(app.handle(), config_path.clone());
            app.manage(ml::MlContext {
                tx,
                pending_count,
                abort,
            });

            let media_server_port = transport::start_media_server(config_path);
            app.manage(transport::MediaServerState {
                port: media_server_port,
            });

            app.manage(WebRtcState {
                active_session: std::sync::Mutex::new(None),
                sync_tx: Arc::new(tokio::sync::Mutex::new(None)),
            });

            // Start periodic background scan
            let app_handle_for_interval = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // 1 hour
                loop {
                    println!("Interval tick: checking for media updates...");
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
            server::generate_pairing_codes,
            server::hash_pairing_code,
            start_webrtc_session,
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
            save_config,
            get_config,
            get_indexing_status,
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
