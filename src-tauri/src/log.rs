use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::{fmt, io};

use serde::Serialize;
use tauri::Emitter;
use tauri::Manager;
use tracing::field::Field;
use tracing::Subscriber;
use tracing_subscriber::field::Visit;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;

/// How many recent log entries to keep in memory for the in-app log viewer.
pub const RING_CAPACITY: usize = 2000;
/// Maximum size of `siegu_debug.log` before it is rotated to `siegu_debug.log.1`.
pub const MAX_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

struct RingBuffer {
    entries: Vec<LogEntry>,
    capacity: usize,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, entry: LogEntry) {
        if self.entries.len() == self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }
}

static RING: OnceLock<Mutex<RingBuffer>> = OnceLock::new();
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

fn ring() -> &'static Mutex<RingBuffer> {
    RING.get_or_init(|| Mutex::new(RingBuffer::new(RING_CAPACITY)))
}

pub fn set_app_handle(handle: tauri::AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

pub fn app_config_dir() -> Option<std::path::PathBuf> {
    if let Some(handle) = APP_HANDLE.get() {
        if let Ok(dir) = handle.path().app_config_dir() {
            return Some(dir);
        }
    }
    crate::config_dir_fallback().map(|dir| dir.join("io.denzyl.siegu"))
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn infer_level(message: &str) -> &'static str {
    let upper = message.to_uppercase();
    if upper.contains("ERROR") || upper.contains("FATAL") {
        "error"
    } else if upper.contains("WARN") || upper.contains("WARNING") {
        "warn"
    } else if upper.contains("DEBUG") {
        "debug"
    } else {
        "info"
    }
}

pub fn persist_log(level: &str, message: &str) {
    let entry = LogEntry {
        timestamp: now_rfc3339(),
        level: level.to_string(),
        message: message.to_string(),
    };

    {
        let mut buffer = ring().lock().unwrap_or_else(|e| e.into_inner());
        buffer.push(entry.clone());
    }

    eprintln!("[siegu] {message}");

    if let Some(handle) = APP_HANDLE.get() {
        let _ = handle.emit("log-message", message.to_string());
    }

    if let Some(dir) = app_config_dir() {
        if let Err(e) = append_log_file(&dir, &entry) {
            eprintln!("[siegu] failed to write debug log: {e}");
        }
    }
}

fn append_log_file(dir: &Path, entry: &LogEntry) -> io::Result<()> {
    let _ = std::fs::create_dir_all(dir);
    let path = dir.join("siegu_debug.log");
    let line = format!(
        "{} [{}] {}\n",
        entry.timestamp,
        entry.level.to_uppercase(),
        entry.message
    );
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() + line.len() as u64 > MAX_LOG_FILE_BYTES {
            let _ = std::fs::rename(&path, dir.join("siegu_debug.log.1"));
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(line.as_bytes())?;
    file.flush()
}

pub fn recent_logs(limit: usize) -> Vec<LogEntry> {
    let buffer = ring().lock().unwrap_or_else(|e| e.into_inner());
    buffer.entries.iter().rev().take(limit).cloned().collect()
}

pub fn clear_logs() {
    let mut buffer = ring().lock().unwrap_or_else(|e| e.into_inner());
    buffer.entries.clear();
}

#[cfg(test)]
pub fn push_test_entry(level: &str, message: &str) {
    let entry = LogEntry {
        timestamp: now_rfc3339(),
        level: level.to_string(),
        message: message.to_string(),
    };
    let mut buffer = ring().lock().unwrap_or_else(|e| e.into_inner());
    buffer.push(entry);
}

struct LogLayer;

struct MessageVisitor {
    message: Option<String>,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        }
    }
}

impl<S> Layer<S> for LogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor { message: None };
        event.record(&mut visitor);
        let message = visitor.message.unwrap_or_default();
        let level = event.metadata().level().as_str().to_lowercase();
        persist_log(&level, &message);
    }
}

pub fn init_tracing() {
    let subscriber = tracing_subscriber::registry()
        .with(LogLayer)
        .with(tracing_subscriber::fmt::layer().with_ansi(false));
    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("[siegu] failed to initialize tracing: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn infer_level_from_text() {
        assert_eq!(infer_level("ERROR: boom"), "error");
        assert_eq!(infer_level("FATAL: boom"), "error");
        assert_eq!(infer_level("WARNING: careful"), "warn");
        assert_eq!(infer_level("debug stuff"), "debug");
        assert_eq!(infer_level("all good"), "info");
    }

    #[test]
    fn ring_bounded_by_capacity() {
        let mut buffer = RingBuffer::new(3);
        for i in 0..5 {
            buffer.push(LogEntry {
                timestamp: "t".to_string(),
                level: "info".to_string(),
                message: format!("m{i}"),
            });
        }
        assert_eq!(buffer.entries.len(), 3);
        assert_eq!(buffer.entries[0].message, "m2");
        assert_eq!(buffer.entries[2].message, "m4");
    }

    #[test]
    fn recent_logs_newest_first() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_logs();
        for i in 0..3 {
            push_test_entry("info", &format!("m{i}"));
        }
        let logs = recent_logs(10);
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].message, "m2");
        assert_eq!(logs[2].message, "m0");
    }

    #[test]
    fn recent_logs_respects_limit() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_logs();
        for i in 0..5 {
            push_test_entry("info", &format!("m{i}"));
        }
        assert_eq!(recent_logs(2).len(), 2);
    }

    #[test]
    fn clear_empties_ring() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        push_test_entry("info", "x");
        clear_logs();
        assert!(recent_logs(10).is_empty());
    }
}
