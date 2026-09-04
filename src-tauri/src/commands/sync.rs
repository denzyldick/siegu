use crate::common::get_config_path;
use crate::database;
use crate::transport;

use crate::common::emit_log;
use rand::RngCore;
use siegu_core::{PeerDevice, SavedSession};
use std::sync::Arc;
use tauri::{Emitter, Manager};

#[derive(Debug, Clone, serde::Serialize)]
pub struct LanHostInfo {
    pub ip: String,
    pub port: u16,
}

/// Storage usage against the configured cap (#10).
#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageUsage {
    pub used: u64,
    pub quota: u64,
}

/// Returns the first non-loopback IPv4 address on the machine.
fn get_local_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:53").ok()?;
    let addr = s.local_addr().ok()?;
    Some(addr.ip().to_string())
}

/// Pure business logic — inserts or updates a peer device.
pub fn do_join_network(db: &database::Database, ip: &str, name: &str) {
    let device_id = uuid::Uuid::new_v4().to_string();
    db.upsert_peer_device(&PeerDevice {
        device_id,
        name: name.to_string(),
        ip: ip.to_string(),
        port: 0,
        device_type: String::new(),
        os: String::new(),
        models_enabled: vec![],
        protocol_version: 1,
        storage_used: 0,
        storage_capacity: 0,
        last_seen: String::new(),
        photo_count: 0,
        video_count: 0,
        remote_photo_count: 0,
        remote_video_count: 0,
    });
}

/// Pure business logic — removes a device by id from the database.
pub fn do_remove_device(db: &database::Database, id: &str) -> Result<(), String> {
    db.remove_peer_device(id);
    Ok(())
}

/// MDI platform logo for a device `os` value (std::env::consts::OS style).
fn os_icon(os: &str) -> &'static str {
    match os {
        "windows" => "mdi-microsoft-windows",
        "macos" => "mdi-apple",
        "ios" => "mdi-apple-ios",
        "android" => "mdi-android",
        "linux" => "mdi-linux",
        "freebsd" | "openbsd" | "netbsd" => "mdi-freebsd",
        _ => "mdi-laptop",
    }
}

/// Pure business logic — lists devices and prepends the host device.
pub fn do_list_devices(db: &database::Database, config_path: &str) -> Vec<database::DeviceInfo> {
    let mut devices: Vec<database::DeviceInfo> = db
        .list_peer_devices()
        .into_iter()
        .map(|peer| database::DeviceInfo {
            id: peer.device_id,
            title: peer.name,
            icon: os_icon(&peer.os).to_string(),
            up_to_date: true,
            host: false,
            photo_count: peer.photo_count,
            video_count: peer.video_count,
            remote_photo_count: peer.remote_photo_count,
            remote_video_count: peer.remote_video_count,
            os: peer.os,
            storage_used: peer.storage_used.max(0) as u64,
            storage_capacity: peer.storage_capacity.max(0) as u64,
        })
        .collect();

    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let state = db.get_state();
    let host_title = state
        .get("device_name")
        .cloned()
        .unwrap_or_else(|| format!("Siegu ({hostname})"));
    let (photo_count, video_count) = db.get_media_counts();

    devices.insert(
        0,
        database::DeviceInfo {
            id: "host".to_string(),
            title: host_title,
            icon: os_icon(std::env::consts::OS).to_string(),
            up_to_date: true,
            host: true,
            photo_count,
            video_count,
            // The host card shows this device's own library; remote_* mirror
            // it so the UI can use one code path.
            remote_photo_count: photo_count,
            remote_video_count: video_count,
            os: std::env::consts::OS.to_string(),
            storage_used: siegu_core::mesh::MeshManager::get_total_storage_used(config_path),
            storage_capacity: siegu_core::mesh::MeshManager::get_storage_quota(config_path),
        },
    );

    devices
}

