use std::sync::Arc;

use siegu_core::mesh_transport::MeshTransport;
use warp::Filter;

pub use siegu_core::mesh::{SyncMessage, SyncProgress};
pub use siegu_core::SignalMessage;

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
) -> MeshTransport {
    let device_id = uuid::Uuid::new_v4().to_string();
    let device_name = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "siegu-device".to_string());
    let models_enabled = Vec::new();

    let sync_tx = external_tx.clone().unwrap_or_else(|| {
        Arc::new(tokio::sync::Mutex::new(None))
    });
    let event = Arc::new(super::tauri_sync_event::TauriSyncEvent {
        app: app.clone(),
        config_path: config_path.clone(),
        sync_tx,
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
