use crate::common::get_config_path;
use crate::database;
use crate::ml;
use std::sync::atomic::{AtomicBool, AtomicUsize};

/// Pure business logic — reads pending count from atomic, clamps to 0 if unreasonably high.
pub fn do_get_indexing_status(pending_count: &AtomicUsize) -> usize {
    let count = pending_count.load(std::sync::atomic::Ordering::SeqCst);
    if count > 1_000_000 {
        0
    } else {
        count
    }
}

/// Pure business logic — counts photos not yet fully indexed.
pub fn do_get_unindexed_count(db: &database::Database) -> usize {
    let count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM photo WHERE indexed < 2", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    count as usize
}

/// Pure business logic — sends ProcessModel job and sets indexing mode.
pub fn do_index_faces(
    db: &database::Database,
    tx: &tokio::sync::mpsc::UnboundedSender<ml::Job>,
) -> Result<(), String> {
    use std::collections::HashMap;
    let mut state_map = HashMap::new();
    state_map.insert("indexing_mode".to_string(), "immediate".to_string());
    db.set_state(state_map);
    let _ = tx.send(ml::Job::ProcessModel("face".to_string()));
    Ok(())
}

/// Pure business logic — sets abort flag and sends AnalyzeSingle job.
pub fn do_analyze_photo(
    abort: &AtomicBool,
    tx: &tokio::sync::mpsc::UnboundedSender<ml::Job>,
    id: &str,
) -> Result<(), String> {
    abort.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = tx.send(ml::Job::AnalyzeSingle(id.to_string()));
    Ok(())
}

/// Pure business logic — sets abort flag and sends AnalyzeSingleWithModel job.
pub fn do_analyze_photo_model(
    abort: &AtomicBool,
    tx: &tokio::sync::mpsc::UnboundedSender<ml::Job>,
    id: &str,
    model_id: &str,
) -> Result<(), String> {
    abort.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = tx.send(ml::Job::AnalyzeSingleWithModel(
        id.to_string(),
        model_id.to_string(),
    ));
    Ok(())
}

/// Pure business logic — sends ProcessModel job.
pub fn do_analyze_model(
    tx: &tokio::sync::mpsc::UnboundedSender<ml::Job>,
    model_id: &str,
) -> Result<(), String> {
    let _ = tx.send(ml::Job::ProcessModel(model_id.to_string()));
    Ok(())
}