/// Pure business logic — renames a device. The host device's name is persisted
/// in config state so it survives restarts and is used when advertising over LAN.
pub fn do_rename_device(db: &database::Database, id: &str, new_name: &str) -> Result<(), String> {
    let name = new_name.trim();
    if name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if id == "host" {
        let mut state = std::collections::HashMap::new();
        state.insert("device_name".to_string(), name.to_string());
        db.set_state(state);
        return Ok(());
    }
    db.rename_peer_device(id, name);
    Ok(())
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

    // Pro gate: only verified Pro subscribers may sync through a hosted
    // (cross-network) signalling server. Free tier is limited to a
    // same-network LAN/localhost signaler. The hosted signaler is what lets
    // devices sync when they are on different networks.
    if !siegu_core::mesh::MeshManager::is_pro(&config_path)
        && siegu_core::mesh::MeshManager::signalling_requires_pro(&signalingUrl)
    {
        return Err(
            "Hosted signaler requires Pro. Sync across networks is a Pro feature.".to_string(),
        );
    }

    let sync_tx = Arc::clone(&state.sync_tx);
    let connected = Arc::clone(&state.connected);
    let rpc_pending = Arc::clone(&state.rpc_pending);

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
                Some(connected),
                Some(rpc_pending),
            );
            if let Err(e) = transport.start().await {
                emit_log(&app, format!("WebRTC session failed: {e}"));
            }
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
) -> Result<LanHostInfo, String> {
    start_host_internal(&app, &state, &mdns_state, room_id, is_initiator).await
}

/// Shared host startup: fresh LAN signaling server + mDNS registration + saved session.
async fn start_host_internal(
    app: &tauri::AppHandle,
    state: &crate::WebRtcState,
    mdns_state: &crate::MdnsState,
    room_id: String,
    is_initiator: bool,
) -> Result<LanHostInfo, String> {
    let app_handle = app.clone();
    let config_path = get_config_path(app);
    if config_path.is_empty() {
        return Err("Config error".to_string());
    }

    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "siegu-host".to_string());

    let daemon = {
        let mut d = mdns_state.daemon.lock().map_err(|e| e.to_string())?;
        match d.as_ref() {
            Some(existing) => existing.clone(),
            None => {
                let new_daemon = siegu_core::mdns::create_daemon().map_err(|e| e.to_string())?;
                *d = Some(new_daemon.clone());
                new_daemon
            }
        }
    };

    // Stop the previous signaling server (if any) so old listeners are not left
    // running on stale ports that the phone could resolve and then be stranded on.
    if let Ok(mut ls) = state.lan_server.lock() {
        if let Some(prev) = ls.take() {
            prev.stop();
        }
    }

    let server =
        siegu_core::lan_server::start(siegu_core::lan_server::DEFAULT_LAN_SIGNALING_PORT).await;
    let port = server.port;
    if let Ok(mut ls) = state.lan_server.lock() {
        *ls = Some(server);
    }
    // Store room + port so generate_album_share_url can build the link later (#16).
    if let Ok(mut hi) = state.host_info.lock() {
        *hi = Some(crate::HostInfo {
            room_id: room_id.clone(),
            port,
        });
    }

    let ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());

    // Firewall check: attempt TCP connect to self
    if ip != "127.0.0.1" {
        let addr = format!("{}:{}", ip, port);
        let target: std::net::SocketAddr = addr
            .parse()
            .map_err(|e| format!("invalid LAN address {addr}: {e}"))?;
        match std::net::TcpStream::connect_timeout(
            &target,
            std::time::Duration::from_secs(2),
        ) {
            Ok(_) => emit_log(app, format!("TCP self-check OK — {addr} reachable")),
            Err(e) => emit_log(
                app,
                format!(
                    "WARNING: Cannot reach own LAN address {addr}. Firewall may block incoming connections: {e}"
                ),
            ),
        }
    }

    let sync_tx = Arc::clone(&state.sync_tx);
    let connected = Arc::clone(&state.connected);
    let rpc_pending = Arc::clone(&state.rpc_pending);

    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            emit_log(app, "Aborting previous WebRTC session".to_string());
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
                Some(connected),
                Some(rpc_pending),
            );

            siegu_core::mdns::unregister_service(&daemon, &hostname_for_task);
            if let Err(e) = siegu_core::mdns::register_service(&daemon, &hostname_for_task, port) {
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

    Ok(LanHostInfo { ip, port })
}

/// Generate a shareable URL for an album (#16).
/// Returns `http://IP:PORT/#CODE.ALBUM_ID` for LAN guests.
#[tauri::command]
pub async fn generate_album_share_url(
    state: tauri::State<'_, crate::WebRtcState>,
    album_id: String,
) -> Result<String, String> {
    let hi = state
        .host_info
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("No active host session — start sharing first")?;
    let ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
    Ok(format!(
        "http://{}:{}/#{}.{}",
        ip, hi.port, hi.room_id, album_id
    ))
}

