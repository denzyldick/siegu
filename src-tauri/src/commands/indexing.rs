//! Tauri command wrappers for ML analysis / indexing.
//!
//! The business logic lives in `siegu_core::ml_commands` (single source of
//! truth shared with the RPC facade, issue #42). These `#[tauri::command]`
//! functions only add the Tauri-shell concerns: pulling the config path and
//! emitting host-UI log lines. Names are part of the frontend contract
//! (`src/services/tauri.ts`) and must not change.

use crate::common::{emit_log, get_config_path};
use crate::database;
use crate::ml;
use siegu_core::ml_commands;

#[tauri::command]
pub fn get_indexing_status(state: tauri::State<'_, ml::MlContext>) -> usize {
    ml_commands::do_get_indexing_status(&state.pending_count)
}

#[tauri::command]
pub fn get_unindexed_count(app: tauri::AppHandle) -> usize {
    let path = get_config_path(&app);
    if path.is_empty() {
        return 0;
    }
    let db = database::Database::new(&path);
    ml_commands::do_get_unindexed_count(&db)
}

/// Snapshot of the photo table's highest rowid, stored as the cutoff when the
/// user enables "skip existing library": photos at or below it are never
/// auto-analyzed until the option is turned off.
#[tauri::command]
pub fn get_max_photo_rowid(app: tauri::AppHandle) -> i64 {
    let path = get_config_path(&app);
    if path.is_empty() {
        return 0;
    }
    database::Database::new(&path).max_photo_rowid()
}

#[tauri::command]
pub async fn index_faces(
    app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
) -> Result<(), String> {
    emit_log(&app, "Looking for faces in your photos…".to_string());
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Config error".to_string());
    }
    let db = database::Database::new(&path);
    ml_commands::do_index_faces(&db)?;
    ml_commands::send_index_faces_job(&state.tx).await
}

#[tauri::command]
pub async fn analyze_photo(
    state: tauri::State<'_, ml::MlContext>,
    id: String,
) -> Result<(), String> {
    ml_commands::do_analyze_photo(&state.abort, &state.tx, &id).await
}

#[tauri::command]
pub async fn analyze_photo_model(
    state: tauri::State<'_, ml::MlContext>,
    id: String,
    model_id: String,
) -> Result<(), String> {
    ml_commands::do_analyze_photo_model(&state.abort, &state.tx, &id, &model_id).await
}

#[tauri::command]
pub async fn analyze_model(
    state: tauri::State<'_, ml::MlContext>,
    model_id: String,
) -> Result<(), String> {
    ml_commands::do_analyze_model(&state.tx, &model_id).await
}

#[tauri::command]
pub async fn abort_indexing(state: tauri::State<'_, ml::MlContext>) -> Result<(), String> {
    ml_commands::do_abort_indexing(&state.abort, &state.pending_count, &state.paused)
}

#[tauri::command]
pub async fn pause_indexing(state: tauri::State<'_, ml::MlContext>) -> Result<(), String> {
    ml_commands::do_pause_indexing(&state.paused)
}

#[tauri::command]
pub async fn resume_indexing(state: tauri::State<'_, ml::MlContext>) -> Result<(), String> {
    ml_commands::do_resume_indexing(&state.paused)
}

#[tauri::command]
pub async fn unload_models(
    app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
) -> Result<(), String> {
    ml_commands::do_unload_models(&state.models)?;
    emit_log(
        &app,
        "AI features paused. They'll start again with your next analysis.".to_string(),
    );
    Ok(())
}

#[tauri::command]
pub async fn reload_models(
    app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
) -> Result<(), String> {
    ml_commands::do_reload_models(&state.tx).await?;
    emit_log(
        &app,
        "Updating AI features with your new settings…".to_string(),
    );
    Ok(())
}

#[tauri::command]
pub async fn get_models_loaded(state: tauri::State<'_, ml::MlContext>) -> Result<bool, String> {
    Ok(ml_commands::do_get_models_loaded(&state.models))
}
