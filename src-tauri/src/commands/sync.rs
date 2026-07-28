use crate::common::get_config_path;
use crate::database;
use crate::transport;

use crate::common::emit_log;
use siegu_core::mesh_transport::MeshTransport;
use siegu_core::SavedSession;
use std::sync::Arc;
use tauri::Manager;

/// Pure business logic — inserts or replaces a device in the database.
pub fn do_join_network(db: &database::Database, ip: &str, name: &str) {
    let _ = db.connection.execute(
        "INSERT OR REPLACE INTO device(ip, name) VALUES(?1, ?2)",
        (ip, name),
    );
}

/// Pure business logic — removes a device by name from the database.
pub fn do_remove_device(db: &database::Database, name: &str) -> Result<(), String> {
    db.connection
        .execute("DELETE FROM device WHERE name = ?1", [name])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Pure business logic — lists devices and prepends the host device.
pub fn do_list_devices(db: &database::Database) -> Vec<database::DeviceInfo> {
    let mut devices = db.list_devices();

    for peer in db.list_peer_devices() {
        devices.push(database::DeviceInfo {
            id: peer.device_id,
            title: peer.name,
            icon: "mdi-cellphone".to_string(),
            up_to_date: true,
            host: false,
            photo_count: 0,
            video_count: 0,
            os: peer.os,
        });
    }

    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let (photo_count, video_count) = db.get_media_counts();

    devices.insert(
        0,
        database::DeviceInfo {
            id: "host".to_string(),
            title: format!("Siegu ({hostname})"),
            icon: "mdi-laptop".to_string(),
            up_to_date: true,
            host: true,
            photo_count,
            video_count,
            os: std::env::consts::OS.to_string(),
        },
    );

    devices
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn start_webrtc_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::WebRtcState>,
    roomId: String,
    isInitiator: bool,
    signalingUrl: String,
) -> Result<(), String> {
    use crate::database;
    let app_handle = app.clone();
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return Err("Config error".to_string());
    }

    let db = database::Database::new(&config_path);
    let config = db.get_state();
    let tier = config.get("tier").map(|s| s.as_str()).unwrap_or("free");
    if tier == "free" && !signalingUrl.contains("127.0.0.1") && !signalingUrl.contains("localhost")
    {
        return Err("Free tier does not support remote sync. Use LAN sync instead.".to_string());
    }

    let sync_tx = Arc::clone(&state.sync_tx);

    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            emit_log(&app, "Aborting previous WebRTC session".to_string());
            handle.abort();
        }

        let config_path_clone = config_path.clone();
        let room_id_clone = roomId.clone();
        let signaling_url_clone = signalingUrl.clone();
        let is_initiator_clone = isInitiator;

        let handle = tauri::async_runtime::spawn(async move {
            let transport = transport::create_transport(
                roomId,
                isInitiator,
                signalingUrl,
                config_path,
                app_handle,
                Some(sync_tx),
            );
            let _ = transport.start().await;
        });

        *session = Some(handle);

        let db2 = database::Database::new(&config_path_clone);
        db2.save_session(&SavedSession {
            room_id: room_id_clone,
            signaling_url: signaling_url_clone,
            port: 0,
            is_initiator: is_initiator_clone,
            passphrase: String::new(),
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn start_lan_host(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::WebRtcState>,
    mdns_state: tauri::State<'_, crate::MdnsState>,
    room_id: String,
    is_initiator: bool,
) -> Result<(), String> {
    let app_handle = app.clone();
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return Err("Config error".to_string());
    }

    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "siegu-host".to_string());

    let daemon = siegu_core::mdns::create_daemon().map_err(|e| e.to_string())?;
    {
        let mut d = mdns_state.daemon.lock().map_err(|e| e.to_string())?;
        *d = Some(daemon.clone());
    }

    let port = MeshTransport::start_lan_server(0)
        .await
        .map_err(|e| e.to_string())?;

    let sync_tx = Arc::clone(&state.sync_tx);

    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            emit_log(&app, "Aborting previous WebRTC session".to_string());
            handle.abort();
        }

        let config_path_for_session = config_path.clone();
        let room_id_for_session = room_id.clone();
        let hostname_for_task = hostname.clone();

        let handle = tauri::async_runtime::spawn(async move {
            let mut transport = transport::create_transport(
                room_id.clone(),
                is_initiator,
                String::new(),
                config_path,
                app_handle.clone(),
                Some(sync_tx),
            );

            if let Err(e) =
                siegu_core::mdns::register_service(&daemon, &hostname_for_task, port, &room_id)
            {
                emit_log(&app_handle, format!("mDNS registration failed: {e}"));
            } else {
                emit_log(
                    &app_handle,
                    format!("mDNS registered: {hostname_for_task} on port {port}"),
                );
            }
            transport.signaling_url = format!("ws://127.0.0.1:{port}");
            if let Err(e) = transport.start().await {
                emit_log(&app_handle, format!("WebRTC start failed: {e}"));
            }
        });

        *session = Some(handle);

        let db2 = database::Database::new(&config_path_for_session);
        db2.save_session(&SavedSession {
            room_id: room_id_for_session,
            signaling_url: format!("ws://127.0.0.1:{port}"),
            port,
            is_initiator,
            passphrase: String::new(),
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn discover_lan_devices(
    app: tauri::AppHandle,
    timeout_secs: u64,
) -> Result<Vec<siegu_core::mdns::DiscoveredHost>, String> {
    let daemon = siegu_core::mdns::create_daemon().map_err(|e| e.to_string())?;
    let hosts =
        siegu_core::mdns::discover_hosts(&daemon, timeout_secs).map_err(|e| e.to_string())?;
    emit_log(&app, format!("Discovered {} LAN device(s)", hosts.len()));
    Ok(hosts)
}

#[tauri::command]
pub async fn stop_webrtc_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::WebRtcState>,
    mdns_state: tauri::State<'_, crate::MdnsState>,
) -> Result<(), String> {
    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            emit_log(&app, "Stopping WebRTC session".to_string());
            handle.abort();
        }
    }
    {
        let mut tx = state.sync_tx.lock().await;
        *tx = None;
    }
    if let Ok(mut d) = mdns_state.daemon.lock() {
        if let Some(daemon) = d.take() {
            let _ = daemon.shutdown();
            emit_log(&app, "mDNS daemon shut down".to_string());
        }
    }
    let config_path = get_config_path(&app);
    if !config_path.is_empty() {
        let db = database::Database::new(&config_path);
        db.clear_session();
    }
    Ok(())
}

