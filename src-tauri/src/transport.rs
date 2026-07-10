use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async, tungstenite::client::IntoClientRequest, tungstenite::protocol::Message,
    tungstenite::Utf8Bytes,
};
use webrtc::{
    api::{
        interceptor_registry::register_default_interceptors, media_engine::MediaEngine, APIBuilder,
    },
    ice_transport::{ice_candidate::RTCIceCandidateInit, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
};

use crate::database::{Database, ImportedPhoto, PhotoSyncInfo};
use crate::emit_log;
use std::collections::HashMap;
use tauri::Emitter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use warp::Filter;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncProgress {
    pub device_id: String,
    pub status: String,
    pub progress: f32,
    pub bytes_per_second: u64,
    pub items_completed: usize,
    pub items_total: usize,
}

struct OutgoingFile {
    id: String,
    path: String,
    created: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
    objects: String,
    faces: String,
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
        size: u64,
        created: String,
        latitude: Option<f64>,
        longitude: Option<f64>,
        objects: String,
        faces: String,
    },
    FileChunk {
        id: String,
        data: Vec<u8>,
    },
    FileEnd {
        id: String,
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
}

// Use the same SignalMessage structure as the axum server
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalMessage {
    #[serde(rename = "join")]
    Join { device_id: String },
    #[serde(rename = "joined")]
    Joined {
        device_id: String,
        room_id: String,
        peer_count: usize,
    },
    #[serde(rename = "offer")]
    Offer { payload: String, target: String },
    #[serde(rename = "answer")]
    Answer { payload: String, target: String },
    #[serde(rename = "ice_candidate")]
    IceCandidate { payload: String, target: String },
    #[serde(rename = "peer_disconnected")]
    PeerDisconnected { device_id: String },
    #[serde(rename = "peer_joined")]
    PeerJoined { device_id: String },
    #[serde(rename = "error")]
    Error { message: String },

    // Go server relay protocol
    #[serde(rename = "create_room")]
    CreateRoom,
    #[serde(rename = "room_created")]
    RoomCreated { code: String },
    #[serde(rename = "join_room")]
    JoinRoom { code: String },
    #[serde(rename = "room_joined")]
    RoomJoined,
    #[serde(rename = "relay")]
    Relay { from: Option<String>, payload: serde_json::Value },
    #[serde(rename = "room_closed")]
    RoomClosed,
}

pub struct WebRtcClient {
    pub room_id: String,
    pub is_initiator: bool,
    pub signaling_url: String,
    pub app_handle: Option<AppHandle>,
    pub config_path: String,
    pub sync_tx: Arc<tokio::sync::Mutex<Option<UnboundedSender<SyncMessage>>>>,
}

use tauri::AppHandle;

struct IncomingFile {
    id: String,
    filename: String,
    size: u64,
    received: u64,
    created: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
    objects: String,
    faces: String,
    file: tokio::fs::File,
}

pub struct MediaServerState {
    pub port: u16,
}

pub fn start_media_server(config_path: String) -> u16 {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let config_path_clone = config_path.clone();
            let images = warp::path("images").and(warp::fs::dir(config_path_clone));

            let routes = images;
            let addr: std::net::SocketAddr = ([127, 0, 0, 1], 0).into();
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            let addr = listener.local_addr().unwrap();
            let port = addr.port();
            let _ = tx.send(port);
            warp::serve(routes).incoming(listener).run().await;
        });
    });

    rx.blocking_recv().unwrap_or(0)
}

impl WebRtcClient {
    fn emit(&self, event: &str, payload: impl Serialize + Clone) {
        if let Some(app) = &self.app_handle {
            let _ = app.emit(event, payload);
        }
    }

    async fn send_sync_message(
        dc: &Arc<webrtc::data_channel::RTCDataChannel>,
        msg: &SyncMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string(msg)?;
        dc.send(&bytes::Bytes::from(json)).await?;
        Ok(())
    }

    async fn send_file(
        &self,
        dc: Arc<webrtc::data_channel::RTCDataChannel>,
        outgoing: OutgoingFile,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

        // 1. Send Header
        WebRtcClient::send_sync_message(
            &dc,
            &SyncMessage::FileHeader {
                id: photo_id.clone(),
                filename: filename.clone(),
                size,
                created: outgoing.created,
                latitude: outgoing.latitude,
                longitude: outgoing.longitude,
                objects: outgoing.objects,
                faces: outgoing.faces,
            },
        )
        .await?;

        // 2. Send Chunks
        let mut buffer = vec![0u8; 65536]; // 64KB chunks
        let mut total_sent = 0u64;
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            WebRtcClient::send_sync_message(
                &dc,
                &SyncMessage::FileChunk {
                    id: photo_id.clone(),
                    data: buffer[..n].to_vec(),
                },
            )
            .await?;

            total_sent += n as u64;

            // Emit progress for sender
            if let Some(app) = &self.app_handle {
                let progress = (total_sent as f32 / size as f32) * 100.0;
                let _ = app.emit(
                    "sync-progress",
                    SyncProgress {
                        device_id: "peer".to_string(),
                        status: format!("Sending {filename}"),
                        progress,
                        bytes_per_second: 0,
                        items_completed: 0,
                        items_total: 0,
                    },
                );
            }

            // Flow control: Wait if buffer is too full (e.g. > 1MB)
            while dc.buffered_amount().await > 1_000_000 {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }

        // 3. Send End
        if let Err(e) = WebRtcClient::send_sync_message(
            &dc,
            &SyncMessage::FileEnd {
                id: photo_id.clone(),
            },
        )
        .await
        {
            if let Some(app) = &self.app_handle {
                emit_log(app, format!("Error sending FileEnd: {e}"));
            }
            return Err(e);
        }

        // Notify host UI that a file was sent
        if let Some(app) = &self.app_handle {
            let _ = app.emit(
                "sync-progress",
                SyncProgress {
                    device_id: "peer".to_string(),
                    status: format!("Finished sending {filename}"),
                    progress: 100.0,
                    bytes_per_second: 0,
                    items_completed: 1,
                    items_total: 1,
                },
            );
            let _ = app.emit("photo-synced", photo_id);
        }

        Ok(())
    }

