use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Sender;

/// Capacity of the worker job channel. Senders block once this many jobs are
/// queued, applying backpressure so the UI cannot outpace analysis.
pub const JOB_CHANNEL_CAPACITY: usize = 1000;

/// Handle to the worker's loaded models cache. `None` means no models are
/// loaded; they reload lazily before the next analysis job.
pub type LoadedModelsHandle = Arc<Mutex<Option<crate::ml_engine::models::LoadedModels>>>;

pub struct MlContext {
    pub tx: Sender<Job>,
    pub pending_count: Arc<AtomicUsize>,
    pub abort: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
    pub models: LoadedModelsHandle,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Job {
    ProcessAll,
    AnalyzeSingle(String),
    AnalyzeSingleWithModel(String, String),
    ProcessModel(String),
    /// Force a reload of all loaded models from the latest config (e.g. after
    /// the user changed AI speed or memory budget). No photos are processed.
    ReloadModels,
}

impl Job {
    pub fn is_single(&self) -> bool {
        matches!(
            self,
            Job::AnalyzeSingle(_) | Job::AnalyzeSingleWithModel(_, _)
        )
    }

    pub fn photo_id(&self) -> Option<&str> {
        match self {
            Job::ProcessAll | Job::ProcessModel(_) | Job::ReloadModels => None,
            Job::AnalyzeSingle(id) | Job::AnalyzeSingleWithModel(id, _) => Some(id),
        }
    }

    pub fn target_model(&self) -> Option<&str> {
        match self {
            Job::AnalyzeSingleWithModel(_, model_id) => Some(model_id),
            Job::ProcessModel(model_id) => Some(model_id),
            _ => None,
        }
    }
}

pub const ALL_MODEL_NAMES: &[&str] = &[
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
];

pub fn job_status_model(model_id: &str) -> Option<&'static str> {
    match model_id {
        "clip" => Some("clip"),
        "ultraface" | "face" | "arcface" => Some("face"),
        "ocr" => Some("ocr"),
        "nsfw" => Some("nsfw"),
        "aesthetics" => Some("aesthetics"),
        "yolo" => Some("yolo"),
        "blip" => Some("blip"),
        "midas" => Some("midas"),
        "whisper" => Some("whisper"),
        _ => None,
    }
}

pub fn should_run_model(
    target_model: Option<&str>,
    model: &str,
    config: Option<&HashMap<String, String>>,
) -> bool {
    if let Some(config) = config {
        let key = format!("model_enabled_{}", model);
        if let Some(val) = config.get(&key) {
            if val != "true" {
                return false;
            }
        }
    }
    target_model.is_none_or(|target| target == model)
}

pub fn any_model_enabled(config: &HashMap<String, String>) -> bool {
    // A model counts as enabled when it has no explicit config entry, mirroring
    // should_run_model and the app's default (missing key => enabled). This keeps
    // the CLI in sync with the Tauri app's model toggles.
    ALL_MODEL_NAMES.iter().any(|m| {
        config
            .get(&format!("model_enabled_{m}"))
            .is_none_or(|v| v == "true")
    })
}

pub const MAX_REASONABLE_PENDING: usize = 1_000_000;

pub fn decrement_pending_count(counter: &std::sync::atomic::AtomicUsize) -> usize {
    use std::sync::atomic::Ordering;
    counter
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            if current > MAX_REASONABLE_PENDING {
                Some(0)
            } else {
                Some(current.saturating_sub(1))
            }
        })
        .map(|previous| {
            if previous > MAX_REASONABLE_PENDING {
                0
            } else {
                previous.saturating_sub(1)
            }
        })
        .unwrap_or(0)
}

pub fn increment_pending_count(counter: &std::sync::atomic::AtomicUsize, amount: usize) -> usize {
    use std::sync::atomic::Ordering;
    counter
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            let base = if current > MAX_REASONABLE_PENDING {
                0
            } else {
                current
            };
            Some(base.saturating_add(amount))
        })
        .map(|previous| {
            let base = if previous > MAX_REASONABLE_PENDING {
                0
            } else {
                previous
            };
            base.saturating_add(amount)
        })
        .unwrap_or(amount)
}

