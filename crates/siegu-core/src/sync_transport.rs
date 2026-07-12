use std::path::{Path, PathBuf};

pub fn sanitize_filename(filename: &str) -> String {
    let mut result = String::with_capacity(filename.len());
    for c in filename.chars() {
        match c {
            '\0' => {}
            '/' | '\\' => result.push('_'),
            ':' if cfg!(target_os = "windows") => result.push('_'),
            '<' | '>' | '"' | '?' | '*' | '|' => result.push('_'),
            c if c.is_control() => {}
            c => result.push(c),
        }
    }
    let result = result
        .trim_matches(|c: char| c == '.' || c == '_')
        .to_string();
    if result.is_empty() {
        "unknown".to_string()
    } else {
        result
    }
}

pub fn resolve_sync_target_dir(
    config_path: &str,
    sync_path: Option<&str>,
    directories: &[String],
) -> PathBuf {
    if let Some(sp) = sync_path {
        PathBuf::from(sp).join("siegu")
    } else if !directories.is_empty() {
        PathBuf::from(&directories[0]).join("siegu")
    } else {
        Path::new(config_path).join("Siegu").join("siegu")
    }
}

pub async fn cleanup_sync_temp(config_path: &str, max_age_secs: u64) {
    let temp_dir = Path::new(config_path).join("sync_temp");
    if !temp_dir.exists() {
        return;
    }
    let now = std::time::SystemTime::now();
    if let Ok(entries) = tokio::fs::read_dir(&temp_dir).await {
        let mut entries = entries;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(metadata) = entry.metadata().await {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age.as_secs() > max_age_secs {
                            let _ = tokio::fs::remove_file(entry.path()).await;
                        }
                    }
                }
            }
        }
    }
}

pub fn sync_temp_dir(config_path: &str) -> PathBuf {
    Path::new(config_path).join("sync_temp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_normal() {
        assert_eq!(sanitize_filename("photo.jpg"), "photo.jpg");
        assert_eq!(sanitize_filename("IMG_2024.jpg"), "IMG_2024.jpg");
    }

    #[test]
    fn test_sanitize_filename_path_traversal() {
        assert_eq!(sanitize_filename("../etc/passwd"), "etc_passwd");
        assert_eq!(sanitize_filename("..\\windows\\system"), "windows_system");
    }

    #[test]
    fn test_sanitize_filename_special_chars() {
        let result = sanitize_filename("file<>\"?*|.jpg");
        assert!(result.starts_with("file"));
        assert!(result.ends_with(".jpg"));
        assert!(!result.contains('<'));
        assert!(!result.contains('>'));
        assert!(!result.contains('"'));
        assert!(!result.contains('?'));
        assert!(!result.contains('*'));
        assert!(!result.contains('|'));
    }

    #[test]
    fn test_sanitize_filename_control_chars() {
        let r1 = sanitize_filename("file\x00name.jpg");
        assert_eq!(r1, "filename.jpg");
    }

    #[test]
    fn test_sanitize_filename_empty_after_sanitize() {
        assert_eq!(sanitize_filename(""), "unknown");
        assert_eq!(sanitize_filename("..."), "unknown");
        assert_eq!(sanitize_filename("\x00\x00"), "unknown");
    }

    #[test]
    fn test_sanitize_filename_only_dots() {
        assert_eq!(sanitize_filename(".."), "unknown");
        assert_eq!(sanitize_filename("...."), "unknown");
    }

    #[test]
    fn test_sanitize_filename_windows_colon() {
        let result = sanitize_filename("C:\\test.jpg");
        assert!(result.starts_with("C"));
        assert!(result.ends_with("test.jpg"));
        assert!(!result.contains('\\'));
    }

    #[test]
    fn test_resolve_sync_target_dir_with_sync_path() {
        let result = resolve_sync_target_dir("/config", Some("/home/user/photos"), &[]);
        assert_eq!(result, PathBuf::from("/home/user/photos/siegu"));
    }

    #[test]
    fn test_resolve_sync_target_dir_with_directories() {
        let dirs = vec!["/home/user/pictures".to_string()];
        let result = resolve_sync_target_dir("/config", None, &dirs);
        assert_eq!(result, PathBuf::from("/home/user/pictures/siegu"));
    }

    #[test]
    fn test_resolve_sync_target_dir_fallback() {
        let result = resolve_sync_target_dir("/config", None, &[]);
        assert_eq!(result, PathBuf::from("/config/Siegu/siegu"));
    }

    #[test]
    fn test_sync_temp_dir() {
        let result = sync_temp_dir("/config");
        assert_eq!(result, PathBuf::from("/config/sync_temp"));
    }

    #[test]
    fn test_resolve_sync_path_priority() {
        let dirs = vec!["/other/dir".to_string()];
        let result = resolve_sync_target_dir("/config", Some("/sync/path"), &dirs);
        assert_eq!(result, PathBuf::from("/sync/path/siegu"));
    }
}
