use crate::emit_log;
use tauri::AppHandle;
use tauri::Emitter;

pub use siegu_core::ml_worker::{Job, MlContext};

use siegu_core::database::Database;
use siegu_core::ml_engine::worker::AnalysisCallbacks;
use siegu_core::ml_engine::PhotoResult;

struct TauriCallbacks {
    app: AppHandle,
    config_path: String,
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

    fn on_scan_complete(&self) {}

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

pub fn start_background_worker(app: &AppHandle, config_path: String) -> MlContext {
    let callbacks = TauriCallbacks {
        app: app.clone(),
        config_path: config_path.clone(),
    };
    siegu_core::ml_engine::worker::start_worker(callbacks, config_path, 32)
}
