//! End-to-end sync test: two `MeshTransport` peers connect over a real
//! in-process LAN signaling server, establish a WebRTC data channel, and
//! exchange protocol messages.
//!
//! This exercises the exact same code path the desktop app and CLI use for
//! LAN sync (signaling server + WebRTC + `MeshManager::handle_sync_message`),
//! without needing the Tauri runtime or downloaded models.
//!
//! Set `SIEGU_SIGNAL_URL` to run the peers against an external signaling
//! server instead (e.g. the published `siegu-signal` Docker container). Both
//! peers will dial `{SIEGU_SIGNAL_URL}/{room}` in LAN mode.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use siegu_core::database::Database;
use siegu_core::mesh::SyncEvent;
use siegu_core::mesh_transport::MeshTransport;
use siegu_core::{SyncMessage, SyncProgress};

struct TestEvent {
    states: Mutex<Vec<String>>,
    logs: Mutex<Vec<String>>,
    peers: Mutex<Vec<String>>,
    connected: AtomicBool,
}

impl Default for TestEvent {
    fn default() -> Self {
        Self {
            states: Mutex::new(Vec::new()),
            logs: Mutex::new(Vec::new()),
            peers: Mutex::new(Vec::new()),
            connected: AtomicBool::new(false),
        }
    }
}

impl TestEvent {
    fn has_state(&self, needle: &str) -> bool {
        self.states
            .lock()
            .unwrap()
            .iter()
            .any(|s| s.contains(needle))
    }

    fn has_log(&self, needle: &str) -> bool {
        self.logs.lock().unwrap().iter().any(|s| s.contains(needle))
    }

    fn peer_names(&self) -> Vec<String> {
        self.peers.lock().unwrap().clone()
    }
}

impl SyncEvent for TestEvent {
    fn on_state_change(&self, state: &str) {
        self.states.lock().unwrap().push(state.to_string());
    }

    fn on_log(&self, message: &str) {
        self.logs.lock().unwrap().push(message.to_string());
    }

    fn on_sync_progress(&self, _progress: SyncProgress) {}

    fn on_photo_received(&self, _photo_id: String, _path: String) {}

    fn on_sync_error(&self, error: String) {
        self.logs.lock().unwrap().push(format!("ERROR: {error}"));
    }

    fn on_peer_connected(
        &self,
        peer_id: String,
        _peer_name: String,
        _peer_os: String,
        _models_enabled: Vec<String>,
        _protocol_version: u8,
    ) {
        self.peers.lock().unwrap().push(peer_id);
        self.connected.store(true, Ordering::SeqCst);
    }

    fn on_peer_disconnected(&self, _peer_id: String) {}

    fn on_device_registered(&self, _db: &Database) {}

    fn on_metadata_updated(
        &self,
        _photo_id: &str,
        _caption: Option<&str>,
        _aesthetics_score: Option<f64>,
    ) {
    }

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

fn new_peer(
    room_id: &str,
    is_initiator: bool,
    signaling_url: &str,
    config_path: &str,
    device_name: &str,
    event: Arc<TestEvent>,
) -> MeshTransport {
    MeshTransport::new(
        room_id.to_string(),
        is_initiator,
        signaling_url.to_string(),
        config_path.to_string(),
        format!("{device_name}-{is_initiator}"),
        device_name.to_string(),
        Vec::new(),
        event,
    )
}

async fn wait_for<F: Fn() -> bool>(timeout: Duration, what: &str, check: F) {
    let start = std::time::Instant::now();
    while !check() {
        if start.elapsed() > timeout {
            panic!("timed out waiting for {what}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lan_mesh_peers_connect_and_exchange_messages() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .try_init();

    let tmp = tempfile::tempdir().expect("tempdir");
    let config_a = tmp.path().join("peer-a").display().to_string();
    let config_b = tmp.path().join("peer-b").display().to_string();
    std::fs::create_dir_all(&config_a).unwrap();
    std::fs::create_dir_all(&config_b).unwrap();

    let room_id = "e2e-test-room";

    let signaling_url = match std::env::var("SIEGU_SIGNAL_URL") {
        Ok(url) => url.trim_end_matches('/').to_string(),
        Err(_) => {
            let server = MeshTransport::start_lan_server(0)
                .await
                .expect("failed to start in-process LAN signaling server");
            format!("ws://127.0.0.1:{}", server.port)
        }
    };

    let event_a = Arc::new(TestEvent::default());
    let event_b = Arc::new(TestEvent::default());

    let peer_a = new_peer(
        room_id,
        true,
        &signaling_url,
        &config_a,
        "peer-a",
        Arc::clone(&event_a),
    );
    let peer_b = new_peer(
        room_id,
        false,
        &signaling_url,
        &config_b,
        "peer-b",
        Arc::clone(&event_b),
    );

    let handle_a = {
        let peer = peer_a.clone();
        tokio::spawn(async move { peer.start().await })
    };
    let handle_b = {
        let peer = peer_b.clone();
        tokio::spawn(async move { peer.start().await })
    };

    let timeout = Duration::from_secs(60);

    wait_for(timeout, "both peers to connect", || {
        event_a.connected.load(Ordering::SeqCst) && event_b.connected.load(Ordering::SeqCst)
    })
    .await;

    wait_for(timeout, "initiator data channel to open", || {
        event_a.has_state("Secure Data Channel Ready")
    })
    .await;

    let names_a = event_a.peer_names();
    let names_b = event_b.peer_names();
    assert!(
        names_a.iter().any(|n| n.contains("peer-b")),
        "peer-a should see peer-b, got {names_a:?}"
    );
    assert!(
        names_b.iter().any(|n| n.contains("peer-a")),
        "peer-b should see peer-a, got {names_b:?}"
    );

    // Bidirectional protocol message: a MetadataUpdate sent by peer-a must be
    // received and applied by peer-b's MeshManager handler.
    let photo_id = "e2e-photo-1";
    peer_a
        .send_message(SyncMessage::MetadataUpdate {
            photo_id: photo_id.to_string(),
            caption: Some("e2e caption".to_string()),
            aesthetics_score: Some(0.5),
            indexed: 2,
            deleted_at: None,
        })
        .await
        .expect("send_message should succeed once channel is open");

    wait_for(timeout, "peer-b to receive the metadata update", || {
        event_b.has_log(&format!("Metadata updated for {photo_id}"))
    })
    .await;

    assert!(
        event_b.has_log("Metadata updated for e2e-photo-1"),
        "peer-b should log the applied metadata update: {:?}",
        event_b.logs.lock().unwrap()
    );

    // Tear down cleanly.
    handle_a.abort();
    handle_b.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lan_mesh_rejects_empty_room() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = tmp.path().join("peer-a").display().to_string();
    std::fs::create_dir_all(&config).unwrap();

    let server = MeshTransport::start_lan_server(0)
        .await
        .expect("failed to start in-process LAN signaling server");
    let url = format!("ws://127.0.0.1:{}", server.port);

    let event = Arc::new(TestEvent::default());
    let peer = new_peer("", true, &url, &config, "peer-a", Arc::clone(&event));

    let result = peer.start().await;
    assert!(result.is_err(), "empty room id must be rejected");

    let states = event.states.lock().unwrap();
    assert!(
        states.iter().any(|s| s.contains("Room ID is missing")),
        "expected Room ID error state, got {states:?}"
    );
}