/// Result of starting a collection share.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AlbumShareInfo {
    /// Browser URL the recipient opens (`http://<origin>/#CODE.TOKEN.ALBUM_ID[.MIN]`).
    pub url: String,
    pub room_id: String,
    pub port: u16,
    /// Sharing mode: `"timed"` or `"one_time"`.
    pub mode: String,
    /// Chosen duration in minutes (`timed` mode only).
    pub duration_min: u32,
}

/// Locate the web client bundle (`index.html` + `assets/`) served to browser
/// guests on the share URL. The SAME build as the app's own UI is used so a
/// shared collection opens inside the full Siegu web app (scoped, with the
/// upsell moment) rather than a separate minimal view. Resolution:
/// `SIEGU_WEB_DIST` (packaged deployment), `./dist` (crate cwd), then the
/// workspace path relative to `src-tauri`. Returns `None` when no bundle.
fn webclient_dist_dir() -> Option<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("SIEGU_WEB_DIST") {
        let p = std::path::PathBuf::from(dir);
        if p.join("index.html").exists() {
            return Some(p);
        }
    }
    let local = std::path::PathBuf::from("dist");
    if local.join("index.html").exists() {
        return Some(local);
    }
    let rel = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist");
    if rel.join("index.html").exists() {
        return Some(rel);
    }
    None
}

