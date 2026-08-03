#[tauri::command]
pub async fn set_wallpaper(app: tauri::AppHandle, path: String) -> Result<(), String> {
    set_wallpaper_impl(&app, &path)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn set_wallpaper_impl(_app: &tauri::AppHandle, path: &str) -> Result<(), String> {
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        if desktop.contains("COSMIC") {
            return set_cosmic_wallpaper(path);
        }
    }

    if let Err(e) = wallpaper::set_from_path(path) {
        let uri = format!("\"file://{}\"", path);
        let output = std::process::Command::new("gsettings")
            .arg("set")
            .arg("org.gnome.desktop.background")
            .arg("picture-uri")
            .arg(&uri)
            .output()
            .map_err(|e| format!("Failed to run gsettings: {}", e))?;

        if output.status.success() {
            return Ok(());
        }

        return Err(format!(
            "wallpaper crate: {}; gsettings: {}",
            e,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn set_cosmic_wallpaper(path: &str) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let config_dir = std::path::Path::new(&home)
        .join(".config")
        .join("cosmic")
        .join("com.system76.CosmicBackground")
        .join("v1");

    let content = format!(
        "(\n    output: \"all\",\n    source: Path(\"{}\"),\n    filter_by_theme: true,\n    rotation_frequency: 300,\n    filter_method: Lanczos,\n    scaling_mode: Zoom,\n    sampling_method: Alphanumeric,\n)",
        path
    );

    std::fs::write(config_dir.join("all"), &content)
        .map_err(|e| format!("Failed to write COSMIC background config: {}", e))?;

    std::fs::write(config_dir.join("same-on-all"), "true")
        .map_err(|e| format!("Failed to write COSMIC same-on-all config: {}", e))?;

    Ok(())
}

#[cfg(target_os = "android")]
fn set_wallpaper_impl(_app: &tauri::AppHandle, path: &str) -> Result<(), String> {
    use tauri::Emitter;
    _app.emit("wallpaper:set", path).map_err(|e| e.to_string())
}

#[cfg(target_os = "ios")]
fn set_wallpaper_impl(_app: &tauri::AppHandle, _path: &str) -> Result<(), String> {
    Err("Setting wallpaper is not supported on this platform".to_string())
}
