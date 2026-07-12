use std::path::PathBuf;

pub const APP_IDENTIFIER: &str = "io.denzyl.siegu";

pub fn default_config_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    if cfg!(target_os = "android") {
        PathBuf::from(format!("/data/data/{APP_IDENTIFIER}/files"))
    } else if cfg!(target_os = "ios") {
        home.join("Library")
            .join("Application Support")
            .join(APP_IDENTIFIER)
    } else if cfg!(target_os = "linux") {
        home.join(".config").join(APP_IDENTIFIER)
    } else if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join(APP_IDENTIFIER)
    } else if cfg!(target_os = "windows") {
        PathBuf::from(
            std::env::var("APPDATA")
                .unwrap_or_else(|_| home.join("AppData\\Roaming").display().to_string()),
        )
        .join(APP_IDENTIFIER)
    } else {
        home.join(".config").join(APP_IDENTIFIER)
    }
}

pub const ALLOWED_CONFIG_KEYS: &[&str] = &[
    "sync_path",
    "scan_threads",
    "indexing_mode",
    "theme",
    "language",
    "tier",
    "model_enabled_clip",
    "model_enabled_face",
    "model_enabled_ocr",
    "model_enabled_nsfw",
    "model_enabled_aesthetics",
    "model_enabled_yolo",
    "model_enabled_blip",
    "model_enabled_arcface",
    "model_enabled_midas",
    "model_enabled_whisper",
    "model_enabled_sam",
    "model_enabled_superres",
    "last_scan_completed",
    "auto_scan",
    "sync_enabled",
];

pub fn is_valid_config_key(key: &str) -> bool {
    ALLOWED_CONFIG_KEYS.contains(&key)
}

pub fn validate_config_value(key: &str, value: &str) -> Result<(), ConfigError> {
    if key.len() > 64 {
        return Err(ConfigError::KeyTooLong);
    }
    if value.len() > 1024 {
        return Err(ConfigError::ValueTooLong);
    }
    if !is_valid_config_key(key) {
        return Err(ConfigError::InvalidKey(key.to_string()));
    }
    match key {
        "scan_threads" => {
            let n: usize = value.parse().map_err(|_| ConfigError::InvalidType {
                key: key.to_string(),
                expected: "usize".to_string(),
                got: value.to_string(),
            })?;
            if n == 0 || n > 32 {
                return Err(ConfigError::OutOfRange {
                    key: key.to_string(),
                    min: 1,
                    max: 32,
                });
            }
        }
        "indexing_mode" => {
            if !["immediate", "idle", "manual"].contains(&value) {
                return Err(ConfigError::InvalidType {
                    key: key.to_string(),
                    expected: "immediate|idle|manual".to_string(),
                    got: value.to_string(),
                });
            }
        }
        "tier" if !["free", "paid"].contains(&value) => {
            return Err(ConfigError::InvalidType {
                key: key.to_string(),
                expected: "free|paid".to_string(),
                got: value.to_string(),
            });
        }
        _ => {}
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    InvalidKey(String),
    KeyTooLong,
    ValueTooLong,
    InvalidType {
        key: String,
        expected: String,
        got: String,
    },
    OutOfRange {
        key: String,
        min: usize,
        max: usize,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidKey(k) => write!(f, "Invalid config key: {k}"),
            ConfigError::KeyTooLong => write!(f, "Config key too long (max 64)"),
            ConfigError::ValueTooLong => write!(f, "Config value too long (max 1024)"),
            ConfigError::InvalidType { key, expected, got } => {
                write!(f, "Invalid type for {key}: expected {expected}, got {got}")
            }
            ConfigError::OutOfRange { key, min, max } => {
                write!(f, "Value for {key} out of range: {min}-{max}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_keys() {
        assert!(is_valid_config_key("theme"));
        assert!(is_valid_config_key("tier"));
        assert!(is_valid_config_key("model_enabled_clip"));
        assert!(!is_valid_config_key("unknown_key"));
        assert!(!is_valid_config_key(""));
    }

    #[test]
    fn test_validate_config_value_valid() {
        assert!(validate_config_value("theme", "dark").is_ok());
        assert!(validate_config_value("tier", "free").is_ok());
        assert!(validate_config_value("tier", "paid").is_ok());
        assert!(validate_config_value("scan_threads", "4").is_ok());
        assert!(validate_config_value("indexing_mode", "immediate").is_ok());
        assert!(validate_config_value("indexing_mode", "idle").is_ok());
        assert!(validate_config_value("indexing_mode", "manual").is_ok());
        assert!(validate_config_value("model_enabled_clip", "true").is_ok());
    }

    #[test]
    fn test_validate_config_value_invalid_key() {
        assert!(matches!(
            validate_config_value("hacked_key", "value"),
            Err(ConfigError::InvalidKey(_))
        ));
    }

    #[test]
    fn test_validate_config_value_key_too_long() {
        let long_key = "a".repeat(65);
        assert!(matches!(
            validate_config_value(&long_key, "value"),
            Err(ConfigError::KeyTooLong)
        ));
    }

    #[test]
    fn test_validate_config_value_value_too_long() {
        let long_value = "a".repeat(1025);
        assert!(matches!(
            validate_config_value("theme", &long_value),
            Err(ConfigError::ValueTooLong)
        ));
    }

    #[test]
    fn test_validate_config_scan_threads_invalid_type() {
        assert!(matches!(
            validate_config_value("scan_threads", "abc"),
            Err(ConfigError::InvalidType { .. })
        ));
    }

    #[test]
    fn test_validate_config_scan_threads_out_of_range() {
        assert!(matches!(
            validate_config_value("scan_threads", "0"),
            Err(ConfigError::OutOfRange { .. })
        ));
        assert!(matches!(
            validate_config_value("scan_threads", "33"),
            Err(ConfigError::OutOfRange { .. })
        ));
    }

    #[test]
    fn test_validate_config_indexing_mode_invalid() {
        assert!(matches!(
            validate_config_value("indexing_mode", "invalid"),
            Err(ConfigError::InvalidType { .. })
        ));
    }

    #[test]
    fn test_validate_config_tier_invalid() {
        assert!(matches!(
            validate_config_value("tier", "premium"),
            Err(ConfigError::InvalidType { .. })
        ));
    }

    #[test]
    fn test_config_error_display() {
        let e = ConfigError::InvalidKey("bad".to_string());
        assert!(format!("{e}").contains("bad"));
        let e = ConfigError::KeyTooLong;
        assert!(!format!("{e}").is_empty());
        let e = ConfigError::ValueTooLong;
        assert!(!format!("{e}").is_empty());
    }
}
