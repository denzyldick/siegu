//! ML command business logic, owned by siegu-core.
//!
//! These are the pure operations behind the ML-trigger facade commands
//! (`analyze_*`, `index_faces`, pause/resume/abort, reload/unload_models and
//! the indexing-status reads). They operate exclusively on siegu-core types
//! ([`MlContext`], `Job`, `Database`, atomics) so that every platform frontend
//! — desktop/Tauri, CLI, webHost, guest — drives analysis through the same
//! shared sieve. The Tauri layer keeps only its `#[tauri::command]` shell
//! (logging/emitting to the host UI); the RPC facade calls these directly.
//!
//! The command catalog (`crate::rpc_catalog`) marks these commands `Owner`-tier
//! and they are gated accordingly in `crate::rpc::dispatch`.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::database::Database;
use crate::ml_worker::{Job, LoadedModelsHandle, MlContext};

/// Pending-count read, clamped to 0 when unreasonably high (the sentinel the
/// worker uses to signal "not really counting").
pub fn do_get_indexing_status(pending_count: &AtomicUsize) -> usize {
    let count = pending_count.load(Ordering::SeqCst);
    if count > 1_000_000 {
        0
    } else {
        count
    }
}

/// Count of photos not yet fully indexed (indexed < 2).
pub fn do_get_unindexed_count(db: &Database) -> usize {
    let count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM photo WHERE indexed < 2", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    count as usize
}

/// Sets indexing mode to "immediate" so the worker picks up new library items.
pub fn do_index_faces(db: &Database) -> Result<(), String> {
    let mut state_map = std::collections::HashMap::new();
    state_map.insert("indexing_mode".to_string(), "immediate".to_string());
    db.set_state(state_map);
    Ok(())
}

/// Sends the face-indexing job on the worker channel.
pub async fn send_index_faces_job(tx: &tokio::sync::mpsc::Sender<Job>) -> Result<(), String> {
    tx.send(Job::ProcessModel("face".to_string()))
        .await
        .map_err(|e| e.to_string())
}

/// Sets the abort flag and enqueues an `AnalyzeSingle` job.
pub async fn do_analyze_photo(
    abort: &AtomicBool,
    tx: &tokio::sync::mpsc::Sender<Job>,
    id: &str,
) -> Result<(), String> {
    abort.store(true, Ordering::SeqCst);
    tx.send(Job::AnalyzeSingle(id.to_string()))
        .await
        .map_err(|e| e.to_string())
}

/// Sets the abort flag and enqueues an `AnalyzeSingleWithModel` job.
pub async fn do_analyze_photo_model(
    abort: &AtomicBool,
    tx: &tokio::sync::mpsc::Sender<Job>,
    id: &str,
    model_id: &str,
) -> Result<(), String> {
    abort.store(true, Ordering::SeqCst);
    tx.send(Job::AnalyzeSingleWithModel(
        id.to_string(),
        model_id.to_string(),
    ))
    .await
    .map_err(|e| e.to_string())
}

/// Enqueues a `ProcessModel` job for a whole-library model pass.
pub async fn do_analyze_model(
    tx: &tokio::sync::mpsc::Sender<Job>,
    model_id: &str,
) -> Result<(), String> {
    tx.send(Job::ProcessModel(model_id.to_string()))
        .await
        .map_err(|e| e.to_string())
}

/// Resets the worker's abort flag, pending count and paused state.
pub fn do_abort_indexing(
    abort: &AtomicBool,
    pending_count: &AtomicUsize,
    paused: &AtomicBool,
) -> Result<(), String> {
    abort.store(true, Ordering::SeqCst);
    pending_count.store(0, Ordering::SeqCst);
    paused.store(false, Ordering::SeqCst);
    Ok(())
}

/// Pauses indexing without aborting.
pub fn do_pause_indexing(paused: &AtomicBool) -> Result<(), String> {
    paused.store(true, Ordering::SeqCst);
    Ok(())
}

/// Resumes paused indexing.
pub fn do_resume_indexing(paused: &AtomicBool) -> Result<(), String> {
    paused.store(false, Ordering::SeqCst);
    Ok(())
}

/// Clears the loaded-models cache so the worker drops all ONNX sessions.
/// Uses `try_lock` (never blocks): the worker may hold the models mutex for a
/// whole analysis batch, and unloading mid-inference is never wanted.
pub fn do_unload_models(models: &LoadedModelsHandle) -> Result<(), String> {
    let mut m = models.try_lock().map_err(|_| {
        "AI models are in use right now — try again once the current analysis finishes.".to_string()
    })?;
    *m = None;
    Ok(())
}

/// Enqueues a model-reload job. The worker owns the models mutex, so unlike
/// `do_unload_models` this can never race or fail.
pub async fn do_reload_models(tx: &tokio::sync::mpsc::Sender<Job>) -> Result<(), String> {
    tx.send(Job::ReloadModels).await.map_err(|e| e.to_string())
}