    /// Starts a WebRTC session using a local embedded signaling server (LAN mode).
    /// Useful for syncing devices on the same network without a remote signaling server.
    pub async fn start_lan(
        mut self,
        signaling_port: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.room_id.is_empty() {
            let err_msg = "Room ID is missing".to_string();
            if let Some(app) = &self.app_handle {
                emit_log(app, err_msg.clone());
            }
            self.emit("webrtc-state", err_msg.clone());
            return Err(err_msg.into());
        }

        let port = if signaling_port > 0 {
            signaling_port
        } else {
            crate::lan_server::start(0).await
        };
        let local_url = format!("ws://127.0.0.1:{}", port);
        if let Some(app) = &self.app_handle {
            emit_log(
                app,
                format!("Local signaling server started on port {}", port),
            );
        }

        self.signaling_url = local_url;
        self.start().await
    }

    pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
        let self_arc = Arc::new(self);

        if self_arc.room_id.is_empty() {
            let err_msg = "Room ID is missing".to_string();
            if let Some(app) = &self_arc.app_handle {
                emit_log(app, err_msg.clone());
            }
            self_arc.emit("webrtc-state", err_msg.clone());
            return Err(err_msg.into());
        }

        if let Some(app) = &self_arc.app_handle {
            emit_log(
                app,
                format!(
                    "Attempting to connect to signaling at {} for room {}",
                    self_arc.signaling_url, self_arc.room_id
                ),
            );
        }
        self_arc.emit("webrtc-state", "Connecting to signaling...");

        let is_remote = self_arc.signaling_url.contains("wss://")
            || (self_arc.signaling_url.contains("ws://")
                && !self_arc.signaling_url.contains("127.0.0.1")
                && !self_arc.signaling_url.contains("localhost"));

        if is_remote {
            let base_url = self_arc.signaling_url.trim_end_matches('/');
            let url_str = base_url.to_string();
            let req = url_str.into_client_request()?;
            let (ws_stream, _) = match connect_async(req).await {
                Ok(s) => {
                    if let Some(app) = &self_arc.app_handle {
                        emit_log(app, "Connected to signaling server!".to_string());
                    }
                    s
                }
                Err(e) => {
                    let err_msg = format!("Signaling connection failed: {e}");
                    if let Some(app) = &self_arc.app_handle {
                        emit_log(app, err_msg.clone());
                    }
                    self_arc.emit("webrtc-state", err_msg);
                    return Err(e.into());
                }
            };

            let (relay_write, relay_read) = ws_stream.split();
            let relay_write = Arc::new(Mutex::new(relay_write));

            return Self::start_remote(&self_arc, relay_write, relay_read).await;
        }

        let base_url = self_arc.signaling_url.trim_end_matches('/');
        let url_str = format!("{}/{}", base_url, self_arc.room_id);

        let req = url_str.into_client_request()?;
        let (ws_stream, _) = match connect_async(req).await {
            Ok(s) => {
                if let Some(app) = &self_arc.app_handle {
                    emit_log(
                        app,
                        "Successfully connected to signaling server!".to_string(),
                    );
                }
                s
            }
            Err(e) => {
                let err_msg = format!("Signaling connection failed: {e}");
                if let Some(app) = &self_arc.app_handle {
                    emit_log(app, err_msg.clone());
                }
                self_arc.emit("webrtc-state", err_msg);
                return Err(e.into());
            }
        };
        self_arc.emit(
            "webrtc-state",
            "Connected to signaling. Waiting for peer...",
        );
        let (write, read) = ws_stream.split();
        let write = Arc::new(Mutex::new(write));

        let (sync_msg_tx, sync_msg_rx) = tokio::sync::mpsc::unbounded_channel::<SyncMessage>();
        {
            let mut tx_lock = self_arc.sync_tx.lock().await;
            *tx_lock = Some(sync_msg_tx);
        }
        let sync_msg_rx_shared = Arc::new(tokio::sync::Mutex::new(sync_msg_rx));

        let my_device_id = uuid::Uuid::new_v4().to_string();
        let join_msg = SignalMessage::Join {
            device_id: my_device_id.clone(),
        };
        write
            .lock()
            .await
            .send(Message::Text(Utf8Bytes::from(serde_json::to_string(
                &join_msg,
            )?)))
            .await?;

        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut m = MediaEngine::default();
        m.register_default_codecs()?;
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        let peer_connection = Arc::new(api.new_peer_connection(config).await?);

        let app_handle_state = self_arc.app_handle.clone();
        let config_path_state = self_arc.config_path.clone();
        let room_id_state = self_arc.room_id.clone();

        peer_connection.on_peer_connection_state_change(Box::new(
            move |s: RTCPeerConnectionState| {
                let app_handle = app_handle_state.clone();
                let config_path = config_path_state.clone();
                let room_id = room_id_state.clone();
                Box::pin(async move {
                    if let Some(app) = &app_handle {
                        emit_log(app, format!("Peer Connection State changed to: {s:?}"));
                    }
                    let status = match s {
                        RTCPeerConnectionState::Connected => "Connected",
                        RTCPeerConnectionState::Connecting => "Connecting WebRTC...",
                        RTCPeerConnectionState::Disconnected => "Peer Disconnected",
                        RTCPeerConnectionState::Failed => "Connection Failed",
                        RTCPeerConnectionState::New => "Waiting for peer...",
                        _ => "Awaiting connection...",
                    };

                    if let Some(app) = &app_handle {
                        let _ = app.emit("webrtc-state", status);

                        if s == RTCPeerConnectionState::Connected {
                            let db = Database::new(&config_path);
                            let peer_name = format!("Peer ({})", &room_id[..8]);
                            let _ = db.connection.execute(
                                "INSERT OR REPLACE INTO device(ip, name) VALUES(?1, ?2)",
                                (&room_id, &peer_name),
                            );
                            let _ = app.emit("refresh-devices", ());
                        }
                    }
                })
            },
        ));

        let pending_ice_candidates = Arc::new(Mutex::new(Vec::new()));

