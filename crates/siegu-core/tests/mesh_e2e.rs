//! Additional mesh E2E coverage that the desktop app and CLI rely on:
//!
//! - `two_joiners_connect_with_initiator_flag`: the `mesh join --initiator`
//!   path — two *joiner* peers (neither is the LAN host) connect through a
//!   signaling server; exactly one is the WebRTC initiator.
//! - `mesh_delta_sync_transfers_only_new_photos`: a joiner reconnects after a
//!   peer adds a photo and must receive *only* the new photo, not a full
//!   re-transfer (delta sync via manifest comparison).
//! - `mdns_discovers_lan_host`: the LAN signaling host registers an mDNS
//!   `_siegu._tcp` service and the discovery API finds it on the same network.
//!
//! All tests use real sockets + real WebRTC; no ML models or Tauri runtime.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use siegu_core::database::{Database, ImportedPhoto};
use siegu_core::mesh::SyncEvent;
use siegu_core::mesh_transport::MeshTransport;
use siegu_core::{SyncMessage, SyncProgress};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/faces");

struct TestEvent {
    states: Mutex<Vec<String>>,
    logs: Mutex<Vec<String>>,
    peers: Mutex<Vec<String>>,
    received: Mutex<Vec<String>>,
    connected: AtomicBool,
}