/// Pure business logic — sets abort flag and resets pending count to 0.
pub fn do_abort_indexing(abort: &AtomicBool, pending_count: &AtomicUsize) -> Result<(), String> {
    abort.store(true, std::sync::atomic::Ordering::SeqCst);
    pending_count.store(0, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn get_indexing_status(state: tauri::State<'_, ml::MlContext>) -> usize {
    do_get_indexing_status(&state.pending_count)
}

#[tauri::command]
pub fn get_unindexed_count(app: tauri::AppHandle) -> usize {
    let path = get_config_path(&app);
    if path.is_empty() {
        return 0;
    }
    let db = database::Database::new(&path);
    do_get_unindexed_count(&db)
}

#[tauri::command]
pub async fn index_faces(
    app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
) -> Result<(), String> {
    use crate::common::emit_log;
    emit_log(&app, "Face indexing requested...".to_string());
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Config error".to_string());
    }
    let db = database::Database::new(&path);
    do_index_faces(&db, &state.tx)
}

#[tauri::command]
pub async fn analyze_photo(
    state: tauri::State<'_, ml::MlContext>,
    id: String,
) -> Result<(), String> {
    do_analyze_photo(&state.abort, &state.tx, &id)
}

#[tauri::command]
pub async fn analyze_photo_model(
    state: tauri::State<'_, ml::MlContext>,
    id: String,
    model_id: String,
) -> Result<(), String> {
    do_analyze_photo_model(&state.abort, &state.tx, &id, &model_id)
}

#[tauri::command]
pub async fn analyze_model(
    state: tauri::State<'_, ml::MlContext>,
    model_id: String,
) -> Result<(), String> {
    do_analyze_model(&state.tx, &model_id)
}

#[tauri::command]
pub async fn abort_indexing(state: tauri::State<'_, ml::MlContext>) -> Result<(), String> {
    do_abort_indexing(&state.abort, &state.pending_count)
}

/// Pure business logic — clears the loaded-models cache so the worker drops
/// all ONNX sessions (freeing their RAM). Models reload lazily before the
/// next analysis job.
pub fn do_unload_models(models: &siegu_core::ml_worker::LoadedModelsHandle) -> Result<(), String> {
    // try_lock so we never block: the worker may hold the models mutex for a
    // whole analysis batch, and unloading mid-inference is never wanted anyway.
    let mut m = models.try_lock().map_err(|_| {
        "AI models are in use right now — try again once the current analysis finishes.".to_string()
    })?;
    *m = None;
    Ok(())
}

#[tauri::command]
pub async fn unload_models(
    app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
) -> Result<(), String> {
    use crate::common::emit_log;
    do_unload_models(&state.models)?;
    emit_log(
        &app,
        "Unloaded AI models from memory. They will reload before the next analysis.".to_string(),
    );
    Ok(())
}

/// True when the worker currently holds loaded models in memory.
///
/// Must never block the UI thread: the worker may hold the models mutex while
/// loading (30s+) or during inference, so this polls with `try_lock` and
/// reports "in use" as loaded.
#[tauri::command]
pub async fn get_models_loaded(state: tauri::State<'_, ml::MlContext>) -> Result<bool, String> {
    Ok(match state.models.try_lock() {
        Ok(m) => m.is_some(),
        Err(_) => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    use std::sync::{Arc, Mutex};

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
    fn index_faces_sends_process_model_job() {
        let (db, _dir) = test_db();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        do_index_faces(&db, &tx).unwrap();
        let job = rx.try_recv().unwrap();
        assert!(matches!(job, ml::Job::ProcessModel(ref m) if m == "face"));
    }

    #[test]
    fn index_faces_sets_indexing_mode() {
        let (db, _dir) = test_db();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        do_index_faces(&db, &tx).unwrap();
        let config = db.get_state();
        assert_eq!(config.get("indexing_mode").unwrap(), "immediate");
    }

    #[test]
    fn analyze_photo_sends_job_and_sets_abort() {
        let abort = AtomicBool::new(false);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        do_analyze_photo(&abort, &tx, "photo1").unwrap();
        assert!(abort.load(std::sync::atomic::Ordering::SeqCst));
        let job = rx.try_recv().unwrap();
        assert!(matches!(job, ml::Job::AnalyzeSingle(ref id) if id == "photo1"));
    }

    #[test]
    fn analyze_photo_model_sends_correct_job() {
        let abort = AtomicBool::new(false);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        do_analyze_photo_model(&abort, &tx, "photo1", "clip").unwrap();
        assert!(abort.load(std::sync::atomic::Ordering::SeqCst));
        let job = rx.try_recv().unwrap();
        assert!(
            matches!(job, ml::Job::AnalyzeSingleWithModel(ref id, ref model) if id == "photo1" && model == "clip")
        );
    }

    #[test]
    fn analyze_model_sends_process_model() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        do_analyze_model(&tx, "yolo").unwrap();
        let job = rx.try_recv().unwrap();
        assert!(matches!(job, ml::Job::ProcessModel(ref m) if m == "yolo"));
    }

    #[test]
    fn abort_indexing_sets_flags() {
        let abort = AtomicBool::new(false);
        let pending = AtomicUsize::new(100);
        do_abort_indexing(&abort, &pending).unwrap();
        assert!(abort.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(pending.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn abort_indexing_when_already_zero() {
        let abort = AtomicBool::new(true);
        let pending = AtomicUsize::new(0);
        do_abort_indexing(&abort, &pending).unwrap();
        assert!(abort.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(pending.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn unload_models_clears_cache() {
        let models: siegu_core::ml_worker::LoadedModelsHandle =
            Arc::new(Mutex::new(Some(dummy_loaded_models())));
        assert!(models.lock().unwrap().is_some());
        do_unload_models(&models).unwrap();
        assert!(models.lock().unwrap().is_none());
    }

    #[test]
    fn unload_models_when_already_empty() {
        let models: siegu_core::ml_worker::LoadedModelsHandle = Arc::new(Mutex::new(None));
        do_unload_models(&models).unwrap();
        assert!(models.lock().unwrap().is_none());
    }

    /// A minimal `LoadedModels` used to prove the cache can be populated and
    /// then cleared without needing any real ONNX sessions.
    fn dummy_loaded_models() -> siegu_core::ml_engine::models::LoadedModels {
        siegu_core::ml_engine::models::LoadedModels {
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
            selected_ep: String::new(),
        }
    }

    #[test]
    fn get_unindexed_count_partial_indexed() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[
            make_photo("ph1", "/a.jpg"),
            make_photo("ph2", "/b.jpg"),
            make_photo("ph3", "/c.jpg"),
        ])
        .unwrap();
        db.update_photo_indexed("ph1", 2);
        assert_eq!(do_get_unindexed_count(&db), 2);
    }
}