        let write_ice = Arc::clone(&write);
        peer_connection.on_ice_candidate(Box::new(
            move |c: Option<webrtc::ice_transport::ice_candidate::RTCIceCandidate>| {
                let write_ice = Arc::clone(&write_ice);
                Box::pin(async move {
                    if let Some(c) = c {
                        if let Ok(json) = c.to_json() {
                            if let Ok(payload) = serde_json::to_string(&json) {
                                let msg = SignalMessage::IceCandidate {
                                    payload,
                                    target: "peer".to_string(),
                                };
                                if let Ok(msg_str) = serde_json::to_string(&msg) {
                                    let _ = write_ice
                                        .lock()
                                        .await
                                        .send(Message::Text(Utf8Bytes::from(msg_str)))
                                        .await;
                                }
                            }
                        }
                    }
                })
            },
        ));

        let incoming_files: Arc<Mutex<HashMap<String, IncomingFile>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let self_initiator = Arc::clone(&self_arc);
        let sync_msg_rx_shared = Arc::clone(&sync_msg_rx_shared);

        if self_initiator.is_initiator {
            let data_channel = peer_connection
                .create_data_channel("file_transfer", None)
                .await?;
            let dc_clone = Arc::clone(&data_channel);
            let app_handle_initiator = self_initiator.app_handle.clone();
            let self_initiator_open = Arc::clone(&self_initiator);
            let sync_msg_rx_initiator = Arc::clone(&sync_msg_rx_shared);

            data_channel.on_open(Box::new(move || {
                let dc_inner = Arc::clone(&dc_clone);
                let self_sync_open = Arc::clone(&self_initiator_open);
                let sync_msg_rx_inner = Arc::clone(&sync_msg_rx_initiator);
                let app_handle = app_handle_initiator.clone();
                Box::pin(async move {
                    if let Some(app) = &app_handle {
                        let _ = app.emit("webrtc-state", "Secure Data Channel Ready");
                    }
                    let _ =
                        WebRtcClient::send_sync_message(&dc_inner, &SyncMessage::ManifestRequest)
                            .await;
                    let _ = WebRtcClient::send_sync_message(&dc_inner, &SyncMessage::CatchUp).await;
                    let mut rx = sync_msg_rx_inner.lock().await;
                    while let Some(msg) = rx.recv().await {
                        let _ = WebRtcClient::send_sync_message(&dc_inner, &msg).await;
                    }
                    drop(self_sync_open);
                })
            }));

            let dc_clone_msg = Arc::clone(&data_channel);
            let incoming_files_clone = Arc::clone(&incoming_files);
            let config_path_dc = self_initiator.config_path.clone();
            let app_handle_initiator_msg = self_initiator.app_handle.clone();
            let self_initiator_msg = Arc::clone(&self_initiator);
            let items_completed_shared = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let items_total_shared = Arc::new(std::sync::atomic::AtomicUsize::new(0));

            data_channel.on_message(Box::new(
                move |msg: webrtc::data_channel::data_channel_message::DataChannelMessage| {
                    let dc = Arc::clone(&dc_clone_msg);
                    let incoming_files = Arc::clone(&incoming_files_clone);
                    let config_path = config_path_dc.clone();
                    let app_handle = app_handle_initiator_msg.clone();
                    let self_inner = self_initiator_msg.clone();
                    let items_completed = Arc::clone(&items_completed_shared);
                    let items_total = Arc::clone(&items_total_shared);

                    Box::pin(async move {
                        let text = String::from_utf8_lossy(&msg.data);
                        if let Ok(sync_msg) = serde_json::from_str::<SyncMessage>(&text) {
                            match sync_msg {
                                SyncMessage::ManifestResponse { photos } => {
                                    let db = Database::new(&config_path);
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

                                        if let Some(app) = &app_handle {
                                            let _ = app.emit("sync-progress", SyncProgress {
                                                device_id: "peer".to_string(),
                                                status: format!("Syncing {total} new files"),
                                                progress: 0.0,
                                                bytes_per_second: 0,
                                                items_completed: 0,
                                                items_total: total,
                                            });
                                        }

                                        // Tell peer our total
                                        let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::PeerProgress {
                                            status: format!("Peer needs {total} files"),
                                            progress: 0.0,
                                            items_completed: 0,
                                            items_total: total,
                                        }).await;

                                        for id in to_request {
                                            let _ = WebRtcClient::send_sync_message(
                                                &dc,
                                                &SyncMessage::FileRequest { id },
                                            )
                                            .await;
                                        }
                                    } else if let Some(app) = &app_handle {
                                        let _ = app.emit("sync-progress", SyncProgress {
                                            device_id: "peer".to_string(),
                                            status: "Up to date".to_string(),
                                            progress: 100.0,
                                            bytes_per_second: 0,
                                            items_completed: 0,
                                            items_total: 0,
                                        });
                                    }
                                }
                                SyncMessage::FileHeader { id, filename, size, created, latitude, longitude, objects, faces } => {
                                    let sanitized = filename.replace("..", "").replace(['/', '\\'], "_");
                                    let save_path = Path::new(&config_path).join("sync_temp").join(&sanitized);
                                    if let Some(parent) = save_path.parent() { let _ = tokio::fs::create_dir_all(parent).await; }
                                    if let Ok(file) = tokio::fs::File::create(&save_path).await {
                                        let mut incoming = incoming_files.lock().await;
                                        incoming.insert(id.clone(), IncomingFile { id, filename: sanitized.clone(), size, received: 0, created, latitude, longitude, objects, faces, file });
                                    }
                                }
                                SyncMessage::FileChunk { id, data } => {
                                    let mut incoming = incoming_files.lock().await;
                                    if let Some(file_state) = incoming.get_mut(&id) {
                                        let _ = file_state.file.write_all(&data).await;
                                        file_state.received += data.len() as u64;
                                        if let Some(app) = &app_handle {
                                            let progress = (file_state.received as f32 / file_state.size as f32) * 100.0;
                                            let _ = app.emit("sync-progress", SyncProgress {
                                                device_id: "peer".to_string(), status: format!("Receiving {}", file_state.filename),
                                                progress, bytes_per_second: 0, items_completed: 0, items_total: 0,
                                            });
                                        }
                                    }
                                }
                                SyncMessage::FileEnd { id } => {
                                    let mut incoming = incoming_files.lock().await;
                                    if let Some(mut file_state) = incoming.remove(&id) {
                                        let _ = file_state.file.flush().await;
                                        drop(file_state.file);
                                        let temp_path = Path::new(&config_path).join("sync_temp").join(&file_state.filename);
                                        let db = Database::new(&config_path);

                                        // Fetch sync_path directly from database state to be sure
                                        let state = db.get_state();
                                        let sync_path_str = state.get("sync_path");
                                        let dirs = db.list_directories();

                                        let target_dir = if let Some(sp) = sync_path_str {
                                            PathBuf::from(sp).join("siegu")
                                        } else if !dirs.is_empty() {
                                            PathBuf::from(&dirs[0]).join("siegu")
                                        } else {
                                            Path::new(&config_path).join("Siegu").join("siegu")
                                        };

                                        let final_path = target_dir.join(&file_state.filename);
                                        if let Err(e) = tokio::fs::rename(&temp_path, &final_path).await {
                                            if let Some(app) = &app_handle {
                                                let _ = app.emit("sync-error", format!("Failed to move file to {final_path:?}. Error: {e}"));
                                            }
                                        } else if let Some(app) = &app_handle {
                                            let completed = items_completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                                            let total = items_total.load(std::sync::atomic::Ordering::SeqCst);

                                            // Wait for thumbnail generation BEFORE importing to DB
                                            let app_thumb = app.clone();
                                            let id_thumb = file_state.id.clone();
                                            let path_thumb = final_path.to_string_lossy().to_string();
                                            let created_thumb = file_state.created.clone();
                                            let lat_thumb = file_state.latitude.unwrap_or(0.0);
                                            let lon_thumb = file_state.longitude.unwrap_or(0.0);
                                            let config_path_thumb = config_path.clone();
                                            let objects_thumb = file_state.objects.clone();
                                            let faces_thumb = file_state.faces.clone();

                                            tokio::task::spawn_blocking(move || {
                                                let thumb = String::new();
                                                let mut db = Database::new(&config_path_thumb);

                                                // Now import with thumbnail included - only now it becomes visible in library
                                                db.import_photo(ImportedPhoto {
                                                    id: &id_thumb,
                                                    location: &path_thumb,
                                                    created: &created_thumb,
                                                    latitude: Some(lat_thumb),
                                                    longitude: Some(lon_thumb),
                                                    objects_json: &objects_thumb,
                                                    faces_json: &faces_thumb,
                                                    encoded: &thumb,
                                                });

                                                let _ = app_thumb.emit("photo-received", crate::database::Photo {
                                                    id: id_thumb,
                                                    encoded: thumb,
                                                    location: path_thumb,
                                                    created: created_thumb,
                                                    objects: HashMap::new(),
                                                    properties: HashMap::new(),
                                                    latitude: lat_thumb,
                                                    longitude: lon_thumb,
                                                    favorite: false,
                                                    indexed: 2,
                                                    caption: None,
                                                    aesthetics_score: None,
                                                    ai_status: crate::database::AiStatus::default(),
                                                });
                                            });

                                            let status = format!("Received {completed}/{total}");
                                            let progress = (completed as f32 / total as f32) * 100.0;

                                            let _ = app.emit("sync-progress", SyncProgress {
                                                device_id: "peer".to_string(), status: status.clone(),
                                                progress, bytes_per_second: 0, items_completed: completed, items_total: total,
                                            });

                                            // Update Peer
                                            let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::PeerProgress {
                                                status: format!("Peer received {completed}/{total}"),
                                                progress,
                                                items_completed: completed,
                                                items_total: total,
                                            }).await;
                                        }
                                    }
                                }
                                SyncMessage::SyncFile { photo } => {
                                    let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::FileRequest { id: photo.id }).await;
                                }
                                SyncMessage::StartSync => {
                                    if let Some(app) = &app_handle { let _ = app.emit("start-sync", ()); }
                                    let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::ManifestRequest).await;
                                }
                                SyncMessage::ManifestRequest => {
                                    let db = Database::new(&config_path);
                                    let photos = db.get_photo_sync_info();
                                    let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::ManifestResponse { photos }).await;
                                }
                                SyncMessage::CatchUp => {
                                    let db = Database::new(&config_path);
                                    // Collect IDs first to avoid Send issues
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
                                        let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::FileRequest { id }).await;
                                    }
                                }
                                SyncMessage::PeerProgress { status, progress, items_completed, items_total } => {
                                    if let Some(app) = &app_handle {
                                        let _ = app.emit("sync-progress", SyncProgress {
                                            device_id: "peer".to_string(),
                                            status,
                                            progress,
                                            bytes_per_second: 0,
                                            items_completed,
                                            items_total,
                                        });
                                    }
                                }
                                SyncMessage::FileRequest { id } => {
                                    let db = Database::new(&config_path);
                                    if let Ok((path, created, lat, lon, objects, faces)) = db.connection.query_row(
                                        "SELECT p.location, p.created, p.latitude, p.longitude,
                                         (SELECT json_group_array(json_object('class', class, 'probability', probability)) FROM object WHERE photo_id = p.id),
                                         (SELECT json_group_array(json_object('face_id', face_id, 'crop_path', crop_path, 'encoded', encoded, 'person_id', person_id)) FROM faces WHERE photo_id = p.id)
                                         FROM photo p WHERE p.id = ?1",
                                        [&id],
                                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<f64>>(2)?, row.get::<_, Option<f64>>(3)?, row.get::<_, String>(4).unwrap_or("[]".to_string()), row.get::<_, String>(5).unwrap_or("[]".to_string()))),
                                    ) {
                                        let dc_send = Arc::clone(&dc);
                                        let self_task = self_inner.clone();
                                        tokio::spawn(async move {
                                            let _ = self_task
                                                .send_file(
                                                    dc_send,
                                                    OutgoingFile {
                                                        id,
                                                        path,
                                                        created,
                                                        latitude: lat,
                                                        longitude: lon,
                                                        objects,
                                                        faces,
                                                    },
                                                )
                                                .await;
                                        });
                                    }
                                }
                            }
                        }
                    })
                },
            ));
        } else {
            let app_handle_opt = self_arc.app_handle.clone();
            let incoming_files_clone = Arc::new(Mutex::new(HashMap::new()));
            let config_path_dc = self_arc.config_path.clone();
            let sync_msg_rx_shared_receiver = Arc::clone(&sync_msg_rx_shared);
            let self_receiver = Arc::clone(&self_arc);
            let items_completed_shared = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let items_total_shared = Arc::new(std::sync::atomic::AtomicUsize::new(0));

            peer_connection.on_data_channel(Box::new(move |d: Arc<webrtc::data_channel::RTCDataChannel>| {
                let dc_clone = Arc::clone(&d);
                let incoming_files = Arc::clone(&incoming_files_clone);
                let config_path = config_path_dc.clone();
                let app_handle = app_handle_opt.clone();
                let sync_msg_rx_inner_shared = Arc::clone(&sync_msg_rx_shared_receiver);
                let self_receiver_msg = Arc::clone(&self_receiver);
                let items_completed = Arc::clone(&items_completed_shared);
                let items_total = Arc::clone(&items_total_shared);

                d.on_message(Box::new(move |msg: webrtc::data_channel::data_channel_message::DataChannelMessage| {
                    let dc = Arc::clone(&dc_clone);
                    let incoming_files = Arc::clone(&incoming_files);
                    let config_path = config_path.clone();
                    let app_handle = app_handle.clone();
                    let self_inner = self_receiver_msg.clone();
                    let items_completed = Arc::clone(&items_completed);
                    let items_total = Arc::clone(&items_total);

                    Box::pin(async move {
                        let text = String::from_utf8_lossy(&msg.data);
                        if let Ok(sync_msg) = serde_json::from_str::<SyncMessage>(&text) {
                            match sync_msg {
                                SyncMessage::SyncFile { photo } => {
                                    let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::FileRequest { id: photo.id }).await;
                                }
                                SyncMessage::StartSync => {
                                    if let Some(app) = &app_handle { let _ = app.emit("start-sync", ()); }
                                    let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::ManifestRequest).await;
                                }
                                SyncMessage::ManifestRequest => {
                                    let db = Database::new(&config_path);
                                    let photos = db.get_photo_sync_info();
                                    let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::ManifestResponse { photos }).await;
                                }
                                SyncMessage::CatchUp => {
                                    let db = Database::new(&config_path);
                                    // Collect IDs first to avoid Send issues
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
                                        let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::FileRequest { id }).await;
                                    }
                                }
                                SyncMessage::ManifestResponse { photos } => {
                                    let db = Database::new(&config_path);
                                    let my_manifest = db.get_photo_sync_info();
                                    let mut to_request = Vec::new();
                                    for peer_photo in &photos {
                                        if !my_manifest.iter().any(|p| p.id == peer_photo.id) {
                                            to_request.push(peer_photo.id.clone());
                                        }
                                    }

                                    if !to_request.is_empty() {
                                        let items_total = to_request.len();
                                        if let Some(app) = &app_handle {
                                            let _ = app.emit("sync-progress", SyncProgress {
                                                device_id: "peer".to_string(),
                                                status: format!("Syncing {items_total} new files"),
                                                progress: 0.0,
                                                bytes_per_second: 0,
                                                items_completed: 0,
                                                items_total,
                                            });
                                        }

                                        // Tell peer our total
                                        let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::PeerProgress {
                                            status: format!("Peer needs {items_total} files"),
                                            progress: 0.0,
                                            items_completed: 0,
                                            items_total,
                                        }).await;

                                        for id in to_request {
                                            let _ = WebRtcClient::send_sync_message(
                                                &dc,
                                                &SyncMessage::FileRequest { id },
                                            )
                                            .await;
                                        }
                                    } else if let Some(app) = &app_handle {
                                        let _ = app.emit("sync-progress", SyncProgress {
                                            device_id: "peer".to_string(),
                                            status: "Up to date".to_string(),
                                            progress: 100.0,
                                            bytes_per_second: 0,
                                            items_completed: 0,
                                            items_total: 0,
                                        });
                                    }
                                }
                                SyncMessage::PeerProgress { status, progress, items_completed, items_total } => {
                                    if let Some(app) = &app_handle {
                                        let _ = app.emit("sync-progress", SyncProgress {
                                            device_id: "peer".to_string(),
                                            status,
                                            progress,
                                            bytes_per_second: 0,
                                            items_completed,
                                            items_total,
                                        });
                                    }
                                }
                                SyncMessage::FileRequest { id } => {
                                    let db = Database::new(&config_path);
                                    if let Ok((path, created, lat, lon, objects, faces)) = db.connection.query_row(
                                        "SELECT p.location, p.created, p.latitude, p.longitude,
                                         (SELECT json_group_array(json_object('class', class, 'probability', probability)) FROM object WHERE photo_id = p.id),
                                         (SELECT json_group_array(json_object('face_id', face_id, 'crop_path', crop_path, 'encoded', encoded, 'person_id', person_id)) FROM faces WHERE photo_id = p.id)
                                         FROM photo p WHERE p.id = ?1",
                                        [&id],
                                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<f64>>(2)?, row.get::<_, Option<f64>>(3)?, row.get::<_, String>(4).unwrap_or("[]".to_string()), row.get::<_, String>(5).unwrap_or("[]".to_string()))),
                                    ) {
                                        let dc_send = Arc::clone(&dc);
                                        let self_task = self_inner.clone();
                                        tokio::spawn(async move {
                                            let _ = self_task
                                                .send_file(
                                                    dc_send,
                                                    OutgoingFile {
                                                        id,
                                                        path,
                                                        created,
                                                        latitude: lat,
                                                        longitude: lon,
                                                        objects,
                                                        faces,
                                                    },
                                                )
                                                .await;
                                        });
                                    }
                                }
                                SyncMessage::FileHeader { id, filename, size, created, latitude, longitude, objects, faces } => {
                                    let sanitized = filename.replace("..", "").replace(['/', '\\'], "_");
                                    let save_path = Path::new(&config_path).join("sync_temp").join(&sanitized);
                                    if let Some(parent) = save_path.parent() { let _ = tokio::fs::create_dir_all(parent).await; }
                                    if let Ok(file) = tokio::fs::File::create(&save_path).await {
                                        let mut incoming = incoming_files.lock().await;
                                        incoming.insert(id.clone(), IncomingFile { id, filename: sanitized.clone(), size, received: 0, created, latitude, longitude, objects, faces, file });
                                    }
                                }
                                SyncMessage::FileChunk { id, data } => {
                                    let mut incoming = incoming_files.lock().await;
                                    if let Some(file_state) = incoming.get_mut(&id) {
                                        let _ = file_state.file.write_all(&data).await;
                                        file_state.received += data.len() as u64;
                                        if let Some(app) = &app_handle {
                                            let progress = (file_state.received as f32 / file_state.size as f32) * 100.0;
                                            let _ = app.emit("sync-progress", SyncProgress {
                                                device_id: "peer".to_string(), status: format!("Receiving {}", file_state.filename),
                                                progress, bytes_per_second: 0, items_completed: 0, items_total: 0,
                                            });
                                        }
                                    }
                                }
                                SyncMessage::FileEnd { id } => {
                                    let mut incoming = incoming_files.lock().await;
                                    if let Some(mut file_state) = incoming.remove(&id) {
                                        let _ = file_state.file.flush().await;
                                        drop(file_state.file);
                                        let temp_path = Path::new(&config_path).join("sync_temp").join(&file_state.filename);
                                        let db = Database::new(&config_path);

                                        // Fetch sync_path directly from database state to be sure
                                        let state = db.get_state();
                                        let sync_path_str = state.get("sync_path");
                                        let dirs = db.list_directories();

                                        let target_dir = if let Some(sp) = sync_path_str {
                                            PathBuf::from(sp).join("siegu")
                                        } else if !dirs.is_empty() {
                                            PathBuf::from(&dirs[0]).join("siegu")
                                        } else {
                                            Path::new(&config_path).join("Siegu").join("siegu")
                                        };

                                        let final_path = target_dir.join(&file_state.filename);
                                        if let Err(e) = tokio::fs::rename(&temp_path, &final_path).await {
                                            if let Some(app) = &app_handle {
                                                let _ = app.emit("sync-error", format!("Failed to move file to {final_path:?}. Error: {e}"));
                                            }
                                        } else if let Some(app) = &app_handle {
                                            let completed = items_completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                                            let total = items_total.load(std::sync::atomic::Ordering::SeqCst);

                                            // Wait for thumbnail generation BEFORE importing to DB
                                            let app_thumb = app.clone();
                                            let id_thumb = file_state.id.clone();
                                            let path_thumb = final_path.to_string_lossy().to_string();
                                            let created_thumb = file_state.created.clone();
                                            let lat_thumb = file_state.latitude.unwrap_or(0.0);
                                            let lon_thumb = file_state.longitude.unwrap_or(0.0);
                                            let config_path_thumb = config_path.clone();
                                            let objects_thumb = file_state.objects.clone();
                                            let faces_thumb = file_state.faces.clone();

                                            tokio::task::spawn_blocking(move || {
                                                let thumb = String::new();
                                                let mut db = Database::new(&config_path_thumb);

                                                // Now import with thumbnail included - only now it becomes visible in library
                                                db.import_photo(ImportedPhoto {
                                                    id: &id_thumb,
                                                    location: &path_thumb,
                                                    created: &created_thumb,
                                                    latitude: Some(lat_thumb),
                                                    longitude: Some(lon_thumb),
                                                    objects_json: &objects_thumb,
                                                    faces_json: &faces_thumb,
                                                    encoded: &thumb,
                                                });

                                                let _ = app_thumb.emit("photo-received", crate::database::Photo {
                                                    id: id_thumb,
                                                    encoded: thumb,
                                                    location: path_thumb,
                                                    created: created_thumb,
                                                    objects: HashMap::new(),
                                                    properties: HashMap::new(),
                                                    latitude: lat_thumb,
                                                    longitude: lon_thumb,
                                                    favorite: false,
                                                    indexed: 2,
                                                    caption: None,
                                                    aesthetics_score: None,
                                                    ai_status: crate::database::AiStatus::default(),
                                                });
                                            });

                                            let status = format!("Received {completed}/{total}");
                                            let progress = (completed as f32 / total as f32) * 100.0;

                                            let _ = app.emit("sync-progress", SyncProgress {
                                                device_id: "peer".to_string(), status: status.clone(),
                                                progress, bytes_per_second: 0, items_completed: completed, items_total: total,
                                            });

                                            // Update Peer
                                            let _ = WebRtcClient::send_sync_message(&dc, &SyncMessage::PeerProgress {
                                                status: format!("Peer received {completed}/{total}"),
                                                progress,
                                                items_completed: completed,
                                                items_total: total,
                                            }).await;
                                        }
                                    }
                                }
                            }
                        }
                    })
                }));

                let dc_open = Arc::clone(&d);
                let sync_msg_rx_inner = Arc::clone(&sync_msg_rx_inner_shared);
                let self_inner_msg_open = self_receiver.clone();
                d.on_open(Box::new(move || {
                    let dc_inner = Arc::clone(&dc_open);
                    let sync_msg_rx_final = Arc::clone(&sync_msg_rx_inner);
                    let self_sync_on_open = self_inner_msg_open.clone();
                    Box::pin(async move {
                        let mut rx = sync_msg_rx_final.lock().await;
                        while let Some(msg) = rx.recv().await {
                            let _ = WebRtcClient::send_sync_message(&dc_inner, &msg).await;
                        }
                        drop(self_sync_on_open);
                    })
                }));

                Box::pin(async move {})
            }));
        }

        let mut read = read;
        let pc = Arc::clone(&peer_connection);
        let write = Arc::clone(&write);
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let signal: SignalMessage = serde_json::from_str(&text)?;
                    match signal {
                        SignalMessage::Joined { peer_count, .. } => {
                            if peer_count == 2 {
                                self_arc.emit("webrtc-state", "Peer Joined");
                            }
                            if self_arc.is_initiator && peer_count == 2 {
                                let offer = pc.create_offer(None).await?;
                                pc.set_local_description(offer.clone()).await?;
                                write
                                    .lock()
                                    .await
                                    .send(Message::Text(Utf8Bytes::from(serde_json::to_string(
                                        &SignalMessage::Offer {
                                            payload: serde_json::to_string(&offer)?,
                                            target: "peer".to_string(),
                                        },
                                    )?)))
                                    .await?;
                            }
                        }
                        SignalMessage::PeerJoined { .. } => {
                            self_arc.emit("webrtc-state", "Peer Joined");
                            if self_arc.is_initiator {
                                let offer = pc.create_offer(None).await?;
                                pc.set_local_description(offer.clone()).await?;
                                write
                                    .lock()
                                    .await
                                    .send(Message::Text(Utf8Bytes::from(serde_json::to_string(
                                        &SignalMessage::Offer {
                                            payload: serde_json::to_string(&offer)?,
                                            target: "peer".to_string(),
                                        },
                                    )?)))
                                    .await?;
                            }
                        }
                        SignalMessage::Offer { payload, .. } => {
                            let sdp: RTCSessionDescription = serde_json::from_str(&payload)?;
                            pc.set_remote_description(sdp).await?;
                            let mut pending = pending_ice_candidates.lock().await;
                            for c in pending.drain(..) {
                                let _ = pc.add_ice_candidate(c).await;
                            }
                            let answer = pc.create_answer(None).await?;
                            pc.set_local_description(answer.clone()).await?;
                            write
                                .lock()
                                .await
                                .send(Message::Text(Utf8Bytes::from(serde_json::to_string(
                                    &SignalMessage::Answer {
                                        payload: serde_json::to_string(&answer)?,
                                        target: "peer".to_string(),
                                    },
                                )?)))
                                .await?;
                        }
                        SignalMessage::Answer { payload, .. } => {
                            let sdp: RTCSessionDescription = serde_json::from_str(&payload)?;
                            pc.set_remote_description(sdp).await?;
                            let mut pending = pending_ice_candidates.lock().await;
                            for c in pending.drain(..) {
                                let _ = pc.add_ice_candidate(c).await;
                            }
                        }
                        SignalMessage::IceCandidate { payload, .. } => {
                            let candidate: RTCIceCandidateInit = serde_json::from_str(&payload)?;
                            if pc.remote_description().await.is_none() {
                                pending_ice_candidates.lock().await.push(candidate);
                            } else {
                                let _ = pc.add_ice_candidate(candidate).await;
                            }
                        }
                        SignalMessage::PeerDisconnected { .. } => {
                            self_arc.emit("webrtc-state", "Peer disconnected");
                        }
                        SignalMessage::Error { message } => {
                            self_arc.emit("webrtc-state", format!("Signaling error: {message}"));
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    self_arc.emit("webrtc-state", format!("WebSocket error: {e}"));
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn start_remote(
        self_arc: &Arc<Self>,
        relay_write: Arc<Mutex< futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
            Message,
        >>>,
        mut relay_read: futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        >,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pending_ice = Arc::new(Mutex::new(Vec::new()));
        let pc = Self::setup_peer_connection_remote(self_arc, &pending_ice, Arc::clone(&relay_write)).await?;

        let write = Arc::clone(&relay_write);
        let pc_write = Arc::clone(&pc);
        let pending_write = Arc::clone(&pending_ice);
        let self_write = Arc::clone(self_arc);

        if self_write.is_initiator {
            write.lock().await.send(Message::Text(Utf8Bytes::from(
                serde_json::to_string(&SignalMessage::CreateRoom)?,
            ))).await?;

            while let Some(msg) = relay_read.next().await {
                if let Ok(Message::Text(text)) = msg {
                    if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                        match signal {
                            SignalMessage::RoomCreated { code } => {
                                if let Some(app) = &self_write.app_handle {
                                    let _ = app.emit("room-code", &code);
                                    let _ = app.emit("webrtc-state", "Waiting for peer to join...");
                                }
                            }
                            SignalMessage::PeerJoined { .. } => {
                                if let Some(app) = &self_write.app_handle {
                                    let _ = app.emit("webrtc-state", "Peer Joined");
                                }
                                let offer = pc_write.create_offer(None).await?;
                                pc_write.set_local_description(offer.clone()).await?;
                                write.lock().await.send(Message::Text(Utf8Bytes::from(
                                    serde_json::to_string(&SignalMessage::Relay {
                                        from: None,
                                        payload: serde_json::json!({"type": "offer", "payload": serde_json::to_string(&offer)?, "target": "peer"}),
                                    })?,
                                ))).await?;
                            }
                            SignalMessage::Relay { payload, .. } => {
                                Self::handle_relay_payload(&pc_write, &write, &pending_write, &self_write, &payload).await?;
                            }
                            SignalMessage::PeerDisconnected { .. } => {
                                if let Some(app) = &self_write.app_handle {
                                    let _ = app.emit("webrtc-state", "Peer disconnected");
                                }
                            }
                            SignalMessage::Error { message } => {
                                if let Some(app) = &self_write.app_handle {
                                    let _ = app.emit("webrtc-state", format!("Signaling error: {message}"));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        } else {
            let room_code = &self_arc.room_id;
            write.lock().await.send(Message::Text(Utf8Bytes::from(
                serde_json::to_string(&SignalMessage::JoinRoom { code: room_code.clone() })?,
            ))).await?;

            let self_joiner = Arc::clone(self_arc);
            let write_j = Arc::clone(&relay_write);
            let pc_j = Arc::clone(&pc);
            let pending_j = Arc::clone(&pending_ice);

            while let Some(msg) = relay_read.next().await {
                if let Ok(Message::Text(text)) = msg {
                    if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                        match signal {
                            SignalMessage::RoomJoined => {
                                if let Some(app) = &self_joiner.app_handle {
                                    let _ = app.emit("webrtc-state", "Room joined. Waiting for peer...");
                                }
                            }
                            SignalMessage::Relay { payload, .. } => {
                                Self::handle_relay_payload(&pc_j, &write_j, &pending_j, &self_joiner, &payload).await?;
                            }
                            SignalMessage::PeerDisconnected { .. } => {
                                if let Some(app) = &self_joiner.app_handle {
                                    let _ = app.emit("webrtc-state", "Peer disconnected");
                                }
                            }
                            SignalMessage::RoomClosed => {
                                if let Some(app) = &self_joiner.app_handle {
                                    let _ = app.emit("webrtc-state", "Room closed");
                                }
                            }
                            SignalMessage::Error { message } => {
                                if let Some(app) = &self_joiner.app_handle {
                                    let _ = app.emit("webrtc-state", format!("Signaling error: {message}"));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_relay_payload(
        pc: &Arc<webrtc::peer_connection::RTCPeerConnection>,
        write: &Arc<Mutex<futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
            Message,
        >>>,
        pending_ice: &Arc<Mutex<Vec<RTCIceCandidateInit>>>,
        _self_arc: &Arc<Self>,
        payload: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(msg_type) = payload.get("type").and_then(|v| v.as_str()) {
            match msg_type {
                "offer" => {
                    if let Some(sdp_str) = payload.get("payload").and_then(|v| v.as_str()) {
                        let sdp: RTCSessionDescription = serde_json::from_str(sdp_str)?;
                        pc.set_remote_description(sdp).await?;
                        let mut pending = pending_ice.lock().await;
                        for c in pending.drain(..) {
                            let _ = pc.add_ice_candidate(c).await;
                        }
                        let answer = pc.create_answer(None).await?;
                        pc.set_local_description(answer.clone()).await?;
                        write.lock().await.send(Message::Text(Utf8Bytes::from(
                            serde_json::to_string(&SignalMessage::Relay {
                                from: None,
                                payload: serde_json::json!({"type": "answer", "payload": serde_json::to_string(&answer)?, "target": "peer"}),
                            })?,
                        ))).await?;
                    }
                }
                "answer" => {
                    if let Some(sdp_str) = payload.get("payload").and_then(|v| v.as_str()) {
                        let sdp: RTCSessionDescription = serde_json::from_str(sdp_str)?;
                        pc.set_remote_description(sdp).await?;
                        let mut pending = pending_ice.lock().await;
                        for c in pending.drain(..) {
                            let _ = pc.add_ice_candidate(c).await;
                        }
                    }
                }
                "ice_candidate" => {
                    if let Some(sdp_str) = payload.get("payload").and_then(|v| v.as_str()) {
                        let candidate: RTCIceCandidateInit = serde_json::from_str(sdp_str)?;
                        if pc.remote_description().await.is_none() {
                            pending_ice.lock().await.push(candidate);
                        } else {
                            let _ = pc.add_ice_candidate(candidate).await;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn setup_peer_connection_remote(
        self_arc: &Arc<Self>,
        _pending_ice: &Arc<Mutex<Vec<RTCIceCandidateInit>>>,
        relay_write: Arc<Mutex<futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
            Message,
        >>>,
    ) -> Result<Arc<webrtc::peer_connection::RTCPeerConnection>, Box<dyn std::error::Error>> {
        let config = RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        let mut m = MediaEngine::default();
        m.register_default_codecs()?;
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        let peer_connection = Arc::new(api.new_peer_connection(config).await?);

        let app_handle_pcs = self_arc.app_handle.clone();
        let config_path_pcs = self_arc.config_path.clone();
        let room_id_pcs = self_arc.room_id.clone();

        peer_connection.on_peer_connection_state_change(Box::new(
            move |s: RTCPeerConnectionState| {
                let app_handle = app_handle_pcs.clone();
                let config_path = config_path_pcs.clone();
                let room_id = room_id_pcs.clone();
                Box::pin(async move {
                    if let Some(app) = &app_handle {
                        emit_log(app, format!("Peer Connection State changed to: {s:?}"));
                    }
                    let status = match s {
                        RTCPeerConnectionState::Connected => "Connected",
                        RTCPeerConnectionState::Connecting => "Connecting WebRTC...",
                        RTCPeerConnectionState::Disconnected => "Peer Disconnected",
                        RTCPeerConnectionState::Failed => "Connection Failed",
                        RTCPeerConnectionState::New => "Waiting for peer...",
                        _ => "Awaiting connection...",
                    };

                    if let Some(app) = &app_handle {
                        let _ = app.emit("webrtc-state", status);

                        if s == RTCPeerConnectionState::Connected {
                            let db = Database::new(&config_path);
                            let peer_name = format!("Peer ({})", &room_id[..8.min(room_id.len())]);
                            let _ = db.connection.execute(
                                "INSERT OR REPLACE INTO device(ip, name) VALUES(?1, ?2)",
                                (&room_id, &peer_name),
                            );
                            let _ = app.emit("refresh-devices", ());
                        }
                    }
                })
            },
        ));

        peer_connection.on_ice_candidate(Box::new({
            let _peer_connection = Arc::clone(&peer_connection);
            let relay_write = Arc::clone(&relay_write);
            move |c: Option<webrtc::ice_transport::ice_candidate::RTCIceCandidate>| {
                let relay_write = Arc::clone(&relay_write);
                Box::pin(async move {
                    if let Some(c) = c {
                        if let Ok(json) = c.to_json() {
                            if let Ok(payload) = serde_json::to_string(&json) {
                                let relay_msg = SignalMessage::Relay {
                                    from: None,
                                    payload: serde_json::json!({"type": "ice_candidate", "payload": payload, "target": "peer"}),
                                };
                                if let Ok(msg_str) = serde_json::to_string(&relay_msg) {
                                    let _ = relay_write
                                        .lock()
                                        .await
                                        .send(Message::Text(Utf8Bytes::from(msg_str)))
                                        .await;
                                }
                            }
                        }
                    }
                })
            }
        }));

        Ok(peer_connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_start_lan_rejects_empty_room_id() {
        let client = WebRtcClient {
            room_id: "".to_string(),
            is_initiator: true,
            signaling_url: "".to_string(),
            app_handle: None,
            config_path: "/tmp".to_string(),
            sync_tx: Arc::new(Mutex::new(None)),
        };
        let result = client.start_lan(0).await;
        assert!(result.is_err(), "Empty room_id should be rejected");
    }
}
