use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

use crate::database::PhotoSyncInfo;
use crate::mesh::{IncomingFile, MeshManager, SyncEvent, SyncMessage, PROTOCOL_VERSION};
use crate::signal::SignalMessage;

type WsWrite = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    WsMessage,
>;
type WsRead = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

/// Split `?token=...` out of a signalling URL so it can be sent in the join
/// message body instead of the WebSocket handshake.
fn extract_token(url: &str) -> (String, Option<String>) {
    match url.split_once('?') {
        Some((base, query)) => {
            let token = url::form_urlencoded::parse(query.as_bytes())
                .find(|(k, _)| k == "token")
                .map(|(_, v)| v.into_owned());
            (base.trim_end_matches('/').to_string(), token)
        }
        None => (url.trim_end_matches('/').to_string(), None),
    }
}

const HEARTBEAT_INTERVAL_MS: u64 = 5_000;
/// A peer that has produced no inbound frame for this long is treated as
/// dead and the link is torn down (see also the PC-state Disconnected/Failed
/// handling which reports the same condition for hard failures).
const HEARTBEAT_TIMEOUT_MS: u64 = 15_000;

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone)]
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
    /// RPC permission for browser guests (#19). `None` (the default) rejects
    /// CommandRequest messages entirely; device-to-device mesh sessions never
    /// opt in, only `siegu web --share-mode ...` does.
    share_mode: Option<crate::rpc::ShareMode>,
    /// When set, the receiver branch skips ManifestRequest/CatchUp on channel
    /// open: the guest drives EnterViewOnly/RPC itself and must never trigger
    /// a full sync push from the sharer (#9/#19).
    view_only_client: bool,
}

/// What the signalling loop needs from a live guest connection. All the
/// heavier state (file buffers, counters, semaphores) lives exclusively
/// inside the data-channel closures, so a session is cheap to keep.
struct PeerSession {
    pc: Arc<webrtc::peer_connection::RTCPeerConnection>,
    pending_ice: Arc<Mutex<Vec<RTCIceCandidateInit>>>,
}