/// True when the worker currently holds loaded models in memory. Never blocks
/// the caller: reports "in use" as loaded when the worker is mid-inference.
pub fn do_get_models_loaded(models: &LoadedModelsHandle) -> bool {
    match models.try_lock() {
        Ok(m) => m.is_some(),
        Err(_) => true,
    }
}

/// Convenience: run a single `do_*` operation against the given live ML
/// worker context. Returns `None`-style errors referencing the facade command
/// so callers (Tauri command or RPC dispatch) get a consistent message.
pub fn require_worker<'a>(ml: Option<&'a MlContext>, name: &str) -> Result<&'a MlContext, String> {
    ml.ok_or_else(|| {
        format!("command '{name}' needs a live ML worker; none is running on this host")
    })
}

// ── synchronous variants for the RPC facade ────────────────────────────────
// `dispatch` is synchronous (it runs inside `spawn_blocking` for the web host
// or on a mesh task). These use `try_send` on the bounded job channel so a full
// queue surfaces as an error instead of blocking the request thread.

fn try_send(tx: &tokio::sync::mpsc::Sender<Job>, job: Job) -> Result<(), String> {
    tx.try_send(job).map_err(|e| match e {
        tokio::sync::mpsc::error::TrySendError::Full(_) => {
            "AI job queue is full; wait for the current analysis to drain.".to_string()
        }
        tokio::sync::mpsc::error::TrySendError::Closed(_) => {
            "AI worker is no longer running.".to_string()
        }
    })
}

/// Synchronous `analyze_photo` for the RPC facade.
pub fn do_analyze_photo_sync(
    abort: &AtomicBool,
    tx: &tokio::sync::mpsc::Sender<Job>,
    id: &str,
) -> Result<(), String> {
    abort.store(true, Ordering::SeqCst);
    try_send(tx, Job::AnalyzeSingle(id.to_string()))
}

/// Synchronous `analyze_photo_model` for the RPC facade.
pub fn do_analyze_photo_model_sync(
    abort: &AtomicBool,
    tx: &tokio::sync::mpsc::Sender<Job>,
    id: &str,
    model_id: &str,
) -> Result<(), String> {
    abort.store(true, Ordering::SeqCst);
    try_send(
        tx,
        Job::AnalyzeSingleWithModel(id.to_string(), model_id.to_string()),
    )
}

/// Synchronous `analyze_model` for the RPC facade.
pub fn do_analyze_model_sync(
    tx: &tokio::sync::mpsc::Sender<Job>,
    model_id: &str,
) -> Result<(), String> {
    try_send(tx, Job::ProcessModel(model_id.to_string()))
}

/// Synchronous `index_faces` job send (state set separately) for the RPC facade.
pub fn do_index_faces_sync(tx: &tokio::sync::mpsc::Sender<Job>) -> Result<(), String> {
    try_send(tx, Job::ProcessModel("face".to_string()))
}

