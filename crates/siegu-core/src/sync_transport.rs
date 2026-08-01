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

/// Sanitize a peer-supplied relative path so it can safely mirror the sender's
/// directory tree under the sync target folder. Each segment is cleaned like a
/// filename and traversal (`..`) or any segment that is not a plain name causes
/// the whole path to be rejected (`None`), in which case callers fall back to
/// the flat filename.
pub fn sanitize_relative_path(relative_path: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for raw in relative_path.split(['/', '\\']) {
        let segment = raw.trim();
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return None;
        }
        let cleaned = sanitize_filename(segment);
        if cleaned == "unknown" {
            continue;
        }
        out.push(cleaned);
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Resolve where a received file should land. Prefers the mirrored `relative_path`
/// under `target_dir`; falls back to the flat sanitized filename when the peer
/// sent no usable relative path.
pub fn resolve_receive_target(
    target_dir: &Path,
    relative_path: &str,
    fallback_filename: &str,
) -> PathBuf {
    match sanitize_relative_path(relative_path) {
        Some(rel) => target_dir.join(rel),
        None => target_dir.join(sanitize_filename(fallback_filename)),
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
    fn test_sanitize_relative_path_preserves_tree() {
        let result = sanitize_relative_path("DCIM/100MEDIA/IMG_0001.jpg").unwrap();
        assert_eq!(
            result,
            PathBuf::from("DCIM").join("100MEDIA").join("IMG_0001.jpg")
        );
    }

    #[test]
    fn test_sanitize_relative_path_rejects_traversal() {
        assert!(sanitize_relative_path("../secret/photo.jpg").is_none());
        assert!(sanitize_relative_path("a/../../b.jpg").is_none());
        assert!(sanitize_relative_path("..").is_none());
    }

    #[test]
    fn test_sanitize_relative_path_normalizes_separators() {
        let result = sanitize_relative_path("DCIM\\100MEDIA\\IMG.jpg").unwrap();
        assert_eq!(
            result,
            PathBuf::from("DCIM").join("100MEDIA").join("IMG.jpg")
        );
    }

    #[test]
    fn test_sanitize_relative_path_absolute_input_is_nested() {
        let result = sanitize_relative_path("/etc/passwd").unwrap();
        assert_eq!(result, PathBuf::from("etc").join("passwd"));
    }

    #[test]
    fn test_sanitize_relative_path_cleans_segments() {
        let result = sanitize_relative_path("my<dir>/IMG*.jpg").unwrap();
        assert_eq!(result, PathBuf::from("my_dir").join("IMG_.jpg"));
    }

    #[test]
    fn test_sanitize_relative_path_empty_falls_back() {
        assert!(sanitize_relative_path("").is_none());
        assert!(sanitize_relative_path(".").is_none());
        assert!(sanitize_relative_path("///").is_none());
    }

    #[test]
    fn test_resolve_receive_target_mirrors_tree() {
        let target = Path::new("/sync/siegu");
        let result = resolve_receive_target(target, "DCIM/100MEDIA/a.jpg", "a.jpg");
        assert_eq!(result, Path::new("/sync/siegu/DCIM/100MEDIA/a.jpg"));
    }

    #[test]
    fn test_resolve_receive_target_fallback() {
        let target = Path::new("/sync/siegu");
        let result = resolve_receive_target(target, "..", "IMG_0001.jpg");
        assert_eq!(result, Path::new("/sync/siegu/IMG_0001.jpg"));
    }

    #[test]
    fn test_resolve_sync_path_priority() {
        let dirs = vec!["/other/dir".to_string()];
        let result = resolve_sync_target_dir("/config", Some("/sync/path"), &dirs);
        assert_eq!(result, PathBuf::from("/sync/path/siegu"));
    }
}
