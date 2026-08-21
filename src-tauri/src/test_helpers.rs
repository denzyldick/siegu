use siegu_core::database::{AiStatus, Database, Photo};
use siegu_core::ml_worker::{Job, MlContext};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::sync::mpsc;

/// Create a temporary test database that is automatically cleaned up.
pub fn test_db() -> (Database, TempDir) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let db = Database::new(&dir.path().display().to_string());
    (db, dir)
}

/// Create a mock MlContext with a receiver to inspect sent jobs.
pub fn mock_ml_context() -> (MlContext, mpsc::Receiver<Job>) {
    let (tx, rx) = mpsc::channel(siegu_core::ml_worker::JOB_CHANNEL_CAPACITY);
    let ctx = MlContext {
        tx,
        pending_count: Arc::new(AtomicUsize::new(0)),
        abort: Arc::new(AtomicBool::new(false)),
        paused: Arc::new(AtomicBool::new(false)),
        models: Arc::new(Mutex::new(None)),
    };
    (ctx, rx)
}

/// Build a minimal Photo for testing.
pub fn make_photo(id: &str, location: &str) -> Photo {
    Photo {
        id: id.to_string(),
        location: location.to_string(),
        encoded: String::new(),
        created: "2026-01-01 12:00:00".to_string(),
        objects: HashMap::new(),
        properties: HashMap::new(),
        latitude: 0.0,
        longitude: 0.0,
        favorite: false,
        indexed: 0,
        caption: None,
        aesthetics_score: None,
        ai_status: AiStatus::default(),
        sync_needed: true,
        received: false,
    }
}

/// Build a Photo with GPS coordinates.
pub fn make_photo_gps(id: &str, lat: f64, lon: f64) -> Photo {
    Photo {
        latitude: lat,
        longitude: lon,
        ..make_photo(id, &format!("/photos/{id}.jpg"))
    }
}
