#![allow(dead_code)]

use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use warp::Filter;

use crate::mesh::MAX_MESH_DEVICES;
use crate::signal::SignalMessage;

type Tx = mpsc::UnboundedSender<warp::ws::Message>;
type Rooms = Arc<RwLock<HashMap<String, Room>>>;

struct Room {
    clients: HashMap<String, Tx>,
}

pub async fn start(port: u16) -> u16 {
    let rooms: Rooms = Arc::new(RwLock::new(HashMap::new()));

    let rooms_filter = warp::any().map({
        let rooms = rooms.clone();
        move || rooms.clone()
    });

    let ws_route = warp::path::param::<String>()
        .and(warp::ws())
        .and(rooms_filter)
        .map(|room_id: String, ws: warp::ws::Ws, rooms: Rooms| {
            ws.on_upgrade(move |socket| handle_connection(socket, room_id, rooms))
        });

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    let listener = TcpListener::bind(addr).await.unwrap();
    let actual_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        warp::serve(ws_route).incoming(listener).run().await;
    });

    actual_addr.port()
}

async fn handle_connection(socket: warp::ws::WebSocket, room_id: String, rooms: Rooms) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel();

    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    let device_id = Arc::new(RwLock::new(String::new()));
    let did = device_id.clone();
    let rooms_write = rooms.clone();
    let room_id_write = room_id.clone();
    let tx_write = tx.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if msg.is_text() || msg.is_binary() {
                let text = match msg.to_str() {
                    Ok(t) => t.to_string(),
                    Err(_) => continue,
                };
                if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                    match signal {
                        SignalMessage::Join { device_id: id, .. } => {
                            *did.write().await = id.clone();

                            let mut rooms = rooms_write.write().await;
                            let room = rooms.entry(room_id_write.clone()).or_insert_with(|| Room {
                                clients: HashMap::new(),
                            });

                            if room.clients.len() >= MAX_MESH_DEVICES {
                                let err = SignalMessage::Error {
                                    message: format!(
                                        "Room is full (max {} devices)",
                                        MAX_MESH_DEVICES
                                    ),
                                };
                                let _ = tx_write.send(warp::ws::Message::text(
                                    serde_json::to_string(&err).unwrap(),
                                ));
                                break;
                            }

                            room.clients.insert(id.clone(), tx_write.clone());

                            let existing_peers: Vec<String> =
                                room.clients.keys().filter(|k| *k != &id).cloned().collect();
                            let peer_list = SignalMessage::PeerList {
                                peers: existing_peers,
                            };
                            let _ = tx_write.send(warp::ws::Message::text(
                                serde_json::to_string(&peer_list).unwrap(),
                            ));

                            for (other_id, client) in room.clients.iter() {
                                if other_id != &id {
                                    let msg = SignalMessage::PeerJoined {
                                        device_id: id.clone(),
                                    };
                                    let _ = client.send(warp::ws::Message::text(
                                        serde_json::to_string(&msg).unwrap(),
                                    ));
                                }
                            }

                            let peer_count = room.clients.len();
                            let joined = SignalMessage::Joined {
                                device_id: id,
                                room_id: room_id_write.clone(),
                                peer_count,
                            };
                            let _ = tx_write.send(warp::ws::Message::text(
                                serde_json::to_string(&joined).unwrap(),
                            ));
                        }
                        SignalMessage::Offer { payload, target } => {
                            let d_id = did.read().await.clone();
                            let msg = SignalMessage::Offer {
                                payload,
                                target: target.clone(),
                            };
                            relay(&rooms_write, &room_id_write, &d_id, &target, msg).await;
                        }
                        SignalMessage::Answer { payload, target } => {
                            let d_id = did.read().await.clone();
                            let msg = SignalMessage::Answer {
                                payload,
                                target: target.clone(),
                            };
                            relay(&rooms_write, &room_id_write, &d_id, &target, msg).await;
                        }
                        SignalMessage::IceCandidate { payload, target } => {
                            let d_id = did.read().await.clone();
                            let msg = SignalMessage::IceCandidate {
                                payload,
                                target: target.clone(),
                            };
                            relay(&rooms_write, &room_id_write, &d_id, &target, msg).await;
                        }
                        SignalMessage::DeviceAnnounce {
                            device_id: id,
                            metadata,
                        } => {
                            let d_id = did.read().await.clone();
                            let msg = SignalMessage::DeviceAnnounce {
                                device_id: d_id,
                                metadata,
                            };
                            let rooms = rooms_write.read().await;
                            if let Some(room) = rooms.get(&room_id_write) {
                                for (peer_id, client) in &room.clients {
                                    if peer_id != &id {
                                        let _ = client.send(warp::ws::Message::text(
                                            serde_json::to_string(&msg).unwrap(),
                                        ));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let d_id = did.read().await.clone();
        if !d_id.is_empty() {
            let mut rooms = rooms_write.write().await;
            if let Some(room) = rooms.get_mut(&room_id_write) {
                room.clients.remove(&d_id);
                for (_, client) in room.clients.iter() {
                    let msg = SignalMessage::PeerDisconnected {
                        device_id: d_id.clone(),
                    };
                    let _ = client.send(warp::ws::Message::text(
                        serde_json::to_string(&msg).unwrap(),
                    ));
                }
                if room.clients.is_empty() {
                    rooms.remove(&room_id_write);
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}

async fn relay(
    rooms: &RwLock<HashMap<String, Room>>,
    room_id: &str,
    sender_id: &str,
    target_id: &str,
    msg: SignalMessage,
) {
    let rooms = rooms.read().await;
    if let Some(room) = rooms.get(room_id) {
        if target_id == "peer" {
            for (id, client) in &room.clients {
                if id != sender_id {
                    let json = serde_json::to_string(&msg).unwrap();
                    let _ = client.send(warp::ws::Message::text(json));
                }
            }
        } else if let Some(client) = room.clients.get(target_id) {
            let json = serde_json::to_string(&msg).unwrap();
            let _ = client.send(warp::ws::Message::text(json));
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

    async fn connect_client(port: u16, room_id: &str) -> TestStream {
        let url = format!("ws://127.0.0.1:{}/{}", port, room_id);
        let (ws, _) = connect_async(&url).await.unwrap();
        ws
    }

    async fn send_join(ws: &mut TestStream, device_id: &str) {
        let join = SignalMessage::Join {
            device_id: device_id.to_string(),
        };
        ws.send(Message::Text(serde_json::to_string(&join).unwrap().into()))
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
        let port = start(0).await;
        assert!(port > 0, "Server should bind to a port");

        let url = format!("ws://127.0.0.1:{}/test-room", port);
        let result = connect_async(&url).await;
        assert!(result.is_ok(), "Should connect to server");
    }

    #[tokio::test]
    async fn test_two_clients_join_same_room() {
        let port = start(0).await;

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
        let port = start(0).await;

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
        let port = start(0).await;

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
        let port = start(0).await;

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
        let port = start(0).await;

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
        let port = start(0).await;

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
        let port = start(0).await;

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
        let port = start(0).await;

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
}
