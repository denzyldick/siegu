use crate::database;
use std::io::Write;
use tauri::Emitter;
use tauri::Manager;

pub fn get_config_path(app: &tauri::AppHandle) -> String {
    app.path()
        .app_config_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "".to_string())
}

pub fn emit_log(app: &tauri::AppHandle, message: String) {
    eprintln!("[siegu] {}", message);
    let _ = app.emit("log-message", message.clone());
    let path = get_config_path(app);
    if !path.is_empty() {
        // Write to flat debug file for adb retrieval
        let debug_path = format!("{}/siegu_debug.log", path);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&debug_path)
        {
            let _ = writeln!(f, "{}", message);
            let _ = f.flush();
        } else {
            eprintln!("[siegu] failed to open debug log file at {}", debug_path);
        }

        // Also store to database
        let database = database::Database::new(&path);
        let upper = message.to_uppercase();
        let level = if upper.contains("ERROR") || upper.contains("FATAL") {
            "error"
        } else if upper.contains("WARN") || upper.contains("WARNING") {
            "warn"
        } else if upper.contains("DEBUG") {
            "debug"
        } else {
            "info"
        };
        database.store_log(level, &message);
    }
}
