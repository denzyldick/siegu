//! Local device inventory shared by the desktop clients.
//!
//! Everything here reads SQLite or the local filesystem only. Network pairing,
//! sync and WebRTC session state stay in the host app; the list of known devices
//! and the host card can be built without any of it.

use crate::{
    database::{Database, DeviceInfo},
    mesh::MeshManager,
};

/// MDI platform logo for a device `os` value (`std::env::consts::OS` style).
/// Kept in the same shape the legacy Vue client expects.
pub fn os_icon(os: &str) -> &'static str {
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

/// Known peers plus a synthetic host card at index 0. Peer rows come back
/// newest-first from SQLite; remote media counts are preserved as announced.
pub fn list_devices(db: &Database, config_path: &str) -> Vec<DeviceInfo> {
    let mut devices: Vec<DeviceInfo> = db
        .list_peer_devices()
        .into_iter()
        .map(|peer| DeviceInfo {
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
        DeviceInfo {
            id: "host".to_string(),
            title: host_title,
            icon: os_icon(std::env::consts::OS).to_string(),
            up_to_date: true,
            host: true,
            photo_count,
            video_count,
            // The host card shows this device's own library; remote_* mirror
            // it so the UI can use one code path for every card.
            remote_photo_count: photo_count,
            remote_video_count: video_count,
            os: std::env::consts::OS.to_string(),
            storage_used: MeshManager::get_total_storage_used(config_path),
            storage_capacity: MeshManager::get_storage_quota(config_path),
        },
    );

    devices
}

/// Rename a device. The host's name lives in config state so it survives
/// restarts and can be advertised during LAN discovery.
pub fn rename_device(db: &Database, id: &str, new_name: &str) -> Result<(), String> {
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

/// Forget a paired device. The host card is synthetic, so removing it is a
/// no-op.
pub fn remove_device(db: &Database, id: &str) -> Result<(), String> {
    if id != "host" {
        db.remove_peer_device(id);
    }
    Ok(())
}

/// Storage usage as a whole percentage of capacity, clamped to 0..=100. A
/// missing or zero capacity shows no progress rather than a misleading bar.
pub fn storage_percent(used: u64, capacity: u64) -> u8 {
    if capacity == 0 {
        return 0;
    }
    ((used.saturating_mul(100) / capacity).min(100)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::PeerDevice;
    use std::collections::HashMap;

    fn peer(id: &str, name: &str, os: &str) -> PeerDevice {
        PeerDevice {
            device_id: id.to_string(),
            name: name.to_string(),
            ip: "192.168.1.20".to_string(),
            port: 8080,
            device_type: "mobile".to_string(),
            os: os.to_string(),
            models_enabled: vec![],
            protocol_version: 1,
            storage_used: 512,
            storage_capacity: 2048,
            last_seen: String::new(),
            photo_count: 3,
            video_count: 1,
            remote_photo_count: 40,
            remote_video_count: 7,
        }
    }

    #[test]
    fn os_icon_maps_platforms() {
        assert_eq!(os_icon("windows"), "mdi-microsoft-windows");
        assert_eq!(os_icon("macos"), "mdi-apple");
        assert_eq!(os_icon("ios"), "mdi-apple-ios");
        assert_eq!(os_icon("android"), "mdi-android");
        assert_eq!(os_icon("linux"), "mdi-linux");
        assert_eq!(os_icon("freebsd"), "mdi-freebsd");
        assert_eq!(os_icon("unknown"), "mdi-laptop");
    }

    #[test]
    fn list_devices_prepends_host_and_preserves_peer_counts() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().to_string_lossy().to_string();
        let db = Database::new(&config);
        let mut state = HashMap::new();
        state.insert("device_name".to_string(), "Living Room PC".to_string());
        db.set_state(state);
        db.upsert_peer_device(&peer("peer-1", "Phone", "android"));
        db.set_peer_remote_counts("peer-1", 40, 7);

        let devices = list_devices(&db, &config);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "host");
        assert_eq!(devices[0].title, "Living Room PC");
        assert!(devices[0].host);
        assert_eq!(devices[0].remote_photo_count, devices[0].photo_count);
        assert_eq!(devices[0].remote_video_count, devices[0].video_count);
        assert_eq!(devices[0].storage_capacity, 10240 * 1024 * 1024);

        assert_eq!(devices[1].id, "peer-1");
        assert_eq!(devices[1].title, "Phone");
        assert_eq!(devices[1].icon, "mdi-android");
        assert!(!devices[1].host);
        assert_eq!(devices[1].photo_count, 3);
        assert_eq!(devices[1].remote_photo_count, 40);
        assert_eq!(devices[1].remote_video_count, 7);
        assert_eq!(devices[1].storage_used, 512);
        assert_eq!(devices[1].storage_capacity, 2048);
    }

    #[test]
    fn rename_device_persists_host_and_peer_names() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().to_string_lossy().to_string();
        let db = Database::new(&config);
        db.upsert_peer_device(&peer("peer-1", "Phone", "android"));

        rename_device(&db, "peer-1", "  Pixel  ").unwrap();
        rename_device(&db, "host", "  Desk  ").unwrap();
        let devices = list_devices(&db, &config);

        assert_eq!(devices[0].title, "Desk");
        assert_eq!(devices[1].title, "Pixel");
        assert!(rename_device(&db, "peer-1", "   ").is_err());
    }

    #[test]
    fn remove_device_only_deletes_peers() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().to_string_lossy().to_string();
        let db = Database::new(&config);
        db.upsert_peer_device(&peer("peer-1", "Phone", "android"));

        remove_device(&db, "host").unwrap();
        assert_eq!(db.list_peer_devices().len(), 1);
        remove_device(&db, "peer-1").unwrap();
        assert!(db.list_peer_devices().is_empty());
    }

    #[test]
    fn storage_percent_clamps_and_handles_zero_capacity() {
        assert_eq!(storage_percent(512, 2048), 25);
        assert_eq!(storage_percent(0, 2048), 0);
        assert_eq!(storage_percent(4096, 2048), 100);
        assert_eq!(storage_percent(512, 0), 0);
    }
}
