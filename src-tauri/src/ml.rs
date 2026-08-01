use std::sync::Arc;

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
    config_path: String,
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
        let db = Database::new(&self.config_path);
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
        let _ = self.app.emit("indexing-progress", remaining);
        let _ = self.app.emit("indexing-eta", (remaining as f64) * 1000.0);

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
            let _ = self.app.emit(
                "scan-progress",
                serde_json::json!({ "status": "complete", "progress": 100 }),
            );
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
        let db = Database::new(&self.config_path);
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

    fn on_progress(&self, _completed: usize, _total: usize, _avg_ms: f64) {}

    fn on_model_status(&self, model: &str, status: &str, pending: usize, total: usize) {
        let _ = self.app.emit(
            "model-progress",
            serde_json::json!({
                "model": model,
                "pending": pending,
                "total": total,
                "status": status,
            }),
        );
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
        config_path: config_path.clone(),
        sync_tx,
    };
    siegu_core::ml_engine::worker::start_worker(callbacks, config_path, 32)
}
