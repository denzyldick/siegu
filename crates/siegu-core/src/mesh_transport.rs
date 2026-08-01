use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async, tungstenite::client::IntoClientRequest, tungstenite::Message as WsMessage,
    tungstenite::Utf8Bytes,
};

use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::database::{Database, PhotoSyncInfo};
use crate::mesh::{IncomingFile, MeshManager, SyncEvent, SyncMessage, PROTOCOL_VERSION};
use crate::signal::SignalMessage;

type WsWrite = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    WsMessage,
>;
#[allow(dead_code)]
type WsRead = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

pub struct MeshTransport {
    pub room_id: String,
    pub is_initiator: bool,
    pub signaling_url: String,
    pub config_path: String,
    pub device_id: String,
    pub device_name: String,
    pub device_os: String,
    pub models_enabled: Vec<String>,
    pub event: Arc<dyn SyncEvent>,
    sync_tx: Arc<Mutex<Option<UnboundedSender<SyncMessage>>>>,
    external_tx: Option<Arc<Mutex<Option<UnboundedSender<SyncMessage>>>>>,
}

impl MeshTransport {
    pub fn new(
        room_id: String,
        is_initiator: bool,
        signaling_url: String,
        config_path: String,
        device_id: String,
        device_name: String,
        models_enabled: Vec<String>,
        event: Arc<dyn SyncEvent>,
    ) -> Self {
        Self {
            room_id,
            is_initiator,
            signaling_url,
            config_path,
            device_id,
            device_name,
            device_os: std::env::consts::OS.to_string(),
            models_enabled,
            event,
            sync_tx: Arc::new(Mutex::new(None)),
            external_tx: None,
        }
    }

    pub fn with_external_tx(
        mut self,
        external_tx: Arc<Mutex<Option<UnboundedSender<SyncMessage>>>>,
    ) -> Self {
        self.external_tx = Some(external_tx);
        self
    }

    pub async fn send_message(
        &self,
        msg: SyncMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tx = self.sync_tx.lock().await;
        if let Some(tx) = tx.as_ref() {
            tx.send(msg).ok();
            Ok(())
        } else {
            Err("Sync channel not initialized".into())
        }
    }

    pub async fn start_lan_server(
        signaling_port: u16,
    ) -> Result<crate::lan_server::LanServer, Box<dyn std::error::Error + Send + Sync>> {
        if signaling_port > 0 {
            Ok(crate::lan_server::LanServer::new(signaling_port))
        } else {
            Ok(crate::lan_server::start(0).await)
        }
    }

