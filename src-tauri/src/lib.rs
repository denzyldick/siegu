pub use common::{emit_log, get_config_path};

mod commands;
pub mod common;
mod file;
mod log;
mod mdns_plugin;
mod ml;
mod permission_plugin;
mod startup;
mod tauri_sync_event;
#[cfg(test)]
mod test;
#[cfg(test)]
pub mod test_helpers;
mod transport;
mod wallpaper_plugin;

pub use siegu_core::database;

use std::sync::Arc;
use tauri::Emitter;
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
pub(crate) fn config_dir_fallback() -> Option<std::path::PathBuf> {
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

const OPEN_WITH_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "heic", "heif", "avif", "mp4", "webm",
    "mov", "avi", "mkv", "m4v",
];

/// Find a media file passed on the command line when the OS opens the app
/// "with" a file (double-click, "Open with… Siegu", etc.).
fn extract_opened_file(args: Vec<String>) -> Option<String> {
    for arg in args {
        let path = std::path::Path::new(&arg);
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str());
        if let Some(ext) = ext {
            let ext = ext.to_ascii_lowercase();
            if OPEN_WITH_EXTENSIONS.contains(&ext.as_str()) {
                return Some(arg);
            }
        }
    }
    None
}

fn notify_opened_file(app: &tauri::AppHandle, path: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("file-opened", path);
    }
    crate::common::debug_log(format!("Opened file: {path}"));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    crate::log::init_tracing();

    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {}", info);
        crate::log::persist_log("error", &msg);
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
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.set_focus();
            if let Some(path) = extract_opened_file(args) {
                notify_opened_file(app, &path);
            }
        }
    }));

    #[allow(clippy::expect_used)] // Fatal: a Tauri app that fails to launch cannot function
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
            crate::log::set_app_handle(app.handle().clone());
            if let Err(e) = ffmpeg_next::init() {
                crate::common::debug_log(format!(
                    "WARNING: ffmpeg init failed (thumbnails for videos disabled): {e}"
                ));
            }

            #[cfg(desktop)]
            {
                let show_i = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
                let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

                let _tray = TrayIconBuilder::new()
                    .menu(&menu)
                    .icon(
                        app.default_window_icon()
                            .ok_or("default window icon missing")?
                            .clone(),
                    )
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

            crate::common::debug_log("App is setting up background tasks...".to_string());
            startup::spawn_background_notification(app.handle());
            startup::spawn_startup_temp_cleanup(app.handle());

            let config_path = get_config_path(app.handle());

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

            if let Some(path) = extract_opened_file(std::env::args().collect()) {
                notify_opened_file(app.handle(), &path);
            }

            startup::spawn_interval_rescan(app.handle());
            startup::spawn_periodic_temp_cleanup(app.handle());
            startup::spawn_file_watcher(app.handle());
            startup::spawn_background_thumbnail_warmup(app.handle());

            Ok(())
        })
        .on_window_event(|_window, event| match event {
            #[cfg(desktop)]
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if let Err(e) = _window.hide() {
                    tracing::error!("failed to hide window on close requested: {e}");
                }
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
            commands::models::get_model_capabilities,
            // Photos
            commands::photos::list_files,
            commands::photos::toggle_favorite,
            commands::photos::set_favorites,
            commands::photos::get_photo_by_id,
            commands::photos::get_photo_ocr,
            commands::photos::get_photo_encoded_batch,
            commands::photos::get_photos_by_ids,
            commands::photos::get_heatmap_data,
            commands::photos::trash_photo,
            commands::photos::restore_photo,
            commands::photos::empty_trash,
            commands::photos::count_trash,
            commands::photos::list_trash,
            // Albums
            commands::albums::create_album,
            commands::albums::create_smart_album,
            commands::albums::update_smart_album_rule,
            commands::albums::get_album_sections,
            commands::albums::rename_album,
            commands::albums::delete_album,
            commands::albums::clear_dismissed_trips,
            commands::albums::sync_trips,
            commands::albums::list_albums,
            commands::albums::get_album,
            commands::albums::add_album_items,
            commands::albums::remove_album_items,
            commands::albums::reorder_album,
            commands::albums::get_album_contents,
            commands::albums::get_clip_categories,
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
            commands::config::get_system_dark_mode,
            commands::signalling::ping_signaling,
            // Sync
            commands::sync::start_webrtc_session,
            commands::sync::start_lan_host,
            commands::sync::stop_webrtc_session,
            commands::sync::discover_lan_devices,
            commands::sync::join_network,
            commands::sync::remove_device,
            commands::sync::rename_device,
            commands::sync::list_devices,
            commands::sync::request_start_sync,
            commands::sync::enter_view_only,
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
            commands::indexing::get_max_photo_rowid,
            commands::indexing::index_faces,
            commands::indexing::analyze_photo,
            commands::indexing::analyze_photo_model,
            commands::indexing::analyze_model,
            commands::indexing::abort_indexing,
            commands::indexing::pause_indexing,
            commands::indexing::resume_indexing,
            commands::indexing::unload_models,
            commands::indexing::reload_models,
            commands::indexing::get_models_loaded,
            // Geocode
            commands::geocode::list_objects,
            commands::search::search_facets,
            commands::search::day_counts,
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
        .expect("fatal: error while running the tauri application");
}
