use std::sync::{Arc, Mutex};

use crate::common::emit_log;
use tauri::AppHandle;
use tauri::Emitter;

pub use siegu_core::ml_worker::{Job, MlContext};

use siegu_core::database::Database;
use siegu_core::mesh::SyncMessage;
use siegu_core::ml_engine::worker::AnalysisCallbacks;
use siegu_core::ml_engine::PhotoResult;

struct TauriCallbacks {
    app: AppHandle,
    db: Mutex<Database>,
    sync_tx: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<SyncMessage>>>>,
}

impl AnalysisCallbacks for TauriCallbacks {
    fn on_photo_complete(
        &self,
        photo_id: &str,
        _location: &str,
        result: &PhotoResult,
        remaining: usize,
        progress_model: Option<&str>,
    ) {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let has_caption: bool = db
            .connection
            .query_row("SELECT caption FROM photo WHERE id = ?1", [photo_id], |r| {
                r.get::<_, Option<String>>(0)
            })
            .unwrap_or(None)
            .is_some();

        let _ = self.app.emit(
            "photo-analysis-result",
            serde_json::json!({
                "id": photo_id,
                "object_count": result.objects.len(),
                "face_count": result.face_count,
                "has_caption": has_caption,
                "indexed": true,
                "model_timings": result.model_timings,
            }),
        );
        let _ = self.app.emit(
            "indexing-progress",
            serde_json::json!({ "remaining": remaining }),
        );
        let _ = self.app.emit(
            "indexing-eta",
            serde_json::json!({ "eta": (remaining as f64) * 1000.0 }),
        );

        if let Some(model) = progress_model {
            let _ = self.app.emit(
                "model-progress",
                serde_json::json!({
                    "model": model,
                    "pending": remaining,
                    "status": if remaining == 0 { "completed" } else { "running" },
                }),
            );
        }

        if remaining == 0 {
            let _ = self
                .app
                .emit("indexing-job", serde_json::json!({ "status": "idle" }));
        }
    }

    fn on_metadata_updated(
        &self,
        photo_id: &str,
        caption: Option<&str>,
        aesthetics_score: Option<f64>,
    ) {
        emit_log(&self.app, format!("Metadata updated for {photo_id}"));
        if let Ok(g) = self.sync_tx.try_lock() {
            if let Some(tx) = g.as_ref() {
                let _ = tx.send(SyncMessage::MetadataUpdate {
                    photo_id: photo_id.to_string(),
                    caption: caption.map(|c| c.to_string()),
                    aesthetics_score,
                    indexed: 2,
                });
            }
        }
    }