pub fn flush_batch_in_transaction<F>(conn: &rusqlite::Connection, ops: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| e.to_string())?;
    match ops() {
        Ok(()) => {
            conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn test_job_is_single() {
        assert!(Job::AnalyzeSingle("a".into()).is_single());
        assert!(Job::AnalyzeSingleWithModel("a".into(), "b".into()).is_single());
        assert!(!Job::ProcessAll.is_single());
        assert!(!Job::ProcessModel("a".into()).is_single());
        assert!(!Job::ReloadModels.is_single());
    }

    #[test]
    fn test_job_photo_id() {
        assert_eq!(Job::AnalyzeSingle("abc".into()).photo_id(), Some("abc"));
        assert_eq!(Job::ProcessAll.photo_id(), None);
        assert_eq!(Job::ProcessModel("x".into()).photo_id(), None);
        assert_eq!(Job::ReloadModels.photo_id(), None);
    }

    #[test]
    fn test_job_target_model() {
        assert_eq!(
            Job::AnalyzeSingleWithModel("a".into(), "clip".into()).target_model(),
            Some("clip")
        );
        assert_eq!(Job::AnalyzeSingle("a".into()).target_model(), None);
        assert_eq!(Job::ProcessAll.target_model(), None);
    }

    #[test]
    fn test_should_run_model_no_config() {
        assert!(should_run_model(None, "clip", None));
        assert!(should_run_model(Some("clip"), "clip", None));
        assert!(!should_run_model(Some("face"), "clip", None));
    }

    #[test]
    fn test_should_run_model_with_config() {
        let mut config = HashMap::new();
        config.insert("model_enabled_clip".to_string(), "true".to_string());
        config.insert("model_enabled_face".to_string(), "false".to_string());

        assert!(should_run_model(None, "clip", Some(&config)));
        assert!(!should_run_model(None, "face", Some(&config)));
        assert!(should_run_model(Some("clip"), "clip", Some(&config)));
        assert!(!should_run_model(Some("face"), "clip", Some(&config)));
    }

    #[test]
    fn test_any_model_enabled() {
        // No explicit config => models default to enabled (matches app default).
        let config = HashMap::new();
        assert!(any_model_enabled(&config));

        let mut config = HashMap::new();
        config.insert("model_enabled_yolo".to_string(), "true".to_string());
        assert!(any_model_enabled(&config));

        // All models explicitly disabled => none enabled.
        let mut config = HashMap::new();
        for m in ALL_MODEL_NAMES {
            config.insert(format!("model_enabled_{m}"), "false".to_string());
        }
        assert!(!any_model_enabled(&config));

        let mut config = HashMap::new();
        config.insert("model_enabled_clip".to_string(), "true".to_string());
        config.insert("model_enabled_face".to_string(), "false".to_string());
        assert!(any_model_enabled(&config));
    }

    #[test]
    fn test_pending_count() {
        let counter = AtomicUsize::new(0);
        increment_pending_count(&counter, 5);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5);
        decrement_pending_count(&counter);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    #[test]
    fn test_pending_count_saturating() {
        let counter = AtomicUsize::new(0);
        decrement_pending_count(&counter);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn test_pending_count_overflow_clamp() {
        let counter = AtomicUsize::new(MAX_REASONABLE_PENDING + 100);
        let result = decrement_pending_count(&counter);
        assert_eq!(result, 0);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn test_flush_batch_in_transaction_success() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER)").unwrap();
        let result = flush_batch_in_transaction(&conn, || {
            conn.execute("INSERT INTO t VALUES (1)", ())
                .map_err(|e| e.to_string())?;
            conn.execute("INSERT INTO t VALUES (2)", ())
                .map_err(|e| e.to_string())?;
            Ok(())
        });
        assert!(result.is_ok());
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM t", (), |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_flush_batch_in_transaction_rollback() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER UNIQUE)")
            .unwrap();
        conn.execute("INSERT INTO t VALUES (1)", ()).unwrap();
        let result = flush_batch_in_transaction(&conn, || {
            conn.execute("INSERT INTO t VALUES (1)", ())
                .map_err(|e| e.to_string())?;
            Ok(())
        });
        assert!(result.is_err());
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM t", (), |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
