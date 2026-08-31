use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Emitter;

use siegu_core::{Database, PeerDevice, SyncEvent, SyncMessage, SyncPhase, SyncProgress};

/// Minimum interval between "sync-progress" IPC emissions. Per-chunk progress
/// updates arrive several hundred times per second; this caps the JS side.
const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(250);

pub struct TauriSyncEvent {
    pub app: tauri::AppHandle,
    pub config_path: String,
    pub sync_tx: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<SyncMessage>>>>,
    /// Tracks whether we already surfaced the "peer offline" notification for the current session.
    pub offline_notified: AtomicBool,
    /// Shared "a live peer connection exists" flag used to protect sessions from auto-reconnect.
    pub connected: Arc<AtomicBool>,
    /// The device_id of the currently connected peer, used to attribute received files.
    pub active_peer: Arc<tokio::sync::Mutex<Option<String>>>,
    /// Timestamp of the last emitted "sync-progress" event, for rate limiting.
    pub last_sync_progress: std::sync::Mutex<Option<Instant>>,
    /// When set, the share is a one-time view: close it as soon as the last
    /// guest's session ends so the link can't be opened again.
    pub one_time_share: Arc<AtomicBool>,
}

impl SyncEvent for TauriSyncEvent {
    fn on_state_change(&self, state: &str) {
        let _ = self.app.emit("webrtc-state", state);
    }

    fn on_log(&self, message: &str) {
        crate::common::emit_log(&self.app, message.to_string());
    }

    fn on_sync_progress(&self, progress: SyncProgress) {
        crate::notify::mark_sync_activity();
        // Per-file start events (carry filename + thumbnail) and completion
        // events always pass through; plain per-chunk updates are rate limited.
        let always_emit = progress.filename.is_some()
            || progress.phase == SyncPhase::Completed
            || progress.progress >= 100.0;
        let emit = match self.last_sync_progress.lock() {
            Ok(mut last) => {
                let now = Instant::now();
                let ok = last
                    .as_ref()
                    .map(|t| now.duration_since(*t) >= PROGRESS_EMIT_INTERVAL)
                    .unwrap_or(true);
                if ok {
                    *last = Some(now);
                }
                ok
            }
            Err(_) => true,
        };
        if always_emit || emit {
            let _ = self.app.emit("sync-progress", progress);
        }
    }

