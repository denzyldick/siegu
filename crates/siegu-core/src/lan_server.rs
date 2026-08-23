#![allow(dead_code)]

use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use warp::reply::Reply;
use warp::Filter;

use crate::mesh::MAX_MESH_DEVICES;

/// Default port for the local LAN signaling server. Kept stable across app
/// restarts so peer devices can reconnect with their saved join URL.
pub const DEFAULT_LAN_SIGNALING_PORT: u16 = 34801;
use crate::server::hash_pairing_code;
use crate::signal::SignalMessage;

type Tx = mpsc::UnboundedSender<warp::ws::Message>;
type Rooms = Arc<RwLock<HashMap<String, Room>>>;

const MAX_CONNECTIONS_PER_IP: usize = 8;
const MAX_TOTAL_CONNECTIONS: usize = 1024;
const WINDOW_ATTEMPTS: usize = 20;
const WINDOW_SECS: u64 = 60;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_ROOMS: usize = 64;

struct Room {
    clients: HashMap<String, Tx>,
}

/// Server-wide runtime state shared across connections.
struct ServerContext {
    rooms: Rooms,
    token: Option<String>,
    limiter: RateLimiter,
    conn_counts: Mutex<HashMap<IpAddr, usize>>,
    global_count: AtomicUsize,
}

/// Configuration for a standalone signalling server.
pub struct ServerConfig {
    pub port: u16,
    /// When set, every `Join`/`JoinRoom`/`CreateRoom` must carry this token.
    pub token: Option<String>,
}

/// A running LAN signaling server. Holds the bound port and an optional
/// handle used to stop the listener task when a new server is started.
pub struct LanServer {
    pub port: u16,
    abort: Option<tokio::task::AbortHandle>,
}

impl LanServer {
    pub fn new(port: u16) -> Self {
        Self { port, abort: None }
    }

    /// Stop the underlying listener task. If this server was created with an
    /// explicit port (no listener spawned), this is a no-op.
    pub fn stop(&self) {
        if let Some(handle) = &self.abort {
            handle.abort();
        }
    }
}

/// Start a LAN signaling server with the default (in-app) configuration.
pub async fn start(port: u16) -> LanServer {
    start_with_config(ServerConfig { port, token: None }).await
}

/// Start a signalling server with optional security configuration. Serves both
/// the LAN room-path protocol (`/room_id`, used by `ws://` clients) and the
/// remote protocol (`/ws`, used by `wss://` clients speaking `CreateRoom` /
/// `JoinRoom` / `Relay`).
pub async fn start_with_config(config: ServerConfig) -> LanServer {
    let ctx = Arc::new(ServerContext {
        rooms: Arc::new(RwLock::new(HashMap::new())),
        token: config.token,
        limiter: RateLimiter::default(),
        conn_counts: Mutex::new(HashMap::new()),
        global_count: AtomicUsize::new(0),
    });

    let ctx_filter = warp::any().map({
        let ctx = ctx.clone();
        move || ctx.clone()
    });
    let remote_addr = warp::addr::remote();

    let health = warp::path("healthz").map(|| -> warp::reply::Response { "ok".into_response() });

    let remote_route = warp::path("ws")
        .and(warp::ws())
        .and(ctx_filter.clone())
        .and(remote_addr)
        .map(
            |ws: warp::ws::Ws,
             ctx: Arc<ServerContext>,
             addr: Option<SocketAddr>|
             -> warp::reply::Response {
                ws.on_upgrade(move |socket| handle_connection(socket, ctx, addr, None, true))
                    .into_response()
            },
        );

    let lan_route = warp::path::param::<String>()
        .and(warp::ws())
        .and(ctx_filter)
        .and(remote_addr)
        .map(
            |room_id: String,
             ws: warp::ws::Ws,
             ctx: Arc<ServerContext>,
             addr: Option<SocketAddr>|
             -> warp::reply::Response {
                ws.on_upgrade(move |socket| {
                    handle_connection(socket, ctx, addr, Some(room_id), false)
                })
                .into_response()
            },
        );

    let routes = remote_route.or(lan_route).or(health).boxed();

    let addr: SocketAddr = ([0, 0, 0, 0], config.port).into();
    // Prefer the configured (stable) port; if it is unavailable, fall back to an
    // ephemeral port so a leftover listener or other occupancy never blocks startup.
    #[allow(clippy::expect_used)]
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(_) if config.port != 0 => {
            eprintln!(
                "[lan-server] port {} unavailable, falling back to an ephemeral port",
                config.port
            );
            TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 0u16)))
                .await
                .expect("failed to bind the LAN signalling server listener")
        }
        Err(e) => {
            // Fatal: the server must return a bound listener, and the signature cannot signal failure.
            panic!("failed to bind the LAN signalling server listener: {e}");
        }
    };
    // Fatal: a successfully bound listener must expose its address; without it the port is unknowable.
    #[allow(clippy::expect_used)]
    let actual_addr = listener
        .local_addr()
        .expect("failed to read the LAN signalling server bound address");

    let task = tokio::spawn(async move {
        warp::serve(routes).incoming(listener).run().await;
    });

    LanServer {
        port: actual_addr.port(),
        abort: Some(task.abort_handle()),
    }
}