/// Start sharing a collection over a browser link.
///
/// Two modes:
/// - `timed`: the share auto-closes after `duration_min` minutes.
/// - `one_time`: a single guest session is served; once that guest leaves, the
///   share closes so the link can't be opened again.
///
/// The signalling origin is either the user's configured LAN signaler (from
/// Settings → Signalling, a plain `ws://host:port`) or, when empty, an embedded
/// server bound to all interfaces serving the view-only webclient. The guest
/// link points at that origin and only exposes the specified album
/// (`EnterAlbumShare`).
#[tauri::command]
pub async fn start_album_share(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::WebRtcState>,
    album_id: String,
    mode: String,
    duration_min: u32,
) -> Result<AlbumShareInfo, String> {
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return Err("Config error".to_string());
    }
    let mode = match mode.as_str() {
        "one_time" => "one_time",
        _ => "timed",
    };
    let duration_min = match duration_min {
        5..=480 if mode == "timed" => duration_min,
        _ => 15,
    };

    let db = database::Database::new(&config_path);
    if db.get_album(&album_id).is_none() {
        return Err("Album not found".to_string());
    }

    let codes = siegu_core::generate_pairing_codes()?;
    let room_id = siegu_core::hash_pairing_code(codes.uuid)?;

    // Signalling-plane token carried by the share link (`#code.token.album`).
    // With an embedded server this gates joins (secure by default); with a
    // configured LAN signaler it must match that signaler's token.
    let mut buf = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let own_token = hex::encode(buf);

    // Read the user's configured signalling server from Settings.
    let configured_url = db
        .get_state()
        .get("signaling_url")
        .cloned()
        .unwrap_or_default()
        .trim()
        .to_string();
    let configured_token = db
        .get_state()
        .get("signaling_token")
        .cloned()
        .unwrap_or_default()
        .trim()
        .to_string();

    // LAN-style configured signaler (plain `ws://host:port` that serves the
    // webclient + /ws) is preferred. Remote `/ws`/`wss://` signalers
    // (siegu.io, paid) are the future extension point.
    let is_remote = configured_url.contains("wss://") || configured_url.trim_end().ends_with("/ws");
    let use_configured = !configured_url.is_empty() && !is_remote;

    let (signaling_url, guest_origin, signaler_token, port) = if use_configured {
        let base = configured_url.trim_end_matches('/').to_string();
        let ws_url = if is_remote {
            base.clone()
        } else {
            // A bare `ws://host:port` — the `/ws` socket is what browsers use.
            format!("{base}/ws")
        };
        let token = if configured_token.is_empty() {
            own_token.clone()
        } else {
            configured_token.clone()
        };
        let origin = ws_url
            .replace("ws://", "http://")
            .replace("wss://", "https://");
        (
            format!("{ws_url}?token={token}"),
            origin,
            token,
            configured_url
                .split(':')
                .nth(2)
                .and_then(|p| p.trim_end_matches('/').parse::<u16>().ok())
                .unwrap_or(0),
        )
    } else {
        // Auto-start an embedded signaler bound to all interfaces.
        if let Ok(mut ls) = state.lan_server.lock() {
            if let Some(prev) = ls.take() {
                prev.stop();
            }
        }
        let dist = webclient_dist_dir().ok_or_else(|| {
            "Web client bundle not found — build webclient/ or set SIEGU_WEB_DIST".to_string()
        })?;
        let server =
            siegu_core::lan_server::start_with_config(siegu_core::lan_server::ServerConfig {
                port: 0,
                token: Some(own_token.clone()),
                web_dist: Some(dist),
            })
            .await;
        let port = server.port;
        if let Ok(mut ls) = state.lan_server.lock() {
            *ls = Some(server);
        }

        let lan_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        // Firewall self-check so the sharer knows if guests on the LAN can reach it.
        if lan_ip != "127.0.0.1" {
            let addr: std::net::SocketAddr = format!("{lan_ip}:{port}")
                .parse()
                .map_err(|e| format!("invalid LAN address {lan_ip}:{port}: {e}"))?;
            match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(2)) {
                Ok(_) => emit_log(&app, format!("TCP self-check OK — {addr} reachable")),
                Err(e) => emit_log(
                    &app,
                    format!(
                        "WARNING: Cannot reach own LAN address {addr}. Firewall may block incoming connections: {e}"
                    ),
                ),
            }
        }

        // The embedded signaler enforces `token`, so the in-mesh initiator URL
        // must carry it too (`extract_token` picks it from the query); the guest
        // origin stays clean — guests present their code+token via the hash.
        (
            format!("ws://{lan_ip}:{port}?token={own_token}"),
            format!("http://{lan_ip}:{port}"),
            own_token.clone(),
            port,
        )
    };

    if let Ok(mut hi) = state.host_info.lock() {
        *hi = Some(crate::HostInfo {
            room_id: room_id.clone(),
            port,
        });
    }

    let sync_tx = Arc::clone(&state.sync_tx);
    let connected = Arc::clone(&state.connected);
    let app_handle = app.clone();
    let one_time = mode == "one_time";
    if one_time {
        state
            .one_time_share
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            emit_log(&app, "Aborting previous WebRTC session".to_string());
            handle.abort();
        }

        let config_path_for_session = config_path.clone();
        let room_id_for_session = room_id.clone();
        let signaling_url_for_session = signaling_url.clone();
        let one_time_flag = Arc::clone(&state.one_time_share);
        let rpc_pending = Arc::clone(&state.rpc_pending);
        let handle = tauri::async_runtime::spawn(async move {
            let transport = if one_time {
                transport::create_transport_with_one_time(
                    room_id_for_session,
                    true,
                    signaling_url_for_session,
                    config_path_for_session,
                    app_handle.clone(),
                    Some(sync_tx),
                    Some(connected),
                    Some(one_time_flag),
                    Some(rpc_pending),
                )
            } else {
                transport::create_transport_with_one_time(
                    room_id_for_session.clone(),
                    true,
                    signaling_url_for_session,
                    config_path_for_session,
                    app_handle.clone(),
                    Some(sync_tx),
                    Some(connected),
                    None,
                    Some(rpc_pending),
                )
            };
            if let Err(e) = transport.start().await {
                emit_log(&app_handle, format!("Album share WebRTC failed: {e}"));
            }
        });
        *session = Some(handle);
    }

    // Timed mode: schedule automatic shutdown after the chosen duration.
    if mode == "timed" {
        let expiry_app = app.clone();
        let handle = tauri::async_runtime::spawn(async move {
            let minutes = duration_min;
            tokio::time::sleep(std::time::Duration::from_secs(minutes as u64 * 60)).await;
            let st = expiry_app.state::<crate::WebRtcState>();
            emit_log(
                &expiry_app,
                format!("Share expired after {minutes} minutes"),
            );
            let _ = do_stop_album_share(&expiry_app, &st).await;
        });
        if let Ok(mut exp) = state.share_expiry.lock() {
            if let Some(prev) = exp.take() {
                prev.abort();
            }
            *exp = Some(handle);
        }
    }

    let fragment = if mode == "one_time" {
        format!("{room_id}.{signaler_token}.{album_id}.once")
    } else {
        format!("{room_id}.{signaler_token}.{album_id}.{duration_min}")
    };
    Ok(AlbumShareInfo {
        url: format!("{guest_origin}/#{fragment}"),
        room_id,
        port,
        mode: mode.to_string(),
        duration_min,
    })
}

