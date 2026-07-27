use crate::common::get_config_path;
use crate::ml;
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
    models: Vec<String>,
    state: tauri::State<'_, ml::MlContext>,
) -> Result<(), String> {
    use crate::common::emit_log;
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
