use tauri::Manager;

pub fn get_config_path(app: &tauri::AppHandle) -> String {
    app.path()
        .app_config_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "".to_string())
}

pub fn emit_log(_app: &tauri::AppHandle, message: String) {
    crate::log::persist_log(crate::log::infer_level(&message), &message);
}
