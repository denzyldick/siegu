//! Shared, consistent log formatting and file rotation for every layer of
//! Siegu (Tauri app, CLI, core). Single source of truth for the symbols and
//! ANSI colors used to make logs readable, and for the on-disk debug log
//! rotation policy.
//!
//! Design goals:
//! - One human-friendly, stable line format so logs can be shared for support
//!   (no telemetry) and traced easily.
//! - Colors/symbols appear only on a real terminal so piped output (CI grep
//!   scripts, log files) stays clean and greppable.
//! - The persisted log file is always plain (no ANSI): easy to paste/ticket.
//! - Size-based rotation with a bounded number of kept files, unit-tested.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

// --------------------------------------------------------------------------
// Level
// --------------------------------------------------------------------------

/// Severity of a log line. Mirrors `tracing` levels plus a `Fatal` used by
/// the desktop app's panic hook, and is ordered loosely by severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Fatal,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Level {
    /// Parse a lowercase/uppercase level name; falls back to `Info` for
    /// anything unrecognised (we never want logging to crash a hot path).
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "fatal" => Level::Fatal,
            "error" => Level::Error,
            "warn" | "warning" => Level::Warn,
            "debug" => Level::Debug,
            "trace" => Level::Trace,
            _ => Level::Info,
        }
    }

    /// Stable serialized name (lowercase) used in files and structured events.
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Fatal => "fatal",
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
            Level::Trace => "trace",
        }
    }

    /// Uppercase tag used in the plain file format and the structured viewer.
    pub fn as_tag(&self) -> &'static str {
        match self {
            Level::Fatal => "FATAL",
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }

    /// Prefix symbol shown on a terminal before the message.
    pub fn symbol(&self) -> &'static str {
        match self {
            Level::Fatal => "✗",
            Level::Error => "✗",
            Level::Warn => "⚠",
            Level::Info => "ℹ",
            Level::Debug => "•",
            Level::Trace => "·",
        }
    }

    /// ANSI color name used on a terminal.
    pub fn color(&self) -> &'static str {
        match self {
            Level::Fatal => "31", // red
            Level::Error => "31", // red
            Level::Warn => "33",  // yellow
            Level::Info => "36",  // cyan
            Level::Debug => "90", // bright black / gray
            Level::Trace => "90",
        }
    }
}

// --------------------------------------------------------------------------
// ANSI helpers
// --------------------------------------------------------------------------

fn ansi(sgr: &str, s: &str) -> String {
    format!("\x1b[{sgr}m{s}\x1b[0m")
}

/// Whether color should be emitted for the given stream right now.
/// Colors are only used when the stream is a real terminal and `NO_COLOR`
/// (https://no-color.org) or `SIEGU_NO_COLOR` is not set.
pub fn color_enabled(stream: Stream) -> bool {
    if std::env::var_os("NO_COLOR").is_some() || std::env::var_os("SIEGU_NO_COLOR").is_some() {
        return false;
    }
    match stream {
        Stream::Stdout => std::io::stdout().is_terminal(),
        Stream::Stderr => std::io::stderr().is_terminal(),
    }
}

/// Which standard stream a log line targets; determines color detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

// --------------------------------------------------------------------------
// Line formatting
// --------------------------------------------------------------------------

/// Format a single log line for a terminal. When `color` is true this
/// includes ANSI styles and the level symbol; otherwise it is plain.
pub fn format_line(level: Level, message: &str, color: bool) -> String {
    if color {
        format!(
            "{} {}",
            ansi(level.color(), level.symbol()),
            ansi("90", &format!("[{}]", level.as_tag().to_ascii_lowercase())),
        ) + &format!(" {message}")
    } else {
        format_plain(level, message)
    }
}

/// Format a single log line as plain text (for files, pipes, CI).
/// Always includes a timestamp-free stable shape: `[LEVEL] message`.
pub fn format_plain(level: Level, message: &str) -> String {
    format!("[{}] {message}", level.as_tag())
}

