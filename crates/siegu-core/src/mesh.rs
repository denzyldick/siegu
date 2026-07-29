use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::database::{Database, ImportedPhoto, PhotoSyncInfo};
use crate::sync_transport::{
    cleanup_sync_temp, resolve_sync_target_dir, sanitize_filename, sync_temp_dir,
};

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_MESH_DEVICES: usize = 5;
pub const CHUNK_SIZE: usize = 65536;
pub const MAX_BUFFERED_BYTES: usize = 1_000_000;
pub const MAX_RETRY_ATTEMPTS: u32 = 3;
pub const RETRY_BACKOFF_MS: u64 = 500;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncProgress {
    pub device_id: String,
    pub status: String,
    pub progress: f32,
    pub bytes_per_second: u64,
    pub items_completed: usize,
    pub items_total: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncMessage {
    ManifestRequest,
    ManifestResponse {
        photos: Vec<PhotoSyncInfo>,
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
        progress: f32,
        items_completed: usize,
        items_total: usize,
    },
    MetadataUpdate {
        photo_id: String,
        caption: Option<String>,
        aesthetics_score: Option<f64>,
        indexed: i32,
    },
    VersionNegotiate {
        version: u8,
        device_id: String,
        device_name: String,
        models_enabled: Vec<String>,
    },
    VersionReject {
        reason: String,
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
        models_enabled: Vec<String>,
        protocol_version: u8,
    );
    fn on_peer_disconnected(&self, peer_id: String);
    fn on_device_registered(&self, db: &Database);
    fn on_metadata_updated(&self, photo_id: &str, caption: Option<&str>, aesthetics_score: Option<f64>);
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

    pub fn compute_file_checksum(
        path: &Path,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        use sha2::{Digest, Sha256};
        let bytes = std::fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
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

    pub async fn send_file_with_retry(
        dc: Arc<webrtc::data_channel::RTCDataChannel>,
        outgoing: OutgoingFile,
        event: Arc<dyn SyncEvent>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut last_err = None;
        for attempt in 0..MAX_RETRY_ATTEMPTS {
            match Self::send_file_inner(&dc, &outgoing, event.as_ref()).await {
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

    async fn send_file_inner(
        dc: &Arc<webrtc::data_channel::RTCDataChannel>,
        outgoing: &OutgoingFile,
        event: &dyn SyncEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let photo_id = outgoing.id.clone();
        let path = Path::new(&outgoing.path);
        if !path.exists() {
            return Err(format!("File not found: {}", outgoing.path).into());
        }

        let mut file = tokio::fs::File::open(path).await?;
        let metadata = file.metadata().await?;
        let size = metadata.len();
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let checksum = Self::compute_file_checksum(path)?;

        Self::send_sync_message(
            dc,
            &SyncMessage::FileHeader {
                id: photo_id.clone(),
                filename: filename.clone(),
                relative_path: outgoing.relative_path.clone(),
                size,
                checksum: checksum.clone(),
                created: outgoing.created.clone(),
                latitude: outgoing.latitude,
                longitude: outgoing.longitude,
                objects: outgoing.objects.clone(),
                faces: outgoing.faces.clone(),
                caption: outgoing.caption.clone(),
                aesthetics_score: outgoing.aesthetics_score,
            },
        )
        .await?;

        let mut buffer = vec![0u8; CHUNK_SIZE];
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

            let progress = (total_sent as f32 / size as f32) * 100.0;
            event.on_sync_progress(SyncProgress {
                device_id: "peer".to_string(),
                status: format!("Sending {filename}"),
                progress,
                bytes_per_second: 0,
                items_completed: 0,
                items_total: 0,
            });

            while dc.buffered_amount().await > MAX_BUFFERED_BYTES {
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

        event.on_sync_progress(SyncProgress {
            device_id: "peer".to_string(),
            status: format!("Finished sending {filename}"),
            progress: 100.0,
            bytes_per_second: 0,
            items_completed: 1,
            items_total: 1,
        });

        Ok(())
    }

    pub async fn handle_sync_message(
        msg: SyncMessage,
        dc: &Arc<webrtc::data_channel::RTCDataChannel>,
        incoming_files: &Arc<Mutex<std::collections::HashMap<String, IncomingFile>>>,
        config_path: &str,
        event: Arc<dyn SyncEvent>,
        items_completed: &Arc<std::sync::atomic::AtomicUsize>,
        items_total: &Arc<std::sync::atomic::AtomicUsize>,
    ) {
        match msg {
            SyncMessage::ManifestRequest => {
                let db = Database::new(config_path);
                let photos = db.get_photo_sync_info();
                let _ =
                    Self::send_sync_message(dc, &SyncMessage::ManifestResponse { photos }).await;
            }
            SyncMessage::ManifestResponse { photos } => {
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

                    event.on_sync_progress(SyncProgress {
                        device_id: "peer".to_string(),
                        status: format!("Syncing {total} new files"),
                        progress: 0.0,
                        bytes_per_second: 0,
                        items_completed: 0,
                        items_total: total,
                    });

                    let _ = Self::send_sync_message(
                        dc,
                        &SyncMessage::PeerProgress {
                            status: format!("Peer needs {total} files"),
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
                    event.on_sync_progress(SyncProgress {
                        device_id: "peer".to_string(),
                        status: "Up to date".to_string(),
                        progress: 100.0,
                        bytes_per_second: 0,
                        items_completed: 0,
                        items_total: 0,
                    });
                }
            }
            SyncMessage::FileRequest { id } => {
                let db = Database::new(config_path);
                if let Ok((path, created, lat, lon, objects, faces, caption, aesthetics_score)) = db.connection.query_row(
                    "SELECT p.location, p.created, p.latitude, p.longitude,
                     (SELECT json_group_array(json_object('class', class, 'probability', probability)) FROM object WHERE photo_id = p.id),
                     (SELECT json_group_array(json_object('face_id', face_id, 'crop_path', crop_path, 'encoded', encoded, 'person_id', person_id)) FROM faces WHERE photo_id = p.id),
                     p.caption, p.aesthetics_score
                     FROM photo p WHERE p.id = ?1",
                    [&id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<f64>>(2)?, row.get::<_, Option<f64>>(3)?, row.get::<_, String>(4).unwrap_or("[]".to_string()), row.get::<_, String>(5).unwrap_or("[]".to_string()), row.get::<_, Option<String>>(6)?, row.get::<_, Option<f64>>(7)?)),
                ) {
                    let dirs = event.get_directories();
                    let relative_path = Self::compute_relative_path(&path, &dirs);
                    let dc_send = Arc::clone(dc);
                    let event_arc = Arc::clone(&event);
                    let config_path_clone = config_path.to_string();
                    let id_clone = id.clone();
                    tokio::spawn(async move {
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
                            },
                            event_arc,
                        )
                        .await;
                        if result.is_ok() {
                            let db = Database::new(&config_path_clone);
                            db.clear_sync_needed(&id_clone);
                        }
                    });
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
            } => {
                if Self::check_storage_quota(config_path, size) {
                    event.on_sync_error(format!(
                        "Storage quota would be exceeded by {} ({} bytes). Set max_storage_mb to increase limit.",
                        filename, size
                    ));
                    return;
                }

                let sanitized = sanitize_filename(&filename);
                let temp_dir = sync_temp_dir(config_path);
                let save_path = temp_dir.join(&sanitized);
                if let Some(parent) = save_path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                if let Ok(file) = tokio::fs::File::create(&save_path).await {
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
                            file,
                        },
                    );
                }
            }
            SyncMessage::FileChunk { id, index: _, data } => {
                let mut incoming = incoming_files.lock().await;
                if let Some(file_state) = incoming.get_mut(&id) {
                    let _ = file_state.file.write_all(&data).await;
                    file_state.received += data.len() as u64;

                    let progress = (file_state.received as f32 / file_state.size as f32) * 100.0;
                    event.on_sync_progress(SyncProgress {
                        device_id: "peer".to_string(),
                        status: format!("Receiving {}", file_state.filename),
                        progress,
                        bytes_per_second: 0,
                        items_completed: 0,
                        items_total: 0,
                    });
                }
            }
            SyncMessage::FileEnd { id, checksum } => {
                let mut incoming = incoming_files.lock().await;
                if let Some(mut file_state) = incoming.remove(&id) {
                    let _ = file_state.file.flush().await;
                    drop(file_state.file);

                    let temp_path = sync_temp_dir(config_path).join(&file_state.filename);

                    let received_checksum = match std::fs::read(&temp_path) {
                        Ok(data) => Self::compute_data_checksum(&data),
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

                    let final_path = target_dir.join(&file_state.filename);
                    if let Some(parent) = final_path.parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }

                    if let Err(e) = tokio::fs::rename(&temp_path, &final_path).await {
                        event.on_sync_error(format!(
                            "Failed to move file to {final_path:?}. Error: {e}"
                        ));
                    } else {
                        let completed =
                            items_completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                        let total = items_total.load(std::sync::atomic::Ordering::SeqCst);

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
                        tokio::task::spawn_blocking(move || {
                            let thumb =
                                crate::thumbnail::generate_thumbnail(&path_str).unwrap_or_default();
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
                            });
                            db.clear_sync_needed(&id_clone);
                        });

                        event.on_photo_received(id_for_event, path_for_event);

                        let status = format!("Received {completed}/{total}");
                        let progress = (completed as f32 / total as f32) * 100.0;

                        event.on_sync_progress(SyncProgress {
                            device_id: "peer".to_string(),
                            status,
                            progress,
                            bytes_per_second: 0,
                            items_completed: completed,
                            items_total: total,
                        });

                        let _ = Self::send_sync_message(
                            dc,
                            &SyncMessage::PeerProgress {
                                status: format!("Peer received {completed}/{total}"),
                                progress,
                                items_completed: completed,
                                items_total: total,
                            },
                        )
                        .await;
                    }
                }
            }
            SyncMessage::SyncFile { photo } => {
                let _ =
                    Self::send_sync_message(dc, &SyncMessage::FileRequest { id: photo.id }).await;
            }
            SyncMessage::StartSync => {
                event.on_state_change("Sync started");
                let _ = Self::send_sync_message(dc, &SyncMessage::ManifestRequest).await;
            }
            SyncMessage::CatchUp => {
                let db = Database::new(config_path);
                let ids: Vec<String> = {
                    let sql = "SELECT id FROM photo WHERE sync_needed = 1 AND location NOT LIKE '%/siegu/%' AND location NOT LIKE '%\\siegu\\%'";
                    if let Ok(mut stmt) = db.connection.prepare(sql) {
                        stmt.query_map([], |row| row.get::<_, String>(0))
                            .map(|rows| rows.flatten().collect())
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                };

                for id in ids {
                    let _ = Self::send_sync_message(dc, &SyncMessage::FileRequest { id }).await;
                }
            }
            SyncMessage::PeerProgress {
                status,
                progress,
                items_completed,
                items_total,
            } => {
                event.on_sync_progress(SyncProgress {
                    device_id: "peer".to_string(),
                    status,
                    progress,
                    bytes_per_second: 0,
                    items_completed,
                    items_total,
                });
            }
            SyncMessage::MetadataUpdate {
                photo_id,
                caption,
                aesthetics_score,
                indexed,
            } => {
                let db = Database::new(config_path);
                db.update_photo_metadata(&photo_id, caption.as_deref(), aesthetics_score, indexed);
                event.on_log(&format!("Metadata updated for {photo_id}"));
            }
            SyncMessage::VersionNegotiate {
                version,
                device_id,
                device_name,
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
                event.on_peer_connected(device_id, device_name, models_enabled, version);
            }
            SyncMessage::VersionReject { reason } => {
                event.on_sync_error(format!("Peer rejected connection: {reason}"));
            }
        }
    }

    pub async fn start_lan_server(
        signaling_port: u16,
    ) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
        let port = if signaling_port > 0 {
            signaling_port
        } else {
            crate::lan_server::start(0).await
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

struct NullSyncEvent;

impl SyncEvent for NullSyncEvent {
    fn on_state_change(&self, _: &str) {}
    fn on_log(&self, _: &str) {}
    fn on_sync_progress(&self, _: SyncProgress) {}
    fn on_photo_received(&self, _: String, _: String) {}
    fn on_sync_error(&self, _: String) {}
    fn on_peer_connected(&self, _: String, _: String, _: Vec<String>, _: u8) {}
    fn on_peer_disconnected(&self, _: String) {}
    fn on_device_registered(&self, _: &Database) {}
    fn on_metadata_updated(&self, _: &str, _: Option<&str>, _: Option<f64>) {}
    fn get_config_path(&self) -> String {
        String::new()
    }
    fn get_sync_path(&self) -> Option<String> {
        None
    }
    fn get_directories(&self) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }

    #[test]
    fn test_max_mesh_devices() {
        assert_eq!(MAX_MESH_DEVICES, 5);
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
            models_enabled: vec!["caption".to_string(), "face".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: SyncMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            SyncMessage::VersionNegotiate {
                version,
                device_id,
                device_name,
                models_enabled,
            } => {
                assert_eq!(version, 1);
                assert_eq!(device_id, "device-1");
                assert_eq!(device_name, "Phone");
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
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: SyncMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            SyncMessage::MetadataUpdate {
                photo_id,
                caption,
                aesthetics_score,
                indexed,
            } => {
                assert_eq!(photo_id, "photo-1");
                assert_eq!(caption, Some("A beautiful sunset".to_string()));
                assert_eq!(aesthetics_score, Some(0.95));
                assert_eq!(indexed, 2);
            }
            _ => panic!("Expected MetadataUpdate"),
        }
    }

    #[test]
    fn test_sync_progress_struct() {
        let progress = SyncProgress {
            device_id: "peer".to_string(),
            status: "Syncing".to_string(),
            progress: 50.0,
            bytes_per_second: 1024,
            items_completed: 5,
            items_total: 10,
        };
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("50.0"));
        assert!(json.contains("Syncing"));
    }
}
