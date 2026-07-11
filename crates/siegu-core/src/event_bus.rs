use std::sync::{Arc, Mutex};

pub use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
    Debug,
}

pub trait EventBus: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
    fn log(&self, level: Level, message: &str);
}

pub struct NullEventBus;

impl EventBus for NullEventBus {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
    fn log(&self, _level: Level, _message: &str) {}
}

pub struct TracingEventBus;

impl EventBus for TracingEventBus {
    fn emit(&self, _event: &str, payload: serde_json::Value) {
        tracing::debug!(event = _event, ?payload, "event emitted");
    }

    fn log(&self, level: Level, message: &str) {
        match level {
            Level::Info => tracing::info!("{}", message),
            Level::Warn => tracing::warn!("{}", message),
            Level::Error => tracing::error!("{}", message),
            Level::Debug => tracing::debug!("{}", message),
        }
    }
}

pub struct CallbackEventBus {
    callback: Box<dyn Fn(&str, serde_json::Value) + Send + Sync>,
    log_callback: Box<dyn Fn(Level, &str) + Send + Sync>,
}

impl CallbackEventBus {
    pub fn new(
        callback: impl Fn(&str, serde_json::Value) + Send + Sync + 'static,
        log_callback: impl Fn(Level, &str) + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Box::new(callback),
            log_callback: Box::new(log_callback),
        }
    }
}

impl EventBus for CallbackEventBus {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        (self.callback)(event, payload);
    }

    fn log(&self, level: Level, message: &str) {
        (self.log_callback)(level, message);
    }
}

#[derive(Clone)]
pub struct ArcEventBus {
    inner: Arc<dyn EventBus>,
}

impl ArcEventBus {
    pub fn new(bus: impl EventBus + 'static) -> Self {
        Self {
            inner: Arc::new(bus),
        }
    }

    pub fn null() -> Self {
        Self::new(NullEventBus)
    }

    pub fn tracing() -> Self {
        Self::new(TracingEventBus)
    }
}

impl EventBus for ArcEventBus {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.inner.emit(event, payload);
    }

    fn log(&self, level: Level, message: &str) {
        self.inner.log(level, message);
    }
}

pub struct LogCollector {
    logs: Mutex<Vec<(Level, String)>>,
}

impl LogCollector {
    pub fn new() -> Self {
        Self {
            logs: Mutex::new(Vec::new()),
        }
    }

    pub fn logs(&self) -> Vec<(Level, String)> {
        self.logs.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.logs.lock().unwrap().clear();
    }
}

impl Default for LogCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus for LogCollector {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}

    fn log(&self, level: Level, message: &str) {
        self.logs.lock().unwrap().push((level, message.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_event_bus() {
        let bus = NullEventBus;
        bus.emit("test", serde_json::json!({"key": "value"}));
        bus.log(Level::Info, "test message");
    }

    #[test]
    fn test_log_collector() {
        let bus = LogCollector::new();
        bus.log(Level::Info, "hello");
        bus.log(Level::Error, "world");

        let logs = bus.logs();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0], (Level::Info, "hello".to_string()));
        assert_eq!(logs[1], (Level::Error, "world".to_string()));
    }

    #[test]
    fn test_log_collector_clear() {
        let bus = LogCollector::new();
        bus.log(Level::Info, "msg");
        assert_eq!(bus.logs().len(), 1);
        bus.clear();
        assert_eq!(bus.logs().len(), 0);
    }

    #[test]
    fn test_arc_event_bus() {
        let bus = ArcEventBus::null();
        bus.emit("test", serde_json::json!({}));
        bus.log(Level::Info, "test");
    }
}