/// Format a line carrying an explicit symbol (e.g. CLI `✓`/`▶` prefixes that
/// differ from the level default). Colored mode emits `SYMBOL [tag] message`;
/// plain mode emits `[TAG] symbol message` so the symbol survives piping while
/// any embedded greppable tokens stay intact.
pub fn format_line_symbol(level: Level, symbol: &str, message: &str, color: bool) -> String {
    if color {
        format!(
            "{} [{}] {message}",
            ansi(level.color(), symbol),
            ansi("90", &format!("[{}]", level.as_tag().to_ascii_lowercase())),
        )
    } else {
        format!("[{}] {symbol} {message}", level.as_tag())
    }
}

// --------------------------------------------------------------------------
// Convenience constructors: consistent step/status prefixes
// --------------------------------------------------------------------------

/// A success/step rendered with a green `✓`. Usually paired with an earlier
/// `step()` line to produce a trace like:
///   ▶ Scanning folder …         (step)
///   ✓ Scan complete (42 new)
pub fn ok(message: impl AsRef<str>) -> String {
    format!("✓ {}", message.as_ref())
}

/// A failure rendered with a red `✗`.
pub fn err(message: impl AsRef<str>) -> String {
    format!("✗ {}", message.as_ref())
}

/// A warning rendered with a yellow `⚠`.
pub fn warn(message: impl AsRef<str>) -> String {
    format!("⚠ {}", message.as_ref())
}

/// An informational line with a cyan `ℹ`.
pub fn info(message: impl AsRef<str>) -> String {
    format!("ℹ {}", message.as_ref())
}

/// A step/sub-step with a `▶` arrow, for multi-step work documented in logs.
pub fn step(message: impl AsRef<str>) -> String {
    format!("▶ {}", message.as_ref())
}

// --------------------------------------------------------------------------
// Log file + rotation
// --------------------------------------------------------------------------

/// Default maximum size of the active `siegu_debug.log` before it becomes `.1`.
pub const MAX_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// Default number of rotated `.1`/`.2`/… files kept (not counting the active).
pub const MAX_ROTATED_FILES: usize = 5;
/// Default total size budget for the active + all rotated files. When a write
/// would push the total over this, the oldest rotated file is pruned.
pub const MAX_TOTAL_LOG_BYTES: u64 = 30 * 1024 * 1024;
/// Name of the active debug log.
pub const LOG_FILE_NAME: &str = "siegu_debug.log";

/// Rotating file settings. Injectable so tests/drivers can exercise rotation
/// with tiny limits without touching the real config directory or waiting for
/// several megabytes of logs.
#[derive(Debug, Clone, Copy)]
pub struct LogConfig {
    /// Per-file rotation threshold (bytes).
    pub max_file_bytes: u64,
    /// Number of rotated files kept (excluding the active file).
    pub max_rotated_files: usize,
    /// Combined size budget for the active + rotated files (bytes).
    pub max_total_bytes: u64,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: MAX_LOG_FILE_BYTES,
            max_rotated_files: MAX_ROTATED_FILES,
            max_total_bytes: MAX_TOTAL_LOG_BYTES,
        }
    }
}

/// Full path to the `siegu_debug.log` file in `dir`.
pub fn log_path(dir: &Path) -> PathBuf {
    dir.join(LOG_FILE_NAME)
}

/// Path to the `siegu_debug.log.N` rotated file (1-based).
pub fn rotated_path(dir: &Path, n: usize) -> PathBuf {
    dir.join(format!("{LOG_FILE_NAME}.{n}"))
}

/// Serialize a single log line (used by the write path/tests). The persisted
/// file line carries an RFC3339 timestamp so shared debug logs are traceable:
/// `2026-01-02T03:04:05Z [INFO] message`.
fn line_bytes(level: Level, message: &str) -> Vec<u8> {
    let ts = chrono::Utc::now().to_rfc3339();
    format!("{ts} {}\n", format_plain(level, message)).into_bytes()
}
/// Rotate the current log file chain forward by one slot: the active file
/// becomes `.1`, `.1` becomes `.2`, … and any slot beyond `cfg.max_rotated_files`
/// is dropped. Missing files are skipped; errors on a single rotation are
/// non-fatal (logging must never crash the app).
fn rotate_forward(dir: &Path, cfg: &LogConfig) {
    // Remove the oldest slot first so renames never collide.
    let oldest = rotated_path(dir, cfg.max_rotated_files);
    let _ = std::fs::remove_file(&oldest);

    for i in (1..cfg.max_rotated_files).rev() {
        let src = rotated_path(dir, i);
        if src.exists() {
            let dst = rotated_path(dir, i + 1);
            let _ = std::fs::rename(&src, &dst);
        }
    }

    let active = log_path(dir);
    if active.exists() {
        let _ = std::fs::rename(&active, rotated_path(dir, 1));
    }
}