    pub async fn start_lan(
        &mut self,
        signaling_port: u16,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let server = Self::start_lan_server(signaling_port).await?;
        let port = server.port;
        self.signaling_url = format!("ws://127.0.0.1:{}", port);
        self.event
            .on_log(&format!("Local signaling server started on port {port}"));
        self.start().await
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.event.on_log(&format!(
            "DEBUG start() called: room_id={} is_init={} url={} device={}",
            self.room_id, self.is_initiator, self.signaling_url, self.device_id
        ));
        if self.room_id.is_empty() {
            let err = "Room ID is missing".to_string();
            self.event.on_state_change(&err);
            return Err(err.into());
        }

        self.event.on_log(&format!(
            "Connecting to signaling at {} for room {}",
            self.signaling_url, self.room_id
        ));
        self.event.on_state_change("Connecting to signaling...");

        let is_remote = self.signaling_url.contains("wss://");

        let base_url = self.signaling_url.trim_end_matches('/');

        let connect_fut = if is_remote {
            connect_async(base_url.to_string().into_client_request()?)
        } else {
            connect_async(format!("{}/{}", base_url, self.room_id).into_client_request()?)
        };

        let (ws_stream, _) = tokio::time::timeout(Duration::from_secs(10), connect_fut)
            .await
            .map_err(|_| {
                let err = "Signaling connection failed: timed out after 10s".to_string();
                self.event.on_state_change(&err);
                err
            })?
            .map_err(|e| {
                let err = format!("Signaling connection failed: {e}");
                self.event.on_state_change(&err);
                err
            })?;

        self.event.on_log("DEBUG WebSocket connected!");
        if !is_remote {
            self.event
                .on_state_change("Connected to signaling. Waiting for peer...");
        }

        let (ws_write, ws_read) = ws_stream.split();
        let ws_write = Arc::new(Mutex::new(ws_write));

        let incoming_files: Arc<Mutex<HashMap<String, IncomingFile>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_manifest: Arc<Mutex<Vec<PhotoSyncInfo>>> = Arc::new(Mutex::new(Vec::new()));
        let transfer_semaphore: Arc<tokio::sync::Semaphore> =
            Arc::new(tokio::sync::Semaphore::new(4));
        let items_completed = Arc::new(AtomicUsize::new(0));
        let items_total = Arc::new(AtomicUsize::new(0));
        let pending_ice: Arc<Mutex<Vec<RTCIceCandidateInit>>> = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<SyncMessage>();
        *self.sync_tx.lock().await = Some(tx.clone());
        if let Some(ext) = &self.external_tx {
            *ext.lock().await = Some(tx);
        }
        let sync_rx = Arc::new(Mutex::new(rx));

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
        let pc = Arc::new(api.new_peer_connection(config).await?);

        let event = Arc::clone(&self.event);

        pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            let event = Arc::clone(&event);
            Box::pin(async move {
                event.on_log(&format!("Peer Connection State changed to: {s:?}"));
                let status = match s {
                    RTCPeerConnectionState::Connected => "Connected",
                    RTCPeerConnectionState::Connecting => "Connecting WebRTC...",
                    RTCPeerConnectionState::Disconnected => "Peer Disconnected",
                    RTCPeerConnectionState::Failed => "Connection Failed",
                    RTCPeerConnectionState::New => "Waiting for peer...",
                    _ => "Awaiting connection...",
                };
                event.on_state_change(status);
                if matches!(
                    s,
                    RTCPeerConnectionState::Disconnected | RTCPeerConnectionState::Failed
                ) {
                    event.on_peer_offline();
                }
            })
        }));

        let ws_ice = Arc::clone(&ws_write);
        let is_remote_ice = is_remote;

        pc.on_ice_candidate(Box::new(move |c: Option<webrtc::ice_transport::ice_candidate::RTCIceCandidate>| {
            if let Some(c) = c {
                let ws = Arc::clone(&ws_ice);
                let remote = is_remote_ice;
                tokio::spawn(async move {
                    if let Ok(json) = c.to_json() {
                        if let Ok(payload) = serde_json::to_string(&json) {
                            let msg = if remote {
                                SignalMessage::Relay {
                                    from: None,
                                    payload: serde_json::json!({"type": "ice_candidate", "payload": payload, "target": "peer"}),
                                }
                            } else {
                                SignalMessage::IceCandidate { payload, target: "peer".to_string() }
                            };
                            if let Ok(msg_str) = serde_json::to_string(&msg) {
                                let _ = ws.lock().await.send(WsMessage::Text(Utf8Bytes::from(msg_str))).await;
                            }
                        }
                    }
                });
            }
            Box::pin(async move {})
        }));

        let event_dc = Arc::clone(&self.event);
        let config_path_dc = self.config_path.clone();
        let incoming_files_dc = Arc::clone(&incoming_files);
        let pending_manifest_dc = Arc::clone(&pending_manifest);
        let items_completed_dc = Arc::clone(&items_completed);
        let items_total_dc = Arc::clone(&items_total);
        let device_id_dc = self.device_id.clone();
        let device_name_dc = self.device_name.clone();
        let device_os_dc = self.device_os.clone();
        let models_enabled_dc = self.models_enabled.clone();

        if self.is_initiator {
            let dc = pc.create_data_channel("file_transfer", None).await?;

            let dc_on_open = Arc::clone(&dc);
            let sync_rx_on_open = Arc::clone(&sync_rx);
            let event_on_open = Arc::clone(&event_dc);
            let device_id_on = device_id_dc.clone();
            let device_name_on = device_name_dc.clone();
            let device_os_on = device_os_dc.clone();
            let models_on = models_enabled_dc.clone();

            dc.on_open(Box::new(move || {
                let dc = Arc::clone(&dc_on_open);
                let sync_rx = Arc::clone(&sync_rx_on_open);
                let event = Arc::clone(&event_on_open);
                let device_id = device_id_on.clone();
                let device_name = device_name_on.clone();
                let device_os = device_os_on.clone();
                let models = models_on.clone();
                Box::pin(async move {
                    event.on_state_change("Secure Data Channel Ready");
                    event.on_log("DEBUG [initiator] data channel OPENED");
                    let _ = MeshManager::send_sync_message(
                        &dc,
                        &SyncMessage::VersionNegotiate {
                            version: PROTOCOL_VERSION,
                            device_id,
                            device_name,
                            os: device_os,
                            models_enabled: models,
                        },
                    )
                    .await;
                    event.on_log("DEBUG [initiator] sent VersionNegotiate");
                    let _ =
                        MeshManager::send_sync_message(&dc, &SyncMessage::ManifestRequest).await;
                    event.on_log("DEBUG [initiator] sent ManifestRequest");
                    let _ = MeshManager::send_sync_message(&dc, &SyncMessage::CatchUp).await;
                    event.on_log("DEBUG [initiator] sent CatchUp");
                    let mut rx = sync_rx.lock().await;
                    while let Some(msg) = rx.recv().await {
                        event.on_log(&format!(
                            "DEBUG [initiator] forwarding sync msg: {:?}",
                            std::mem::discriminant(&msg)
                        ));
                        let _ = MeshManager::send_sync_message(&dc, &msg).await;
                    }
                })
            }));

            let dc_msg = Arc::clone(&dc);
            let incoming_msg = Arc::clone(&incoming_files_dc);
            let pending_msg = Arc::clone(&pending_manifest_dc);
            let transfer_msg = Arc::clone(&transfer_semaphore);
            let config_msg = config_path_dc.clone();
            let event_msg = Arc::clone(&event_dc);
            let completed_msg = Arc::clone(&items_completed_dc);
            let total_msg = Arc::clone(&items_total_dc);

            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let dc = Arc::clone(&dc_msg);
                let incoming = Arc::clone(&incoming_msg);
                let pending = Arc::clone(&pending_msg);
                let transfer = Arc::clone(&transfer_msg);
                let config = config_msg.clone();
                let event = Arc::clone(&event_msg);
                let completed = Arc::clone(&completed_msg);
                let total = Arc::clone(&total_msg);
                Box::pin(async move {
                    let text = String::from_utf8_lossy(&msg.data);
                    event.on_log("DEBUG [initiator] received message over data channel");
                    if let Ok(sync_msg) = serde_json::from_str::<SyncMessage>(&text) {
                        MeshManager::handle_sync_message(
                            sync_msg, &dc, &incoming, &pending, &transfer, &config, event,
                            &completed, &total,
                        )
                        .await;
                    }
                })
            }));
        } else {
            let incoming_rcv = Arc::clone(&incoming_files_dc);
            let pending_rcv = Arc::clone(&pending_manifest_dc);
            let transfer_rcv = Arc::clone(&transfer_semaphore);
            let config_rcv = config_path_dc.clone();
            let event_rcv = Arc::clone(&event_dc);
            let completed_rcv = Arc::clone(&items_completed_dc);
            let total_rcv = Arc::clone(&items_total_dc);
            let sync_rx_rcv = Arc::clone(&sync_rx);
            let device_id_rcv = device_id_dc.clone();
            let device_name_rcv = device_name_dc.clone();
            let device_os_rcv = device_os_dc.clone();
            let models_rcv = models_enabled_dc.clone();

            pc.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
                let incoming = Arc::clone(&incoming_rcv);
                let pending = Arc::clone(&pending_rcv);
                let transfer = Arc::clone(&transfer_rcv);
                let config = config_rcv.clone();
                let event = Arc::clone(&event_rcv);
                let completed = Arc::clone(&completed_rcv);
                let total = Arc::clone(&total_rcv);
                let sync_rx_inner = Arc::clone(&sync_rx_rcv);

                let dc_msg = Arc::clone(&d);
                let incoming_msg = Arc::clone(&incoming);
                let pending_msg = Arc::clone(&pending);
                let transfer_msg = Arc::clone(&transfer);
                let config_msg = config.clone();
                let event_msg = Arc::clone(&event);
                let completed_msg = Arc::clone(&completed);
                let total_msg = Arc::clone(&total);

                d.on_message(Box::new(move |msg: DataChannelMessage| {
                    let dc = Arc::clone(&dc_msg);
                    let incoming = Arc::clone(&incoming_msg);
                    let pending = Arc::clone(&pending_msg);
                    let transfer = Arc::clone(&transfer_msg);
                    let config = config_msg.clone();
                    let event = Arc::clone(&event_msg);
                    let completed = Arc::clone(&completed_msg);
                    let total = Arc::clone(&total_msg);
                    Box::pin(async move {
                        let text = String::from_utf8_lossy(&msg.data);
                        event.on_log("DEBUG [receiver] received message over data channel");
                        if let Ok(sync_msg) = serde_json::from_str::<SyncMessage>(&text) {
                            MeshManager::handle_sync_message(
                                sync_msg, &dc, &incoming, &pending, &transfer, &config, event,
                                &completed, &total,
                            )
                            .await;
                        }
                    })
                }));

                let dc_open = Arc::clone(&d);
                let sync_rx_final = Arc::clone(&sync_rx_inner);
                let event_open = Arc::clone(&event);
                let device_id = device_id_rcv.clone();
                let device_name = device_name_rcv.clone();
                let device_os = device_os_rcv.clone();
                let models = models_rcv.clone();

                d.on_open(Box::new(move || {
                    let dc = Arc::clone(&dc_open);
                    let sync_rx = Arc::clone(&sync_rx_final);
                    let event_open = Arc::clone(&event_open);
                    Box::pin(async move {
                        event_open.on_log("DEBUG [receiver] data channel OPENED");
                        let _ = MeshManager::send_sync_message(
                            &dc,
                            &SyncMessage::VersionNegotiate {
                                version: PROTOCOL_VERSION,
                                device_id,
                                device_name,
                                os: device_os,
                                models_enabled: models,
                            },
                        )
                        .await;
                        event_open.on_log("DEBUG [receiver] sent VersionNegotiate");
                        let _ = MeshManager::send_sync_message(&dc, &SyncMessage::ManifestRequest)
                            .await;
                        event_open.on_log("DEBUG [receiver] sent ManifestRequest");
                        let _ = MeshManager::send_sync_message(&dc, &SyncMessage::CatchUp).await;
                        event_open.on_log("DEBUG [receiver] sent CatchUp");
                        let mut rx = sync_rx.lock().await;
                        while let Some(msg) = rx.recv().await {
                            event_open.on_log(&format!(
                                "DEBUG [receiver] forwarding sync msg: {:?}",
                                std::mem::discriminant(&msg)
                            ));
                            let _ = MeshManager::send_sync_message(&dc, &msg).await;
                        }
                    })
                }));

                Box::pin(async move {})
            }));
        }

        self.event.on_log("DEBUG entering signaling loop");
        let mut read = ws_read;

        if is_remote {
            if self.is_initiator {
                self.send_signal(
                    &ws_write,
                    &SignalMessage::JoinRoom {
                        code: self.room_id.clone(),
                    },
                )
                .await?;
            } else {
                self.send_signal(&ws_write, &SignalMessage::CreateRoom)
                    .await?;
            }
        } else {
            self.event
                .on_log(&format!("DEBUG sending Join device_id={}", self.device_id));
            self.send_signal(
                &ws_write,
                &SignalMessage::Join {
                    device_id: self.device_id.clone(),
                },
            )
            .await?;
            self.event.on_log("DEBUG Join sent successfully");
        }

        // A LAN joiner (initiator) must see a peer shortly after joining. If the
        // connected server is orphaned/stale (e.g. a previous host incarnation),
        // no peer ever appears and the UI would spin on "joining" forever. Arm a
        // short watchdog that surfaces a rescan-and-retry error instead.
        let join_watchdog: Option<Arc<AtomicBool>> = if !is_remote && self.is_initiator {
            let has_peer = Arc::new(AtomicBool::new(false));
            let peer_flag = Arc::clone(&has_peer);
            let event = Arc::clone(&self.event);
            self.event
                .on_log("DEBUG [initiator] armed join watchdog (5s)");
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if !peer_flag.load(Ordering::Relaxed) {
                    event.on_state_change(
                        "Join failed: no host session found on this port - rescan and retry",
                    );
                }
            });
            Some(has_peer)
        } else {
            None
        };
        let mark_peer = || {
            if let Some(flag) = &join_watchdog {
                flag.store(true, Ordering::Relaxed);
            }
        };

        while let Some(msg) = read.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    let signal: SignalMessage = match serde_json::from_str(&text) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    match signal {
                        SignalMessage::Joined { peer_count, .. } if !is_remote => {
                            mark_peer();
                            eprintln!(
                                "[siegu-transport] received Joined peer_count={} is_init={}",
                                peer_count, self.is_initiator
                            );
                            if peer_count > 1 {
                                self.event.on_state_change("Peer Joined");
                            }
                            if self.is_initiator && peer_count > 1 {
                                eprintln!("[siegu-transport] creating offer...");
                                let offer = pc.create_offer(None).await?;
                                eprintln!("[siegu-transport] offer created, setting local desc");
                                pc.set_local_description(offer.clone()).await?;
                                eprintln!("[siegu-transport] sending offer");
                                self.send_signal(
                                    &ws_write,
                                    &SignalMessage::Offer {
                                        payload: serde_json::to_string(&offer)?,
                                        target: "peer".to_string(),
                                    },
                                )
                                .await?;
                                eprintln!("[siegu-transport] offer sent");
                            }
                        }
                        SignalMessage::PeerJoined { .. } if !is_remote => {
                            mark_peer();
                            eprintln!("[siegu-transport] received PeerJoined");
                            self.event.on_state_change("Peer Joined");
                            if self.is_initiator {
                                let offer = pc.create_offer(None).await?;
                                pc.set_local_description(offer.clone()).await?;
                                self.send_signal(
                                    &ws_write,
                                    &SignalMessage::Offer {
                                        payload: serde_json::to_string(&offer)?,
                                        target: "peer".to_string(),
                                    },
                                )
                                .await?;
                            }
                        }
                        SignalMessage::Offer { payload, .. } if !is_remote => {
                            mark_peer();
                            eprintln!("[siegu-transport] received Offer");
                            let sdp: RTCSessionDescription = serde_json::from_str(&payload)?;
                            pc.set_remote_description(sdp).await?;
                            {
                                let mut ice = pending_ice.lock().await;
                                for c in ice.drain(..) {
                                    let _ = pc.add_ice_candidate(c).await;
                                }
                            }
                            let answer = pc.create_answer(None).await?;
                            pc.set_local_description(answer.clone()).await?;
                            self.send_signal(
                                &ws_write,
                                &SignalMessage::Answer {
                                    payload: serde_json::to_string(&answer)?,
                                    target: "peer".to_string(),
                                },
                            )
                            .await?;
                        }
                        SignalMessage::Answer { payload, .. } if !is_remote => {
                            mark_peer();
                            eprintln!("[siegu-transport] received Answer");
                            if let Ok(sdp) = serde_json::from_str::<RTCSessionDescription>(&payload)
                            {
                                pc.set_remote_description(sdp).await?;
                                let mut ice = pending_ice.lock().await;
                                for c in ice.drain(..) {
                                    let _ = pc.add_ice_candidate(c).await;
                                }
                            }
                        }
                        SignalMessage::IceCandidate { payload, .. } if !is_remote => {
                            mark_peer();
                            eprintln!("[siegu-transport] received ICE candidate");
                            if let Ok(candidate) =
                                serde_json::from_str::<RTCIceCandidateInit>(&payload)
                            {
                                if pc.remote_description().await.is_none() {
                                    pending_ice.lock().await.push(candidate);
                                } else {
                                    let _ = pc.add_ice_candidate(candidate).await;
                                }
                            }
                        }
                        SignalMessage::RoomJoined if is_remote => {
                            self.event
                                .on_state_change("Room joined. Waiting for peer...");
                        }
                        SignalMessage::RoomCreated { code } if is_remote => {
                            self.event.on_log(&format!("Room code: {code}"));
                            self.event.on_state_change("Waiting for peer to join...");
                        }
                        SignalMessage::Relay { payload, .. } if is_remote => {
                            if let Some(msg_type) = payload.get("type").and_then(|v| v.as_str()) {
                                match msg_type {
                                    "offer" => {
                                        if let Some(sdp_str) =
                                            payload.get("payload").and_then(|v| v.as_str())
                                        {
                                            if let Ok(sdp) =
                                                serde_json::from_str::<RTCSessionDescription>(
                                                    sdp_str,
                                                )
                                            {
                                                pc.set_remote_description(sdp).await?;
                                                let answer = pc.create_answer(None).await?;
                                                pc.set_local_description(answer.clone()).await?;
                                                let answer_payload = serde_json::json!({
                                                    "type": "answer",
                                                    "payload": serde_json::to_string(&answer)?,
                                                    "target": "peer"
                                                });
                                                self.send_signal(
                                                    &ws_write,
                                                    &SignalMessage::Relay {
                                                        from: None,
                                                        payload: answer_payload,
                                                    },
                                                )
                                                .await?;
                                            }
                                        }
                                    }
                                    "answer" => {
                                        if let Some(sdp_str) =
                                            payload.get("payload").and_then(|v| v.as_str())
                                        {
                                            if let Ok(sdp) =
                                                serde_json::from_str::<RTCSessionDescription>(
                                                    sdp_str,
                                                )
                                            {
                                                pc.set_remote_description(sdp).await?;
                                                let mut ice = pending_ice.lock().await;
                                                for c in ice.drain(..) {
                                                    let _ = pc.add_ice_candidate(c).await;
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        SignalMessage::PeerJoined { .. } if is_remote => {
                            self.event.on_state_change("Peer Joined");
                            if self.is_initiator {
                                let offer = pc.create_offer(None).await?;
                                pc.set_local_description(offer.clone()).await?;
                                let offer_payload = serde_json::json!({
                                    "type": "offer",
                                    "payload": serde_json::to_string(&offer)?,
                                    "target": "peer"
                                });
                                self.send_signal(
                                    &ws_write,
                                    &SignalMessage::Relay {
                                        from: None,
                                        payload: offer_payload,
                                    },
                                )
                                .await?;
                            }
                        }
                        SignalMessage::PeerDisconnected { .. } => {
                            self.event.on_state_change("Peer disconnected");
                            self.event.on_peer_offline();
                        }
                        SignalMessage::RoomClosed => {
                            self.event.on_state_change("Room closed");
                        }
                        SignalMessage::Error { message } => {
                            eprintln!("[siegu-transport] Signaling error: {message}");
                            self.event
                                .on_state_change(&format!("Signaling error: {message}"));
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    eprintln!("[siegu-transport] WebSocket error: {e}");
                    self.event.on_state_change(&format!("WebSocket error: {e}"));
                    break;
                }
                _ => {}
            }
        }

        self.event.on_log("Sync session ended");
        Ok(())
    }

    async fn send_signal(
        &self,
        write: &Arc<Mutex<WsWrite>>,
        msg: &SignalMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let json = serde_json::to_string(msg)?;
        write
            .lock()
            .await
            .send(WsMessage::Text(Utf8Bytes::from(json)))
            .await?;
        Ok(())
    }

    pub async fn cleanup_temp_files(&self) {
        MeshManager::cleanup_temp_files(&self.config_path).await;
    }

    pub fn check_storage_quota(&self, additional_bytes: u64) -> bool {
        MeshManager::check_storage_quota(&self.config_path, additional_bytes)
    }

    pub fn get_storage_quota(&self) -> u64 {
        MeshManager::get_storage_quota(&self.config_path)
    }

    pub fn get_storage_used(&self) -> u64 {
        MeshManager::get_storage_used(&self.config_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{SyncEvent, SyncProgress, PROTOCOL_VERSION};

    struct TestEvent;

    impl SyncEvent for TestEvent {
        fn on_state_change(&self, _state: &str) {}
        fn on_log(&self, _msg: &str) {}
        fn on_sync_progress(&self, _p: SyncProgress) {}
        fn on_photo_received(&self, _id: String, _path: String) {}
        fn on_sync_error(&self, _e: String) {}
        fn on_peer_connected(
            &self,
            _id: String,
            _name: String,
            _os: String,
            _models: Vec<String>,
            _version: u8,
        ) {
        }
        fn on_peer_disconnected(&self, _id: String) {}
        fn on_device_registered(&self, _db: &Database) {}
        fn on_metadata_updated(
            &self,
            _photo_id: &str,
            _caption: Option<&str>,
            _aesthetics_score: Option<f64>,
        ) {
        }
        fn get_config_path(&self) -> String {
            "/tmp/test".into()
        }
        fn get_sync_path(&self) -> Option<String> {
            None
        }
        fn get_directories(&self) -> Vec<String> {
            Vec::new()
        }
    }

    #[test]
    fn test_new() {
        let transport = MeshTransport::new(
            "test-room".into(),
            true,
            "ws://127.0.0.1:9876".into(),
            "/tmp/test".into(),
            "dev-1".into(),
            "TestDevice".into(),
            vec!["caption".into()],
            Arc::new(TestEvent),
        );
        assert_eq!(transport.room_id, "test-room");
        assert!(transport.is_initiator);
    }
}