fn admission_reject(ctx: &Arc<ServerContext>, remote: Option<SocketAddr>) -> Option<String> {
    if ctx.global_count.load(Ordering::Relaxed) >= MAX_TOTAL_CONNECTIONS {
        return Some("Server is at capacity, try again later".to_string());
    }
    let ip = remote?.ip();
    if !ctx.limiter.allow(ip) {
        return Some("Too many connection attempts, try again shortly".to_string());
    }
    let mut counts = ctx.conn_counts.lock().unwrap_or_else(|e| e.into_inner());
    let n = counts.entry(ip).or_insert(0);
    if *n >= MAX_CONNECTIONS_PER_IP {
        return Some("Too many connections from this address".to_string());
    }
    *n += 1;
    ctx.global_count.fetch_add(1, Ordering::Relaxed);
    None
}

fn send(tx: &Tx, msg: &SignalMessage) {
    if let Ok(json) = serde_json::to_string(msg) {
        let _ = tx.send(warp::ws::Message::text(json));
    }
}

fn generate_room_code() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill(&mut bytes);
    hex::encode(bytes)
}

#[derive(Default)]
struct RateLimiter {
    attempts: Mutex<HashMap<IpAddr, Vec<Instant>>>,
}

impl RateLimiter {
    fn allow(&self, ip: IpAddr) -> bool {
        let mut map = self.attempts.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() > 10_000 {
            let cutoff = Instant::now() - Duration::from_secs(WINDOW_SECS);
            map.retain(|_, v| v.last().map(|t| *t >= cutoff).unwrap_or(false));
        }
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(WINDOW_SECS);
        let bucket = map.entry(ip).or_default();
        bucket.retain(|t| *t >= cutoff);
        if bucket.len() >= WINDOW_ATTEMPTS {
            return false;
        }
        bucket.push(now);
        true
    }
}