/// Remove the oldest rotated file(s) until the total log size is within
/// `cfg.max_total_bytes`. Best-effort; never fatal. Stops if no file was
/// removed (nothing left to prune) to avoid an infinite loop.
fn prune_to_budget(dir: &Path, cfg: &LogConfig) {
    loop {
        let total: u64 = (1..=cfg.max_rotated_files)
            .map(|i| {
                std::fs::metadata(rotated_path(dir, i))
                    .map(|m| m.len())
                    .unwrap_or(0)
            })
            .sum::<u64>()
            + std::fs::metadata(log_path(dir))
                .map(|m| m.len())
                .unwrap_or(0);
        if total <= cfg.max_total_bytes {
            return;
        }
        let mut removed = false;
        for i in (1..=cfg.max_rotated_files).rev() {
            let p = rotated_path(dir, i);
            if p.exists() {
                let _ = std::fs::remove_file(&p);
                removed = true;
                break;
            }
        }
        if !removed {
            // Nothing removable left (only the active file exceeds budget and
            // that is handled by per-file rotation, not pruning).
            return;
        }
    }
}

/// Append one entry to the active debug log, rotating/pruning as needed.
/// Uses default rotation settings (5 MB / 5 files / 30 MB budget).
/// Returns the number of bytes written. Never panics.
pub fn append_log_entry(dir: &Path, level: Level, message: &str) -> io::Result<usize> {
    append_log_entry_cfg(dir, &LogConfig::default(), level, message)
}

/// Append one entry using explicit rotation settings (used directly by tests
/// and by callers that want non-default limits).
fn append_log_entry_cfg(
    dir: &Path,
    cfg: &LogConfig,
    level: Level,
    message: &str,
) -> io::Result<usize> {
    let _ = std::fs::create_dir_all(dir);
    let bytes = line_bytes(level, message);
    let path = log_path(dir);

    // Rotate when the active file would exceed the per-file limit.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() + bytes.len() as u64 > cfg.max_file_bytes {
            rotate_forward(dir, cfg);
        }
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(&bytes)?;
    file.flush()?;
    let written = bytes.len();

    // Prune oldest files once the whole set grows too large.
    prune_to_budget(dir, cfg);
    Ok(written)
}

