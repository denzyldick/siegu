use std::sync::Arc;

use siegu_core::mesh_transport::MeshTransport;
use warp::Filter;

pub use siegu_core::mesh::{SyncMessage, SyncProgress};
pub use siegu_core::SignalMessage;

pub fn get_or_create_device_id(config_path: &str) -> String {
    use crate::database;
    let db = database::Database::new(config_path);
    let state = db.get_state();
    if let Some(id) = state.get("device_id") {
        return id.clone();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let mut new_state = std::collections::HashMap::new();
    new_state.insert("device_id".to_string(), id.clone());
    db.set_state(new_state);
    id
}

pub struct MediaServerState {
    pub port: u16,
}

pub fn start_media_server(_config_path: String) -> u16 {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let media = warp::path("media").and(warp::fs::dir(std::path::PathBuf::from("/")));
            let routes = media;
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

pub fn create_transport(
    room_id: String,
    is_initiator: bool,
    signaling_url: String,
    config_path: String,
    app: tauri::AppHandle,
    external_tx: Option<
        Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<SyncMessage>>>>,
    >,
    connected: Option<Arc<std::sync::atomic::AtomicBool>>,
) -> MeshTransport {
    let device_id = get_or_create_device_id(&config_path);
    let device_name = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "siegu-device".to_string());
    let models_enabled = Vec::new();

    let sync_tx = external_tx
        .clone()
        .unwrap_or_else(|| Arc::new(tokio::sync::Mutex::new(None)));
    let event = Arc::new(super::tauri_sync_event::TauriSyncEvent {
        app: app.clone(),
        config_path: config_path.clone(),
        sync_tx,
        offline_notified: std::sync::atomic::AtomicBool::new(false),
        connected: connected.unwrap_or_default(),
    });

    let mut transport = MeshTransport::new(
        room_id,
        is_initiator,
        signaling_url,
        config_path,
        device_id,
        device_name,
        models_enabled,
        event,
    );

    if let Some(ext) = external_tx {
        transport = transport.with_external_tx(ext);
    }

    transport
}
