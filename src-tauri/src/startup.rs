use crate::commands;
use crate::common::{emit_log, get_config_path};
use crate::file;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// One-time notification so the user knows the app is running in the background.
pub fn spawn_background_notification(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = app_handle
            .notification()
            .builder()
            .title("Siegu")
            .body("Siegu is running in the background")
            .show();
    });
}

/// Clean up stale temp files on startup.
pub fn spawn_startup_temp_cleanup(app: &AppHandle) {
    let cp = get_config_path(app);
    if cp.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        siegu_core::mesh::MeshManager::cleanup_temp_files(&cp).await;
    });
}

/// Re-scan the library hourly so long-running sources (mobile uploads, a peer's
/// backup drive) are reflected without a manual scan.
pub fn spawn_interval_rescan(app: &AppHandle) {
    let app_handle_for_interval = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        // `tokio::time::interval` fires its first tick immediately, which
        // would trigger a full library rescan on every app launch and slow
        // startup for large libraries. Consume that tick so the first scan
        // only runs after a full hour; the file watcher and the manual
        // "Scan" button still cover live changes.
        interval.tick().await;
        loop {
            interval.tick().await;
            emit_log(
                &app_handle_for_interval,
                "Interval tick: checking for media updates...".to_string(),
            );
            commands::scan::scan_files(app_handle_for_interval.clone());
        }
    });
}

/// Periodic temp file cleanup every 30 minutes.
pub fn spawn_periodic_temp_cleanup(app: &AppHandle) {
    let app_handle_for_cleanup = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1800));
        loop {
            interval.tick().await;
            let cp = get_config_path(&app_handle_for_cleanup);
            if !cp.is_empty() {
                siegu_core::mesh::MeshManager::cleanup_temp_files(&cp).await;
            }
        }
    });
}

/// Watch configured folders and rescan when new media files appear.
pub fn spawn_file_watcher(app: &AppHandle) {
    let app_handle_for_watcher = app.clone();
    tauri::async_runtime::spawn(async move {
        file::start_watcher(app_handle_for_watcher).await;
    });
}