/// Shared teardown for stopping a share. Closes the WebRTC session, cancels
/// the expiry timer, clears the one-time flag and stops the embedded server.
async fn do_stop_album_share(
    app: &tauri::AppHandle,
    state: &crate::WebRtcState,
) -> Result<(), String> {
    if let Ok(mut session) = state.active_session.lock() {
        if let Some(handle) = session.take() {
            emit_log(app, "Stopping album share session".to_string());
            handle.abort();
        }
    }
    if let Ok(mut exp) = state.share_expiry.lock() {
        if let Some(handle) = exp.take() {
            handle.abort();
        }
    }
    state
        .one_time_share
        .store(false, std::sync::atomic::Ordering::SeqCst);
    {
        let mut tx = state.sync_tx.lock().await;
        *tx = None;
    }
    // Drain any pending RPCs so callers don't hang for 60s. (#mirror)
    if let Ok(mut pending) = state.rpc_pending.try_lock() {
        for (_, tx) in pending.drain() {
            let _ = tx.send((false, None, Some("Session ended".into())));
        }
    }
    siegu_core::view_only::state().reset_session();
    if let Ok(mut ls) = state.lan_server.lock() {
        if let Some(server) = ls.take() {
            server.stop();
            emit_log(app, "Album share server stopped".to_string());
        }
    }
    if let Ok(mut hi) = state.host_info.lock() {
        *hi = None;
    }
    Ok(())
}

/// Stop a running album share: abort the WebRTC session and close the
/// signalling + web server so the shared port is released.
#[tauri::command]
pub async fn stop_album_share(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::WebRtcState>,
) -> Result<(), String> {
    do_stop_album_share(&app, &state).await
}