#[tauri::command]
pub async fn join_network(app: tauri::AppHandle, ip: String, name: String) {
    use crate::database;
    emit_log(&app, format!("Adding new device: {name} at {ip}"));
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let db = database::Database::new(&path);
    do_join_network(&db, &ip, &name);
}

#[tauri::command]
pub async fn remove_device(app: tauri::AppHandle, name: String) -> Result<(), String> {
    use crate::database;
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Config error".to_string());
    }
    let db = database::Database::new(&path);
    do_remove_device(&db, &name)
}

#[tauri::command]
pub async fn list_devices(app: tauri::AppHandle) -> String {
    use crate::database;
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let db = database::Database::new(&path);
    let devices = do_list_devices(&db);
    serde_json::to_string(&devices).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn request_start_sync(state: tauri::State<'_, crate::WebRtcState>) -> Result<(), String> {
    let mut tx_lock = state.sync_tx.lock().await;
    if let Some(tx) = tx_lock.as_mut() {
        tx.send(transport::SyncMessage::StartSync)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn initialize_sync_folder(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let target = std::path::PathBuf::from(&path).join("siegu");
    if let Err(e) = std::fs::create_dir_all(&target) {
        return Err(format!("Failed to create folder at {target:?}: {e}"));
    }
    crate::commands::directories::add_directory(app, path).await;
    Ok(())
}

#[tauri::command]
pub async fn get_media_server_port(app: tauri::AppHandle) -> u16 {
    app.state::<transport::MediaServerState>().port
}

#[tauri::command]
pub async fn generate_pairing_codes() -> Result<siegu_core::PairingCodes, String> {
    siegu_core::generate_pairing_codes()
}

#[tauri::command]
pub async fn hash_pairing_code(input: String) -> Result<String, String> {
    siegu_core::hash_pairing_code(input)
}

#[tauri::command]
pub async fn auto_reconnect(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::WebRtcState>,
) -> Result<bool, String> {
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return Ok(false);
    }
    let db = database::Database::new(&config_path);
    match db.load_session() {
        Some(session) => {
            emit_log(
                &app,
                format!("Auto-reconnecting to room {}", session.room_id),
            );
            let app_handle = app.clone();
            let sync_tx = Arc::clone(&state.sync_tx);

            if let Ok(mut active) = state.active_session.lock() {
                let handle = tauri::async_runtime::spawn(async move {
                    let transport = transport::create_transport(
                        session.room_id,
                        session.is_initiator,
                        session.signaling_url,
                        config_path,
                        app_handle,
                        Some(sync_tx),
                    );
                    let _ = transport.start().await;
                });
                *active = Some(handle);
            }
            Ok(true)
        }
        None => Ok(false),
    }
}

#[tauri::command]
pub async fn clear_saved_session(app: tauri::AppHandle) -> Result<(), String> {
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return Err("Config error".to_string());
    }
    let db = database::Database::new(&config_path);
    db.clear_session();
    Ok(())
}

#[tauri::command]
pub async fn list_peer_devices(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let db = database::Database::new(&path);
    let peers = db.list_peer_devices();
    serde_json::to_string(&peers).unwrap_or("[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn join_network_adds_device() {
        let (db, _dir) = test_db();
        do_join_network(&db, "192.168.1.10", "Phone");
        let devices = db.list_devices();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].title, "Phone");
    }

    #[test]
    fn join_network_multiple_devices() {
        let (db, _dir) = test_db();
        do_join_network(&db, "192.168.1.10", "Phone");
        do_join_network(&db, "192.168.1.11", "Tablet");
        let devices = db.list_devices();
        assert_eq!(devices.len(), 2);
    }

    #[test]
    fn remove_device() {
        let (db, _dir) = test_db();
        do_join_network(&db, "192.168.1.10", "Phone");
        do_remove_device(&db, "Phone").unwrap();
        assert!(db.list_devices().is_empty());
    }

    #[test]
    fn remove_nonexistent_device_no_error() {
        let (db, _dir) = test_db();
        do_remove_device(&db, "Ghost").unwrap();
    }

    #[test]
    fn list_devices_includes_host() {
        let (db, _dir) = test_db();
        let devices = do_list_devices(&db);
        assert_eq!(devices.len(), 1);
        assert!(devices[0].host);
        assert_eq!(devices[0].id, "host");
        assert!(devices[0].title.starts_with("Siegu"));
    }

    #[test]
    fn list_devices_with_remote_devices() {
        let (db, _dir) = test_db();
        do_join_network(&db, "192.168.1.10", "Tablet");
        do_join_network(&db, "192.168.1.11", "Phone");
        let devices = do_list_devices(&db);
        assert_eq!(devices.len(), 3);
        assert!(devices[0].host);
        assert_eq!(devices[1].title, "Tablet");
        assert_eq!(devices[2].title, "Phone");
    }

    #[test]
    fn list_devices_host_has_media_counts() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("ph1", "/a.jpg"), make_photo("ph2", "/b.jpg")])
            .unwrap();
        let devices = do_list_devices(&db);
        assert_eq!(devices[0].photo_count, 2);
    }
}
