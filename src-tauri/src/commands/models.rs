use crate::common::get_config_path;
use crate::ml;
use std::collections::HashMap;
use std::path::Path;
use tauri::Emitter;

/// Pure business logic — checks filesystem for model files.
pub fn do_check_models(models_dir: &Path) -> Vec<String> {
    siegu_core::model_manager::check_models_downloaded(models_dir)
}

#[derive(serde::Serialize, Clone)]
pub struct DownloadProgress {
    model: String,
    downloaded: u64,
    total: Option<u64>,
}

#[derive(serde::Serialize, Clone)]
pub struct ModelCapability {
    model: String,
    runnable: bool,
    reason: Option<String>,
}

/// Per-model verdicts for whether a model can run on this device. The UI uses
/// this to disable the enable toggle / download button and to explain why.
#[tauri::command]
pub async fn get_model_capabilities(app: tauri::AppHandle) -> Vec<ModelCapability> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Vec::new();
    }
    let models_dir = std::path::Path::new(&path).join("models");
    let config = siegu_core::database::Database::new(&path).get_state();
    siegu_core::ml_engine::models::model_feasibility(&models_dir, &config, &|_| {})
        .into_iter()
        .map(|v| ModelCapability {
            model: v.model,
            runnable: v.runnable,
            reason: v.reason,
        })
        .collect()
}

#[tauri::command]
pub async fn check_models(app: tauri::AppHandle) -> Vec<String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Vec::new();
    }
    let models_dir = std::path::Path::new(&path).join("models");
    do_check_models(&models_dir)
}

#[tauri::command]
pub async fn download_models(
    app: tauri::AppHandle,
    mut models: Vec<String>,
    state: tauri::State<'_, ml::MlContext>,
) -> Result<(), String> {
    use crate::common::emit_log;
    use siegu_core::model_manager::{download_file, remote_size, DownloadOutcome};

    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Could not resolve config dir".to_string());
    }
    let models_dir = std::path::PathBuf::from(&path).join("models");
    std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

    // Never download models that can't run on this device (low RAM, memory
    // budget) — tell the user why instead of wasting bandwidth.
    let config = siegu_core::database::Database::new(&path).get_state();
    let blocked: Vec<(String, String)> =
        siegu_core::ml_engine::models::model_feasibility(&models_dir, &config, &|_| {})
            .into_iter()
            .filter(|v| {
                !v.runnable
                    && v.reason
                        .as_deref()
                        .is_some_and(|r| r != siegu_core::ml_engine::models::REASON_NOT_DOWNLOADED)
            })
            .map(|v| (v.model.clone(), v.reason.unwrap_or_default()))
            .collect();
    let blocked_names: Vec<&str> = blocked.iter().map(|(name, _)| name.as_str()).collect();
    if !blocked_names.is_empty() {
        emit_log(
            &app,
            format!(
                "Skipping models that can't run on this device: {}",
                blocked_names.join(", ")
            ),
        );
        let allowed: Vec<String> = models
            .into_iter()
            .filter(|m| !blocked_names.contains(&m.as_str()))
            .collect();
        if allowed.is_empty() {
            return Err("None of the selected models can run on this device".to_string());
        }
        models = allowed;
    }

    let needed = siegu_core::model_manager::needed_download_bytes(&models_dir, &models);
    if needed > 0 {
        let free = siegu_core::model_manager::available_disk_bytes(&models_dir);
        let free_mb = free / (1024 * 1024);
        let needed_mb = needed / (1024 * 1024);
        if free > 0 && free < needed {
            return Err(format!(
                "Not enough disk space: {needed_mb} MB needed, only {free_mb} MB free"
            ));
        }
        emit_log(
            &app,
            format!("Disk space check: {needed_mb} MB to download, {free_mb} MB free"),
        );
    }

    let resolved = siegu_core::model_manager::resolve_files_for_models(&models);
    let files_to_download: Vec<(String, String, String, u64, String)> = resolved
        .iter()
        .map(|(entry, _)| {
            (
                entry.model_name.to_string(),
                entry.url.to_string(),
                entry.filename.to_string(),
                entry.expected_size,
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

        let mut model_totals: HashMap<String, u64> = HashMap::new();
        for (_, url, _, expected_size, _) in &files_to_download {
            let est = remote_size(&client, url).await.unwrap_or(*expected_size);
            *model_totals.entry(url.clone()).or_insert(0) += est;
        }

        let mut model_done: HashMap<String, u64> = HashMap::new();

        for (model_name, url, filename, expected_size, expected_hash) in files_to_download {
            emit_log(&app, format!("Initiating download: {filename}"));

            let base = *model_done.get(&model_name).unwrap_or(&0);
            let total = model_totals.get(&url).copied().or(Some(expected_size));
            let mut last_reported: u64 = 0;

            let emit_progress = |app: &tauri::AppHandle, model: &str, downloaded: u64| {
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress {
                        model: model.to_string(),
                        downloaded,
                        total,
                    },
                );
            };

            let mut file_downloaded: u64 = 0;
            let result = download_file(
                &client,
                &url,
                &filename,
                expected_size,
                &models_dir,
                |downloaded, _file_total| {
                    file_downloaded = downloaded;
                    let model_progress = base + downloaded;
                    if model_progress.saturating_sub(last_reported) > 1024 * 1024
                        || total.is_some_and(|t| downloaded >= t)
                    {
                        last_reported = model_progress;
                        emit_progress(&app, &model_name, model_progress);
                    }
                },
            )
            .await;

            match result {
                Ok(DownloadOutcome::Skipped) => {
                    emit_log(&app, format!("Skipping {filename}: already downloaded"));
                }
                Ok(DownloadOutcome::Completed) => {
                    emit_log(&app, format!("SUCCESS: Finished downloading {filename}"));
                }
                Err(e) => {
                    emit_log(&app, format!("ERROR: Failed to download {filename}: {e}"));
                    continue;
                }
            }

            let final_path = models_dir.join(&filename);
            if final_path.exists() {
                match siegu_core::model_manager::verify_sha256(&final_path, &expected_hash) {
                    Ok(true) => {
                        emit_log(&app, format!("{filename}: SHA-256 verified"));
                    }
                    Ok(false) => {
                        emit_log(
                            &app,
                            format!("ERROR: SHA-256 mismatch for {filename}, deleting"),
                        );
                        let _ = std::fs::remove_file(&final_path);
                    }
                    Err(e) => {
                        emit_log(
                            &app,
                            format!("WARNING: Could not verify hash for {filename}: {e}"),
                        );
                    }
                }
            }

            if let Ok(meta) = std::fs::metadata(models_dir.join(&filename)) {
                *model_done.entry(model_name.clone()).or_insert(0) += meta.len();
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress {
                        model: model_name.clone(),
                        downloaded: *model_done.get(&model_name).unwrap_or(&0),
                        total,
                    },
                );
            }
        }
        let _ = tx.send(ml::Job::ProcessAll);
        let _ = app.emit("download-complete", ());
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn check_models_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = do_check_models(dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn check_models_no_files() {
        let dir = tempfile::tempdir().unwrap();
        let result = do_check_models(dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn check_models_partial_files_ignored() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("ocr_det.onnx"), vec![0u8; 2048]).unwrap();
        let result = do_check_models(dir.path());
        assert!(result.is_empty());
    }
}
