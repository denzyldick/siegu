use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::database::{Database, ImportedPhoto, PhotoSyncInfo};
use crate::sync_transport::{
    cleanup_sync_temp, resolve_sync_target_dir, sanitize_filename, sync_temp_dir,
};

/// Shared view-only session state (#9). One mesh session can be active at a
/// time, so a process-global registry is sufficient.
fn view_state() -> &'static crate::view_only::ViewOnlyState {
    crate::view_only::state()
}

pub const PROTOCOL_VERSION: u8 = 2;
pub const MAX_MESH_DEVICES: usize = 5;

/// Max raw file bytes carried in a single `FileChunk` payload. The `Vec<u8>` is
/// serialized as a JSON array of numbers (~4 chars/byte), so the whole message
/// must stay well under the SCTP max message size (65536). A chunk payload this
/// size serializes to ~56 KB.
pub const FILE_CHUNK_PAYLOAD: usize = 14000;

/// Serialized-byte budget for a single sync message. The WebRTC data channel
/// rejects messages larger than the negotiated SCTP max message size (65536 by
/// default), so anything that could grow past that — like a photo manifest for a
/// large library — must be split into chunks below this budget.
pub const SYNC_MESSAGE_BUDGET: usize = 48000;
/// Hard ceiling for a single message on the wire. The receiver's read buffer is
/// `u16::MAX` bytes; a message at or above that makes the peer error out and
/// close the data channel, aborting the whole sync. Keep well below it.
pub const MAX_DATA_CHANNEL_MSG_SIZE: usize = 60000;
pub const MAX_BUFFERED_BYTES: usize = 1_000_000;
pub const MAX_RETRY_ATTEMPTS: u32 = 3;
pub const RETRY_BACKOFF_MS: u64 = 500;
pub const BACKPRESSURE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SyncPhase {
    #[default]
    Idle,
    Syncing,
    /// Browsing a peer's library without writing anything (#9).
    ViewOnly,
    Completed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncProgress {
    pub device_id: String,
    pub status: String,
    #[serde(default)]
    pub phase: SyncPhase,
    pub progress: f32,
    pub bytes_per_second: u64,
    pub items_completed: usize,
    pub items_total: usize,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncMessage {
    ManifestRequest,
    ManifestResponse {
        photos: Vec<PhotoSyncInfo>,
        #[serde(default)]
        more: bool,
    },
    FileRequest {
        id: String,
    },
    FileHeader {
        id: String,
        filename: String,
        relative_path: String,
        size: u64,
        checksum: String,
        created: String,
        latitude: Option<f64>,
        longitude: Option<f64>,
        objects: String,
        faces: String,
        caption: Option<String>,
        aesthetics_score: Option<f64>,
        #[serde(default)]
        encoded: String,
    },
    FileChunk {
        id: String,
        index: u32,
        data: Vec<u8>,
    },
    FileEnd {
        id: String,
        checksum: String,
    },
    SyncFile {
        photo: PhotoSyncInfo,
    },
    StartSync,
    CatchUp,
    PeerProgress {
        status: String,
        #[serde(default)]
        phase: SyncPhase,
        progress: f32,
        items_completed: usize,
        items_total: usize,
    },
    /// One-shot announcement of this device's full library size so the peer's
    /// linked-devices page can show real totals instead of per-session counts.
    PeerLibraryStats {
        photo_count: i64,
        video_count: i64,
    },
    MetadataUpdate {
        photo_id: String,
        caption: Option<String>,
        aesthetics_score: Option<f64>,
        indexed: i32,
        #[serde(default)]
        deleted_at: Option<String>,
    },
    VersionNegotiate {
        version: u8,
        device_id: String,
        device_name: String,
        os: String,
        models_enabled: Vec<String>,
    },
    VersionReject {
        reason: String,
    },
    /// Viewer asks to browse the peer's library without any writes (#9).
    EnterViewOnly,
    /// Chunked manifest of the sharer's library, sent to a view-only client.
    ViewOnlyManifest {
        photos: Vec<PhotoSyncInfo>,
        #[serde(default)]
        more: bool,
    },
    /// View-only client requests media for one photo. Thumbnails stream back
    /// as a direct `ViewMedia` reply; originals reuse FileHeader/Chunk/End.
    FetchMediaRequest {
        id: String,
        #[serde(default)]
        thumbnail: bool,
    },
    /// Sharer replies with directly-served bytes (thumbnails) for one photo.
    ViewMedia {
        id: String,
        mime: String,
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub struct OutgoingFile {
    pub id: String,
    pub path: String,
    pub relative_path: String,
    pub created: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub objects: String,
    pub faces: String,
    pub caption: Option<String>,
    pub aesthetics_score: Option<f64>,
    pub encoded: String,
}

pub struct IncomingFile {
    pub id: String,
    pub filename: String,
    pub relative_path: String,
    pub size: u64,
    pub received: u64,
    pub checksum: String,
    pub created: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub objects: String,
    pub faces: String,
    pub caption: Option<String>,
    pub aesthetics_score: Option<f64>,
    pub encoded: String,
    pub file: tokio::fs::File,
}

pub trait SyncEvent: Send + Sync {
    fn on_state_change(&self, state: &str);
    fn on_log(&self, message: &str);
    fn on_sync_progress(&self, progress: SyncProgress);
    fn on_photo_received(&self, photo_id: String, path: String);
    fn on_sync_error(&self, error: String);
    fn on_peer_connected(
        &self,
        peer_id: String,
        peer_name: String,
        peer_os: String,
        models_enabled: Vec<String>,
        protocol_version: u8,
    );
    fn on_peer_disconnected(&self, peer_id: String);
    /// Called when the connected peer drops off (transport failure / explicit leave).
    fn on_peer_offline(&self) {}
    /// Called when the peer announces its full library size (photo, video).
    fn on_peer_library_stats(&self, _photo_count: i64, _video_count: i64) {}
    /// Called once when a view-only peer finishes receiving our manifest
    /// chunks (#9): the UI builds an ephemeral read-only gallery from it.
    fn on_view_manifest(&self, _photos: &[PhotoSyncInfo]) {}
    fn on_device_registered(&self, db: &Database);
    fn on_metadata_updated(
        &self,
        photo_id: &str,
        caption: Option<&str>,
        aesthetics_score: Option<f64>,
    );
    fn get_config_path(&self) -> String;
    fn get_sync_path(&self) -> Option<String>;
    fn get_directories(&self) -> Vec<String>;
}

pub struct MeshManager {
    pub room_id: String,
    pub is_initiator: bool,
    pub signaling_url: String,
    pub config_path: String,
    pub sync_tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<SyncMessage>>>>,
}

impl MeshManager {
    pub fn new(
        room_id: String,
        is_initiator: bool,
        signaling_url: String,
        config_path: String,
    ) -> Self {
        Self {
            room_id,
            is_initiator,
            signaling_url,
            config_path,
            sync_tx: Arc::new(Mutex::new(None)),
        }
    }

    pub fn compute_relative_path(absolute_path: &str, known_dirs: &[String]) -> String {
        let path = Path::new(absolute_path);
        for dir in known_dirs {
            if let Ok(relative) = path.strip_prefix(dir) {
                return relative.to_string_lossy().to_string();
            }
        }
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    pub async fn compute_file_checksum(
        path: &Path,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        use sha2::{Digest, Sha256};
        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 1024 * 1024];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub fn compute_data_checksum(data: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    pub async fn send_sync_message(
        dc: &Arc<webrtc::data_channel::RTCDataChannel>,
        msg: &SyncMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let json = serde_json::to_string(msg)?;
        dc.send(&bytes::Bytes::from(json)).await?;
        Ok(())
    }

    /// Serialized size of a message, or `usize::MAX` if serialization fails.
    fn serialized_len(msg: &SyncMessage) -> usize {
        serde_json::to_string(msg)
            .map(|s| s.len())
            .unwrap_or(usize::MAX)
    }

    /// Build a `FileHeader` message whose serialized size stays under the data
    /// channel ceiling. Face crops (`faces` entries carry base64 JPEGs that can
    /// exceed 64KiB on their own) are trimmed first, then the thumbnail, so a
    /// single oversized header can never take the whole channel down. Returns
    /// the message and whether anything was trimmed.
    #[allow(clippy::too_many_arguments)]
    fn fit_file_header(
        id: String,
        filename: String,
        relative_path: String,
        size: u64,
        checksum: String,
        created: String,
        latitude: Option<f64>,
        longitude: Option<f64>,
        objects: String,
        mut faces: String,
        caption: Option<String>,
        aesthetics_score: Option<f64>,
        encoded: String,
    ) -> (SyncMessage, bool) {
        let build = |faces: &str, encoded: &str| SyncMessage::FileHeader {
            id: id.clone(),
            filename: filename.clone(),
            relative_path: relative_path.clone(),
            size,
            checksum: checksum.clone(),
            created: created.clone(),
            latitude,
            longitude,
            objects: objects.clone(),
            faces: faces.to_string(),
            caption: caption.clone(),
            aesthetics_score,
            encoded: encoded.to_string(),
        };

        let msg = build(&faces, &encoded);
        if Self::serialized_len(&msg) < MAX_DATA_CHANNEL_MSG_SIZE {
            return (msg, false);
        }

        // Trim the bulky face crops first (they are the largest field).
        faces = Self::strip_face_crops(&faces);
        let msg = build(&faces, &encoded);
        if Self::serialized_len(&msg) < MAX_DATA_CHANNEL_MSG_SIZE {
            return (msg, true);
        }

        // Still too big (e.g. a large thumbnail): drop it too.
        (build(&faces, ""), true)
    }

    /// Re-serialize a `faces` JSON array, blanking the `encoded` field (the
    /// base64 face crop) while keeping face ids, crop paths and person ids.
    fn strip_face_crops(faces_json: &str) -> String {
        let trimmed: Vec<serde_json::Value> = serde_json::from_str(faces_json)
            .ok()
            .and_then(|v: serde_json::Value| v.as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .map(|mut face| {
                face["encoded"] = serde_json::Value::String(String::new());
                face
            })
            .collect();
        serde_json::to_string(&trimmed).unwrap_or_else(|_| "[]".to_string())
    }

    /// Split a manifest into chunks, each serializing to less than the data
    /// channel's max message size. Returns (chunk, more).
    pub fn split_manifest_chunks(photos: Vec<PhotoSyncInfo>) -> Vec<(Vec<PhotoSyncInfo>, bool)> {
        let mut chunks = Vec::new();
        let mut remaining = photos.into_iter().peekable();
        loop {
            let mut chunk = Vec::new();
            let mut bytes = 0usize;
            while let Some(photo) = remaining.peek() {
                let approx = serde_json::to_string(photo).map(|s| s.len()).unwrap_or(256);
                if !chunk.is_empty() && bytes + approx > SYNC_MESSAGE_BUDGET {
                    break;
                }
                bytes += approx;
                chunk.push(photo.clone());
                remaining.next();
            }
            let more = remaining.peek().is_some();
            chunks.push((chunk, more));
            if !more {
                return chunks;
            }
        }
    }

    /// Send a photo manifest as a series of `ManifestResponse` messages, each
    /// staying below the data channel's max message size. The last message has
    /// `more: false` so the receiver knows the manifest is complete.
    pub async fn send_manifest_response(
        dc: &Arc<webrtc::data_channel::RTCDataChannel>,
        photos: Vec<PhotoSyncInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for (chunk, more) in Self::split_manifest_chunks(photos) {
            Self::send_sync_message(
                dc,
                &SyncMessage::ManifestResponse {
                    photos: chunk,
                    more,
                },
            )
            .await?;
        }
        Ok(())
    }

    /// Chunked manifest for a view-only client (#9): metadata only, no
    /// counters or sync state involved.
    pub async fn send_manifest_view_only(
        dc: &Arc<webrtc::data_channel::RTCDataChannel>,
        photos: Vec<PhotoSyncInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for (chunk, more) in Self::split_manifest_chunks(photos) {
            Self::send_sync_message(
                dc,
                &SyncMessage::ViewOnlyManifest {
                    photos: chunk,
                    more,
                },
            )
            .await?;
        }
        Ok(())
    }

    pub async fn send_file_with_retry(
        dc: Arc<webrtc::data_channel::RTCDataChannel>,
        outgoing: OutgoingFile,
        event: Arc<dyn SyncEvent>,
        items_completed: &Arc<std::sync::atomic::AtomicUsize>,
        items_total: &Arc<std::sync::atomic::AtomicUsize>,
        mirror_completed: &Arc<std::sync::atomic::AtomicUsize>,
        mirror_total: &Arc<std::sync::atomic::AtomicUsize>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut last_err = None;
        for attempt in 0..MAX_RETRY_ATTEMPTS {
            match Self::send_file_inner(
                &dc,
                &outgoing,
                event.as_ref(),
                items_completed,
                items_total,
                mirror_completed,
                mirror_total,
                true,
            )
            .await
            {
                Ok(()) => return Ok(()),
                Err(e) => {
                    event.on_log(&format!(
                        "File send attempt {}/{} failed for {}: {}",
                        attempt + 1,
                        MAX_RETRY_ATTEMPTS,
                        outgoing.id,
                        e
                    ));
                    last_err = Some(e);
                    if attempt < MAX_RETRY_ATTEMPTS - 1 {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            RETRY_BACKOFF_MS * (attempt as u64 + 1),
                        ))
                        .await;
                    }
                }
            }
        }
        Err(last_err.unwrap_or_else(|| "Send failed after retries".into()))
    }

    /// Stream one file over the data channel. `track_progress` gates batch
    /// counter emissions: sync transfers report progress; on-demand view-only
    /// reads must stay silent (a friend browsing should not move our k/N).
    #[allow(clippy::too_many_arguments)]
    async fn send_file_inner(
        dc: &Arc<webrtc::data_channel::RTCDataChannel>,
        outgoing: &OutgoingFile,
        event: &dyn SyncEvent,
        items_completed: &Arc<std::sync::atomic::AtomicUsize>,
        items_total: &Arc<std::sync::atomic::AtomicUsize>,
        mirror_completed: &Arc<std::sync::atomic::AtomicUsize>,
        mirror_total: &Arc<std::sync::atomic::AtomicUsize>,
        track_progress: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let photo_id = outgoing.id.clone();
        let path = Path::new(&outgoing.path);
        if !path.exists() {
            return Err(format!("File not found: {}", outgoing.path).into());
        }

        // Batch counters are per-direction: the local pair tracks files this
        // side pulls from the peer, the mirror pair mirrors the batch the peer
        // pulls from us (arriving via PeerProgress). The UI shows their sum so
        // concurrent opposite-direction transfers compose into one k/N.
        let counters = || {
            (
                items_completed.load(std::sync::atomic::Ordering::SeqCst)
                    + mirror_completed.load(std::sync::atomic::Ordering::SeqCst),
                items_total.load(std::sync::atomic::Ordering::SeqCst)
                    + mirror_total.load(std::sync::atomic::Ordering::SeqCst),
            )
        };

        let mut file = tokio::fs::File::open(path).await?;
        let metadata = file.metadata().await?;
        let size = metadata.len();
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Surface the per-file start event immediately (before the thumbnail
        // pass) so the UI shows the next file without waiting on a decode.
        // Counters ride along so batch progress in the UI never resets to 0/0.
        if track_progress {
            let (completed, total) = counters();
            event.on_sync_progress(SyncProgress {
                device_id: "peer".to_string(),
                status: format!("Sending {filename}"),
                phase: SyncPhase::Syncing,
                progress: 0.0,
                bytes_per_second: 0,
                items_completed: completed,
                items_total: total,
                filename: Some(filename.clone()),
                thumbnail: None,
            });
        }

        // Generate a missing thumbnail here (on the send path) so the peer's
        // FileHeader always carries one, then surface it to the local UI and
        // cache it in the DB so later transfers of the same photo are instant.
        let encoded = if outgoing.encoded.is_empty() {
            let thumb = crate::thumbnail::generate_thumbnail(&outgoing.path).unwrap_or_default();
            if !thumb.is_empty() {
                if track_progress {
                    let (completed, total) = counters();
                    event.on_sync_progress(SyncProgress {
                        device_id: "peer".to_string(),
                        status: format!("Sending {filename}"),
                        phase: SyncPhase::Syncing,
                        progress: 0.0,
                        bytes_per_second: 0,
                        items_completed: completed,
                        items_total: total,
                        filename: Some(filename.clone()),
                        thumbnail: Some(thumb.clone()),
                    });
                }
                let db = Database::new(&event.get_config_path());
                db.update_photo_thumbnail(&photo_id, &thumb);
            }
            thumb
        } else {
            outgoing.encoded.clone()
        };

        let checksum = Self::compute_file_checksum(path).await?;

        let (header, trimmed) = Self::fit_file_header(
            photo_id.clone(),
            filename.clone(),
            outgoing.relative_path.clone(),
            size,
            checksum.clone(),
            outgoing.created.clone(),
            outgoing.latitude,
            outgoing.longitude,
            outgoing.objects.clone(),
            outgoing.faces.clone(),
            outgoing.caption.clone(),
            outgoing.aesthetics_score,
            encoded.clone(),
        );
        if trimmed {
            event.on_log(&format!(
                "WARN trimmed oversized FileHeader for {filename} (face crops/thumbnail dropped)"
            ));
        }
        Self::send_sync_message(dc, &header).await?;

        let mut buffer = vec![0u8; FILE_CHUNK_PAYLOAD];
        let mut total_sent = 0u64;
        let mut chunk_index = 0u32;
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            Self::send_sync_message(
                dc,
                &SyncMessage::FileChunk {
                    id: photo_id.clone(),
                    index: chunk_index,
                    data: buffer[..n].to_vec(),
                },
            )
            .await?;

            chunk_index += 1;
            total_sent += n as u64;

            // `progress` carries the overall batch percentage so every UI
            // surface moves monotonically; per-file byte detail lives only in
            // the status text.
            if track_progress {
                let (completed, total) = counters();
                let batch_progress = if total > 0 {
                    (completed as f32 / total as f32) * 100.0
                } else {
                    (total_sent as f32 / size as f32) * 100.0
                };
                event.on_sync_progress(SyncProgress {
                    device_id: "peer".to_string(),
                    status: format!("Sending {filename}"),
                    phase: SyncPhase::Syncing,
                    progress: batch_progress,
                    bytes_per_second: 0,
                    items_completed: completed,
                    items_total: total,
                    filename: None,
                    thumbnail: None,
                });
            }

            let buffer_wait_start = std::time::Instant::now();
            while dc.buffered_amount().await > MAX_BUFFERED_BYTES {
                if buffer_wait_start.elapsed() > BACKPRESSURE_TIMEOUT {
                    return Err(format!(
                        "Peer stopped reading data channel after {}",
                        BACKPRESSURE_TIMEOUT.as_secs_f32()
                    )
                    .into());
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }

        if let Err(e) = Self::send_sync_message(
            dc,
            &SyncMessage::FileEnd {
                id: photo_id.clone(),
                checksum: checksum.clone(),
            },
        )
        .await
        {
            event.on_log(&format!("Error sending FileEnd: {e}"));
            return Err(e);
        }

        if track_progress {
            event.on_sync_progress(SyncProgress {
                device_id: "peer".to_string(),
                status: format!("Finished sending {filename}"),
                phase: SyncPhase::Syncing,
                progress: {
                    let (completed, total) = counters();
                    if total > 0 {
                        (completed as f32 / total as f32) * 100.0
                    } else {
                        100.0
                    }
                },
                bytes_per_second: 0,
                items_completed: counters().0,
                items_total: counters().1,
                filename: Some(filename.clone()),
                thumbnail: Some(encoded.clone()),
            });
        }

        event.on_log(&format!("File {filename} sent over"));

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn handle_sync_message(
        msg: SyncMessage,
        dc: &Arc<webrtc::data_channel::RTCDataChannel>,
        incoming_files: &Arc<Mutex<std::collections::HashMap<String, IncomingFile>>>,
        pending_manifest: &Arc<tokio::sync::Mutex<Vec<PhotoSyncInfo>>>,
        transfer_semaphore: &Arc<tokio::sync::Semaphore>,
        config_path: &str,
        event: Arc<dyn SyncEvent>,
        items_completed: &Arc<std::sync::atomic::AtomicUsize>,
        items_total: &Arc<std::sync::atomic::AtomicUsize>,
        mirror_completed: &Arc<std::sync::atomic::AtomicUsize>,
        mirror_total: &Arc<std::sync::atomic::AtomicUsize>,
        pending_view_manifest: &Arc<tokio::sync::Mutex<Vec<PhotoSyncInfo>>>,
    ) {
        match msg {
            SyncMessage::ManifestRequest => {
                event.on_log("DEBUG handle ManifestRequest");
                let db = Database::new(config_path);
                // Announce our full library size so the peer's linked-devices
                // page can show real totals for this device.
                let (photo_count, video_count) = db.get_media_counts();
                let _ = Self::send_sync_message(
                    dc,
                    &SyncMessage::PeerLibraryStats {
                        photo_count,
                        video_count,
                    },
                )
                .await;
                let photos = db.get_photo_sync_info();
                if let Err(e) = Self::send_manifest_response(dc, photos).await {
                    event.on_log(&format!("ERROR sending manifest response: {e}"));
                }
                event.on_log("DEBUG sent ManifestResponse (chunked)");
            }
            SyncMessage::ManifestResponse { photos, more } => {
                if view_state()
                    .serving_view_only
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    // A view-only guest feeding us a manifest would turn this
                    // side into a puller that writes files. Never.
                    event.on_log("DEBUG ignoring ManifestResponse from view-only peer");
                    return;
                }
                let chunk_len = photos.len();
                let mut pending = pending_manifest.lock().await;
                pending.extend(photos);
                event.on_log(&format!(
                    "DEBUG handle ManifestResponse chunk with {} photos (more={more}, total={})",
                    chunk_len,
                    pending.len()
                ));
                if more {
                    return;
                }
                let photos = std::mem::take(&mut *pending);
                drop(pending);
                event.on_log(&format!(
                    "DEBUG manifest complete, comparing {} photos",
                    photos.len()
                ));
                let db = Database::new(config_path);
                let my_manifest = db.get_photo_sync_info();
                let mut to_request = Vec::new();
                for peer_photo in &photos {
                    if !my_manifest.iter().any(|p| p.id == peer_photo.id) {
                        to_request.push(peer_photo.id.clone());
                    }
                }

                if !to_request.is_empty() {
                    let total = to_request.len();
                    items_total.store(total, std::sync::atomic::Ordering::SeqCst);
                    items_completed.store(0, std::sync::atomic::Ordering::SeqCst);

                    // Numeric fields carry the combined k/N (local batch plus
                    // whatever the peer's batch already reports) so the UI
                    // never regresses while both directions run concurrently.
                    let combined_completed = items_completed
                        .load(std::sync::atomic::Ordering::SeqCst)
                        + mirror_completed.load(std::sync::atomic::Ordering::SeqCst);
                    let combined_total =
                        total + mirror_total.load(std::sync::atomic::Ordering::SeqCst);

                    event.on_sync_progress(SyncProgress {
                        device_id: "peer".to_string(),
                        status: format!("Syncing {total} new files"),
                        phase: SyncPhase::Syncing,
                        progress: 0.0,
                        bytes_per_second: 0,
                        items_completed: combined_completed,
                        items_total: combined_total,
                        filename: None,
                        thumbnail: None,
                    });

                    let _ = Self::send_sync_message(
                        dc,
                        &SyncMessage::PeerProgress {
                            status: format!("Peer needs {total} files"),
                            phase: SyncPhase::Syncing,
                            progress: 0.0,
                            items_completed: 0,
                            items_total: total,
                        },
                    )
                    .await;

                    for id in to_request {
                        let _ = Self::send_sync_message(dc, &SyncMessage::FileRequest { id }).await;
                    }
                } else {
                    // Only declare "Up to date" when the peer's own pull batch
                    // (mirrored here) is not still in flight.
                    let mirror_done = {
                        let (mc, mt) = (
                            mirror_completed.load(std::sync::atomic::Ordering::SeqCst),
                            mirror_total.load(std::sync::atomic::Ordering::SeqCst),
                        );
                        mt == 0 || mc >= mt
                    };
                    if mirror_done {
                        event.on_sync_progress(SyncProgress {
                            device_id: "peer".to_string(),
                            status: "Up to date".to_string(),
                            phase: SyncPhase::Completed,
                            progress: 100.0,
                            bytes_per_second: 0,
                            items_completed: mirror_completed
                                .load(std::sync::atomic::Ordering::SeqCst),
                            items_total: mirror_total.load(std::sync::atomic::Ordering::SeqCst),
                            filename: None,
                            thumbnail: None,
                        });
                    }
                }
            }
            SyncMessage::FileRequest { id } => {
                let db = Database::new(config_path);
                match db.connection.query_row(
                    "SELECT p.location, p.created, p.latitude, p.longitude,
                     (SELECT json_group_array(json_object('class', class, 'probability', probability)) FROM object WHERE photo_id = p.id),
                     (SELECT json_group_array(json_object('face_id', face_id, 'crop_path', crop_path, 'encoded', encoded, 'person_id', person_id)) FROM faces WHERE photo_id = p.id),
                     p.caption, p.aesthetics_score, p.encoded
                     FROM photo p WHERE p.id = ?1",
                    [&id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<f64>>(2)?, row.get::<_, Option<f64>>(3)?, row.get::<_, String>(4).unwrap_or("[]".to_string()), row.get::<_, String>(5).unwrap_or("[]".to_string()), row.get::<_, Option<String>>(6)?, row.get::<_, Option<f64>>(7)?, row.get::<_, String>(8).unwrap_or_default())),
                ) {
                    Ok((path, created, lat, lon, objects, faces, caption, aesthetics_score, encoded)) => {
                        let dirs = event.get_directories();
                        let relative_path = Self::compute_relative_path(&path, &dirs);
                        let dc_send = Arc::clone(dc);
                        let event_arc = Arc::clone(&event);
                        let config_path_clone = config_path.to_string();
                        let id_clone = id.clone();
                        let semaphore = Arc::clone(&transfer_semaphore);
                        let completed_task = Arc::clone(items_completed);
                        let total_task = Arc::clone(items_total);
                        let mirror_completed_task = Arc::clone(mirror_completed);
                        let mirror_total_task = Arc::clone(mirror_total);
                        tokio::spawn(async move {
                            let _permit = match semaphore.acquire_owned().await {
                                Ok(p) => {
                                    // Combined k/N: this side serves the
                                    // peer's pull batch, whose canonical
                                    // counters live in the mirror pair.
                                    let (completed, total) = (
                                        completed_task
                                            .load(std::sync::atomic::Ordering::SeqCst)
                                            + mirror_completed_task
                                                .load(std::sync::atomic::Ordering::SeqCst),
                                        total_task.load(std::sync::atomic::Ordering::SeqCst)
                                            + mirror_total_task
                                                .load(std::sync::atomic::Ordering::SeqCst),
                                    );
                                    event_arc.on_sync_progress(SyncProgress {
                                        device_id: "peer".to_string(),
                                        status: "Preparing to send files".to_string(),
                                        phase: SyncPhase::Syncing,
                                        progress: if total > 0 {
                                            (completed as f32 / total as f32) * 100.0
                                        } else {
                                            0.0
                                        },
                                        bytes_per_second: 0,
                                        items_completed: completed,
                                        items_total: total,
                                        filename: None,
                                        thumbnail: None,
                                    });
                                    p
                                }
                                Err(e) => {
                                    event_arc.on_log(&format!(
                                        "DEBUG transfer semaphore closed, skipping {id_clone}: {e}"
                                    ));
                                    return;
                                }
                            };
                            let result = Self::send_file_with_retry(
                                dc_send,
                                OutgoingFile {
                                    id: id_clone.clone(),
                                    path,
                                    relative_path,
                                    created,
                                    latitude: lat,
                                    longitude: lon,
                                    objects,
                                    faces,
                                    caption,
                                    aesthetics_score,
                                    encoded,
                                },
                                event_arc,
                                &completed_task,
                                &total_task,
                                &mirror_completed_task,
                                &mirror_total_task,
                            )
                            .await;
                            if result.is_ok() {
                                let db = Database::new(&config_path_clone);
                                db.clear_sync_needed(&id_clone);
                                // No local counter bump here: the requesting
                                // side owns the canonical batch counters and
                                // mirrors them to us via PeerProgress, so both
                                // screens display the exact same k/N.
                            }
                        });
                    }
                    Err(e) => {
                        event.on_log(&format!("ERROR FileRequest {id}: photo lookup failed: {e}"));
                    }
                }
            }
            SyncMessage::FileHeader {
                id,
                filename,
                relative_path,
                size,
                checksum,
                created,
                latitude,
                longitude,
                objects,
                faces,
                caption,
                aesthetics_score,
                encoded,
            } => {
                if view_state()
                    .viewing
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    // View-only session: stage an in-memory buffer keyed by
                    // photo id; chunks append and FileEnd verifies+caches.
                    view_state().insert_partial(&id, checksum, sanitize_filename(&filename));
                    return;
                }
                if Self::check_storage_quota(config_path, size) {
                    event.on_sync_error(format!(
                        "Storage quota would be exceeded by {} ({} bytes). Set max_storage_mb to increase limit.",
                        filename, size
                    ));
                    return;
                }

                let sanitized = sanitize_filename(&filename);
                let temp_dir = sync_temp_dir(config_path);
                // Key the temp file by photo id (not basename): concurrent
                // transfers (semaphore-limited) can share a basename, and a
                // shared temp path would interleave chunks and fail checksums.
                let save_path = temp_dir.join(sanitize_filename(&id));
                if let Some(parent) = save_path.parent() {
                    if let Err(e) = tokio::fs::create_dir_all(parent).await {
                        event.on_log(&format!(
                            "ERROR could not create sync_temp dir {parent:?} for {filename}: {e}"
                        ));
                    }
                }
                match tokio::fs::File::create(&save_path).await {
                    Ok(file) => {
                        let mut incoming = incoming_files.lock().await;
                        incoming.insert(
                            id.clone(),
                            IncomingFile {
                                id,
                                filename: sanitized.clone(),
                                relative_path,
                                size,
                                received: 0,
                                checksum,
                                created,
                                latitude,
                                longitude,
                                objects,
                                faces,
                                caption,
                                aesthetics_score,
                                encoded: encoded.clone(),
                                file,
                            },
                        );
                        event.on_sync_progress(SyncProgress {
                            device_id: "peer".to_string(),
                            status: format!("Receiving {sanitized}"),
                            phase: SyncPhase::Syncing,
                            progress: 0.0,
                            bytes_per_second: 0,
                            items_completed: items_completed
                                .load(std::sync::atomic::Ordering::SeqCst)
                                + mirror_completed.load(std::sync::atomic::Ordering::SeqCst),
                            items_total: items_total.load(std::sync::atomic::Ordering::SeqCst)
                                + mirror_total.load(std::sync::atomic::Ordering::SeqCst),
                            filename: Some(sanitized.clone()),
                            thumbnail: Some(encoded),
                        });
                    }
                    Err(e) => {
                        event.on_log(&format!(
                            "ERROR could not create temp file for {id} ({filename}) at {save_path:?}: {e}"
                        ));
                    }
                }
            }
            SyncMessage::FileChunk { id, index: _, data } => {
                if view_state()
                    .viewing
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    // View-only session: buffer in memory, never touch disk.
                    view_state().append_chunk(&id, &data);
                    return;
                }
                let mut incoming = incoming_files.lock().await;
                if let Some(file_state) = incoming.get_mut(&id) {
                    if let Err(e) = file_state.file.write_all(&data).await {
                        event.on_log(&format!("ERROR write to temp file failed for {id}: {e}"));
                    }
                    file_state.received += data.len() as u64;

                    // Overall batch percentage keeps the UI monotonic; the
                    // per-file byte fraction is not useful across files.
                    let completed = items_completed.load(std::sync::atomic::Ordering::SeqCst);
                    let total = items_total.load(std::sync::atomic::Ordering::SeqCst);
                    let progress = if total > 0 {
                        (completed as f32 / total as f32) * 100.0
                    } else {
                        (file_state.received as f32 / file_state.size as f32) * 100.0
                    };
                    event.on_sync_progress(SyncProgress {
                        device_id: "peer".to_string(),
                        status: format!("Receiving {}", file_state.filename),
                        phase: SyncPhase::Syncing,
                        progress,
                        bytes_per_second: 0,
                        items_completed: completed,
                        items_total: total,
                        filename: None,
                        thumbnail: None,
                    });
                }
            }
            SyncMessage::FileEnd { id, checksum } => {
                if view_state()
                    .viewing
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    // Verify and cache in memory; nothing is persisted.
                    let ok = view_state().complete(&id, |bytes| Self::compute_data_checksum(bytes));
                    if !ok {
                        event.on_sync_error(format!("Checksum mismatch for view-only media {id}"));
                    }
                    return;
                }
                let mut incoming = incoming_files.lock().await;
                if let Some(mut file_state) = incoming.remove(&id) {
                    let _ = file_state.file.flush().await;
                    drop(file_state.file);

                    let temp_path =
                        sync_temp_dir(config_path).join(sanitize_filename(&file_state.id));

                    let received_checksum = match Self::compute_file_checksum(&temp_path).await {
                        Ok(cs) => cs,
                        Err(_) => String::new(),
                    };

                    if !checksum.is_empty() && received_checksum != checksum {
                        event.on_sync_error(format!(
                            "Checksum mismatch for {}: expected {}, got {}",
                            file_state.filename, checksum, received_checksum
                        ));
                        let _ = tokio::fs::remove_file(&temp_path).await;
                        return;
                    }

                    let db = Database::new(config_path);
                    let state = db.get_state();
                    let sync_path_str = state.get("sync_path");
                    let dirs = db.list_directories();

                    let target_dir = resolve_sync_target_dir(
                        config_path,
                        sync_path_str.map(|s| s.as_str()),
                        &dirs,
                    );

                    let final_path = crate::sync_transport::resolve_receive_target(
                        &target_dir,
                        &file_state.relative_path,
                        &file_state.filename,
                    );
                    if let Some(parent) = final_path.parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }

                    if let Err(e) = tokio::fs::rename(&temp_path, &final_path).await {
                        event.on_sync_error(format!(
                            "Failed to move file to {final_path:?}. Error: {e}"
                        ));
                    } else {
                        // Count into the LOCAL pair: this side pulled the
                        // file. Display the combined k/N so concurrent
                        // opposite-direction transfers stay coherent.
                        let completed =
                            items_completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                        let total = items_total.load(std::sync::atomic::Ordering::SeqCst);
                        let (display_completed, display_total) = (
                            completed + mirror_completed.load(std::sync::atomic::Ordering::SeqCst),
                            total + mirror_total.load(std::sync::atomic::Ordering::SeqCst),
                        );

                        let path_str = final_path.to_string_lossy().to_string();
                        let id_clone = file_state.id.clone();
                        let created_clone = file_state.created.clone();
                        let lat_clone = file_state.latitude.unwrap_or(0.0);
                        let lon_clone = file_state.longitude.unwrap_or(0.0);
                        let objects_clone = file_state.objects.clone();
                        let faces_clone = file_state.faces.clone();
                        let config_clone = config_path.to_string();

                        let id_for_event = id_clone.clone();
                        let path_for_event = path_str.clone();

                        let caption_clone = file_state.caption.clone();
                        let aesthetics_clone = file_state.aesthetics_score;
                        let encoded_clone = file_state.encoded.clone();
                        tokio::task::spawn_blocking(move || {
                            let thumb = if encoded_clone.is_empty() {
                                crate::thumbnail::generate_thumbnail(&path_str).unwrap_or_default()
                            } else {
                                encoded_clone
                            };
                            let mut db = Database::new(&config_clone);
                            db.import_photo(ImportedPhoto {
                                id: &id_clone,
                                location: &path_str,
                                created: &created_clone,
                                latitude: Some(lat_clone),
                                longitude: Some(lon_clone),
                                objects_json: &objects_clone,
                                faces_json: &faces_clone,
                                encoded: &thumb,
                                caption: caption_clone.as_deref(),
                                aesthetics_score: aesthetics_clone,
                                received: true,
                            });
                            db.clear_sync_needed(&id_clone);
                        });

                        event.on_photo_received(id_for_event, path_for_event);

                        let status = format!("Received {display_completed}/{display_total}");
                        let progress = if display_total > 0 {
                            (display_completed as f32 / display_total as f32) * 100.0
                        } else {
                            100.0
                        };

                        event.on_sync_progress(SyncProgress {
                            device_id: "peer".to_string(),
                            status,
                            phase: SyncPhase::Syncing,
                            progress,
                            bytes_per_second: 0,
                            items_completed: display_completed,
                            items_total: display_total,
                            filename: None,
                            thumbnail: None,
                        });

                        if display_total > 0 && display_completed >= display_total {
                            event.on_sync_progress(SyncProgress {
                                device_id: "peer".to_string(),
                                status: "All files synced".to_string(),
                                phase: SyncPhase::Completed,
                                progress: 100.0,
                                bytes_per_second: 0,
                                items_completed: display_completed,
                                items_total: display_total,
                                filename: None,
                                thumbnail: None,
                            });
                        }

                        // The PeerProgress payload carries only THIS side's
                        // local pull batch; the peer stores it into its own
                        // mirror pair and adds it to whatever it counts
                        // locally.
                        let _ = Self::send_sync_message(
                            dc,
                            &SyncMessage::PeerProgress {
                                status: format!("Peer received {completed}/{total}"),
                                phase: SyncPhase::Syncing,
                                progress: if total > 0 {
                                    (completed as f32 / total as f32) * 100.0
                                } else {
                                    100.0
                                },
                                items_completed: completed,
                                items_total: total,
                            },
                        )
                        .await;
                    }
                } else {
                    event.on_log(&format!(
                        "ERROR FileEnd for {id}: no matching incoming file (FileHeader was never received or temp file creation failed)"
                    ));
                }
            }
            SyncMessage::SyncFile { photo } => {
                let _ =
                    Self::send_sync_message(dc, &SyncMessage::FileRequest { id: photo.id }).await;
            }
            SyncMessage::StartSync => {
                if view_state()
                    .serving_view_only
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    // A view-only guest cannot trigger a pull that would make
                    // us write anything to our library.
                    event.on_log("DEBUG ignoring StartSync from view-only peer");
                    return;
                }
                event.on_log("DEBUG handle StartSync");
                event.on_state_change("Sync started");
                let _ = Self::send_sync_message(dc, &SyncMessage::ManifestRequest).await;
            }
            SyncMessage::CatchUp => {
                // No-op. Both peers already send `ManifestRequest` immediately
                // before `CatchUp` when the data channel opens, and that
                // comparison pulls anything this side is missing. Re-triggering
                // the comparison here duplicated requests (files transferred
                // twice), and the earlier "request my local `sync_needed`
                // photos" implementation flooded the joiner with FileRequests
                // for files it can't have.
                event.on_log("DEBUG handle CatchUp (no-op; manifest compare already runs)");
            }
            SyncMessage::PeerProgress {
                status,
                phase,
                progress: _,
                items_completed: peer_completed,
                items_total: peer_total,
            } => {
                // The requesting side owns its batch ("Peer needs N files",
                // then "Peer received k/N" per file). Store into the MIRROR
                // pair — never the local pair, which counts this side's own
                // concurrent pull. UI emissions report both pairs summed so
                // opposite-direction batches compose into one k/N.
                mirror_completed.store(peer_completed, std::sync::atomic::Ordering::SeqCst);
                mirror_total.store(peer_total, std::sync::atomic::Ordering::SeqCst);
                let combined_completed =
                    peer_completed + items_completed.load(std::sync::atomic::Ordering::SeqCst);
                let combined_total =
                    peer_total + items_total.load(std::sync::atomic::Ordering::SeqCst);
                event.on_sync_progress(SyncProgress {
                    device_id: "peer".to_string(),
                    status,
                    phase,
                    progress: if combined_total > 0 {
                        (combined_completed as f32 / combined_total as f32) * 100.0
                    } else {
                        100.0
                    },
                    bytes_per_second: 0,
                    items_completed: combined_completed,
                    items_total: combined_total,
                    filename: None,
                    thumbnail: None,
                });
                if combined_total > 0 && combined_completed >= combined_total {
                    event.on_sync_progress(SyncProgress {
                        device_id: "peer".to_string(),
                        status: "All files synced".to_string(),
                        phase: SyncPhase::Completed,
                        progress: 100.0,
                        bytes_per_second: 0,
                        items_completed: combined_completed,
                        items_total: combined_total,
                        filename: None,
                        thumbnail: None,
                    });
                }
            }
            SyncMessage::PeerLibraryStats {
                photo_count,
                video_count,
            } => {
                event.on_log(&format!(
                    "Peer library: {photo_count} photos, {video_count} videos"
                ));
                event.on_peer_library_stats(photo_count, video_count);
            }
            SyncMessage::MetadataUpdate {
                photo_id,
                caption,
                aesthetics_score,
                indexed,
                deleted_at,
            } => {
                if view_state()
                    .serving_view_only
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    event.on_log("DEBUG ignoring MetadataUpdate from view-only peer");
                    return;
                }
                let db = Database::new(config_path);
                if let Some(ref dt) = deleted_at {
                    if !dt.is_empty() {
                        let _ = db.trash_photo(&photo_id);
                        event.on_log(&format!("Photo {photo_id} trashed by peer"));
                    }
                } else {
                    db.update_photo_metadata(
                        &photo_id,
                        caption.as_deref(),
                        aesthetics_score,
                        indexed,
                    );
                    event.on_log(&format!("Metadata updated for {photo_id}"));
                }
            }
            SyncMessage::VersionNegotiate {
                version,
                device_id,
                device_name,
                os,
                models_enabled,
            } => {
                if version != PROTOCOL_VERSION {
                    let _ = Self::send_sync_message(
                        dc,
                        &SyncMessage::VersionReject {
                            reason: format!(
                                "Protocol version mismatch: peer={}, local={}",
                                version, PROTOCOL_VERSION
                            ),
                        },
                    )
                    .await;
                    return;
                }
                event.on_log(&format!(
                    "Peer connected with {} models: {:?}",
                    models_enabled.len(),
                    models_enabled
                ));
                event.on_peer_connected(device_id, device_name, os, models_enabled, version);
            }
            SyncMessage::VersionReject { reason } => {
                event.on_sync_error(format!("Peer rejected connection: {reason}"));
            }
            SyncMessage::EnterViewOnly => {
                // Serve our manifest as read-only metadata. Nothing on this
                // device may mutate from here on until the session ends.
                view_state()
                    .serving_view_only
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                event.on_log("DEBUG peer entered view-only mode; serving manifest");
                let db = Database::new(config_path);
                let photos = db.get_photo_sync_info();
                if let Err(e) = Self::send_manifest_view_only(dc, photos).await {
                    event.on_log(&format!("ERROR sending view-only manifest: {e}"));
                }
            }
            SyncMessage::ViewOnlyManifest { photos, more } => {
                let mut pending = pending_view_manifest.lock().await;
                pending.extend(photos);
                if more {
                    return;
                }
                let photos = std::mem::take(&mut *pending);
                drop(pending);
                event.on_log(&format!(
                    "DEBUG view-only manifest complete ({} items)",
                    photos.len()
                ));
                event.on_view_manifest(&photos);
            }
            SyncMessage::FetchMediaRequest { id, thumbnail } => {
                if thumbnail {
                    // Small direct reply: DB-cached thumbnail or generate.
                    let db = Database::new(config_path);
                    let bytes = match db.get_photo_thumbnail_bytes(&id) {
                        Some(b) => Some(b),
                        None => db
                            .get_photo_location(&id)
                            .and_then(|loc| crate::thumbnail::generate_thumbnail_bytes(&loc)),
                    };
                    match bytes {
                        Some(data) if data.len() <= MAX_DATA_CHANNEL_MSG_SIZE / 2 => {
                            let _ = Self::send_sync_message(
                                dc,
                                &SyncMessage::ViewMedia {
                                    id,
                                    mime: "image/jpeg".to_string(),
                                    data,
                                },
                            )
                            .await;
                        }
                        _ => event.on_log(&format!(
                            "WARN no thumbnail available for view-only request {id}"
                        )),
                    }
                    return;
                }

                // Original: stream via the normal header/chunk/end protocol.
                let db = Database::new(config_path);
                let Ok((path, created, lat, lon, objects, faces, caption, aesthetics_score, encoded)) =
                    db.connection.query_row(
                        "SELECT p.location, p.created, p.latitude, p.longitude,
                         (SELECT json_group_array(json_object('class', class, 'probability', probability)) FROM object WHERE photo_id = p.id),
                         (SELECT json_group_array(json_object('face_id', face_id, 'crop_path', crop_path, 'encoded', encoded, 'person_id', person_id)) FROM faces WHERE photo_id = p.id),
                         p.caption, p.aesthetics_score, p.encoded
                         FROM photo p WHERE p.id = ?1",
                        [&id],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<f64>>(2)?, row.get::<_, Option<f64>>(3)?, row.get::<_, String>(4).unwrap_or("[]".to_string()), row.get::<_, String>(5).unwrap_or("[]".to_string()), row.get::<_, Option<String>>(6)?, row.get::<_, Option<f64>>(7)?, row.get::<_, String>(8).unwrap_or_default())),
                    )
                else {
                    event.on_log(&format!("ERROR FetchMediaRequest {id}: photo not found"));
                    return;
                };
                let relative_path = Self::compute_relative_path(&path, &event.get_directories());
                let outgoing = OutgoingFile {
                    id,
                    path,
                    relative_path,
                    created,
                    latitude: lat,
                    longitude: lon,
                    objects,
                    faces,
                    caption,
                    aesthetics_score,
                    encoded,
                };
                // Untracked: no counters, no batch progress — this is an
                // on-demand read for a viewer, not a sync transfer.
                if let Err(e) = Self::send_file_inner(
                    dc,
                    &outgoing,
                    event.as_ref(),
                    items_completed,
                    items_total,
                    mirror_completed,
                    mirror_total,
                    false,
                )
                .await
                {
                    event.on_log(&format!("ERROR serving view-only media: {e}"));
                }
            }
            SyncMessage::ViewMedia { id, mime, data } => {
                view_state().insert_completed(
                    format!("thumb:{id}"),
                    crate::view_only::RemoteMedia {
                        bytes: Arc::new(data),
                        mime,
                    },
                );
            }
        }
    }

    pub async fn start_lan_server(
        signaling_port: u16,
    ) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
        let port = if signaling_port > 0 {
            signaling_port
        } else {
            crate::lan_server::start(0).await.port
        };
        Ok(port)
    }

    pub async fn cleanup_temp_files(config_path: &str) {
        cleanup_sync_temp(config_path, 3600).await;
    }
}

