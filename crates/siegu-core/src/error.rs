use thiserror::Error;

#[derive(Error, Debug)]
pub enum SieguError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Model error: {0}")]
    Model(String),

    #[error("Scan error: {0}")]
    Scan(String),

    #[error("Sync error: {0}")]
    Sync(String),

    #[error("Shutdown requested")]
    Shutdown,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SieguError>;

impl SieguError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn model(msg: impl Into<String>) -> Self {
        Self::Model(msg.into())
    }

    pub fn scan(msg: impl Into<String>) -> Self {
        Self::Scan(msg.into())
    }

    pub fn sync(msg: impl Into<String>) -> Self {
        Self::Sync(msg.into())
    }

    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SieguError::config("invalid key");
        assert_eq!(err.to_string(), "Config error: invalid key");
    }

    #[test]
    fn test_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let siegu_err: SieguError = io_err.into();
        assert!(matches!(siegu_err, SieguError::Io(_)));
    }

    #[test]
    fn test_shutdown_error() {
        let err = SieguError::Shutdown;
        assert_eq!(err.to_string(), "Shutdown requested");
    }
}
