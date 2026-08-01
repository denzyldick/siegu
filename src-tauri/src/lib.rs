pub use common::{emit_log, get_config_path};

mod commands;
pub mod common;
mod file;
mod mdns_plugin;
mod ml;
mod permission_plugin;
mod tauri_sync_event;
#[cfg(test)]
mod test;
#[cfg(test)]
pub mod test_helpers;
mod transport;
mod wallpaper_plugin;

pub use siegu_core::database;

use std::sync::Arc;
use tauri::Manager;

struct WebRtcState {
    active_session: std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    sync_tx:
        Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<transport::SyncMessage>>>>,
    connected: Arc<std::sync::atomic::AtomicBool>,
    lan_server: std::sync::Mutex<Option<siegu_core::lan_server::LanServer>>,
}

struct ScanState {
    guard: siegu_core::ScanGuard,
}

struct MdnsState {
    daemon: std::sync::Mutex<Option<siegu_core::mdns::DaemonHandle>>,
}

struct ShutdownState {
    coordinator: siegu_core::shutdown::ShutdownCoordinator,
}

impl Default for ShutdownState {
    fn default() -> Self {
        Self {
            coordinator: siegu_core::shutdown::ShutdownCoordinator::new(),
        }
    }
}

#[cfg(desktop)]
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// Best-effort config dir resolution for the panic hook (no AppHandle available there).
fn config_dir_fallback() -> Option<std::path::PathBuf> {
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(std::path::PathBuf::from(x));
    }
    let home = std::env::var_os("HOME")?;
    #[cfg(target_os = "macos")]
    let base = std::path::PathBuf::from(&home).join("Library/Application Support");
    #[cfg(not(target_os = "macos"))]
    let base = std::path::PathBuf::from(&home).join(".config");
    Some(base)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        let msg = format!("PANIC: {}", info);
        eprintln!("[siegu] {msg}");
        if let Some(config_dir) = config_dir_fallback() {
            let app_config = config_dir.join("io.denzyl.siegu");
            let _ = std::fs::create_dir_all(&app_config);
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(app_config.join("siegu_debug.log"))
            {
                let _ = writeln!(f, "{msg}");
                let _ = f.flush();
            }
        }
        if std::env::var("RUST_BACKTRACE")
            .map(|v| v != "0")
            .unwrap_or(false)
        {
            eprintln!("[siegu] {}", std::backtrace::Backtrace::force_capture());
        }
    }));

    let builder = tauri::Builder::default();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));
    builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(wallpaper_plugin::init())
        .plugin(mdns_plugin::init())
        .plugin(permission_plugin::init())
        .setup(|app| {
            if let Err(e) = ffmpeg_next::init() {
                emit_log(
                    app.handle(),
                    format!("WARNING: ffmpeg init failed (thumbnails for videos disabled): {e}"),
                );
            }

            #[cfg(desktop)]
            {
                let show_i = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
                let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

                let _tray = TrayIconBuilder::new()
                    .menu(&menu)
                    .icon(app.default_window_icon().unwrap().clone())
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "quit" => {
                            if let Some(state) = app.try_state::<ShutdownState>() {
                                state.coordinator.signal();
                            }
                            app.exit(0);
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app)?;

                #[cfg(target_os = "linux")]
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.with_webview(|webview| {
                        use webkit2gtk::{WebContextExt, WebViewExt};
                        let wv = webview.inner();
                        if let Some(ctx) = wv.context() {
                            ctx.set_spell_checking_enabled(false);
                        }
                    });
                }
            }

            emit_log(
                app.handle(),
                "App is setting up background tasks...".to_string(),
            );
            use tauri_plugin_notification::NotificationExt;
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = app_handle
                    .notification()
                    .builder()
                    .title("Siegu")
                    .body("Siegu is running in the background")
                    .show();
            });

            let config_path = get_config_path(app.handle());

            // Clean up stale temp files on startup
            {
                let cp = config_path.clone();
                tauri::async_runtime::spawn(async move {
                    siegu_core::mesh::MeshManager::cleanup_temp_files(&cp).await;
                });
            }

            let sync_tx = Arc::new(tokio::sync::Mutex::new(None));
            let ml_context = ml::start_background_worker(
                app.handle(),
                config_path.clone(),
                Arc::clone(&sync_tx),
            );
            app.manage(ml_context);

            let media_server_port = transport::start_media_server(config_path);
            app.manage(transport::MediaServerState {
                port: media_server_port,
            });

            app.manage(WebRtcState {
                active_session: std::sync::Mutex::new(None),
                sync_tx,
                connected: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                lan_server: std::sync::Mutex::new(None),
            });

            app.manage(ScanState {
                guard: siegu_core::ScanGuard::new(),
            });

            app.manage(MdnsState {
                daemon: std::sync::Mutex::new(None),
            });

            app.manage(ShutdownState::default());

            let app_handle_for_interval = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
                loop {
                    emit_log(
                        &app_handle_for_interval,
                        "Interval tick: checking for media updates...".to_string(),
                    );
                    interval.tick().await;
                    commands::scan::scan_files(app_handle_for_interval.clone());
                }
            });

            // Periodic temp file cleanup every 30 minutes
            let app_handle_for_cleanup = app.handle().clone();
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

            let app_handle_for_watcher = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                file::start_watcher(app_handle_for_watcher).await;
            });

            Ok(())
        })
        .on_window_event(|_window, event| match event {
            #[cfg(desktop)]
            tauri::WindowEvent::CloseRequested { api, .. } => {
                _window.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            // Scan
            commands::scan::scan_files,
            // Models
            commands::models::check_models,
            commands::models::download_models,
            // Photos
            commands::photos::list_files,
            commands::photos::toggle_favorite,
            commands::photos::get_photo_by_id,
            commands::photos::get_photo_encoded_batch,
            commands::photos::get_photos_for_map_click,
            commands::photos::get_heatmap_data,
            // Directories
            commands::directories::add_directory,
            commands::directories::list_directories,
            commands::directories::remove_directory,
            commands::directories::remove_directory_full,
            commands::directories::is_initialized,
            commands::directories::mark_onboarding_complete,
            // People
            commands::people::get_people,
            commands::people::get_unnamed_faces,
            commands::people::assign_name_to_face,
            commands::people::get_person_photos,
            commands::people::get_person_faces,
            commands::people::get_faces_for_photo,
            commands::people::delete_face,
            commands::people::get_top_tags,
            commands::people::merge_people,
            commands::people::rename_person,
            // Config
            commands::config::save_config,
            commands::config::get_config,
            commands::config::get_os,
            // Sync
            commands::sync::start_webrtc_session,
            commands::sync::start_lan_host,
            commands::sync::stop_webrtc_session,
            commands::sync::discover_lan_devices,
            commands::sync::join_network,
            commands::sync::remove_device,
            commands::sync::list_devices,
            commands::sync::request_start_sync,
            commands::sync::initialize_sync_folder,
            commands::sync::get_media_server_port,
            commands::sync::generate_pairing_codes,
            commands::sync::hash_pairing_code,
            commands::sync::auto_reconnect,
            commands::sync::clear_saved_session,
            commands::sync::list_peer_devices,
            // Indexing
            commands::indexing::get_indexing_status,
            commands::indexing::get_unindexed_count,
            commands::indexing::index_faces,
            commands::indexing::analyze_photo,
            commands::indexing::analyze_photo_model,
            commands::indexing::analyze_model,
            commands::indexing::abort_indexing,
            // Geocode
            commands::geocode::list_objects,
            commands::logging::resolve_photo_locations,
            commands::logging::get_location_names,
            // Wallpaper
            commands::wallpaper::set_wallpaper,
            // Logging
            commands::logging::get_logs,
            commands::logging::clear_logs,
            commands::logging::get_last_scan_time,
            commands::logging::cleanup_database,
            // File
            file::read_file_base64,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
