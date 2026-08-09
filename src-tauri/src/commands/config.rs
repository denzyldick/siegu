use crate::common::get_config_path;
use crate::database;
use crate::database::Database;
use std::collections::HashMap;

/// Pure business logic — testable without Tauri.
pub fn do_save_config(db: &Database, key: &str, value: &str) -> Result<(), String> {
    siegu_core::config::validate_config_value(key, value)
        .map_err(|e| format!("Invalid config: {e}"))?;
    let mut state = HashMap::new();
    state.insert(key.to_string(), value.to_string());
    db.set_state(state);
    Ok(())
}

/// Pure business logic — testable without Tauri.
pub fn do_get_config(db: &Database) -> HashMap<String, String> {
    db.get_state()
}

/// Pure business logic — testable without Tauri.
pub fn do_get_os() -> String {
    std::env::consts::OS.to_string()
}

#[tauri::command]
pub async fn save_config(app: tauri::AppHandle, key: String, value: String) {
    use crate::common::emit_log;
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let db = database::Database::new(&path);
    if let Err(e) = do_save_config(&db, &key, &value) {
        emit_log(&app, e);
    }
}

#[tauri::command]
pub async fn get_config(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "{}".to_string();
    }
    let db = database::Database::new(&path);
    serde_json::to_string(&do_get_config(&db)).unwrap_or("{}".to_string())
}

#[tauri::command]
pub async fn get_os() -> String {
    do_get_os()
}

/// Best-effort system dark-mode detection.
///
/// Linux WebKitGTK reports `prefers-color-scheme` unreliably, so read the
/// desktop's actual setting directly. Returns `None` when it cannot be
/// determined (non-Linux, gsettings missing, schema not present).
pub fn do_get_system_dark_mode() -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        gsettings_flag("org.gnome.desktop.interface", "color-scheme", "prefer-dark")
            .or_else(|| gsettings_flag("org.gnome.desktop.interface", "gtk-theme", "dark"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Run `gsettings get <schema> <key>` and return whether its value contains
/// `needle` (case-insensitive). `None` when the command is missing or fails.
#[cfg(target_os = "linux")]
fn gsettings_flag(schema: &str, key: &str, needle: &str) -> Option<bool> {
    let output = std::process::Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout);
    let value = value.trim().trim_matches('\'').to_lowercase();
    if value.is_empty() {
        return None;
    }
    Some(value.contains(needle))
}

#[tauri::command]
pub async fn get_system_dark_mode() -> Option<bool> {
    do_get_system_dark_mode()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn save_and_get_config() {
        let (db, _dir) = test_db();
        do_save_config(&db, "theme", "dark").unwrap();
        let config = do_get_config(&db);
        assert_eq!(config.get("theme").unwrap(), "dark");
    }

    #[test]
    fn save_config_overwrites() {
        let (db, _dir) = test_db();
        do_save_config(&db, "theme", "dark").unwrap();
        do_save_config(&db, "theme", "light").unwrap();
        let config = do_get_config(&db);
        assert_eq!(config.get("theme").unwrap(), "light");
    }

    #[test]
    fn get_config_empty() {
        let (db, _dir) = test_db();
        let config = do_get_config(&db);
        assert!(config.is_empty());
    }

    #[test]
    fn get_os_returns_string() {
        let os = do_get_os();
        assert!(!os.is_empty());
        assert!(["linux", "macos", "windows"].contains(&os.as_str()));
    }

    #[test]
    fn get_system_dark_mode_never_panics() {
        // Returns Some(bool) when determinable, None otherwise — never panics.
        let _ = do_get_system_dark_mode();
    }

    #[test]
    fn save_config_invalid_key_rejected() {
        let (db, _dir) = test_db();
        let result = do_save_config(&db, "invalid_key_xyz", "value");
        assert!(result.is_err());
    }

    #[test]
    fn save_config_multiple_keys() {
        let (db, _dir) = test_db();
        do_save_config(&db, "theme", "dark").unwrap();
        do_save_config(&db, "tier", "paid").unwrap();
        let config = do_get_config(&db);
        assert_eq!(config.len(), 2);
        assert_eq!(config.get("theme").unwrap(), "dark");
        assert_eq!(config.get("tier").unwrap(), "paid");
    }
}
