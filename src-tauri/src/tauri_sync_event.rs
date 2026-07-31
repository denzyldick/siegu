use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;

use siegu_core::{Database, PeerDevice, SyncEvent, SyncMessage, SyncProgress};

pub struct TauriSyncEvent {
    pub app: tauri::AppHandle,
    pub config_path: String,
    pub sync_tx: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<SyncMessage>>>>,
    /// Tracks whether we already surfaced the "peer offline" notification for the current session.
    pub offline_notified: AtomicBool,
    /// Shared "a live peer connection exists" flag used to protect sessions from auto-reconnect.
    pub connected: Arc<AtomicBool>,
}

impl SyncEvent for TauriSyncEvent {
    fn on_state_change(&self, state: &str) {
        let _ = self.app.emit("webrtc-state", state);
    }

    fn on_log(&self, message: &str) {
        crate::common::emit_log(&self.app, message.to_string());
    }

    fn on_sync_progress(&self, progress: SyncProgress) {
        let _ = self.app.emit("sync-progress", progress);
    }

    fn on_photo_received(&self, photo_id: String, path: String) {
        use crate::database::Photo;
        use std::collections::HashMap;
        let _ = self.app.emit(
            "photo-received",
            Photo {
                id: photo_id,
                encoded: String::new(),
                location: path,
                created: String::new(),
                objects: HashMap::new(),
                properties: HashMap::new(),
                latitude: 0.0,
                longitude: 0.0,
                favorite: false,
                indexed: 2,
                caption: None,
                aesthetics_score: None,
                ai_status: crate::database::AiStatus::default(),
            },
        );
    }

    fn on_sync_error(&self, error: String) {
        let _ = self.app.emit("sync-error", error);
    }

    fn on_peer_connected(
        &self,
        device_id: String,
        device_name: String,
        models_enabled: Vec<String>,
        protocol_version: u8,
    ) {
        let db = Database::new(&self.config_path);
        let device = PeerDevice {
            device_id: device_id.clone(),
            name: device_name,
            ip: String::new(),
            port: 0,
            device_type: String::new(),
            os: String::new(),
            models_enabled,
            protocol_version,
            storage_used: 0,
            storage_capacity: 0,
            last_seen: String::new(),
        };
        db.upsert_peer_device(&device);
        self.on_log(&format!("Peer registered: {device_id}"));
        self.offline_notified.store(false, Ordering::SeqCst);
        self.connected.store(true, Ordering::SeqCst);
        let _ = self.app.emit("webrtc-state", "Peer Connected");
        let _ = self.app.emit("peer-connected", &device);
    }

    fn on_peer_offline(&self) {
        self.connected.store(false, Ordering::SeqCst);
        if !self.offline_notified.swap(true, Ordering::SeqCst) {
            self.on_log("Peer offline: sync paused until device reconnects");
            notify_sync_paused(&self.app);
        }
    }

    fn on_peer_disconnected(&self, peer_id: String) {
        let db = Database::new(&self.config_path);
        db.update_peer_device_seen(&peer_id);
        self.on_log(&format!("Peer disconnected: {peer_id}"));
        let _ = self.app.emit("webrtc-state", "Peer Disconnected");
        let _ = self.app.emit("peer-disconnected", &peer_id);
    }

    fn on_device_registered(&self, db: &Database) {
        let _ = self.app.emit("refresh-devices", ());
        let _ = db;
    }

    fn get_config_path(&self) -> String {
        self.config_path.clone()
    }

    fn get_sync_path(&self) -> Option<String> {
        let db = Database::new(&self.config_path);
        db.get_state().get("sync_path").cloned()
    }

    fn get_directories(&self) -> Vec<String> {
        let db = Database::new(&self.config_path);
        db.list_directories()
    }

    fn on_metadata_updated(
        &self,
        photo_id: &str,
        caption: Option<&str>,
        aesthetics_score: Option<f64>,
    ) {
        self.on_log(&format!("Metadata updated for {photo_id}"));
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
}

pub fn notify_sync_paused(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        use tauri_plugin_notification::NotificationExt;
        let _ = app
            .notification()
            .builder()
            .title("Siegu")
            .body("Sync paused — device went offline. It will resume when it reconnects.")
            .show();
    });
}

pub fn notify_photos_ready(app: &tauri::AppHandle, pending: i64) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        use tauri_plugin_notification::NotificationExt;
        let _ = app
            .notification()
            .builder()
            .title("Siegu")
            .body(format!(
                "Scan complete — {pending} photo(s) ready to sync. Connect a device to sync them."
            ))
            .show();
    });
}