impl MeshTransport {
    #[allow(clippy::too_many_arguments)]
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
            share_mode: None,
            view_only_client: false,
        }
    }

    pub fn with_external_tx(
        mut self,
        external_tx: Arc<Mutex<Option<UnboundedSender<SyncMessage>>>>,
    ) -> Self {
        self.external_tx = Some(external_tx);
        self
    }

    /// Enable RPC dispatch for this session with the given permission level
    /// (#19). Only web-share hosts should call this.
    pub fn with_share_mode(mut self, mode: crate::rpc::ShareMode) -> Self {
        self.share_mode = Some(mode);
        self
    }

    pub fn with_view_only_client(mut self, flag: bool) -> Self {
        self.view_only_client = flag;
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

    /// ICE configuration: Google STUN by default, plus a TURN server when
    /// `SIEGU_TURN_URLS` is set (comma-separated URLs), with optional
    /// `SIEGU_TURN_USERNAME` / `SIEGU_TURN_CREDENTIAL` (#16). TURN lets
    /// guests on cellular networks reach a home NAT.
    fn rtc_configuration() -> RTCConfiguration {
        let mut ice_servers = vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            username: String::new(),
            credential: String::new(),
        }];
        if let Ok(turn_urls) = std::env::var("SIEGU_TURN_URLS") {
            let urls: Vec<String> = turn_urls
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
            if !urls.is_empty() {
                ice_servers.push(RTCIceServer {
                    urls,
                    username: std::env::var("SIEGU_TURN_USERNAME").unwrap_or_default(),
                    credential: std::env::var("SIEGU_TURN_CREDENTIAL").unwrap_or_default(),
                });
            }
        }
        RTCConfiguration {
            ice_servers,
            ..Default::default()
        }
    }

    /// Create the WebRTC pieces for one guest: a dedicated PC whose ICE
    /// candidates are signalled back to that guest only, the standard
    /// "file_transfer" data-channel pump, and (optionally) our SDP offer
    /// targeted at them. Sessions are registered under `peer_key` by the
    /// caller.
    #[allow(clippy::too_many_lines)]
    async fn spawn_host_session(
        &self,
        ws_write: &Arc<Mutex<WsWrite>>,
        is_remote: bool,
        peer_key: String,
        with_offer: bool,
    ) -> Result<PeerSession, Box<dyn std::error::Error + Send + Sync>> {
        let mut m = MediaEngine::default();
        m.register_default_codecs()?;
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;
        let pc = Arc::new(
            APIBuilder::new()
                .with_media_engine(m)
                .with_interceptor_registry(registry)
                .build()
                .new_peer_connection(Self::rtc_configuration())
                .await?,
        );

        let event_pc = Arc::clone(&self.event);
        pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            let event = Arc::clone(&event_pc);
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
                    // Parity with the guest loop: tear the peer down so the
                    // host clears its "Connected" state and surfaces the
                    // reconnect path instead of appearing alive forever.
                    event.on_peer_offline();
                }
            })
        }));

        // This guest's ICE candidates go back to this guest only.
        let ws_ice = Arc::clone(ws_write);
        let event_ice = Arc::clone(&self.event);
        let ice_target = if peer_key == "peer" {
            "peer".to_string()
        } else {
            peer_key.clone()
        };
        let remote_ice = is_remote;
        pc.on_ice_candidate(Box::new(move |c: Option<webrtc::ice_transport::ice_candidate::RTCIceCandidate>| {
            let ws = Arc::clone(&ws_ice);
            let event = Arc::clone(&event_ice);
            let target = ice_target.clone();
            Box::pin(async move {
                if let Some(c) = c {
                    if let Ok(json) = c.to_json() {
                        event.on_log(&format!("ICE candidate (send): {}", json.candidate));
                        if let Ok(payload) = serde_json::to_string(&json) {
                            let msg = if remote_ice {
                                SignalMessage::Relay {
                                    from: None,
                                    payload: serde_json::json!(
                                        {"type": "ice_candidate", "payload": payload, "target": target}
                                    ),
                                }
                            } else {
                                SignalMessage::IceCandidate {
                                    payload,
                                    target: target.clone(),
                                    from: None,
                                }
                            };
                            if let Ok(msg_str) = serde_json::to_string(&msg) {
                                let _ =
                                    ws.lock().await.send(WsMessage::Text(Utf8Bytes::from(msg_str))).await;
                            }
                        }
                    }
                }
            })
        }));

        let dc = Arc::new(pc.create_data_channel("file_transfer", None).await?);

        // Outbound queue: CLI callers push SyncMessages through here and the
        // on-open pump drains them onto this guest's channel. Binding per
        // session means the most recent guest wins the public send_message /
        // /remote restore path; proper per-peer fan-out lands with #19
        // Phase 1.
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<SyncMessage>();
        *self.sync_tx.lock().await = Some(tx.clone());
        if let Some(ext) = &self.external_tx {
            *ext.lock().await = Some(tx.clone());
        }
        crate::view_only::state().bind_session(tx);

        let incoming_files = Arc::new(Mutex::new(HashMap::new()));
        let pending_manifest = Arc::new(Mutex::new(Vec::new()));
        let pending_view_manifest = Arc::new(Mutex::new(Vec::new()));
        let transfer_semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let items_completed = Arc::new(AtomicUsize::new(0));
        let items_total = Arc::new(AtomicUsize::new(0));
        let mirror_completed = Arc::new(AtomicUsize::new(0));
        let mirror_total = Arc::new(AtomicUsize::new(0));

        // Heartbeat liveness (#heartbeat): millis of last inbound frame.
        let last_seen: Arc<std::sync::atomic::AtomicU64> =
            Arc::new(std::sync::atomic::AtomicU64::new(now_millis()));

        let dc_open = Arc::clone(&dc);
        let sync_rx_open = Arc::new(Mutex::new(rx));
        let event_open = Arc::clone(&self.event);
        let last_seen_on_open = Arc::clone(&last_seen);
        let device_id_open = self.device_id.clone();
        let device_name_open = self.device_name.clone();
        let device_os_open = self.device_os.clone();
        let models_open = self.models_enabled.clone();
        let skip_pull = self.view_only_client;
        dc.on_open(Box::new(move || {
            let dc = Arc::clone(&dc_open);
            let sync_rx = Arc::clone(&sync_rx_open);
            let event = Arc::clone(&event_open);
            let last_seen_open = Arc::clone(&last_seen_on_open);
            let device_id = device_id_open.clone();
            let device_name = device_name_open.clone();
            let device_os = device_os_open.clone();
            let models = models_open.clone();
            Box::pin(async move {
                event.on_state_change("Secure Data Channel Ready");
                event.on_log("DEBUG [host] data channel OPENED");
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
                event.on_log("DEBUG [host] sent VersionNegotiate");
                // View-only/RPC guests drive their own pulls; a stray
                // ManifestRequest would trigger a full sync push (#9/#19).
                if !skip_pull {
                    let _ =
                        MeshManager::send_sync_message(&dc, &SyncMessage::ManifestRequest).await;
                    event.on_log("DEBUG [host] sent ManifestRequest");
                    let _ = MeshManager::send_sync_message(&dc, &SyncMessage::CatchUp).await;
                    event.on_log("DEBUG [host] sent CatchUp");
                }
                let mut rx = sync_rx.lock().await;
                let mut heartbeat =
                    tokio::time::interval(Duration::from_millis(HEARTBEAT_INTERVAL_MS));
                heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    tokio::select! {
                        msg = rx.recv() => {
                            let Some(msg) = msg else { break; };
                            let _ = MeshManager::send_sync_message(&dc, &msg).await;
                        }
                        _ = heartbeat.tick() => {
                            let _ =
                                MeshManager::send_sync_message(&dc, &SyncMessage::Ping).await;
                            let now = now_millis();
                            if now.saturating_sub(last_seen_open.load(Ordering::Relaxed))
                                > HEARTBEAT_TIMEOUT_MS
                            {
                                event.on_log("DEBUG [host] heartbeat timeout — peer silent");
                                event.on_peer_offline();
                                break;
                            }
                        }
                    }
                }
            })
        }));

        let dc_msg = Arc::clone(&dc);
        let event_msg = Arc::clone(&self.event);
        let config_msg = self.config_path.clone();
        let share_mode_msg = self.share_mode;
        let session_scope: std::sync::Arc<
            tokio::sync::Mutex<Option<crate::view_only::AlbumScope>>,
        > = std::sync::Arc::new(tokio::sync::Mutex::new(None));
        let last_seen_dc = Arc::clone(&last_seen);
        dc.on_message(Box::new(move |msg: DataChannelMessage| {
            let dc = Arc::clone(&dc_msg);
            let incoming = Arc::clone(&incoming_files);
            let pending = Arc::clone(&pending_manifest);
            let pending_view = Arc::clone(&pending_view_manifest);
            let transfer = Arc::clone(&transfer_semaphore);
            let config = config_msg.clone();
            let event = Arc::clone(&event_msg);
            let completed = Arc::clone(&items_completed);
            let total = Arc::clone(&items_total);
            let mirror_completed = Arc::clone(&mirror_completed);
            let mirror_total = Arc::clone(&mirror_total);
            let last_seen_msg = Arc::clone(&last_seen_dc);
            let session_scope = std::sync::Arc::clone(&session_scope);
            Box::pin(async move {
                let text = String::from_utf8_lossy(&msg.data);
                if let Ok(sync_msg) = serde_json::from_str::<SyncMessage>(&text) {
                    last_seen_msg.store(now_millis(), Ordering::Relaxed);
                    MeshManager::handle_sync_message(
                        sync_msg,
                        &dc,
                        &incoming,
                        &pending,
                        &transfer,
                        &config,
                        event,
                        &completed,
                        &total,
                        &mirror_completed,
                        &mirror_total,
                        &pending_view,
                        share_mode_msg,
                        &session_scope,
                    )
                    .await;
                }
            })
        }));

        if with_offer {
            let offer = pc.create_offer(None).await?;
            pc.set_local_description(offer.clone()).await?;
            let payload = serde_json::to_string(&offer)?;
            let target = if peer_key == "peer" {
                "peer".to_string()
            } else {
                peer_key.clone()
            };
            let msg = if is_remote {
                SignalMessage::Relay {
                    from: None,
                    payload: serde_json::json!({
                        "type": "offer",
                        "payload": payload,
                        "target": target
                    }),
                }
            } else {
                SignalMessage::Offer {
                    payload,
                    target: target.clone(),
                    from: None,
                }
            };
            self.send_signal(ws_write, &msg).await?;
            self.event
                .on_log(&format!("DEBUG [host] offer sent to {target}"));
        }

        Ok(PeerSession {
            pc,
            pending_ice: Arc::new(Mutex::new(Vec::new())),
        })
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

        let is_remote = self.signaling_url.contains("wss://")
            || extract_token(&self.signaling_url).0.ends_with("/ws");

        let (base_url, _token) = extract_token(&self.signaling_url);

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

        // A short-lived LAN joiner watchdog: if the connected server is
        // orphaned/stale no peer ever appears and the UI would spin forever.
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
        // Hosts (and any initiator) keep one session per peer so guests are
        // isolated: a second join works after the first guest left, and
        // simultaneous viewers never share WebRTC state (#16).
        if self.is_initiator {
            let mut sessions: HashMap<String, PeerSession> = HashMap::new();
            self.host_signal_loop(
                ws_write,
                ws_read,
                is_remote,
                Self::mark_peer_active(join_watchdog),
                &mut sessions,
            )
            .await?;
        } else {
            self.guest_signal_loop(
                ws_write,
                ws_read,
                is_remote,
                Self::mark_peer_active(join_watchdog),
            )
            .await?;
        }

        self.event.on_log("Sync session ended");
        // Session over: drop view-only buffers, cache and flags.
        crate::view_only::state().reset_session();
        Ok(())
    }

    fn mark_peer_active(flag: Option<Arc<AtomicBool>>) -> impl Fn() + use<> {
        move || {
            if let Some(f) = &flag {
                f.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Send the protocol frames that announce us to the signalling server.
    /// Remote initiators join an existing room; remote receivers mint one;
    /// LAN peers speak the room-path `Join` dialect.
    async fn send_join_frames(
        &self,
        ws_write: &Arc<Mutex<WsWrite>>,
        is_remote: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (_, token) = extract_token(&self.signaling_url);
        if is_remote {
            if self.is_initiator {
                self.send_signal(
                    ws_write,
                    &SignalMessage::JoinRoom {
                        code: self.room_id.clone(),
                        token: token.clone(),
                    },
                )
                .await?;
            } else {
                self.send_signal(
                    ws_write,
                    &SignalMessage::CreateRoom {
                        token: token.clone(),
                    },
                )
                .await?;
            }
        } else {
            self.send_signal(
                ws_write,
                &SignalMessage::Join {
                    device_id: self.device_id.clone(),
                    token: token.clone(),
                },
            )
            .await?;
        }
        Ok(())
    }

    fn route_key(from: Option<String>) -> String {
        match from {
            Some(f) if !f.is_empty() => f,
            _ => "peer".to_string(),
        }
    }

    /// Look up (or adopt) the session a peer-keyed frame belongs to. When
    /// this host joined the room *after* a peer it only sees an anonymous
    /// `Joined` event, so that first session lives under "peer"; the first
    /// frame bearing a concrete identity re-keys it so teardown and future
    /// joins stay correct.
    fn session_for<'a>(
        sessions: &'a mut HashMap<String, PeerSession>,
        key: &str,
    ) -> Option<&'a mut PeerSession> {
        if !sessions.contains_key(key) {
            if let Some(anon) = sessions.remove("peer") {
                sessions.insert(key.to_string(), anon);
            }
        }
        sessions.get_mut(key)
    }

    /// Guarantee a host-side session exists for `key`, adopting the anonymous
    /// legacy session when possible and creating a fresh one otherwise.
    async fn ensure_host_session(
        sessions: &mut HashMap<String, PeerSession>,
        key: &str,
        transport: &MeshTransport,
        ws_write: &Arc<tokio::sync::Mutex<WsWrite>>,
        is_remote: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !sessions.contains_key(key) && Self::session_for(sessions, key).is_none() {
            let session = transport
                .spawn_host_session(ws_write, is_remote, key.to_string(), false)
                .await?;
            sessions.insert(key.to_string(), session);
        }
        Ok(())
    }

    /// Signalling loop for initiating hosts (#16): every peer gets its own
    /// [`PeerSession`] keyed by the identity the signalling server stamps on
    /// frames (LAN `device_id`, remote conn key). Offers are targeted at one
    /// guest, answers/ICE are routed back to the session that owns them, and
    /// a leaving guest tears down only its own PC — so the next guest joins a
    /// fresh connection instead of a corpse (stale-session bug).
    #[allow(clippy::too_many_lines)]
    async fn host_signal_loop(
        &self,
        ws_write: Arc<Mutex<WsWrite>>,
        mut ws_read: WsRead,
        is_remote: bool,
        mark_peer: impl Fn(),
        sessions: &mut HashMap<String, PeerSession>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.send_join_frames(&ws_write, is_remote).await?;

        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    let signal: SignalMessage = match serde_json::from_str(&text) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    match signal {
                        SignalMessage::Joined { peer_count, .. } if !is_remote => {
                            mark_peer();
                            if peer_count > 1 {
                                self.event.on_state_change("Peer Joined");
                                // Sessions come from PeerJoined frames, which
                                // carry concrete identities (the server
                                // introduces pre-existing peers too).
                            }
                        }
                        SignalMessage::PeerJoined { device_id } => {
                            mark_peer();
                            self.event.on_state_change("Peer Joined");
                            let key = Self::route_key(Some(device_id));
                            if let std::collections::hash_map::Entry::Vacant(e) =
                                sessions.entry(key)
                            {
                                let session = self
                                    .spawn_host_session(&ws_write, is_remote, e.key().clone(), true)
                                    .await?;
                                e.insert(session);
                            }
                        }
                        SignalMessage::Offer { payload, from, .. } => {
                            mark_peer();
                            let key = Self::route_key(from);
                            Self::ensure_host_session(sessions, &key, self, &ws_write, is_remote)
                                .await?;
                            let Some(session) = sessions.get(&key) else {
                                continue;
                            };
                            let sdp: RTCSessionDescription = serde_json::from_str(&payload)?;
                            session.pc.set_remote_description(sdp).await?;
                            {
                                let mut ice = session.pending_ice.lock().await;
                                for c in ice.drain(..) {
                                    let _ = session.pc.add_ice_candidate(c).await;
                                }
                            }
                            let answer = session.pc.create_answer(None).await?;
                            session.pc.set_local_description(answer.clone()).await?;
                            let answer_payload = serde_json::to_string(&answer)?;
                            if is_remote {
                                self.send_signal(
                                    &ws_write,
                                    &SignalMessage::Relay {
                                        from: None,
                                        payload: serde_json::json!({
                                            "type": "answer",
                                            "payload": answer_payload,
                                            "target": key
                                        }),
                                    },
                                )
                                .await?;
                            } else {
                                self.send_signal(
                                    &ws_write,
                                    &SignalMessage::Answer {
                                        payload: answer_payload,
                                        target: key,
                                        from: None,
                                    },
                                )
                                .await?;
                            }
                        }
                        SignalMessage::Answer { payload, from, .. } => {
                            mark_peer();
                            let key = Self::route_key(from);
                            let Some(session) = Self::session_for(sessions, &key) else {
                                self.event
                                    .on_log(&format!("WARN answer from unknown peer '{key}'"));
                                continue;
                            };
                            if let Ok(sdp) = serde_json::from_str::<RTCSessionDescription>(&payload)
                            {
                                session.pc.set_remote_description(sdp).await?;
                                let mut ice = session.pending_ice.lock().await;
                                for c in ice.drain(..) {
                                    let _ = session.pc.add_ice_candidate(c).await;
                                }
                            }
                        }
                        SignalMessage::IceCandidate { payload, from, .. } => {
                            mark_peer();
                            let key = Self::route_key(from);
                            let Some(session) = Self::session_for(sessions, &key) else {
                                self.event.on_log(&format!(
                                    "WARN ICE candidate from unknown peer '{key}'"
                                ));
                                continue;
                            };
                            if let Ok(candidate) =
                                serde_json::from_str::<RTCIceCandidateInit>(&payload)
                            {
                                self.event.on_log(&format!(
                                    "ICE candidate (recv): {}",
                                    candidate.candidate
                                ));
                                if session.pc.remote_description().await.is_none() {
                                    session.pending_ice.lock().await.push(candidate);
                                } else {
                                    let _ = session.pc.add_ice_candidate(candidate).await;
                                }
                            }
                        }
                        SignalMessage::Relay { from, payload } if is_remote => {
                            let key = Self::route_key(from);
                            let msg_type =
                                payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            match msg_type {
                                // Guest-initiated offer (legacy remote flow):
                                // give it its own session and answer there.
                                "offer" => {
                                    let Some(sdp_str) =
                                        payload.get("payload").and_then(|v| v.as_str())
                                    else {
                                        continue;
                                    };
                                    Self::ensure_host_session(
                                        sessions, &key, self, &ws_write, is_remote,
                                    )
                                    .await?;
                                    let Some(session) = sessions.get(&key) else {
                                        continue;
                                    };
                                    let Ok(sdp) =
                                        serde_json::from_str::<RTCSessionDescription>(sdp_str)
                                    else {
                                        continue;
                                    };
                                    session.pc.set_remote_description(sdp).await?;
                                    {
                                        let mut ice = session.pending_ice.lock().await;
                                        for c in ice.drain(..) {
                                            let _ = session.pc.add_ice_candidate(c).await;
                                        }
                                    }
                                    let answer = session.pc.create_answer(None).await?;
                                    session.pc.set_local_description(answer.clone()).await?;
                                    self.send_signal(
                                        &ws_write,
                                        &SignalMessage::Relay {
                                            from: None,
                                            payload: serde_json::json!({
                                                "type": "answer",
                                                "payload": serde_json::to_string(&answer)?,
                                                "target": key
                                            }),
                                        },
                                    )
                                    .await?;
                                }
                                "answer" | "ice_candidate" => {
                                    let Some(session) = Self::session_for(sessions, &key) else {
                                        self.event.on_log(&format!(
                                            "WARN relay {msg_type} from unknown peer '{key}'"
                                        ));
                                        continue;
                                    };
                                    let Some(inner) =
                                        payload.get("payload").and_then(|v| v.as_str())
                                    else {
                                        continue;
                                    };
                                    if msg_type == "answer" {
                                        if let Ok(sdp) =
                                            serde_json::from_str::<RTCSessionDescription>(inner)
                                        {
                                            session.pc.set_remote_description(sdp).await?;
                                            let mut ice = session.pending_ice.lock().await;
                                            for c in ice.drain(..) {
                                                let _ = session.pc.add_ice_candidate(c).await;
                                            }
                                        }
                                    } else if let Ok(candidate) =
                                        serde_json::from_str::<RTCIceCandidateInit>(inner)
                                    {
                                        if session.pc.remote_description().await.is_none() {
                                            session.pending_ice.lock().await.push(candidate);
                                        } else {
                                            let _ = session.pc.add_ice_candidate(candidate).await;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        SignalMessage::RoomJoined => {
                            self.event
                                .on_state_change("Room joined. Waiting for peer...");
                        }
                        SignalMessage::RoomCreated { code } => {
                            self.event.on_log(&format!("Room code: {code}"));
                            self.event.on_state_change("Waiting for peer to join...");
                        }
                        SignalMessage::PeerDisconnected { device_id } => {
                            let key = Self::route_key(Some(device_id));
                            if let Some(session) = sessions.remove(&key) {
                                let _ = session.pc.close().await;
                                self.event
                                    .on_log(&format!("DEBUG guest session '{key}' torn down"));
                            }
                            if sessions.is_empty() {
                                self.event.on_state_change("Peer disconnected");
                                self.event.on_peer_offline();
                                // Ephemeral view-only state dies only with the
                                // last guest; remaining viewers keep serving.
                                crate::view_only::state().reset_session();
                            }
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
        // WS gone: every guest session dies with it.
        for (_, session) in sessions.drain() {
            let _ = session.pc.close().await;
        }
        Ok(())
    }

    /// Legacy single-peer loop for joining/receiving guests: one PC, one
    /// data channel, exactly the pre-#16 behavior.
    #[allow(clippy::too_many_lines)]
    async fn guest_signal_loop(
        &self,
        ws_write: Arc<Mutex<WsWrite>>,
        mut ws_read: WsRead,
        is_remote: bool,
        mark_peer: impl Fn(),
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let incoming_files: Arc<Mutex<HashMap<String, IncomingFile>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_manifest: Arc<Mutex<Vec<PhotoSyncInfo>>> = Arc::new(Mutex::new(Vec::new()));
        let pending_view_manifest: Arc<Mutex<Vec<PhotoSyncInfo>>> =
            Arc::new(Mutex::new(Vec::new()));
        let transfer_semaphore: Arc<tokio::sync::Semaphore> =
            Arc::new(tokio::sync::Semaphore::new(1));
        let items_completed = Arc::new(AtomicUsize::new(0));
        let items_total = Arc::new(AtomicUsize::new(0));
        let mirror_completed = Arc::new(AtomicUsize::new(0));
        let mirror_total = Arc::new(AtomicUsize::new(0));
        // Heartbeat liveness (#heartbeat): millis of last inbound frame.
        let last_seen: Arc<std::sync::atomic::AtomicU64> =
            Arc::new(std::sync::atomic::AtomicU64::new(now_millis()));
        let pending_ice: Arc<Mutex<Vec<RTCIceCandidateInit>>> = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<SyncMessage>();
        *self.sync_tx.lock().await = Some(tx.clone());
        if let Some(ext) = &self.external_tx {
            *ext.lock().await = Some(tx.clone());
        }
        // Bind on-demand media pulls (/remote/{id}) to this session: covers
        // view-only browsing (#9) and restore pulls of evicted items (#10).
        crate::view_only::state().bind_session(tx);
        let sync_rx = Arc::new(Mutex::new(rx));

        // In multi-guest rooms the server relays every guest's answer/ICE to
        // all other peers (target "peer" broadcasts), so a guest must only
        // accept frames from the host it actually negotiated with (#16).
        let mut host_key: Option<String> = None;

        let mut m = MediaEngine::default();
        m.register_default_codecs()?;
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;
        let pc = Arc::new(
            APIBuilder::new()
                .with_media_engine(m)
                .with_interceptor_registry(registry)
                .build()
                .new_peer_connection(Self::rtc_configuration())
                .await?,
        );

        let event_pc = Arc::clone(&self.event);
        pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            let event = Arc::clone(&event_pc);
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

        let event_ice = Arc::clone(&self.event);
        pc.on_ice_connection_state_change(Box::new(
            move |s: webrtc::ice_transport::ice_connection_state::RTCIceConnectionState| {
                let event = Arc::clone(&event_ice);
                Box::pin(async move {
                    event.on_log(&format!("ICE Connection State changed to: {s:?}"));
                })
            },
        ));

        let ws_ice = Arc::clone(&ws_write);
        let event_send_ice = Arc::clone(&self.event);
        pc.on_ice_candidate(Box::new(move |c: Option<webrtc::ice_transport::ice_candidate::RTCIceCandidate>| {
            if let Some(c) = c {
                let ws = Arc::clone(&ws_ice);
                let event = Arc::clone(&event_send_ice);
                tokio::spawn(async move {
                    if let Ok(json) = c.to_json() {
                        event.on_log(&format!("ICE candidate (send): {}", json.candidate));
                        if let Ok(payload) = serde_json::to_string(&json) {
                            let msg = if is_remote {
                                SignalMessage::Relay {
                                    from: None,
                                    payload: serde_json::json!({"type": "ice_candidate", "payload": payload, "target": "peer"}),
                                }
                            } else {
                                SignalMessage::IceCandidate {
                                    payload,
                                    target: "peer".to_string(),
                                    from: None,
                                }
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

        // Receiver-side data channel: the host created it; wire the message
        // pump and the on-open announcements.
        let incoming_rcv = Arc::clone(&incoming_files);
        let pending_rcv = Arc::clone(&pending_manifest);
        let pending_view_rcv = Arc::clone(&pending_view_manifest);
        let transfer_rcv = Arc::clone(&transfer_semaphore);
        let config_rcv = self.config_path.clone();
        let event_rcv = Arc::clone(&self.event);
        let completed_rcv = Arc::clone(&items_completed);
        let total_rcv = Arc::clone(&items_total);
        let mirror_completed_rcv = Arc::clone(&mirror_completed);
        let mirror_total_rcv = Arc::clone(&mirror_total);
        let sync_rx_rcv = Arc::clone(&sync_rx);
        let device_id_rcv = self.device_id.clone();
        let device_name_rcv = self.device_name.clone();
        let device_os_rcv = self.device_os.clone();
        let models_rcv = self.models_enabled.clone();
        let view_only_client_rcv = self.view_only_client;
        let share_mode = self.share_mode;

        pc.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
            let share_mode_msg = share_mode;
            let last_seen_on_msg = Arc::clone(&last_seen);
            let last_seen_on_open = Arc::clone(&last_seen);
            let dc_outer = Arc::clone(&d);
            let incoming_msg = Arc::clone(&incoming_rcv);
            let pending_msg = Arc::clone(&pending_rcv);
            let pending_view_msg = Arc::clone(&pending_view_rcv);
            let transfer_msg = Arc::clone(&transfer_rcv);
            let config_msg = config_rcv.clone();
            let event_msg = Arc::clone(&event_rcv);
            let completed_msg = Arc::clone(&completed_rcv);
            let total_msg = Arc::clone(&total_rcv);
            let mirror_completed_msg = Arc::clone(&mirror_completed_rcv);
            let mirror_total_msg = Arc::clone(&mirror_total_rcv);

            let session_scope: std::sync::Arc<
                tokio::sync::Mutex<Option<crate::view_only::AlbumScope>>,
            > = std::sync::Arc::new(tokio::sync::Mutex::new(None));

            d.on_message(Box::new(move |msg: DataChannelMessage| {
                let dc = Arc::clone(&dc_outer);
                let incoming = Arc::clone(&incoming_msg);
                let pending = Arc::clone(&pending_msg);
                let pending_view = Arc::clone(&pending_view_msg);
                let transfer = Arc::clone(&transfer_msg);
                let config = config_msg.clone();
                let event = Arc::clone(&event_msg);
                let completed = Arc::clone(&completed_msg);
                let total = Arc::clone(&total_msg);
                let mirror_completed = Arc::clone(&mirror_completed_msg);
                let mirror_total = Arc::clone(&mirror_total_msg);
                let session_scope = std::sync::Arc::clone(&session_scope);
                let last_seen_msg = Arc::clone(&last_seen_on_msg);
                Box::pin(async move {
                    let text = String::from_utf8_lossy(&msg.data);
                    if let Ok(sync_msg) = serde_json::from_str::<SyncMessage>(&text) {
                        last_seen_msg.store(now_millis(), Ordering::Relaxed);
                        MeshManager::handle_sync_message(
                            sync_msg,
                            &dc,
                            &incoming,
                            &pending,
                            &transfer,
                            &config,
                            event,
                            &completed,
                            &total,
                            &mirror_completed,
                            &mirror_total,
                            &pending_view,
                            share_mode_msg,
                            &session_scope,
                        )
                        .await;
                    }
                })
            }));
            let dc_open = Arc::clone(&d);
            let sync_rx_final = Arc::clone(&sync_rx_rcv);
            let event_open = Arc::clone(&event_rcv);
            let device_id = device_id_rcv.clone();
            let device_name = device_name_rcv.clone();
            let device_os = device_os_rcv.clone();
            let models = models_rcv.clone();
            let view_only_client = view_only_client_rcv;

            d.on_open(Box::new(move || {
                let dc = Arc::clone(&dc_open);
                let sync_rx = Arc::clone(&sync_rx_final);
                let event_open = Arc::clone(&event_open);
                let last_seen_open = Arc::clone(&last_seen_on_open);
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
                    // View-only/RPC guests never ask for the library:
                    // they drive EnterViewOnly / CommandRequest themselves
                    // and a stray ManifestRequest would trigger a full
                    // sync push before EnterViewOnly even lands.
                    if !view_only_client {
                        let _ = MeshManager::send_sync_message(&dc, &SyncMessage::ManifestRequest)
                            .await;
                        event_open.on_log("DEBUG [receiver] sent ManifestRequest");
                        let _ = MeshManager::send_sync_message(&dc, &SyncMessage::CatchUp).await;
                        event_open.on_log("DEBUG [receiver] sent CatchUp");
                    }
                    let mut rx = sync_rx.lock().await;
                    let mut heartbeat =
                        tokio::time::interval(Duration::from_millis(HEARTBEAT_INTERVAL_MS));
                    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    loop {
                        tokio::select! {
                            msg = rx.recv() => {
                                let Some(msg) = msg else { break; };
                                event_open.on_log(&format!(
                                    "DEBUG [receiver] forwarding sync msg: {:?}",
                                    std::mem::discriminant(&msg)
                                ));
                                let _ =
                                    MeshManager::send_sync_message(&dc, &msg).await;
                            }
                            _ = heartbeat.tick() => {
                                let _ = MeshManager::send_sync_message(
                                    &dc,
                                    &SyncMessage::Ping,
                                )
                                .await;
                                let now = now_millis();
                                if now.saturating_sub(last_seen_open.load(Ordering::Relaxed))
                                    > HEARTBEAT_TIMEOUT_MS
                                {
                                    event_open.on_log(
                                        "DEBUG [receiver] heartbeat timeout — peer silent",
                                    );
                                    event_open.on_peer_offline();
                                    break;
                                }
                            }
                        }
                    }
                })
            }));

            Box::pin(async move {})
        }));

        self.send_join_frames(&ws_write, is_remote).await?;

        while let Some(msg) = ws_read.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    let signal: SignalMessage = match serde_json::from_str(&text) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    match signal {
                        SignalMessage::Joined { peer_count, .. } if !is_remote => {
                            mark_peer();
                            if peer_count > 1 {
                                self.event.on_state_change("Peer Joined");
                            }
                        }
                        SignalMessage::PeerJoined { .. } => {
                            mark_peer();
                            self.event.on_state_change("Peer Joined");
                        }
                        SignalMessage::Offer { payload, from, .. } if !is_remote => {
                            mark_peer();
                            host_key = from.clone().filter(|f| !f.is_empty()).or(host_key);
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
                                    from: None,
                                },
                            )
                            .await?;
                        }
                        SignalMessage::Answer { payload, from, .. } if !is_remote => {
                            mark_peer();
                            match (&host_key, &from) {
                                (Some(expected), Some(actual)) if actual != expected => continue,
                                (None, Some(_)) => continue, // foreign guest's reply
                                _ => {}
                            }
                            if let Ok(sdp) = serde_json::from_str::<RTCSessionDescription>(&payload)
                            {
                                pc.set_remote_description(sdp).await?;
                                let mut ice = pending_ice.lock().await;
                                for c in ice.drain(..) {
                                    let _ = pc.add_ice_candidate(c).await;
                                }
                            }
                        }
                        SignalMessage::IceCandidate { payload, from, .. } if !is_remote => {
                            mark_peer();
                            match (&host_key, &from) {
                                (Some(expected), Some(actual)) if actual != expected => continue,
                                (None, Some(_)) => continue,
                                _ => {}
                            }
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
                                    "ice_candidate" => {
                                        if let Some(c_str) =
                                            payload.get("payload").and_then(|v| v.as_str())
                                        {
                                            if let Ok(candidate) =
                                                serde_json::from_str::<RTCIceCandidateInit>(c_str)
                                            {
                                                if pc.remote_description().await.is_none() {
                                                    pending_ice.lock().await.push(candidate);
                                                } else {
                                                    let _ = pc.add_ice_candidate(candidate).await;
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        SignalMessage::RoomJoined => {
                            self.event
                                .on_state_change("Room joined. Waiting for peer...");
                        }
                        SignalMessage::RoomCreated { code } => {
                            self.event.on_log(&format!("Room code: {code}"));
                            self.event.on_state_change("Waiting for peer to join...");
                        }
                        SignalMessage::PeerDisconnected { device_id } => {
                            // In multi-guest rooms this fires when ANY peer
                            // leaves; only our negotiated host owns the
                            // ephemeral view-only state (#16).
                            let foreign_guest = match (&host_key, &device_id) {
                                (Some(expected), id) => !id.is_empty() && id != expected,
                                _ => false,
                            };
                            if foreign_guest {
                                continue;
                            }
                            self.event.on_state_change("Peer disconnected");
                            self.event.on_peer_offline();
                            // Ephemeral view-only state dies with the peer.
                            crate::view_only::state().reset_session();
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
    use crate::database::Database;
    use crate::mesh::{SyncEvent, SyncProgress};

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

    // --- extract_token ---

    #[test]
    fn test_extract_token_with_token() {
        let (base, token) = extract_token("ws://127.0.0.1:8080?token=abc123");
        assert_eq!(base, "ws://127.0.0.1:8080");
        assert_eq!(token.as_deref(), Some("abc123"));
    }

    #[test]
    fn test_extract_token_no_query() {
        let (base, token) = extract_token("ws://127.0.0.1:8080");
        assert_eq!(base, "ws://127.0.0.1:8080");
        assert!(token.is_none());
    }

    #[test]
    fn test_extract_token_empty_token() {
        let (base, token) = extract_token("ws://127.0.0.1:8080?token=");
        assert_eq!(base, "ws://127.0.0.1:8080");
        assert_eq!(token.as_deref(), Some(""));
    }

    #[test]
    fn test_extract_token_other_params() {
        let (base, token) = extract_token("ws://127.0.0.1:8080?foo=bar&token=xyz&baz=1");
        assert_eq!(base, "ws://127.0.0.1:8080");
        assert_eq!(token.as_deref(), Some("xyz"));
    }

    #[test]
    fn test_extract_token_strips_trailing_slash() {
        let (base, _) = extract_token("ws://127.0.0.1:8080/");
        assert_eq!(base, "ws://127.0.0.1:8080");
    }

    // --- route_key ---

    #[test]
    fn test_route_key_with_identity() {
        assert_eq!(MeshTransport::route_key(Some("dev-42".into())), "dev-42");
    }

    #[test]
    fn test_route_key_empty_string() {
        assert_eq!(MeshTransport::route_key(Some("".into())), "peer");
    }

    #[test]
    fn test_route_key_none() {
        assert_eq!(MeshTransport::route_key(None), "peer");
    }

    // --- rtc_configuration ---

    #[test]
    fn test_rtc_configuration_default() {
        std::env::remove_var("SIEGU_TURN_URLS");
        std::env::remove_var("SIEGU_TURN_USERNAME");
        std::env::remove_var("SIEGU_TURN_CREDENTIAL");
        let cfg = MeshTransport::rtc_configuration();
        assert_eq!(cfg.ice_servers.len(), 1);
        assert_eq!(
            cfg.ice_servers[0].urls,
            vec!["stun:stun.l.google.com:19302"]
        );
    }

    #[test]
    fn test_rtc_configuration_turn() {
        std::env::set_var("SIEGU_TURN_URLS", "turn:my.turn.server:3478");
        std::env::set_var("SIEGU_TURN_USERNAME", "user1");
        std::env::set_var("SIEGU_TURN_CREDENTIAL", "pass1");
        let cfg = MeshTransport::rtc_configuration();
        assert_eq!(cfg.ice_servers.len(), 2);
        assert_eq!(cfg.ice_servers[1].urls, vec!["turn:my.turn.server:3478"]);
        assert_eq!(cfg.ice_servers[1].username, "user1");
        assert_eq!(cfg.ice_servers[1].credential, "pass1");
        // cleanup
        std::env::remove_var("SIEGU_TURN_URLS");
        std::env::remove_var("SIEGU_TURN_USERNAME");
        std::env::remove_var("SIEGU_TURN_CREDENTIAL");
    }

    #[test]
    fn test_rtc_configuration_multiple_turn_urls() {
        std::env::set_var("SIEGU_TURN_URLS", "turn:a.com:3478, turn:b.com:3478");
        std::env::remove_var("SIEGU_TURN_USERNAME");
        std::env::remove_var("SIEGU_TURN_CREDENTIAL");
        let cfg = MeshTransport::rtc_configuration();
        assert_eq!(cfg.ice_servers.len(), 2);
        assert_eq!(
            cfg.ice_servers[1].urls,
            vec!["turn:a.com:3478", "turn:b.com:3478"]
        );
        // cleanup
        std::env::remove_var("SIEGU_TURN_URLS");
    }

    // --- builder methods ---

    #[test]
    fn test_builder_with_share_mode() {
        let t = MeshTransport::new(
            "r".into(),
            false,
            "ws://x".into(),
            "/tmp".into(),
            "d".into(),
            "n".into(),
            vec![],
            Arc::new(TestEvent),
        )
        .with_share_mode(crate::rpc::ShareMode::ReadOnly);
        assert!(t.share_mode.is_some());
    }

    #[test]
    fn test_builder_with_view_only_client() {
        let t = MeshTransport::new(
            "r".into(),
            false,
            "ws://x".into(),
            "/tmp".into(),
            "d".into(),
            "n".into(),
            vec![],
            Arc::new(TestEvent),
        )
        .with_view_only_client(true);
        assert!(t.view_only_client);
    }

    // --- send_message ---

    #[tokio::test]
    async fn test_send_message_ok() {
        let t = MeshTransport::new(
            "r".into(),
            false,
            "ws://x".into(),
            "/tmp".into(),
            "d".into(),
            "n".into(),
            vec![],
            Arc::new(TestEvent),
        );
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        *t.sync_tx.lock().await = Some(tx);
        t.send_message(SyncMessage::VersionNegotiate {
            version: PROTOCOL_VERSION,
            device_id: "d".into(),
            device_name: "n".into(),
            os: "linux".into(),
            models_enabled: vec![],
        })
        .await
        .unwrap();
        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, SyncMessage::VersionNegotiate { .. }));
    }

    #[tokio::test]
    async fn test_send_message_not_connected() {
        let t = MeshTransport::new(
            "r".into(),
            false,
            "ws://x".into(),
            "/tmp".into(),
            "d".into(),
            "n".into(),
            vec![],
            Arc::new(TestEvent),
        );
        let err = t
            .send_message(SyncMessage::VersionNegotiate {
                version: PROTOCOL_VERSION,
                device_id: "d".into(),
                device_name: "n".into(),
                os: "linux".into(),
                models_enabled: vec![],
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not initialized"));
    }

    // --- mark_peer_active ---

    #[test]
    fn test_mark_peer_active() {
        let flag = Arc::new(AtomicBool::new(false));
        let cb = MeshTransport::mark_peer_active(Some(flag.clone()));
        assert!(!flag.load(Ordering::Relaxed));
        cb();
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn test_mark_peer_active_none() {
        let cb = MeshTransport::mark_peer_active(None);
        cb(); // should not panic
    }
}
