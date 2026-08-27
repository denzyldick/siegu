use std::fmt;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tauri::Emitter;
use tauri::Manager;
use tracing::field::Field;
use tracing::Subscriber;
use tracing_subscriber::field::Visit;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;

use siegu_core::logfmt::{self, Level};

/// How many recent log entries to keep in memory for the in-app log viewer.
pub const RING_CAPACITY: usize = 2000;

/// A single structured log entry, matching what the frontend renders.
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

/// Sniff a severity label from free-form message text. Used by higher-level
/// log helpers (e.g. scan feed) that don't carry an explicit severity.
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

/// Persist a structured log entry: updates the in-memory ring for the viewer,
/// appends to the rotated debug log file (plain, no ANSI), prints to stderr
/// (colored when attached to a terminal), and emits a structured `log-message`
/// event to the frontend so it can render by real severity.
///
/// Accepts a level *string* (e.g. `"error"`, `"info"`) so CLI/panic-hook callers
/// can pass raw labels; unknown/empty strings fall back to `info`.
pub fn persist_log(level: &str, message: &str) {
    let level = Level::from_str(level);
    let entry = LogEntry {
        timestamp: now_rfc3339(),
        level: level.as_str().to_string(),
        message: message.to_string(),
    };

    {
        let mut buffer = ring().lock().unwrap_or_else(|e| e.into_inner());
        buffer.push(entry.clone());
    }

    // Single-line, symbol-prefixed text for the terminal (colored on a real
    // TTY, plain otherwise so piped output stays grep-friendly).
    if logfmt::color_enabled(logfmt::Stream::Stderr) {
        let colored = logfmt::format_line(level, &entry.message, true);
        eprintln!("[siegu] {colored}");
    } else {
        eprintln!("[siegu] {}", logfmt::format_plain(level, &entry.message));
    }

    if let Some(handle) = APP_HANDLE.get() {
        // Structured payload: { timestamp, level, message }
        let _ = handle.emit("log-message", &entry);
    }

    if let Some(dir) = app_config_dir() {
        if let Err(e) = logfmt::append_log_entry(&dir, level, &entry.message) {
            eprintln!("[siegu] failed to write debug log: {e}");
        }
    }
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
        let level = Level::from_str(event.metadata().level().as_str());
        persist_log(level.as_str(), &message);
    }
}

pub fn init_tracing() {
    // Default filter keeps the persisted log readable: info everywhere, with
    // whisper's per-token decode logs and zbus/dbus traffic capped. Set
    // RUST_LOG (e.g. RUST_LOG=debug) to override.
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        "info,dbus=warn,zbus=warn,siegu_core::ml_engine::whisper=warn".to_string()
    });
    let env_filter = tracing_subscriber::EnvFilter::try_new(filter)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
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
    fn level_from_str_known_values() {
        assert_eq!(Level::from_str("error").as_str(), "error");
        assert_eq!(Level::from_str("fatal").as_str(), "fatal");
        assert_eq!(Level::from_str("warn").as_str(), "warn");
        assert_eq!(Level::from_str("debug").as_str(), "debug");
        assert_eq!(Level::from_str("info").as_str(), "info");
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