    fn on_photo_received(&self, photo_id: String, path: String) {
        use crate::database::Photo;
        use std::collections::HashMap;

        // Attribute the received file to the active peer so its card shows live counts.
        let is_video = crate::database::is_video_path(&path);
        if let Ok(peer) = self.active_peer.try_lock() {
            if let Some(peer_id) = peer.as_ref() {
                let db = Database::new(&self.config_path);
                if is_video {
                    db.increment_peer_device_counts(peer_id, 0, 1);
                } else {
                    db.increment_peer_device_counts(peer_id, 1, 0);
                }
            }
        }
        let _ = self.app.emit("refresh-devices", ());

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
                sync_needed: false,
                received: true,
                view_only: false,
                last_opened: 0,
            },
        );
    }

    fn on_sync_error(&self, error: String) {
        crate::log::persist_log("error", &error);
        let _ = self.app.emit("sync-error", error);
    }

    fn on_peer_connected(
        &self,
        device_id: String,
        device_name: String,
        peer_os: String,
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
            os: peer_os,
            models_enabled,
            protocol_version,
            storage_used: 0,
            storage_capacity: 0,
            last_seen: String::new(),
            photo_count: 0,
            video_count: 0,
            remote_photo_count: 0,
            remote_video_count: 0,
        };
        db.upsert_peer_device(&device);
        if let Ok(mut peer) = self.active_peer.try_lock() {
            *peer = Some(device_id.clone());
        }
        self.on_log(&format!("Peer registered: {device_id}"));
        self.offline_notified.store(false, Ordering::SeqCst);
        self.connected.store(true, Ordering::SeqCst);
        cancel_sync_paused(&self.app);
        let _ = self.app.emit("webrtc-state", "Peer Connected");
        let _ = self.app.emit("peer-connected", &device);
        let _ = self.app.emit("refresh-devices", ());
    }

    fn on_peer_offline(&self) {
        self.connected.store(false, Ordering::SeqCst);
        if let Ok(mut peer) = self.active_peer.try_lock() {
            *peer = None;
        }
        if !self.offline_notified.swap(true, Ordering::SeqCst) {
            self.on_log("Peer offline: sync paused until device reconnects");
            notify_sync_paused(&self.app, &self.config_path);
        }
        // One-time share: the guest's session ended, so the link can't be
        // opened again. Signal the frontend to close the whole share.
        if self.one_time_share.swap(false, Ordering::SeqCst) {
            self.on_log("One-time share: guest session ended — closing share");
            let _ = self.app.emit("share-one-time-done", ());
        }
    }

    fn on_peer_library_stats(&self, photo_count: i64, video_count: i64) {
        let peer_id = self.active_peer.try_lock().ok().and_then(|p| p.clone());
        if let Some(peer_id) = peer_id {
            Database::new(&self.config_path).set_peer_remote_counts(
                &peer_id,
                photo_count,
                video_count,
            );
            self.on_log(&format!(
                "Peer library: {photo_count} photos, {video_count} videos"
            ));
            let _ = self.app.emit("refresh-devices", ());
        }
    }

    /// Peer sent us a read-only manifest (#9): hand it to the frontend so the
    /// guest gallery can render without any data being written locally.
    fn on_view_manifest(&self, photos: &[siegu_core::PhotoSyncInfo]) {
        match serde_json::to_string(&photos) {
            Ok(json) => {
                let _ = self.app.emit("view-manifest", json);
            }
            Err(e) => self.on_log(&format!("Failed to serialize view manifest: {e}")),
        }
    }

    fn on_peer_disconnected(&self, peer_id: String) {
        let db = Database::new(&self.config_path);
        db.update_peer_device_seen(&peer_id);
        self.on_log(&format!("Peer disconnected: {peer_id}"));
        let _ = self.app.emit("webrtc-state", "Peer Disconnected");
        let _ = self.app.emit("peer-disconnected", &peer_id);
    }

    fn on_album_viewed(&self, album_id: &str, album_name: &str) {
        let db = Database::new(&self.config_path);
        let count = db.increment_album_share_count(album_id);
        self.on_log(&format!(
            "Album share viewed: '{album_name}' ({count} total view{})",
            if count == 1 { "" } else { "s" }
        ));
        let _ = self.app.emit(
            "album-share-viewed",
            serde_json::json!({ "albumId": album_id, "count": count }),
        );
        let _ = self.app.emit("refresh-albums", ());
        crate::notify::notify_routine(
            &self.app,
            format!("Someone is viewing your collection '{album_name}'"),
        );
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
                    deleted_at: None,
                });
            }
        }
    }
}
pub const SYNC_PAUSED_NOTIFICATION_ID: i32 = 4201;
pub const SYNC_PAUSED_CHANNEL_ID: &str = "sync_paused";

pub fn notify_sync_paused(app: &tauri::AppHandle, config_path: &str) {
    let (pending_photos, pending_videos) = Database::new(config_path).get_pending_sync_counts();
    let mut parts = Vec::new();
    if pending_photos > 0 {
        parts.push(format!("{pending_photos} photo(s)"));
    }
    if pending_videos > 0 {
        parts.push(format!("{pending_videos} video(s)"));
    }
    let body = if parts.is_empty() {
        "Sync paused — device went offline. It will resume when it reconnects.".to_string()
    } else {
        format!(
            "{} not synced. Reconnect a device to back them up.",
            parts.join(" · ")
        )
    };

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        use tauri_plugin_notification::NotificationExt;
        #[cfg(target_os = "android")]
        {
            let channel =
                tauri_plugin_notification::Channel::builder(SYNC_PAUSED_CHANNEL_ID, "Sync status")
                    .description("Ongoing sync progress and not-synced items")
                    .importance(tauri_plugin_notification::Importance::High)
                    .build();
            let _ = app.notification().create_channel(channel);
        }
        let _ = app
            .notification()
            .builder()
            .id(SYNC_PAUSED_NOTIFICATION_ID)
            .channel_id(SYNC_PAUSED_CHANNEL_ID)
            .title("Siegu")
            .body(body)
            .ongoing()
            .show();
    });
}

pub fn cancel_sync_paused(app: &tauri::AppHandle) {
    #[cfg(mobile)]
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            use tauri_plugin_notification::NotificationExt;
            let _ = app.notification().cancel(vec![SYNC_PAUSED_NOTIFICATION_ID]);
        });
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
    }
}

pub fn notify_photos_ready(app: &tauri::AppHandle, pending: i64) {
    crate::notify::notify_critical(
        app,
        format!("Scan complete — {pending} photo(s) ready to sync. Connect a device to sync them."),
    );
}