/// Synchronous `reload_models` for the RPC facade.
pub fn do_reload_models_sync(tx: &tokio::sync::mpsc::Sender<Job>) -> Result<(), String> {
    try_send(tx, Job::ReloadModels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{AiStatus, Photo};
    use std::collections::HashMap;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::sync::Mutex;

    fn test_db() -> (Database, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "siegu_mlcmd_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _db = Database::new(&dir.display().to_string());
        (_db, dir)
    }

    fn make_photo(id: &str, location: &str) -> Photo {
        Photo {
            id: id.to_string(),
            location: location.to_string(),
            encoded: String::new(),
            created: "2026-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: false,
            received: false,
            view_only: false,
            last_opened: 0,
        }
    }

    #[test]
    fn get_indexing_status_zero() {
        let count = AtomicUsize::new(0);
        assert_eq!(do_get_indexing_status(&count), 0);
    }

    #[test]
    fn get_indexing_status_normal() {
        let count = AtomicUsize::new(42);
        assert_eq!(do_get_indexing_status(&count), 42);
    }

    #[test]
    fn get_indexing_status_overflow_clamps_to_zero() {
        let count = AtomicUsize::new(2_000_000);
        assert_eq!(do_get_indexing_status(&count), 0);
    }

    #[test]
    fn get_unindexed_count_empty_db() {
        let (db, _dir) = test_db();
        assert_eq!(do_get_unindexed_count(&db), 0);
    }

    #[test]
    fn get_unindexed_count_with_new_photo() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("ph1", "/a.jpg")])
            .unwrap();
        assert_eq!(do_get_unindexed_count(&db), 1);
    }

    #[test]
    fn get_unindexed_count_fully_indexed() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("ph1", "/a.jpg")])
            .unwrap();
        db.update_photo_indexed("ph1", 2);
        assert_eq!(do_get_unindexed_count(&db), 0);
    }

    #[test]
    fn index_faces_sets_indexing_mode() {
        let (db, _dir) = test_db();
        do_index_faces(&db).unwrap();
        let config = db.get_state();
        assert_eq!(config.get("indexing_mode").unwrap(), "immediate");
    }

    #[tokio::test]
    async fn index_faces_sends_process_model_job() {
        let (tx, mut rx) =
            tokio::sync::mpsc::channel(super::super::ml_worker::JOB_CHANNEL_CAPACITY);
        send_index_faces_job(&tx).await.unwrap();
        let job = rx.try_recv().unwrap();
        assert!(matches!(job, Job::ProcessModel(ref m) if m == "face"));
    }

    #[tokio::test]
    async fn analyze_photo_sends_job_and_sets_abort() {
        let abort = AtomicBool::new(false);
        let (tx, mut rx) =
            tokio::sync::mpsc::channel(super::super::ml_worker::JOB_CHANNEL_CAPACITY);
        do_analyze_photo(&abort, &tx, "photo1").await.unwrap();
        assert!(abort.load(Ordering::SeqCst));
        let job = rx.try_recv().unwrap();
        assert!(matches!(job, Job::AnalyzeSingle(ref id) if id == "photo1"));
    }

    #[tokio::test]
    async fn analyze_photo_model_sends_correct_job() {
        let abort = AtomicBool::new(false);
        let (tx, mut rx) =
            tokio::sync::mpsc::channel(super::super::ml_worker::JOB_CHANNEL_CAPACITY);
        do_analyze_photo_model(&abort, &tx, "photo1", "clip")
            .await
            .unwrap();
        assert!(abort.load(Ordering::SeqCst));
        let job = rx.try_recv().unwrap();
        assert!(
            matches!(job, Job::AnalyzeSingleWithModel(ref id, ref model) if id == "photo1" && model == "clip")
        );
    }

    #[tokio::test]
    async fn analyze_model_sends_process_model() {
        let (tx, mut rx) =
            tokio::sync::mpsc::channel(super::super::ml_worker::JOB_CHANNEL_CAPACITY);
        do_analyze_model(&tx, "yolo").await.unwrap();
        let job = rx.try_recv().unwrap();
        assert!(matches!(job, Job::ProcessModel(ref m) if m == "yolo"));
    }

    #[test]
    fn abort_indexing_sets_flags() {
        let abort = AtomicBool::new(false);
        let pending = AtomicUsize::new(100);
        let paused = AtomicBool::new(true);
        do_abort_indexing(&abort, &pending, &paused).unwrap();
        assert!(abort.load(Ordering::SeqCst));
        assert_eq!(pending.load(Ordering::SeqCst), 0);
        assert!(!paused.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn reload_models_sends_reload_job() {
        let (tx, mut rx) =
            tokio::sync::mpsc::channel(super::super::ml_worker::JOB_CHANNEL_CAPACITY);
        do_reload_models(&tx).await.unwrap();
        let job = rx.try_recv().unwrap();
        assert!(matches!(job, Job::ReloadModels));
    }

    fn dummy_loaded_models() -> crate::ml_engine::models::LoadedModels {
        crate::ml_engine::models::LoadedModels {
            clip_visual: None,
            clip_text: None,
            text_embeddings: Vec::new(),
            face_detector: None,
            arcface: None,
            ocr_det: None,
            ocr_rec: None,
            ocr_alphabet: Vec::new(),
            nsfw: None,
            aesthetics: None,
            yolo: None,
            blip: None,
            blip_decoder: None,
            blip_tokenizer: None,
            midas: None,
            whisper_encoder: None,
            whisper_decoder: None,
            whisper_tokenizer: None,
            known_people: Vec::new(),
            known_people_named: 0,
            selected_ep: String::new(),
        }
    }

    #[test]
    fn unload_models_clears_cache() {
        let models: LoadedModelsHandle = Arc::new(Mutex::new(Some(dummy_loaded_models())));
        assert!(models.lock().unwrap().is_some());
        do_unload_models(&models).unwrap();
        assert!(models.lock().unwrap().is_none());
    }

    #[test]
    fn unload_models_when_already_empty() {
        let models: LoadedModelsHandle = Arc::new(Mutex::new(None));
        do_unload_models(&models).unwrap();
        assert!(models.lock().unwrap().is_none());
    }

    #[test]
    fn get_models_loaded_reflects_state() {
        let models: LoadedModelsHandle = Arc::new(Mutex::new(None));
        assert!(!do_get_models_loaded(&models));
        *models.lock().unwrap() = Some(dummy_loaded_models());
        assert!(do_get_models_loaded(&models));
    }

    #[test]
    fn require_worker_present_and_absent() {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let ctx = MlContext {
            tx,
            pending_count: Arc::new(AtomicUsize::new(0)),
            abort: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            models: Arc::new(Mutex::new(None)),
        };
        assert!(require_worker(Some(&ctx), "analyze_photo").is_ok());
        match require_worker(None, "analyze_photo") {
            Err(err) => {
                assert!(err.contains("analyze_photo"));
                assert!(err.contains("worker"));
            }
            Ok(_) => panic!("expected error for missing worker"),
        }
    }
}