    fn on_scan_complete(&self) {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let pending: i64 = db
            .connection
            .query_row(
                "SELECT COUNT(*) FROM photo WHERE sync_needed = 1 AND received = 0",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if pending == 0 {
            emit_log(
                &self.app,
                "Scan complete: no photos pending sync".to_string(),
            );
            return;
        }

        let mut pushed = false;
        if let Ok(g) = self.sync_tx.try_lock() {
            if let Some(tx) = g.as_ref() {
                pushed = tx.send(SyncMessage::StartSync).is_ok();
            }
        }

        if pushed {
            emit_log(
                &self.app,
                format!("Scan complete: pushed StartSync to peer ({pending} photos pending sync)"),
            );
        } else {
            emit_log(
                &self.app,
                format!(
                    "Scan complete with {pending} photos pending sync, but no active peer session"
                ),
            );
            crate::tauri_sync_event::notify_photos_ready(&self.app, pending);
        }
    }

    fn on_progress(&self, completed: usize, total: usize, _avg_ms: f64) {
        let _ = self.app.emit(
            "indexing-job",
            serde_json::json!({
                "status": "running",
                "completed": completed,
                "total": total,
            }),
        );
    }

    fn on_model_status(&self, model: &str, status: &str, pending: usize, total: usize) {
        self.on_model_status_with_reason(model, status, pending, total, None)
    }

    fn on_model_status_with_reason(
        &self,
        model: &str,
        status: &str,
        pending: usize,
        total: usize,
        reason: Option<&str>,
    ) {
        let mut payload = serde_json::json!({
            "model": model,
            "pending": pending,
            "total": total,
            "status": status,
        });
        if let Some(reason) = reason {
            payload["message"] = serde_json::Value::String(reason.to_string());
        }
        let _ = self.app.emit("model-progress", payload);
    }

    fn on_ep_selected(&self, ep: &str) {
        emit_log(
            &self.app,
            format!("ML Worker: Using {ep} execution provider"),
        );
    }

    fn on_log(&self, msg: &str) {
        emit_log(&self.app, format!("ML Worker: {msg}"));
    }

    fn should_abort(&self) -> bool {
        false
    }
}

pub fn start_background_worker(
    app: &AppHandle,
    config_path: String,
    sync_tx: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<SyncMessage>>>>,
) -> MlContext {
    let callbacks = TauriCallbacks {
        app: app.clone(),
        db: Mutex::new(Database::new(&config_path)),
        sync_tx,
    };
    siegu_core::ml_engine::worker::start_worker(callbacks, config_path, 32)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use siegu_core::database::AiStatus;
    use siegu_core::ml_engine::pipeline::{analyze_photo, analyze_video, is_video_file};
    use siegu_core::ml_engine::whisper::whisper_transcribe;

    const REQUIRED_MODELS: &[&str] = &[
        "clip-vit-base-patch32-visual.onnx",
        "clip-vit-base-patch32-text.onnx",
        "tokenizer.json",
        "face_detection_yunet_2023mar.onnx",
        "arcface.onnx",
        "ocr_det.onnx",
        "ocr_rec.onnx",
        "nsfw.onnx",
        "aesthetics.onnx",
        "yolov8.onnx",
        "blip.onnx",
        "blip_decoder.onnx",
        "midas.onnx",
        "whisper.onnx",
        "whisper-decoder.onnx",
        "whisper-tokenizer.json",
    ];

    fn test_models_dir() -> Option<std::path::PathBuf> {
        // Prefer the repo-local `test_models/` (used by CI, which downloads a
        // fresh ~5GB suite). Locally, fall back to the app's real models dir
        // (`~/.config/io.denzyl.siegu/models`) so tests reuse already-downloaded
        // models instead of fetching a second copy.
        let mut candidates = vec![std::path::Path::new("test_models").to_path_buf()];
        if let Some(base) = crate::config_dir_fallback() {
            candidates.push(base.join("io.denzyl.siegu").join("models"));
        }
        for dir in candidates {
            if dir.exists() && REQUIRED_MODELS.iter().all(|f| dir.join(f).exists()) {
                // Canonicalize so the symlink target below is absolute: a relative
                // target would resolve against the temp config dir, not the repo.
                return dir.canonicalize().ok();
            }
        }
        None
    }

    #[cfg(unix)]
    fn link_models(from: &Path, to: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(from, to)
    }

    #[cfg(windows)]
    fn link_models(from: &Path, to: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(from, to)
    }

    /// Point a throwaway config dir's `models/` at the downloaded test models
    /// and return the config dir. `load_models` resolves models as
    /// `{config_path}/models`, mirroring the app layout.
    fn prepare_config(models_dir: &Path) -> std::path::PathBuf {
        let config_dir = std::env::temp_dir().join(format!("siegu-ai-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&config_dir);
        std::fs::create_dir_all(&config_dir).unwrap();
        let models_link = config_dir.join("models");
        let _ = std::fs::remove_dir_all(&models_link);
        link_models(models_dir, &models_link).expect("symlink test_models into config");
        std::fs::create_dir_all(config_dir.join("faces")).unwrap();
        config_dir
    }

    #[test]
    #[ignore] // CI only: requires the ~5GB model suite in test_models/
    fn test_full_inference_on_sample() {
        let models_dir = match test_models_dir() {
            Some(d) => d,
            None => {
                println!(
                    "Skipping: required models not present in test_models/ or the app models dir"
                );
                return;
            }
        };
        let sample = Path::new("../tests/fixtures/faces/einstein_1.jpg");
        if !sample.exists() {
            println!("Skipping: sample photo not found at {sample:?}");
            return;
        }
        if is_video_file(sample.to_str().unwrap()) {
            panic!("expected a photo fixture, got a video path");
        }

        let config_dir = prepare_config(&models_dir);
        let config_path = config_dir.display().to_string();
        let faces_dir = config_dir.join("faces").display().to_string();

        let config: HashMap<String, String> = HashMap::new();
        let loaded =
            siegu_core::ml_engine::models::load_models(&config_path, &config, Vec::new(), &|msg| {
                println!("[models] {msg}")
            });
        assert!(
            loaded.clip_visual.is_some(),
            "CLIP visual model should load"
        );
        assert!(
            loaded.face_detector.is_some(),
            "YuNet face detector should load"
        );
        assert!(loaded.arcface.is_some(), "ArcFace should load");
        assert!(loaded.nsfw.is_some(), "NSFW model should load");
        assert!(loaded.aesthetics.is_some(), "aesthetics model should load");
        assert!(loaded.yolo.is_some(), "YOLO model should load");
        assert!(loaded.blip.is_some(), "BLIP vision encoder should load");
        assert!(
            loaded.blip_decoder.is_some(),
            "BLIP text decoder should load"
        );
        assert!(loaded.midas.is_some(), "MiDaS should load");
        assert!(
            loaded.whisper_encoder.is_some(),
            "Whisper encoder should load"
        );
        assert!(
            loaded.whisper_decoder.is_some(),
            "Whisper decoder should load"
        );
        assert!(
            loaded.whisper_tokenizer.is_some(),
            "Whisper tokenizer should load"
        );

        let start = std::time::Instant::now();
        let result = analyze_photo(
            "e2e-inference",
            sample.to_str().unwrap(),
            &AiStatus::default(),
            &loaded,
            &config,
            None,
            &faces_dir,
        );
        let elapsed = start.elapsed();

        println!(
            "inference completed in {:.1}s: models={:?}",
            elapsed.as_secs_f64(),
            result.completed_models
        );

        assert!(
            result.completed_models.contains(&"clip"),
            "CLIP visual should produce embeddings: {:?}",
            result.completed_models
        );
        assert!(
            result.completed_models.contains(&"aesthetics"),
            "aesthetics should score the photo"
        );
        assert!(
            result.completed_models.contains(&"nsfw"),
            "NSFW should classify the photo"
        );
        assert!(
            result.completed_models.contains(&"yolo"),
            "YOLO should run object detection: {:?}",
            result.completed_models
        );
        assert!(
            result.completed_models.contains(&"arcface"),
            "ArcFace should embed the detected face: {:?}",
            result.completed_models
        );
        assert!(
            result.face_count >= 1,
            "einstein fixture is a portrait, expected >=1 face, got {}",
            result.face_count
        );
        assert!(result.aesthetics.is_some(), "expected an aesthetics score");
        let aesthetics = result.aesthetics.unwrap();
        assert!(
            aesthetics.is_finite(),
            "aesthetics score must be finite, got {aesthetics}"
        );
        assert!(result.nsfw.is_some(), "expected an NSFW label");
        let nsfw_score: f64 = result.nsfw.as_deref().unwrap().parse().expect("nsfw score");
        assert!(
            (0.0..=1.0).contains(&nsfw_score),
            "nsfw sigmoid must be in [0,1], got {nsfw_score}"
        );

        // BLIP caption generation is the slowest step and the most valuable
        // full-pipeline signal.
        assert!(
            result.completed_models.contains(&"blip"),
            "BLIP captioning should run"
        );
        let caption = result
            .caption
            .as_deref()
            .expect("BLIP should produce a caption");
        assert!(!caption.trim().is_empty(), "BLIP caption must be non-empty");
        println!("caption: {caption:?}");

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[test]
    #[ignore] // CI only: requires the ~5GB model suite in test_models/
    fn test_full_inference_on_video_sample() {
        let models_dir = match test_models_dir() {
            Some(d) => d,
            None => {
                println!(
                    "Skipping: required models not present in test_models/ or the app models dir"
                );
                return;
            }
        };
        let sample = Path::new("../tests/fixtures/videos/einstein_2s.mp4");
        if !sample.exists() {
            println!("Skipping: sample video not found at {sample:?}");
            return;
        }
        assert!(
            is_video_file(sample.to_str().unwrap()),
            "expected a video fixture, got: {}",
            sample.display()
        );

        let config_dir = prepare_config(&models_dir);
        let config_path = config_dir.display().to_string();
        let faces_dir = config_dir.join("faces").display().to_string();

        let config: HashMap<String, String> = HashMap::new();
        let loaded =
            siegu_core::ml_engine::models::load_models(&config_path, &config, Vec::new(), &|msg| {
                println!("[models] {msg}")
            });
        assert!(loaded.clip_visual.is_some(), "CLIP visual should load");
        assert!(
            loaded.face_detector.is_some(),
            "YuNet face detector should load"
        );
        assert!(loaded.arcface.is_some(), "ArcFace should load");
        assert!(loaded.nsfw.is_some(), "NSFW model should load");
        assert!(loaded.aesthetics.is_some(), "aesthetics model should load");
        assert!(loaded.yolo.is_some(), "YOLO model should load");
        assert!(loaded.blip.is_some(), "BLIP vision encoder should load");
        assert!(
            loaded.blip_decoder.is_some(),
            "BLIP text decoder should load"
        );
        assert!(
            loaded.whisper_encoder.is_some(),
            "Whisper encoder should load"
        );
        assert!(
            loaded.whisper_decoder.is_some(),
            "Whisper decoder should load"
        );
        assert!(
            loaded.whisper_tokenizer.is_some(),
            "Whisper tokenizer should load"
        );

        // A 2s silent clip of the einstein portrait: exercises the whole video
        // path — ffmpeg frame extraction, per-frame visual analysis, and the
        // whisper audio extraction/transcription branch on silence.
        let start = std::time::Instant::now();
        let result = analyze_video(
            "e2e-inference-video",
            sample.to_str().unwrap(),
            &AiStatus::default(),
            &loaded,
            &config,
            None,
            &faces_dir,
        );
        let elapsed = start.elapsed();

        println!(
            "video inference completed in {:.1}s: models={:?}",
            elapsed.as_secs_f64(),
            result.completed_models
        );

        assert!(
            result.completed_models.contains(&"clip"),
            "CLIP visual should embed the extracted frame: {:?}",
            result.completed_models
        );
        assert!(
            result.completed_models.contains(&"aesthetics"),
            "aesthetics should score the frame"
        );
        assert!(
            result.completed_models.contains(&"nsfw"),
            "NSFW should classify the frame"
        );
        assert!(
            result.completed_models.contains(&"yolo"),
            "YOLO should run on the frame: {:?}",
            result.completed_models
        );
        assert!(
            result.completed_models.contains(&"arcface"),
            "ArcFace should embed the detected face: {:?}",
            result.completed_models
        );
        assert!(
            result.face_count >= 1,
            "einstein video fixture is a portrait, expected >=1 face, got {}",
            result.face_count
        );
        assert!(result.aesthetics.is_some(), "expected an aesthetics score");
        let aesthetics = result.aesthetics.unwrap();
        assert!(
            aesthetics.is_finite(),
            "aesthetics score must be finite, got {aesthetics}"
        );
        assert!(result.nsfw.is_some(), "expected an NSFW label");
        let nsfw_score: f64 = result.nsfw.as_deref().unwrap().parse().expect("nsfw score");
        assert!(
            (0.0..=1.0).contains(&nsfw_score),
            "nsfw sigmoid must be in [0,1], got {nsfw_score}"
        );
        assert!(
            result.completed_models.contains(&"blip"),
            "BLIP captioning should run on the frame"
        );
        let caption = result
            .caption
            .as_deref()
            .expect("BLIP should produce a caption");
        assert!(!caption.trim().is_empty(), "BLIP caption must be non-empty");
        println!("video caption: {caption:?}");

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[test]
    #[ignore] // CI only: requires whisper models in test_models/
    fn test_whisper_smoke() {
        let models_dir = match test_models_dir() {
            Some(d) => d,
            None => {
                println!(
                    "Skipping: required models not present in test_models/ or the app models dir"
                );
                return;
            }
        };

        let config_dir = prepare_config(&models_dir);
        let config_path = config_dir.display().to_string();

        let config: HashMap<String, String> = HashMap::new();
        let loaded =
            siegu_core::ml_engine::models::load_models(&config_path, &config, Vec::new(), &|msg| {
                println!("[models] {msg}")
            });
        let encoder = loaded.whisper_encoder.expect("whisper encoder should load");
        let decoder = loaded.whisper_decoder.expect("whisper decoder should load");
        let tokenizer = loaded
            .whisper_tokenizer
            .expect("whisper tokenizer should load");

        // 30 seconds of silence: proves the full encoder + autoregressive
        // decoder pipeline runs (not that it hears anything).
        let audio: Vec<f32> = vec![0.0; 30 * 16000];
        let transcript = whisper_transcribe(&encoder, &decoder, &tokenizer, &audio);
        println!("whisper silence transcript: [{transcript}]");

        assert!(
            !transcript.starts_with("Encoder error"),
            "encoder should run without error, got: {transcript}"
        );

        let _ = std::fs::remove_dir_all(&config_dir);
    }
}