async fn handle_connection(
    socket: warp::ws::WebSocket,
    ctx: Arc<ServerContext>,
    remote: Option<SocketAddr>,
    path_room: Option<String>,
    remote_protocol: bool,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    if let Some(reason) = admission_reject(&ctx, remote) {
        // No client IP in logs: reject reasons carry no identifying data.
        eprintln!(
            "siegu-signal: connection rejected ({reason}), total={}",
            ctx.global_count.load(Ordering::Relaxed)
        );
        let tx_reject = tx.clone();
        send(&tx_reject, &SignalMessage::Error { message: reason });
        let _ = tokio::time::timeout(Duration::from_millis(100), &mut send_task).await;
        send_task.abort();
        return;
    }

    eprintln!(
        "siegu-signal: connection open (total={})",
        ctx.global_count.load(Ordering::Relaxed)
    );

    let room_id: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(path_room));
    let conn_key: Arc<RwLock<String>> = Arc::new(RwLock::new(String::new()));
    let did: Arc<RwLock<String>> = Arc::new(RwLock::new(String::new()));

    let rooms_write = ctx.rooms.clone();
    let conn_key_w = conn_key.clone();
    let room_id_w = room_id.clone();
    let did_w = did.clone();
    let tx_write = tx.clone();
    let ctx_r = ctx.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if !(msg.is_text() || msg.is_binary()) {
                continue;
            }
            if msg.as_bytes().len() > MAX_MESSAGE_BYTES {
                send(
                    &tx_write,
                    &SignalMessage::Error {
                        message: "Message too large".to_string(),
                    },
                );
                break;
            }
            let text = match msg.to_str() {
                Ok(t) => t.to_string(),
                Err(_) => continue,
            };
            let signal: SignalMessage = match serde_json::from_str(&text) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if let Some(expected) = &ctx_r.token {
                let sent = match &signal {
                    SignalMessage::Join { token, .. } => token.as_deref(),
                    SignalMessage::JoinRoom { token, .. } => token.as_deref(),
                    SignalMessage::CreateRoom { token } => token.as_deref(),
                    _ => None,
                };
                if sent != Some(expected) {
                    eprintln!("siegu-signal: rejected join with invalid token");
                    send(
                        &tx_write,
                        &SignalMessage::Error {
                            message: "Invalid or missing signalling token".to_string(),
                        },
                    );
                    break;
                }
            }

            match signal {
                SignalMessage::Join { device_id: id, .. } => {
                    *did_w.write().await = id.clone();

                    let room_id_val = room_id_w.read().await.clone();
                    let room_key = room_id_val.clone().unwrap_or_default();
                    let mut rooms = rooms_write.write().await;
                    if !rooms.contains_key(&room_key) && rooms.len() >= MAX_ROOMS {
                        send(
                            &tx_write,
                            &SignalMessage::Error {
                                message: "Server at room capacity".to_string(),
                            },
                        );
                        break;
                    }
                    let room = rooms.entry(room_key).or_insert_with(|| Room {
                        clients: HashMap::new(),
                    });

                    if room.clients.len() >= MAX_MESH_DEVICES {
                        send(
                            &tx_write,
                            &SignalMessage::Error {
                                message: format!("Room is full (max {} devices)", MAX_MESH_DEVICES),
                            },
                        );
                        break;
                    }

                    room.clients.insert(id.clone(), tx_write.clone());
                    *conn_key_w.write().await = id.clone();

                    let existing_peers: Vec<String> =
                        room.clients.keys().filter(|k| *k != &id).cloned().collect();
                    send(
                        &tx_write,
                        &SignalMessage::PeerList {
                            peers: existing_peers,
                        },
                    );

                    for (other_id, client) in room.clients.iter() {
                        if other_id != &id {
                            send(
                                client,
                                &SignalMessage::PeerJoined {
                                    device_id: id.clone(),
                                },
                            );
                        }
                    }

                    let peer_count = room.clients.len();
                    send(
                        &tx_write,
                        &SignalMessage::Joined {
                            device_id: id,
                            room_id: room_id_val.unwrap_or_default(),
                            peer_count,
                        },
                    );
                }
                SignalMessage::CreateRoom { .. } => {
                    if remote_protocol {
                        let mut raw = generate_room_code();
                        let mut key = hash_pairing_code(raw.clone()).unwrap_or_default();
                        {
                            let rooms = rooms_write.read().await;
                            if rooms.contains_key(&key) {
                                drop(rooms);
                                raw = generate_room_code();
                                key = hash_pairing_code(raw.clone()).unwrap_or_default();
                            }
                        }
                        let ck = uuid::Uuid::new_v4().to_string();
                        {
                            let mut rooms = rooms_write.write().await;
                            if rooms.len() >= MAX_ROOMS {
                                send(
                                    &tx_write,
                                    &SignalMessage::Error {
                                        message: "Server at room capacity".to_string(),
                                    },
                                );
                                break;
                            }
                            let room = rooms.entry(key.clone()).or_insert_with(|| Room {
                                clients: HashMap::new(),
                            });
                            room.clients.insert(ck.clone(), tx_write.clone());
                        }
                        *room_id_w.write().await = Some(key);
                        *conn_key_w.write().await = ck;
                        send(&tx_write, &SignalMessage::RoomCreated { code: raw });
                    }
                }
                SignalMessage::JoinRoom { code, .. } => {
                    if remote_protocol {
                        let mut rooms = rooms_write.write().await;
                        let key = if rooms.contains_key(&code) {
                            code
                        } else {
                            match hash_pairing_code(code.clone()) {
                                Ok(h) if rooms.contains_key(&h) => h,
                                _ => {
                                    send(
                                        &tx_write,
                                        &SignalMessage::Error {
                                            message:
                                                "Room not found. Check the code and try again."
                                                    .to_string(),
                                        },
                                    );
                                    break;
                                }
                            }
                        };
                        let room = rooms.entry(key.clone()).or_insert_with(|| Room {
                            clients: HashMap::new(),
                        });

                        if room.clients.len() >= MAX_MESH_DEVICES {
                            send(
                                &tx_write,
                                &SignalMessage::Error {
                                    message: format!(
                                        "Room is full (max {} devices)",
                                        MAX_MESH_DEVICES
                                    ),
                                },
                            );
                            break;
                        }

                        let ck = uuid::Uuid::new_v4().to_string();
                        room.clients.insert(ck.clone(), tx_write.clone());
                        *room_id_w.write().await = Some(key.clone());
                        *conn_key_w.write().await = ck.clone();

                        send(&tx_write, &SignalMessage::RoomJoined);

                        let existing_peers: Vec<String> =
                            room.clients.keys().filter(|k| *k != &ck).cloned().collect();
                        send(
                            &tx_write,
                            &SignalMessage::PeerList {
                                peers: existing_peers,
                            },
                        );

                        let notify: Vec<Tx> = room.clients.values().cloned().collect();
                        for client in notify {
                            send(
                                &client,
                                &SignalMessage::PeerJoined {
                                    device_id: String::new(),
                                },
                            );
                        }
                    }
                }
                SignalMessage::Offer { payload, target } => {
                    if !remote_protocol {
                        let d_id = did_w.read().await.clone();
                        let rid = room_id_w.read().await.clone();
                        relay(
                            &rooms_write,
                            rid,
                            &d_id,
                            &target,
                            SignalMessage::Offer {
                                payload,
                                target: target.clone(),
                            },
                        )
                        .await;
                    }
                }
                SignalMessage::Answer { payload, target } => {
                    if !remote_protocol {
                        let d_id = did_w.read().await.clone();
                        let rid = room_id_w.read().await.clone();
                        relay(
                            &rooms_write,
                            rid,
                            &d_id,
                            &target,
                            SignalMessage::Answer {
                                payload,
                                target: target.clone(),
                            },
                        )
                        .await;
                    }
                }
                SignalMessage::IceCandidate { payload, target } => {
                    if !remote_protocol {
                        let d_id = did_w.read().await.clone();
                        let rid = room_id_w.read().await.clone();
                        relay(
                            &rooms_write,
                            rid,
                            &d_id,
                            &target,
                            SignalMessage::IceCandidate {
                                payload,
                                target: target.clone(),
                            },
                        )
                        .await;
                    }
                }
                SignalMessage::Relay { payload, .. } => {
                    if remote_protocol {
                        let ck = conn_key_w.read().await.clone();
                        let rid = room_id_w.read().await.clone();
                        if let Some(rid) = rid {
                            let rooms = rooms_write.read().await;
                            if let Some(room) = rooms.get(&rid) {
                                for (id, client) in &room.clients {
                                    if id != &ck {
                                        send(
                                            client,
                                            &SignalMessage::Relay {
                                                from: None,
                                                payload: payload.clone(),
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                SignalMessage::DeviceAnnounce {
                    device_id: id,
                    metadata,
                } => {
                    let d_id = did_w.read().await.clone();
                    let rid = room_id_w.read().await.clone();
                    let rooms = rooms_write.read().await;
                    if let Some(rid) = rid {
                        if let Some(room) = rooms.get(&rid) {
                            for (peer_id, client) in &room.clients {
                                if peer_id != &id {
                                    send(
                                        client,
                                        &SignalMessage::DeviceAnnounce {
                                            device_id: d_id.clone(),
                                            metadata: metadata.clone(),
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let d_id = did.read().await.clone();
        let ck = conn_key.read().await.clone();
        let rid = room_id.read().await.clone();
        if let Some(rid_val) = rid {
            if !ck.is_empty() || !d_id.is_empty() {
                let mut rooms = rooms_write.write().await;
                if let Some(room) = rooms.get_mut(&rid_val) {
                    if !ck.is_empty() {
                        room.clients.remove(&ck);
                    } else if !d_id.is_empty() {
                        room.clients.remove(&d_id);
                    }
                    for client in room.clients.values() {
                        send(
                            client,
                            &SignalMessage::PeerDisconnected {
                                device_id: d_id.clone(),
                            },
                        );
                    }
                    if room.clients.is_empty() {
                        rooms.remove(&rid_val);
                    }
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    if let Some(addr) = remote {
        if let Ok(mut counts) = ctx.conn_counts.lock() {
            if let Some(n) = counts.get_mut(&addr.ip()) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    counts.remove(&addr.ip());
                }
            }
        }
    }
    ctx.global_count.fetch_sub(1, Ordering::Relaxed);
    eprintln!(
        "siegu-signal: connection closed (total={})",
        ctx.global_count.load(Ordering::Relaxed)
    );
}

async fn relay(
    rooms: &RwLock<HashMap<String, Room>>,
    room_id: Option<String>,
    sender_id: &str,
    target_id: &str,
    msg: SignalMessage,
) {
    let rooms = rooms.read().await;
    if let Some(room_id) = room_id {
        if let Some(room) = rooms.get(&room_id) {
            if target_id == "peer" {
                for (id, client) in &room.clients {
                    if id != sender_id {
                        send(client, &msg);
                    }
                }
            } else if let Some(client) = room.clients.get(target_id) {
                send(client, &msg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::MaybeTlsStream;

    type TestStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

    async fn connect_client(port: u16, path: &str) -> TestStream {
        let url = format!("ws://127.0.0.1:{}/{}", port, path);
        let (ws, _) = connect_async(&url).await.unwrap();
        ws
    }

    async fn send_join(ws: &mut TestStream, device_id: &str) {
        let join = SignalMessage::Join {
            device_id: device_id.to_string(),
            token: None,
        };
        ws.send(Message::Text(serde_json::to_string(&join).unwrap().into()))
            .await
            .unwrap();
    }

    async fn send_join_room(ws: &mut TestStream, code: &str) {
        let msg = SignalMessage::JoinRoom {
            code: code.to_string(),
            token: None,
        };
        ws.send(Message::Text(serde_json::to_string(&msg).unwrap().into()))
            .await
            .unwrap();
    }

    async fn recv_signal(ws: &mut TestStream) -> SignalMessage {
        let msg = ws.next().await.unwrap().unwrap();
        let text = msg.to_text().unwrap().to_string();
        serde_json::from_str(&text).unwrap()
    }

    async fn recv_until_joined(ws: &mut TestStream) -> SignalMessage {
        loop {
            let msg = recv_signal(ws).await;
            match &msg {
                SignalMessage::Joined { .. } => return msg,
                SignalMessage::Error { .. } => return msg,
                _ => continue,
            }
        }
    }

    #[tokio::test]
    async fn test_server_starts_and_listens() {
        let server = start(0).await;
        let port = server.port;
        assert!(port > 0, "Server should bind to a port");

        let url = format!("ws://127.0.0.1:{}/test-room", port);
        let result = connect_async(&url).await;
        assert!(result.is_ok(), "Should connect to server");
    }

    #[tokio::test]
    async fn test_server_stop_closes_listener() {
        let server = start(0).await;
        let port = server.port;
        assert!(port > 0);

        server.stop();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let url = format!("ws://127.0.0.1:{}/test-room", port);
        let result = connect_async(&url).await;
        assert!(
            result.is_err(),
            "Server should stop accepting connections after stop()"
        );
    }

    #[tokio::test]
    async fn test_two_clients_join_same_room() {
        let port = start(0).await.port;

        let mut ws_a = connect_client(port, "room1").await;
        send_join(&mut ws_a, "device-a").await;

        let response = recv_until_joined(&mut ws_a).await;
        match response {
            SignalMessage::Joined {
                device_id,
                room_id,
                peer_count,
            } => {
                assert_eq!(device_id, "device-a");
                assert_eq!(room_id, "room1");
                assert_eq!(peer_count, 1);
            }
            other => panic!("Expected Joined, got {other:?}"),
        }

        let mut ws_b = connect_client(port, "room1").await;
        send_join(&mut ws_b, "device-b").await;

        let response_b = recv_until_joined(&mut ws_b).await;
        match response_b {
            SignalMessage::Joined {
                device_id,
                room_id,
                peer_count,
            } => {
                assert_eq!(device_id, "device-b");
                assert_eq!(room_id, "room1");
                assert_eq!(peer_count, 2);
            }
            other => panic!("Expected Joined for B, got {other:?}"),
        }

        let response_a = recv_signal(&mut ws_a).await;
        match response_a {
            SignalMessage::PeerJoined { device_id } => {
                assert_eq!(device_id, "device-b");
            }
            other => panic!("Expected PeerJoined, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_sixth_client_rejected() {
        let port = start(0).await.port;

        let mut ws_a = connect_client(port, "room2").await;
        send_join(&mut ws_a, "device-a").await;
        recv_signal(&mut ws_a).await;

        let mut ws_b = connect_client(port, "room2").await;
        send_join(&mut ws_b, "device-b").await;
        recv_signal(&mut ws_b).await;
        recv_signal(&mut ws_a).await;

        let mut ws_c = connect_client(port, "room2").await;
        send_join(&mut ws_c, "device-c").await;
        recv_signal(&mut ws_c).await;
        recv_signal(&mut ws_a).await;
        recv_signal(&mut ws_b).await;

        let mut ws_d = connect_client(port, "room2").await;
        send_join(&mut ws_d, "device-d").await;
        recv_signal(&mut ws_d).await;
        recv_signal(&mut ws_a).await;
        recv_signal(&mut ws_b).await;
        recv_signal(&mut ws_c).await;

        let mut ws_e = connect_client(port, "room2").await;
        send_join(&mut ws_e, "device-e").await;
        recv_signal(&mut ws_e).await;
        recv_signal(&mut ws_a).await;
        recv_signal(&mut ws_b).await;
        recv_signal(&mut ws_c).await;
        recv_signal(&mut ws_d).await;

        let mut ws_f = connect_client(port, "room2").await;
        send_join(&mut ws_f, "device-f").await;

        let response = recv_signal(&mut ws_f).await;
        match response {
            SignalMessage::Error { message } => {
                assert!(
                    message.contains("Room is full"),
                    "Expected room full error, got: {message}"
                );
            }
            other => panic!("Expected Error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_relay_offer_between_peers() {
        let port = start(0).await.port;

        let mut ws_a = connect_client(port, "room3").await;
        send_join(&mut ws_a, "device-a").await;
        recv_until_joined(&mut ws_a).await;

        let mut ws_b = connect_client(port, "room3").await;
        send_join(&mut ws_b, "device-b").await;
        recv_until_joined(&mut ws_b).await;
        recv_signal(&mut ws_a).await;

        let offer = SignalMessage::Offer {
            payload: "sdp-offer-123".to_string(),
            target: "peer".to_string(),
        };
        ws_a.send(Message::Text(serde_json::to_string(&offer).unwrap().into()))
            .await
            .unwrap();

        let received = recv_signal(&mut ws_b).await;
        match received {
            SignalMessage::Offer { payload, target } => {
                assert_eq!(payload, "sdp-offer-123");
                assert_eq!(target, "peer");
            }
            other => panic!("Expected Offer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_relay_answer_between_peers() {
        let port = start(0).await.port;

        let mut ws_a = connect_client(port, "room4").await;
        send_join(&mut ws_a, "device-a").await;
        recv_until_joined(&mut ws_a).await;

        let mut ws_b = connect_client(port, "room4").await;
        send_join(&mut ws_b, "device-b").await;
        recv_until_joined(&mut ws_b).await;
        recv_signal(&mut ws_a).await;

        let answer = SignalMessage::Answer {
            payload: "sdp-answer-456".to_string(),
            target: "peer".to_string(),
        };
        ws_b.send(Message::Text(
            serde_json::to_string(&answer).unwrap().into(),
        ))
        .await
        .unwrap();

        let received = recv_signal(&mut ws_a).await;
        match received {
            SignalMessage::Answer { payload, target } => {
                assert_eq!(payload, "sdp-answer-456");
                assert_eq!(target, "peer");
            }
            other => panic!("Expected Answer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_relay_ice_candidate() {
        let port = start(0).await.port;

        let mut ws_a = connect_client(port, "room5").await;
        send_join(&mut ws_a, "device-a").await;
        recv_until_joined(&mut ws_a).await;

        let mut ws_b = connect_client(port, "room5").await;
        send_join(&mut ws_b, "device-b").await;
        recv_until_joined(&mut ws_b).await;
        recv_signal(&mut ws_a).await;

        let ice = SignalMessage::IceCandidate {
            payload: r#"{"candidate":"candidate:1 1 UDP 2122252543 192.168.1.5 54321 typ host"}"#
                .to_string(),
            target: "peer".to_string(),
        };
        ws_b.send(Message::Text(serde_json::to_string(&ice).unwrap().into()))
            .await
            .unwrap();

        let received = recv_signal(&mut ws_a).await;
        match received {
            SignalMessage::IceCandidate { payload, target } => {
                assert!(payload.contains("192.168.1.5"));
                assert_eq!(target, "peer");
            }
            other => panic!("Expected IceCandidate, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_peer_disconnected_notification() {
        let port = start(0).await.port;

        let mut ws_a = connect_client(port, "room6").await;
        send_join(&mut ws_a, "device-a").await;
        recv_until_joined(&mut ws_a).await;

        let mut ws_b = connect_client(port, "room6").await;
        send_join(&mut ws_b, "device-b").await;
        let joined = recv_until_joined(&mut ws_b).await;
        match &joined {
            SignalMessage::Joined { peer_count, .. } => assert_eq!(*peer_count, 2),
            other => panic!("Expected Joined, got {other:?}"),
        }
        recv_signal(&mut ws_a).await;
        drop(ws_b);

        let received = recv_signal(&mut ws_a).await;
        match received {
            SignalMessage::PeerDisconnected { device_id } => {
                assert_eq!(device_id, "device-b");
            }
            other => panic!("Expected PeerDisconnected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_multiple_rooms_independent() {
        let port = start(0).await.port;

        let mut ws_a = connect_client(port, "alpha").await;
        send_join(&mut ws_a, "device-a").await;
        recv_until_joined(&mut ws_a).await;

        let mut ws_b = connect_client(port, "alpha").await;
        send_join(&mut ws_b, "device-b").await;
        recv_until_joined(&mut ws_b).await;
        recv_signal(&mut ws_a).await;

        let mut ws_c = connect_client(port, "beta").await;
        send_join(&mut ws_c, "device-c").await;
        recv_until_joined(&mut ws_c).await;

        let mut ws_d = connect_client(port, "beta").await;
        send_join(&mut ws_d, "device-d").await;
        recv_until_joined(&mut ws_d).await;
        recv_signal(&mut ws_c).await;

        let offer = SignalMessage::Offer {
            payload: "alpha-offer".to_string(),
            target: "peer".to_string(),
        };
        ws_a.send(Message::Text(serde_json::to_string(&offer).unwrap().into()))
            .await
            .unwrap();

        let received_b = recv_signal(&mut ws_b).await;
        match received_b {
            SignalMessage::Offer { payload, .. } => {
                assert_eq!(payload, "alpha-offer");
            }
            other => panic!("Expected Offer in alpha, got {other:?}"),
        }

        let offer_c = SignalMessage::Offer {
            payload: "beta-offer".to_string(),
            target: "peer".to_string(),
        };
        ws_c.send(Message::Text(
            serde_json::to_string(&offer_c).unwrap().into(),
        ))
        .await
        .unwrap();

        let received_d = recv_signal(&mut ws_d).await;
        match received_d {
            SignalMessage::Offer { payload, .. } => {
                assert_eq!(payload, "beta-offer", "Beta should receive its own offer");
            }
            other => panic!("Expected Offer in beta, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_room_cleanup_on_empty() {
        let port = start(0).await.port;

        let mut ws_a = connect_client(port, "cleanup-room").await;
        send_join(&mut ws_a, "device-a").await;
        recv_until_joined(&mut ws_a).await;
        drop(ws_a);

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let mut ws_b = connect_client(port, "cleanup-room").await;
        send_join(&mut ws_b, "device-b").await;
        let response = recv_until_joined(&mut ws_b).await;
        match response {
            SignalMessage::Joined { peer_count, .. } => {
                assert_eq!(peer_count, 1, "Room should be fresh after cleanup");
            }
            other => panic!("Expected Joined, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_remote_create_and_join_room() {
        let port = start(0).await.port;

        let mut ws_host = connect_client(port, "ws").await;
        ws_host
            .send(Message::Text(
                serde_json::to_string(&SignalMessage::CreateRoom { token: None })
                    .unwrap()
                    .into(),
            ))
            .await
            .unwrap();

        let code = match recv_signal(&mut ws_host).await {
            SignalMessage::RoomCreated { code } => code,
            other => panic!("Expected RoomCreated, got {other:?}"),
        };

        let mut ws_peer = connect_client(port, "ws").await;
        let key = hash_pairing_code(code.clone()).unwrap();
        send_join_room(&mut ws_peer, &key).await;

        match recv_signal(&mut ws_peer).await {
            SignalMessage::RoomJoined => {}
            other => panic!("Expected RoomJoined, got {other:?}"),
        }

        loop {
            match recv_signal(&mut ws_peer).await {
                SignalMessage::PeerJoined { .. } => break,
                SignalMessage::PeerList { .. } => continue,
                other => panic!("Expected PeerJoined for peer, got {other:?}"),
            }
        }

        loop {
            match recv_signal(&mut ws_host).await {
                SignalMessage::PeerJoined { .. } => break,
                SignalMessage::PeerList { .. } => continue,
                other => panic!("Expected PeerJoined for host, got {other:?}"),
            }
        }

        let relay = SignalMessage::Relay {
            from: None,
            payload: serde_json::json!({"type": "offer", "payload": "sdp", "target": "peer"}),
        };
        ws_peer
            .send(Message::Text(serde_json::to_string(&relay).unwrap().into()))
            .await
            .unwrap();

        match recv_signal(&mut ws_host).await {
            SignalMessage::Relay { payload, .. } => {
                assert_eq!(payload["type"], "offer");
            }
            other => panic!("Expected Relay, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_join_unknown_room_rejected() {
        let port = start(0).await.port;

        let mut ws = connect_client(port, "ws").await;
        send_join_room(&mut ws, "deadbeefdeadbeefdeadbeefdeadbeef").await;

        match recv_signal(&mut ws).await {
            SignalMessage::Error { message } => {
                assert!(
                    message.contains("Room not found"),
                    "Expected room not found, got: {message}"
                );
            }
            other => panic!("Expected Error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_token_required() {
        let server = start_with_config(ServerConfig {
            port: 0,
            token: Some("s3cret".to_string()),
        })
        .await;
        let port = server.port;

        let mut ws_bad = connect_client(port, "room-token").await;
        send_join(&mut ws_bad, "device-bad").await;

        match recv_signal(&mut ws_bad).await {
            SignalMessage::Error { message } => {
                assert!(
                    message.contains("token"),
                    "Expected token error, got: {message}"
                );
            }
            other => panic!("Expected Error, got {other:?}"),
        }

        let mut ws_good = connect_client(port, "room-token").await;
        ws_good
            .send(Message::Text(
                serde_json::to_string(&SignalMessage::Join {
                    device_id: "device-good".to_string(),
                    token: Some("s3cret".to_string()),
                })
                .unwrap()
                .into(),
            ))
            .await
            .unwrap();

        match recv_until_joined(&mut ws_good).await {
            SignalMessage::Joined { device_id, .. } => {
                assert_eq!(device_id, "device-good");
            }
            other => panic!("Expected Joined, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_rate_limit_rejects_bursts() {
        let server = start_with_config(ServerConfig {
            port: 0,
            token: None,
        })
        .await;
        let port = server.port;

        let mut rejects = 0;
        for _ in 0..(WINDOW_ATTEMPTS + 5) {
            let mut ws = connect_client(port, "ws").await;
            match tokio::time::timeout(std::time::Duration::from_millis(500), ws.next()).await {
                Ok(Some(Ok(msg))) => {
                    if msg.is_close() {
                        rejects += 1;
                        continue;
                    }
                    let text = msg.to_text().unwrap_or("");
                    if text.contains("\"type\":\"error\"") || text.contains("\"type\": \"error\"") {
                        rejects += 1;
                    }
                }
                Ok(Some(Err(_))) => rejects += 1,
                Ok(None) => rejects += 1,
                Err(_) => {}
            }
            drop(ws);
        }

        assert!(rejects > 0, "Expected at least one rate-limited rejection");
    }

    #[tokio::test]
    async fn test_oversize_message_rejected() {
        let port = start(0).await.port;

        let mut ws = connect_client(port, "room-big").await;
        let big = "x".repeat(MAX_MESSAGE_BYTES + 1);
        let payload = serde_json::json!({
            "type": "relay",
            "from": null,
            "payload": { "big": big }
        });
        ws.send(Message::Text(payload.to_string().into()))
            .await
            .unwrap();

        match recv_signal(&mut ws).await {
            SignalMessage::Error { message } => {
                assert!(
                    message.contains("too large"),
                    "Expected too-large error, got: {message}"
                );
            }
            other => panic!("Expected Error, got {other:?}"),
        }
    }
}