#[tauri::command]
pub async fn discover_lan_devices(
    app: tauri::AppHandle,
    timeout_secs: u64,
) -> Result<Vec<siegu_core::mdns::DiscoveredHost>, String> {
    let daemon = siegu_core::mdns::create_daemon().map_err(|e| e.to_string())?;
    let hosts =
        siegu_core::mdns::discover_hosts(&daemon, timeout_secs).map_err(|e| e.to_string())?;

    let local_ip = get_local_ip();

    let filtered: Vec<siegu_core::mdns::DiscoveredHost> = hosts
        .into_iter()
        .filter(|h| {
            // Filter out own host
            if let Some(ref ip) = local_ip {
                if h.ip == *ip {
                    return false;
                }
            }
            true
        })
        .map(|mut h| {
            // Strip mDNS technical suffix from name
            if let Some(pos) = h.name.find("._siegu") {
                h.name = h.name[..pos].to_string();
            }
            h
        })
        .collect();

    emit_log(&app, format!("Discovered {} LAN device(s)", filtered.len()));
    Ok(filtered)
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
    // Drain any pending RPCs so callers don't hang for 60s. (#mirror)
    if let Ok(mut pending) = state.rpc_pending.try_lock() {
        for (_, tx) in pending.drain() {
            let _ = tx.send((false, None, Some("Session ended".into())));
        }
    }
    // Drop any ephemeral view-only buffers/cache bound to the session.
    siegu_core::view_only::state().reset_session();
    if let Ok(mut d) = mdns_state.daemon.lock() {
        if let Some(daemon) = d.take() {
            daemon.shutdown();
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
pub async fn remove_device(app: tauri::AppHandle, id: String) -> Result<(), String> {
    use crate::database;
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Config error".to_string());
    }
    let db = database::Database::new(&path);
    do_remove_device(&db, &id)
}

#[tauri::command]
pub async fn rename_device(
    app: tauri::AppHandle,
    id: String,
    new_name: String,
) -> Result<(), String> {
    use crate::database;
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Config error".to_string());
    }
    let db = database::Database::new(&path);
    do_rename_device(&db, &id, &new_name)
}

#[tauri::command]
pub async fn list_devices(app: tauri::AppHandle) -> String {
    use crate::database;
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let db = database::Database::new(&path);
    let devices = do_list_devices(&db, &path);
    serde_json::to_string(&devices).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn request_start_sync(state: tauri::State<'_, crate::WebRtcState>) -> Result<(), String> {
    let mut tx_lock = state.sync_tx.lock().await;
    let Some(tx) = tx_lock.as_mut() else {
        return Err("Not connected to a device".to_string());
    };
    tx.send(transport::SyncMessage::StartSync)
        .map_err(|e| e.to_string())
}

/// Enter view-only mode on the active connection (#9): bind the media cache
/// to this session and ask the peer for its read-only manifest.
#[tauri::command]
pub async fn enter_view_only(state: tauri::State<'_, crate::WebRtcState>) -> Result<(), String> {
    let mut tx_lock = state.sync_tx.lock().await;
    let Some(tx) = tx_lock.as_mut() else {
        return Err("Not connected to a device".to_string());
    };
    let view = siegu_core::view_only::state();
    view.viewing
        .store(true, std::sync::atomic::Ordering::SeqCst);
    view.bind_session(tx.clone());
    tx.send(transport::SyncMessage::EnterViewOnly)
        .map_err(|e| e.to_string())
}

/// Pull the original of an evicted (view-only) photo from the peer (#10).
/// The transfer persists via the normal receive path and clears view_only.
#[tauri::command]
pub async fn fetch_original(
    state: tauri::State<'_, crate::WebRtcState>,
    id: String,
) -> Result<(), String> {
    let tx_lock = state.sync_tx.lock().await;
    let Some(tx) = tx_lock.as_ref() else {
        return Err("Not connected to a device".to_string());
    };
    tx.send(transport::SyncMessage::FetchMediaRequest {
        id,
        thumbnail: false,
        restore: true,
    })
    .map_err(|e| e.to_string())
}

/// Current storage usage vs. the configured cap, in bytes (#10).
#[tauri::command]
pub async fn storage_usage(app: tauri::AppHandle) -> Result<StorageUsage, String> {
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return Err("Config error".to_string());
    }
    let quota = siegu_core::mesh::MeshManager::get_storage_quota(&config_path);
    let used = siegu_core::mesh::MeshManager::get_total_storage_used(&config_path);
    Ok(StorageUsage { used, quota })
}

#[tauri::command]
pub async fn initialize_sync_folder(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let target = std::path::PathBuf::from(&path).join("siegu");
    if let Err(e) = std::fs::create_dir_all(&target) {
        return Err(format!("Failed to create folder at {target:?}: {e}"));
    }
    // Persist the chosen received-data folder so resolve_sync_target_dir uses it
    // instead of always defaulting to the first library directory.
    let config_path = get_config_path(&app);
    if !config_path.is_empty() {
        let mut state = std::collections::HashMap::new();
        state.insert("sync_path".to_string(), path.clone());
        database::Database::new(&config_path).set_state(state);
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
    mdns_state: tauri::State<'_, crate::MdnsState>,
    discovered_url: Option<String>,
) -> Result<bool, String> {
    use std::sync::atomic::Ordering;
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return Ok(false);
    }

    // Never abort a live session: if a peer is connected, there is nothing to reconnect.
    if state.connected.load(Ordering::SeqCst) {
        emit_log(
            &app,
            "Auto-reconnect skipped: session already connected".to_string(),
        );
        return Ok(false);
    }

    let db = database::Database::new(&config_path);
    let Some(session) = db.load_session() else {
        return Ok(false);
    };

    emit_log(
        &app,
        format!("Auto-reconnecting to room {}", session.room_id),
    );

    if !session.is_initiator {
        // Host session: the saved URL may point at a stale LAN port. Re-host on a fresh
        // port, re-register mDNS and persist the updated session.
        match start_host_internal(
            &app,
            &state,
            &mdns_state,
            session.room_id.clone(),
            session.is_initiator,
        )
        .await
        {
            Ok(info) => emit_log(
                &app,
                format!("Host session restarted on port {}", info.port),
            ),
            Err(e) => emit_log(&app, format!("Failed to restart host session: {e}")),
        }
        return Ok(true);
    }

    // Joiner session: try candidate signaling URLs in order until one connects.
    // 1. A host URL discovered by the frontend — on Android that comes from the
    //    NsdManager plugin, because raw UDP multicast (the Rust mDNS path below)
    //    is unreliable there without a multicast lock, which would otherwise
    //    strand the joiner on the saved URL of a dead port.
    // 2. The room the host currently advertises over mDNS (host may have
    //    restarted on a fresh LAN port).
    // 3. The saved session URL as a last resort.
    let mut candidates: Vec<String> = Vec::new();
    if let Some(url) = discovered_url.filter(|u| !u.is_empty()) {
        candidates.push(url);
    }
    if let Ok(daemon) = siegu_core::mdns::create_daemon() {
        let discovered = siegu_core::mdns::discover_hosts(&daemon, 3);
        daemon.shutdown();
        if let Ok(hosts) = discovered {
            if let Some(matched) = hosts.iter().find(|h| h.room_id == session.room_id) {
                let url = format!("ws://{}:{}", matched.ip, matched.port);
                emit_log(&app, format!("Rediscovered host {} at {url}", matched.name));
                candidates.push(url);
            }
        }
    }
    candidates.push(session.signaling_url.clone());
    candidates.dedup();

    let app_handle = app.clone();
    let sync_tx = Arc::clone(&state.sync_tx);
    let connected = Arc::clone(&state.connected);
    let rpc_pending = Arc::clone(&state.rpc_pending);
    let config_path_for_session = config_path.clone();
    let room_id_for_session = session.room_id.clone();
    let room_id_for_save = room_id_for_session.clone();
    let signaling_url_for_session = candidates[0].clone();
    let is_initiator = session.is_initiator;

    if let Ok(mut active) = state.active_session.lock() {
        if let Some(handle) = active.take() {
            handle.abort();
        }
        let config_path_for_task = config_path_for_session.clone();
        // Surface progress on the same channel the UI already listens to, so
        // pressing Rejoin shows what is happening instead of silently failing.
        let _ = app_handle.emit("webrtc-state", "Reconnecting: searching for host");
        let handle = tauri::async_runtime::spawn(async move {
            // Retry forever (the joiner knows a host is expected from the
            // saved session) with capped exponential backoff, so a briefly
            // unavailable host is rejoined automatically instead of the
            // session giving up after a couple of fixed tries.
            use std::sync::atomic::Ordering;
            let mut backoff_ms: u64 = 2000;
            const BACKOFF_CAP_MS: u64 = 30_000;
            let mut cursor = 0usize;
            loop {
                if connected.load(Ordering::SeqCst) {
                    break;
                }
                if candidates.is_empty() {
                    break;
                }
                let url = candidates[cursor % candidates.len()].clone();
                let transport = transport::create_transport(
                    room_id_for_session.clone(),
                    is_initiator,
                    url.clone(),
                    config_path_for_task.clone(),
                    app_handle.clone(),
                    Some(sync_tx.clone()),
                    Some(connected.clone()),
                    Some(rpc_pending.clone()),
                );
                match transport.start().await {
                    Ok(()) => break,
                    Err(e) => {
                        emit_log(
                            &app_handle,
                            format!("Reconnect to {url} failed ({backoff_ms}ms backoff): {e}"),
                        );
                        let _ = app_handle
                            .emit("webrtc-state", format!("Reconnecting: {url} unreachable"));
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(BACKOFF_CAP_MS);
                cursor += 1;
            }
            if !connected.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = app_handle.emit(
                    "webrtc-state",
                    "Reconnect failed — host not found. Is the host app running?",
                );
            }
        });
        *active = Some(handle);

        let db2 = database::Database::new(&config_path_for_session);
        db2.save_session(&SavedSession {
            room_id: room_id_for_save,
            signaling_url: signaling_url_for_session,
            port: 0,
            is_initiator,
            passphrase: String::new(),
        });
    }

    Ok(true)
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
        let peers = db.list_peer_devices();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].name, "Phone");
        assert_eq!(peers[0].ip, "192.168.1.10");
    }

    #[test]
    fn join_network_multiple_devices() {
        let (db, _dir) = test_db();
        do_join_network(&db, "192.168.1.10", "Phone");
        do_join_network(&db, "192.168.1.11", "Tablet");
        let peers = db.list_peer_devices();
        assert_eq!(peers.len(), 2);
    }

    #[test]
    fn remove_device() {
        let (db, _dir) = test_db();
        do_join_network(&db, "192.168.1.10", "Phone");
        let peers = db.list_peer_devices();
        assert_eq!(peers.len(), 1);
        do_remove_device(&db, &peers[0].device_id).unwrap();
        assert!(db.list_peer_devices().is_empty());
    }

    #[test]
    fn remove_nonexistent_device_no_error() {
        let (db, _dir) = test_db();
        do_remove_device(&db, "non-existent-id").unwrap();
    }

    #[test]
    fn list_devices_includes_host() {
        let (db, dir) = test_db();
        let config = dir.path().display().to_string();
        let devices = do_list_devices(&db, &config);
        assert_eq!(devices.len(), 1);
        assert!(devices[0].host);
        assert_eq!(devices[0].id, "host");
        assert!(devices[0].title.starts_with("Siegu"));
        // Host card reports this device's own storage cap (default 10 GB) so
        // the devices list shows real free-space numbers (#storage).
        assert_eq!(devices[0].storage_capacity, 10240 * 1024 * 1024);
    }

    #[test]
    fn peer_device_storage_fields_survive_listing() {
        let (db, dir) = test_db();
        let config = dir.path().display().to_string();
        do_join_network(&db, "192.168.1.10", "Phone");
        let peers = db.list_peer_devices();
        let id = peers[0].device_id.clone();
        db.update_peer_device_storage(&id, 5000, 20000);
        let devices = do_list_devices(&db, &config);
        let phone = devices
            .iter()
            .find(|d| d.title == "Phone")
            .expect("phone listed");
        assert_eq!(phone.storage_used, 5000);
        assert_eq!(phone.storage_capacity, 20000);
    }

    #[test]
    fn list_devices_with_remote_devices() {
        let (db, dir) = test_db();
        let config = dir.path().display().to_string();
        do_join_network(&db, "192.168.1.10", "Tablet");
        do_join_network(&db, "192.168.1.11", "Phone");
        let devices = do_list_devices(&db, &config);
        assert_eq!(devices.len(), 3);
        assert!(devices[0].host);
        let titles: Vec<&str> = devices.iter().map(|d| d.title.as_str()).collect();
        assert!(titles.contains(&"Tablet"));
        assert!(titles.contains(&"Phone"));
    }

    #[test]
    fn list_devices_host_has_media_counts() {
        let (mut db, dir) = test_db();
        let config = dir.path().display().to_string();
        db.store_photo_batch(&[make_photo("ph1", "/a.jpg"), make_photo("ph2", "/b.jpg")])
            .unwrap();
        let devices = do_list_devices(&db, &config);
        assert_eq!(devices[0].photo_count, 2);
    }

    #[test]
    fn rename_peer_device_persists() {
        let (db, _dir) = test_db();
        do_join_network(&db, "192.168.1.10", "Phone");
        let peers = db.list_peer_devices();
        let id = peers[0].device_id.clone();
        do_rename_device(&db, &id, "Galaxy").unwrap();
        let peer = db.get_peer_device(&id).expect("peer exists");
        assert_eq!(peer.name, "Galaxy");
    }

    #[test]
    fn rename_host_persists_in_state() {
        let (db, dir) = test_db();
        let config = dir.path().display().to_string();
        do_rename_device(&db, "host", "Living Room PC").unwrap();
        let state = db.get_state();
        assert_eq!(
            state.get("device_name").map(|s| s.as_str()),
            Some("Living Room PC")
        );
        let devices = do_list_devices(&db, &config);
        assert_eq!(devices[0].title, "Living Room PC");
    }

    #[test]
    fn rename_rejects_empty_name() {
        let (db, _dir) = test_db();
        do_join_network(&db, "192.168.1.10", "Phone");
        let peers = db.list_peer_devices();
        let id = peers[0].device_id.clone();
        assert!(do_rename_device(&db, &id, "   ").is_err());
        let peer = db.get_peer_device(&id).expect("peer exists");
        assert_eq!(peer.name, "Phone");
    }
}
