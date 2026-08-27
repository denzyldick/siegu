//! Unified CLI output: single-line, symbol-prefixed status lines matching the
//! shared `siegu_core::logfmt` convention. Colors are applied only when the
//! stream is a real terminal (`NO_COLOR`/`SIEGU_NO_COLOR` respected) so piped
//! output and CI grep scripts stay plain and reliable.
//!
//! Every line is `[LEVEL] <symbol> <text>` (the `[LEVEL]` tag is what makes the
//! output grep-friendly and consistent with the app). Levels:
//!   - `ok!`    → ✓   (Info,  stdout)
//!   - `info!`  → ℹ   (Info,  stdout)
//!   - `step!`  → ▶   (Info,  stdout)
//!   - `warn!`  → ⚠   (Warn,  stderr)
//!   - `err!`   → ✗   (Error, stderr)
//!   - `fatal!` → ✗   (Fatal, stderr)
//!
//! `line!` writes raw text with the `[LEVEL]` tag but no symbol, for
//! hand-tuned greppable markers (e.g. `VIEWONLY ...`, `RPC RESULT ...`).
use std::io::Write;

use siegu_core::logfmt::{self, Level};

fn stream_for(level: Level) -> logfmt::Stream {
    match level {
        Level::Fatal | Level::Error | Level::Warn => logfmt::Stream::Stderr,
        _ => logfmt::Stream::Stdout,
    }
}

/// Emit a single formatted line to the given stream. `symbol` overrides the
/// level's default prefix (used by `ok!`/`step!` etc.); `None` uses the level
/// symbol. Colored on a real terminal, plain otherwise.
fn write_line(
    stream: logfmt::Stream,
    level: Level,
    symbol: Option<&str>,
    args: std::fmt::Arguments,
) {
    let sym = symbol.unwrap_or(level.symbol());
    let use_color = logfmt::color_enabled(stream);
    let line = logfmt::format_line_symbol(level, sym, &format!("{args}"), use_color);
    match stream {
        logfmt::Stream::Stdout => {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{line}");
        }
        logfmt::Stream::Stderr => {
            let mut out = std::io::stderr().lock();
            let _ = writeln!(out, "{line}");
        }
    }
}

/// Emit a line with an explicit symbol and level tag.
pub fn emit(level: Level, symbol: &str, args: std::fmt::Arguments) {
    write_line(stream_for(level), level, Some(symbol), args);
}

/// Emit a plain level-tagged line (no custom symbol), for greppable markers.
pub fn line(level: Level, args: std::fmt::Arguments) {
    write_line(stream_for(level), level, None, args);
}

#[macro_export]
macro_rules! cli_line {
    ($($arg:tt)*) => {
        $crate::logging::line(siegu_core::logfmt::Level::Info, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_ok {
    ($($arg:tt)*) => {
        $crate::logging::emit(siegu_core::logfmt::Level::Info, "✓", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_info {
    ($($arg:tt)*) => {
        $crate::logging::emit(siegu_core::logfmt::Level::Info, "ℹ", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_step {
    ($($arg:tt)*) => {
        $crate::logging::emit(siegu_core::logfmt::Level::Info, "▶", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_warn {
    ($($arg:tt)*) => {
        $crate::logging::line(siegu_core::logfmt::Level::Warn, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_err {
    ($($arg:tt)*) => {
        $crate::logging::line(siegu_core::logfmt::Level::Error, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_fatal {
    ($($arg:tt)*) => {
        $crate::logging::line(siegu_core::logfmt::Level::Fatal, format_args!($($arg)*))
    };
}

// ── tracing-subscriber integration ──────────────────────────────────────────
// The CLI uses `tracing` for a few `warn!`/`info!`/`error!` calls (e.g. TUI
// spinner fallback). Route them through the same single-line, symbol-prefixed
// format instead of the default multi-field fmt output.

use std::fmt;
use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::registry::LookupSpan;

struct CliFormatter;

struct MessageVisitor {
    message: Option<String>,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }
}

impl<S, N> FormatEvent<S, N> for CliFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> fmt::Result {
        let mut visitor = MessageVisitor { message: None };
        event.record(&mut visitor);
        let msg = visitor.message.unwrap_or_default();

        let level = level_from_str(event.metadata().level().as_str());
        let color = logfmt::color_enabled(logfmt::Stream::Stderr);
        let text = logfmt::format_line(level, &msg, color);
        writeln!(writer, "{text}")
    }
}

fn level_from_str(s: &str) -> Level {
    Level::from_str(s)
}

/// Install the CLI-wide tracing subscriber with the symbol formatter.
pub fn init_tracing() {
    use tracing_subscriber::prelude::*;
    let filter = std::env::var("RUST_LOG")
        .ok()
        .and_then(|f| tracing_subscriber::EnvFilter::try_new(f).ok())
        .unwrap_or_else(|| tracing_subscriber::EnvFilter::new("info"));
    let subscriber = tracing_subscriber::registry().with(filter).with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .event_format(CliFormatter),
    );
    let _ = tracing::subscriber::set_global_default(subscriber);
}