impl Default for TestEvent {
    fn default() -> Self {
        Self {
            states: Mutex::new(Vec::new()),
            logs: Mutex::new(Vec::new()),
            peers: Mutex::new(Vec::new()),
            received: Mutex::new(Vec::new()),
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

    fn received_ids(&self) -> Vec<String> {
        self.received.lock().unwrap().clone()
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

    fn on_photo_received(&self, photo_id: String, _path: String) {
        self.received.lock().unwrap().push(photo_id);
    }

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

/// Seed a peer config with one photo pointing at a real file on disk.
fn import_photo(config_path: &str, id: &str, file: &std::path::Path) {
    let mut db = Database::new(config_path);
    db.import_photo(ImportedPhoto {
        id,
        location: file.to_string_lossy().as_ref(),
        created: "2024-01-01T00:00:00Z",
        latitude: None,
        longitude: None,
        objects_json: "[]",
        faces_json: "[]",
        encoded: "e2e",
        caption: None,
        aesthetics_score: None,
        received: false,
    });
}

/// Whether the joiner has already imported the given photo into its DB.
fn joiner_has_photo(config_path: &str, id: &str) -> bool {
    Database::new(config_path)
        .get_photo_sync_info()
        .iter()
        .any(|p| p.id == id)
}

async fn start_lan_signal() -> String {
    let server = MeshTransport::start_lan_server(0)
        .await
        .expect("failed to start in-process LAN signaling server");
    format!("ws://127.0.0.1:{}", server.port)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_joiners_connect_with_initiator_flag() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .try_init();

    let tmp = tempfile::tempdir().expect("tempdir");
    let config_a = tmp.path().join("joiner-a").display().to_string();
    let config_b = tmp.path().join("joiner-b").display().to_string();
    std::fs::create_dir_all(&config_a).unwrap();
    std::fs::create_dir_all(&config_b).unwrap();

    let url = start_lan_signal().await;
    let room_id = "join-initiator-room";

    let event_a = Arc::new(TestEvent::default());
    let event_b = Arc::new(TestEvent::default());

    // Both peers are JOINERS: neither runs the LAN host. This is the exact
    // shape of `siegu mesh join <room> --server <url> --initiator` on one
    // device and plain `siegu mesh join <room> --server <url>` on the other.
    let peer_a = new_peer(
        room_id,
        true,
        &url,
        &config_a,
        "initiator-joiner",
        Arc::clone(&event_a),
    );
    let peer_b = new_peer(
        room_id,
        false,
        &url,
        &config_b,
        "passive-joiner",
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
    wait_for(timeout, "both joiners to connect", || {
        event_a.connected.load(Ordering::SeqCst) && event_b.connected.load(Ordering::SeqCst)
    })
    .await;
    wait_for(timeout, "initiator joiner data channel to open", || {
        event_a.has_state("Secure Data Channel Ready")
    })
    .await;

    let names_a = event_a.peer_names();
    let names_b = event_b.peer_names();
    assert!(
        names_a.iter().any(|n| n.contains("passive-joiner")),
        "initiator joiner should see the passive joiner, got {names_a:?}"
    );
    assert!(
        names_b.iter().any(|n| n.contains("initiator-joiner")),
        "passive joiner should see the initiator joiner, got {names_b:?}"
    );

    // Bidirectional message over the join-to-join channel.
    peer_a
        .send_message(SyncMessage::MetadataUpdate {
            photo_id: "join-e2e-photo".to_string(),
            caption: Some("two-joiner caption".to_string()),
            aesthetics_score: Some(0.25),
            indexed: 3,
        })
        .await
        .expect("send_message should succeed once channel is open");
    wait_for(
        timeout,
        "passive joiner to receive the metadata update",
        || event_b.has_log("Metadata updated for join-e2e-photo"),
    )
    .await;

    handle_a.abort();
    handle_b.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mesh_delta_sync_transfers_only_new_photos() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .try_init();

    let tmp = tempfile::tempdir().expect("tempdir");
    let config_a = tmp.path().join("host-a").display().to_string();
    let config_b = tmp.path().join("joiner-b").display().to_string();
    std::fs::create_dir_all(&config_a).unwrap();
    std::fs::create_dir_all(&config_b).unwrap();

    let photo_a = std::path::Path::new(FIXTURES).join("einstein_1.jpg");
    let photo_b = std::path::Path::new(FIXTURES).join("einstein_2.jpg");
    assert!(photo_a.exists(), "fixture missing: {photo_a:?}");
    assert!(photo_b.exists(), "fixture missing: {photo_b:?}");
    let sha_a = siegu_core::mesh::MeshManager::compute_file_checksum(&photo_a).unwrap();
    let sha_b = siegu_core::mesh::MeshManager::compute_file_checksum(&photo_b).unwrap();

    // Host starts with one photo; the joiner starts empty.
    import_photo(&config_a, "photo-a", &photo_a);

    let url = start_lan_signal().await;
    let room_id = "delta-sync-room";

    let event_a = Arc::new(TestEvent::default());
    let event_b = Arc::new(TestEvent::default());

    async fn spawn_round(
        url: String,
        room_id: &'static str,
        config_a: String,
        config_b: String,
        event_a: Arc<TestEvent>,
        event_b: Arc<TestEvent>,
    ) -> (
        tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
        tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    ) {
        let peer_a = new_peer(
            room_id,
            true,
            &url,
            &config_a,
            "host-a",
            Arc::clone(&event_a),
        );
        let peer_b = new_peer(
            room_id,
            false,
            &url,
            &config_b,
            "joiner-b",
            Arc::clone(&event_b),
        );
        (
            tokio::spawn(async move { peer_a.start().await }),
            tokio::spawn(async move { peer_b.start().await }),
        )
    }

    // ── Round 1: full initial sync ────────────────────────────────────────
    let (handle_a, handle_b) = spawn_round(
        url.clone(),
        room_id,
        config_a.clone(),
        config_b.clone(),
        Arc::clone(&event_a),
        Arc::clone(&event_b),
    )
    .await;

    let timeout = Duration::from_secs(90);
    wait_for(timeout, "round 1 joiner to receive photo-a", || {
        event_b.received_ids().contains(&"photo-a".to_string())
    })
    .await;
    // on_photo_received fires before the async DB import; make sure the joiner
    // actually recorded photo-a in its manifest before we tear down.
    wait_for(timeout, "round 1 joiner DB to import photo-a", || {
        joiner_has_photo(&config_b, "photo-a")
    })
    .await;
    handle_a.abort();
    handle_b.abort();

    let received_b = config_b.clone() + "/Siegu/siegu/einstein_1.jpg";
    let received_a_path = std::path::Path::new(&received_b);
    assert!(
        received_a_path.exists(),
        "received file missing: {received_b}"
    );
    let received_sha =
        siegu_core::mesh::MeshManager::compute_file_checksum(received_a_path).unwrap();
    assert_eq!(
        received_sha, sha_a,
        "round 1 file must match source byte-for-byte"
    );

    // ── Round 2: host adds a second photo; only it may transfer ───────────
    import_photo(&config_a, "photo-b", &photo_b);
    event_b.received.lock().unwrap().clear();

    let (handle_a, handle_b) = spawn_round(
        url.clone(),
        room_id,
        config_a.clone(),
        config_b.clone(),
        Arc::clone(&event_a),
        Arc::clone(&event_b),
    )
    .await;

    wait_for(timeout, "round 2 joiner to receive photo-b", || {
        event_b.received_ids().contains(&"photo-b".to_string())
    })
    .await;
    // Give the manifest comparison a beat so a spurious photo-a re-transfer
    // would surface before we tear down.
    tokio::time::sleep(Duration::from_secs(3)).await;
    handle_a.abort();
    handle_b.abort();

    let round2 = event_b.received_ids();
    assert!(
        round2 == vec!["photo-b".to_string()],
        "delta sync must transfer ONLY the new photo, got {round2:?}"
    );

    let received_b_path = std::path::Path::new(&config_b)
        .join("Siegu")
        .join("siegu")
        .join("einstein_2.jpg");
    assert!(received_b_path.exists(), "received photo-b missing");
    let received_b_sha =
        siegu_core::mesh::MeshManager::compute_file_checksum(&received_b_path).unwrap();
    assert_eq!(
        received_b_sha, sha_b,
        "round 2 file must match source byte-for-byte"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mdns_discovers_lan_host() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .try_init();

    let server = MeshTransport::start_lan_server(0)
        .await
        .expect("failed to start in-process LAN signaling server");

    let daemon = match siegu_core::mdns::create_daemon() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping mDNS test: cannot create daemon on this host: {e}");
            return;
        }
    };
    siegu_core::mdns::register_service(&daemon, "siegu-e2e-host", server.port)
        .expect("register mDNS service");

    // Give the daemon a moment to announce, then browse the network.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let hosts = siegu_core::mdns::discover_hosts(&daemon, 5).expect("browse for hosts");

    siegu_core::mdns::unregister_service(&daemon, "siegu-e2e-host");
    daemon.shutdown();

    assert!(
        hosts.iter().any(|h| h.port == server.port),
        "expected mDNS discovery to find the LAN host on port {}, got {hosts:?}",
        server.port
    );
}
