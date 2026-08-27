use tauri::Emitter;
use tauri::Manager;

pub fn get_config_path(app: &tauri::AppHandle) -> String {
    app.path()
        .app_config_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "".to_string())
}

pub fn emit_log(app: &tauri::AppHandle, message: String) {
    crate::log::persist_log(crate::log::infer_level(&message), &message);
    let _ = app.emit("scan-log", message);
}

/// Log for developers only (persisted log file) without showing it to the
/// user in the scan activity feed. Use for internal/technical messages.
pub fn debug_log(message: String) {
    crate::log::persist_log(crate::log::infer_level(&message), &message);
}

/// Current UNIX time in milliseconds since the epoch.
pub fn unix_now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
