# Backend (Tauri Integration)

File: `src-tauri/src/lib.rs`

## Managed State

5 shared states managed by Tauri:

| State | Type | Purpose |
|-------|------|---------|
| `WebRtcState` | `sync_tx`, `active_session` | WebRTC session + sync channel |
| `ScanState` | `is_scanning` | Prevents concurrent scans |
| `MdnsState` | `daemon` | mDNS daemon handle |
| `ShutdownState` | `coordinator` | Graceful shutdown signal |
| `MlContext` | `tx`, `pending_count`, `abort` | ML worker control |
| `MediaServerState` | `port` | Media HTTP server port |

## Tauri Commands

~50 commands across 11 modules in `src-tauri/src/commands/`:

### sync (14 commands)

`start_webrtc_session`, `start_lan_host`, `stop_webrtc_session`, `discover_lan_devices`, `join_network`, `remove_device`, `list_devices`, `request_start_sync`, `initialize_sync_folder`, `get_media_server_port`, `generate_pairing_codes`, `hash_pairing_code`, `auto_reconnect`, `clear_saved_session`, `list_peer_devices`

### photos (6 commands)

`list_files`, `toggle_favorite`, `get_photo_by_id`, `get_photo_encoded_batch`, `get_photos_for_map_click`, `get_heatmap_data`

### scan (1 command)

`scan_files`

### models (2 commands)

`check_models`, `download_models`

### indexing (6 commands)

`get_indexing_status`, `get_unindexed_count`, `index_faces`, `analyze_photo`, `analyze_photo_model`, `analyze_model`, `abort_indexing`

### people (10 commands)

`get_people`, `get_unnamed_faces`, `assign_name_to_face`, `get_person_photos`, `get_person_faces`, `get_faces_for_photo`, `delete_face`, `get_top_tags`, `merge_people`, `rename_person`

### directories (5 commands)

`add_directory`, `list_directories`, `remove_directory`, `remove_directory_full`, `is_initialized`

### config (3 commands)

`save_config`, `get_config`, `get_os`

### geocode (3 commands)

`list_objects`, `resolve_photo_locations`, `get_location_names`

### logging (4 commands)

`get_logs`, `clear_logs`, `get_last_scan_time`, `cleanup_database`

### wallpaper (1 command)

`set_wallpaper`

## Tauri Plugins

| Plugin | Purpose |
|--------|---------|
| `tauri-plugin-updater` | App update checks |
| `tauri-plugin-notification` | System notifications |
| `tauri-plugin-os` | OS info |
| `tauri-plugin-fs` | Filesystem access |
| `tauri-plugin-dialog` | File picker dialogs |
| `tauri-plugin-opener` | Open files with system handler |
| `tauri-plugin-shell` | Shell commands |
| `tauri-plugin-process` | Process management |
| `tauri-plugin-global-shortcut` | Keyboard shortcuts (desktop) |
| `tauri-plugin-clipboard-manager` | Clipboard (desktop) |
| `tauri-plugin-wallpaper` | Wallpaper setting (Android) |
| `tauri-plugin-devtools` | DevTools (dev mode) |
| `tauri-plugin-notification` | Push/local notifications |

## Error Handling

`SieguError` enum with 7 variants:

| Variant | Context |
|---------|---------|
| `Database` | SQLite operations |
| `Io` | File system operations |
| `Network` | WebSocket / WebRTC failures |
| `Config` | Invalid config values |
| `Model` | ONNX model loading or inference |
| `Scan` | Concurrent scan prevention |
| `Sync` | Protocol or transfer errors |
| `Shutdown` | Graceful shutdown signaling |

All commands return `Result<T, String>` for Tauri IPC compatibility, mapping internal errors to user-facing messages.