use std::io;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_from_str_round_trip() {
        assert_eq!(Level::from_str("info"), Level::Info);
        assert_eq!(Level::from_str("INFO"), Level::Info);
        assert_eq!(Level::from_str("warn"), Level::Warn);
        assert_eq!(Level::from_str("Warning"), Level::Warn);
        assert_eq!(Level::from_str("error"), Level::Error);
        assert_eq!(Level::from_str("fatal"), Level::Fatal);
        assert_eq!(Level::from_str("debug"), Level::Debug);
        assert_eq!(Level::from_str("trace"), Level::Trace);
        // Unknown falls back to Info, never errors.
        assert_eq!(Level::from_str("bogus"), Level::Info);
        assert_eq!(Level::from_str(""), Level::Info);
    }

    #[test]
    fn level_as_str_stable() {
        assert_eq!(Level::Info.as_str(), "info");
        assert_eq!(Level::Warn.as_str(), "warn");
        assert_eq!(Level::Error.as_str(), "error");
        assert_eq!(Level::Fatal.as_str(), "fatal");
    }

    #[test]
    fn format_plain_no_ansi() {
        let line = format_plain(Level::Info, "hello world");
        assert_eq!(line, "[INFO] hello world");
        assert!(!line.contains('\x1b'));
        let err_line = format_plain(Level::Error, "boom");
        assert_eq!(err_line, "[ERROR] boom");
        assert!(!err_line.contains('\x1b'));
    }

    #[test]
    fn format_line_colored_has_ansi_and_symbol() {
        let line = format_line(Level::Error, "boom", true);
        assert!(line.contains('\x1b'));
        assert!(line.contains("✗"), "error should carry ✗ symbol: {line}");
        assert!(line.contains('m'), "should contain ANSI SGR sequences");
        assert!(line.contains("boom"));
    }

    #[test]
    fn format_line_not_colored_is_plain() {
        let line = format_line(Level::Info, "hi", false);
        assert!(!line.contains('\x1b'));
        assert_eq!(line, "[INFO] hi");
    }

    #[test]
    fn symbols_match_levels() {
        assert_eq!(Level::Info.symbol(), "ℹ");
        assert_eq!(Level::Warn.symbol(), "⚠");
        assert_eq!(Level::Error.symbol(), "✗");
        assert_eq!(Level::Fatal.symbol(), "✗");
    }

    #[test]
    fn helper_prefixes() {
        assert_eq!(ok("done"), "✓ done");
        assert_eq!(err("fail"), "✗ fail");
        assert_eq!(warn("careful"), "⚠ careful");
        assert_eq!(info("note"), "ℹ note");
        assert_eq!(step("next"), "▶ next");
    }

    // ── rotation ──────────────────────────────────────────────────────────

    /// Tiny config so tests exercise rotation without allocating megabytes.
    fn tiny_cfg(max_file_bytes: u64) -> LogConfig {
        LogConfig {
            max_file_bytes,
            max_rotated_files: 3,
            // Generous enough to let a couple rotated files coexist (so `.1`
            // survives the budget prune) while staying far below real sizes.
            max_total_bytes: max_file_bytes * 20,
        }
    }

    #[test]
    fn append_creates_plain_file() {
        let dir = tempfile::tempdir().unwrap();
        append_log_entry(dir.path(), Level::Info, "first").unwrap();
        append_log_entry(dir.path(), Level::Error, "second").unwrap();
        let content = std::fs::read_to_string(log_path(dir.path())).unwrap();
        assert!(content.contains("[INFO] first"));
        assert!(content.contains("[ERROR] second"));
        assert!(
            !content.contains('\x1b'),
            "file must stay plain, got {content:?}"
        );
    }

    #[test]
    fn rotate_creates_rotated_files_and_caps_count() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = tiny_cfg(16); // every ~1-2 lines triggers rotation
        append_log_entry_cfg(dir.path(), &cfg, Level::Info, "aaaaaaaaaaaaaaaa").unwrap(); // > 16 bytes
        append_log_entry_cfg(dir.path(), &cfg, Level::Info, "aaaaaaaaaaaaaaaa").unwrap();
        append_log_entry_cfg(dir.path(), &cfg, Level::Info, "aaaaaaaaaaaaaaaa").unwrap();
        append_log_entry_cfg(dir.path(), &cfg, Level::Info, "aaaaaaaaaaaaaaaa").unwrap();
        // No slot above the cap may exist.
        assert!(
            !rotated_path(dir.path(), 4).exists(),
            "slot 4 must not exist"
        );
        assert!(
            !rotated_path(dir.path(), 5).exists(),
            "slot 5 must not exist"
        );
        // The active file survives.
        assert!(
            log_path(dir.path()).exists(),
            "active file must survive rotation"
        );
    }

    #[test]
    fn write_pressure_produces_multiple_rotations() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = tiny_cfg(16);
        for i in 0..40 {
            append_log_entry_cfg(dir.path(), &cfg, Level::Info, &"x".repeat(24)).unwrap();
            let _ = i;
        }
        assert!(
            rotated_path(dir.path(), 1).exists(),
            "expected at least one .1 file"
        );
        assert!(
            log_path(dir.path()).exists(),
            "active file must still exist"
        );
        // Total budget (6 * 16 = 96 bytes) bounds the set.
        let total: u64 = (1..=cfg.max_rotated_files)
            .map(|i| {
                std::fs::metadata(rotated_path(dir.path(), i))
                    .map(|m| m.len())
                    .unwrap_or(0)
            })
            .sum::<u64>()
            + std::fs::metadata(log_path(dir.path()))
                .map(|m| m.len())
                .unwrap_or(0);
        assert!(
            total <= cfg.max_total_bytes,
            "rotation set must respect the total budget, got {total} bytes"
        );
    }
}