impl MeshManager {
    /// Check if adding `additional_bytes` would exceed the configured storage quota.
    /// Returns true if quota would be exceeded.
    pub fn check_storage_quota(config_path: &str, additional_bytes: u64) -> bool {
        let db = Database::new(config_path);
        let state = db.get_state();
        let max_mb = state
            .get("max_storage_mb")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10240);
        let max_bytes = max_mb * 1024 * 1024;

        let used = Self::get_storage_used(config_path);
        used + additional_bytes > max_bytes
    }

    /// Get the configured storage quota in bytes.
    pub fn get_storage_quota(config_path: &str) -> u64 {
        let db = Database::new(config_path);
        let state = db.get_state();
        let max_mb = state
            .get("max_storage_mb")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10240);
        max_mb * 1024 * 1024
    }

    /// Get the actual storage used in bytes by walking the sync directory and sync_temp.
    pub fn get_storage_used(config_path: &str) -> u64 {
        let db = Database::new(config_path);
        let state = db.get_state();
        let sync_path = state.get("sync_path");
        let dirs = db.list_directories();

        let target_dir = resolve_sync_target_dir(config_path, sync_path.map(|s| s.as_str()), &dirs);

        let mut total = 0u64;
        let temp_dir = sync_temp_dir(config_path);
        total += Self::dir_size(&temp_dir);
        total += Self::dir_size(&target_dir);
        total
    }

    fn dir_size(path: &Path) -> u64 {
        let mut total = 0u64;
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        total += Self::dir_size(&entry.path());
                    } else {
                        total += meta.len();
                    }
                }
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version() {
        assert_eq!(PROTOCOL_VERSION, 2);
    }

    #[test]
    fn test_split_manifest_chunks_fit_data_channel_limit() {
        let photos: Vec<PhotoSyncInfo> = (0..1000)
            .map(|i| PhotoSyncInfo {
                id: format!("photo_{i}"),
                location: format!("/home/user/pictures/{}/image_{i}.jpg", i % 10),
                created: "2024-01-01".to_string(),
                latitude: Some(52.0),
                longitude: Some(4.9),
                objects: "[]".to_string(),
                faces: "[]".to_string(),
                caption: None,
                aesthetics_score: None,
            })
            .collect();

        // Sanity: the full manifest serialized in one message would be rejected.
        let single = serde_json::to_string(&SyncMessage::ManifestResponse {
            photos: photos.clone(),
            more: false,
        })
        .unwrap();
        assert!(
            single.len() > 65536,
            "sanity: full manifest must exceed the SCTP max message size"
        );

        let chunks = MeshManager::split_manifest_chunks(photos.clone());
        assert!(
            chunks.len() >= 2,
            "expected multiple chunks, got {}",
            chunks.len()
        );

        let mut all: Vec<PhotoSyncInfo> = Vec::new();
        for (i, (chunk, more)) in chunks.iter().enumerate() {
            let msg = SyncMessage::ManifestResponse {
                photos: chunk.clone(),
                more: *more,
            };
            let json = serde_json::to_string(&msg).unwrap();
            assert!(
                json.len() < 65536,
                "chunk {} must fit in the data channel max message size, got {} bytes",
                i,
                json.len()
            );
            assert_eq!(
                *more,
                i < chunks.len() - 1,
                "only the last chunk ends the manifest"
            );
            assert!(!chunk.is_empty(), "no empty chunks");
            all.extend(chunk.iter().cloned());
        }

        assert_eq!(all.len(), photos.len());
        assert_eq!(all[0].id, "photo_0");
        assert_eq!(all[all.len() - 1].id, "photo_999");
    }

    #[test]
    fn test_split_manifest_chunks_single_chunk_when_small() {
        let photos: Vec<PhotoSyncInfo> = (0..3)
            .map(|i| PhotoSyncInfo {
                id: format!("photo_{i}"),
                location: format!("/tmp/photo_{i}.jpg"),
                created: "2024-01-01".to_string(),
                latitude: None,
                longitude: None,
                objects: "[]".to_string(),
                faces: "[]".to_string(),
                caption: None,
                aesthetics_score: None,
            })
            .collect();

        let chunks = MeshManager::split_manifest_chunks(photos);
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].1, "single chunk must be the final chunk");
    }

    #[test]
    fn test_strip_face_crops_keeps_ids_drops_encoded() {
        let faces =
            r#"[{"face_id":"f1","crop_path":"crop.jpg","encoded":"AAAA","person_id":"p1"}]"#;
        let stripped = MeshManager::strip_face_crops(faces);
        assert!(stripped.contains("f1"), "face id must survive: {stripped}");
        assert!(
            stripped.contains("crop.jpg"),
            "crop path must survive: {stripped}"
        );
        assert!(
            !stripped.contains("AAAA"),
            "encoded crop must be dropped: {stripped}"
        );
    }

    #[test]
    fn test_fit_file_header_trims_oversized_face_crops() {
        // A single face crop can exceed the data channel message limit; the
        // header must be trimmed so the receiver never errors and closes the
        // channel (which would abort the whole sync session).
        let big = "data:image/jpeg;base64,".to_string() + &"A".repeat(120_000);
        let faces = format!(
            r#"[{{"face_id":"f1","crop_path":"crop.jpg","encoded":"{big}","person_id":"p1"}}]"#
        );

        let (msg, trimmed) = MeshManager::fit_file_header(
            "id-1".into(),
            "photo.jpg".into(),
            String::new(),
            100,
            "checksum".into(),
            "2024-01-01".into(),
            None,
            None,
            "[]".into(),
            faces,
            None,
            None,
            String::new(),
        );

        assert!(trimmed, "oversized header must be flagged as trimmed");
        match &msg {
            SyncMessage::FileHeader { faces, .. } => {
                assert!(!faces.contains("AAAA"), "face crops must be trimmed");
            }
            _ => panic!("expected FileHeader"),
        }
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            json.len() < MAX_DATA_CHANNEL_MSG_SIZE,
            "trimmed header must fit in the channel, got {} bytes",
            json.len()
        );
    }

    #[test]
    fn test_fit_file_header_keeps_small_headers_untouched() {
        let (msg, trimmed) = MeshManager::fit_file_header(
            "id-1".into(),
            "photo.jpg".into(),
            String::new(),
            100,
            "checksum".into(),
            "2024-01-01".into(),
            None,
            None,
            "[]".into(),
            "[]".into(),
            None,
            None,
            String::new(),
        );
        assert!(!trimmed, "small header must not be trimmed");
        match &msg {
            SyncMessage::FileHeader { faces, encoded, .. } => {
                assert_eq!(faces, "[]");
                assert_eq!(encoded, "");
            }
            _ => panic!("expected FileHeader"),
        }
    }

    #[test]
    fn test_max_mesh_devices() {
        assert_eq!(MAX_MESH_DEVICES, 5);
    }

    #[test]
    fn test_file_chunk_fits_data_channel_limit() {
        let data = vec![255u8; FILE_CHUNK_PAYLOAD];
        let msg = SyncMessage::FileChunk {
            id: "photo_0".to_string(),
            index: 42,
            data,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            json.len() < 65536,
            "FileChunk must fit in the SCTP max message size, got {} bytes",
            json.len()
        );

        let parsed: SyncMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            SyncMessage::FileChunk { id, index, data } => {
                assert_eq!(id, "photo_0");
                assert_eq!(index, 42);
                assert_eq!(data.len(), FILE_CHUNK_PAYLOAD);
                assert_eq!(data[0], 255);
            }
            other => panic!("unexpected message variant: {:?}", other),
        }
    }

    #[test]
    fn test_compute_relative_path() {
        let dirs = vec!["/home/user/pictures".to_string()];
        let result =
            MeshManager::compute_relative_path("/home/user/pictures/2024/photo.jpg", &dirs);
        assert_eq!(result, "2024/photo.jpg");
    }

    #[test]
    fn test_compute_relative_path_no_match() {
        let dirs = vec!["/other/dir".to_string()];
        let result = MeshManager::compute_relative_path("/home/user/photo.jpg", &dirs);
        assert_eq!(result, "photo.jpg");
    }

    #[test]
    fn test_compute_data_checksum() {
        let data = b"hello world";
        let checksum = MeshManager::compute_data_checksum(data);
        assert!(!checksum.is_empty());
        assert_eq!(checksum.len(), 64);
    }

    #[test]
    fn test_sync_message_roundtrip() {
        let msg = SyncMessage::ManifestRequest;
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: SyncMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, SyncMessage::ManifestRequest));
    }

    #[test]
    fn test_sync_message_file_header() {
        let msg = SyncMessage::FileHeader {
            id: "test-id".to_string(),
            filename: "photo.jpg".to_string(),
            relative_path: "2024/photo.jpg".to_string(),
            size: 1024,
            checksum: "abc123".to_string(),
            created: "2024-01-01".to_string(),
            latitude: Some(40.7128),
            longitude: Some(-74.0060),
            objects: "[]".to_string(),
            faces: "[]".to_string(),
            caption: None,
            aesthetics_score: None,
            encoded: String::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: SyncMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            SyncMessage::FileHeader {
                id,
                filename,
                relative_path,
                size,
                checksum,
                ..
            } => {
                assert_eq!(id, "test-id");
                assert_eq!(filename, "photo.jpg");
                assert_eq!(relative_path, "2024/photo.jpg");
                assert_eq!(size, 1024);
                assert_eq!(checksum, "abc123");
            }
            _ => panic!("Expected FileHeader"),
        }
    }

    #[test]
    fn test_sync_message_version_negotiate() {
        let msg = SyncMessage::VersionNegotiate {
            version: 1,
            device_id: "device-1".to_string(),
            device_name: "Phone".to_string(),
            os: "android".to_string(),
            models_enabled: vec!["caption".to_string(), "face".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: SyncMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            SyncMessage::VersionNegotiate {
                version,
                device_id,
                device_name,
                os,
                models_enabled,
            } => {
                assert_eq!(version, 1);
                assert_eq!(device_id, "device-1");
                assert_eq!(device_name, "Phone");
                assert_eq!(os, "android");
                assert_eq!(models_enabled.len(), 2);
            }
            _ => panic!("Expected VersionNegotiate"),
        }
    }

    #[test]
    fn test_sync_message_metadata_update() {
        let msg = SyncMessage::MetadataUpdate {
            photo_id: "photo-1".to_string(),
            caption: Some("A beautiful sunset".to_string()),
            aesthetics_score: Some(0.95),
            indexed: 2,
            deleted_at: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: SyncMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            SyncMessage::MetadataUpdate {
                photo_id,
                caption,
                aesthetics_score,
                indexed,
                deleted_at,
            } => {
                assert_eq!(photo_id, "photo-1");
                assert_eq!(caption, Some("A beautiful sunset".to_string()));
                assert_eq!(aesthetics_score, Some(0.95));
                assert_eq!(indexed, 2);
                assert_eq!(deleted_at, None);
            }
            _ => panic!("Expected MetadataUpdate"),
        }
    }

    #[test]
    fn test_sync_progress_struct() {
        let progress = SyncProgress {
            device_id: "peer".to_string(),
            status: "Syncing".to_string(),
            phase: SyncPhase::Syncing,
            progress: 50.0,
            bytes_per_second: 1024,
            items_completed: 5,
            items_total: 10,
            filename: None,
            thumbnail: None,
        };
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("50.0"));
        assert!(json.contains("Syncing"));
        assert!(json.contains("\"phase\":\"syncing\""));
        let parsed: SyncProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.filename, None);
        assert_eq!(parsed.thumbnail, None);
    }
}
