use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

/// Managed state tracking whether the main window currently has focus. When the
/// app is focused the in-app UI already shows progress, so routine notifications
/// are suppressed; critical ones still fire (see `notify_routine`/`notify_critical`).
pub struct FocusState(pub AtomicBool);

impl Default for FocusState {
    fn default() -> Self {
        Self(AtomicBool::new(true))
    }
}

/// Last time a sync/upload reported progress (unix millis). Used by the
/// stalled-backup monitor to detect a hanging transfer even while the session
/// itself remains "connected".
static LAST_SYNC_ACTIVITY_MS: AtomicU64 = AtomicU64::new(0);
/// Tracks whether we already surfaced a "backup stalled" alert for the current
/// stalled stretch, so we don't repeat it every poll tick.
static STALL_ALERTED: AtomicBool = AtomicBool::new(false);

fn is_focused(app: &AppHandle) -> bool {
    app.try_state::<FocusState>()
        .map(|s| s.0.load(Ordering::Relaxed))
        .unwrap_or(true)
}

/// Show an OS notification for routine background activity. Suppressed while the
/// main window is focused so it doesn't double up on the in-app progress UI.
pub fn notify_routine(app: &AppHandle, body: impl Into<String> + Send) {
    if is_focused(app) {
        return;
    }
    let body: String = body.into();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = app
            .notification()
            .builder()
            .title("Siegu")
            .body(body)
            .show();
    });
}

/// Show an OS notification for a critical state (sync paused, backup stalled,
/// photos ready to sync). Always fires regardless of window focus.
pub fn notify_critical(app: &AppHandle, body: impl Into<String> + Send) {
    let body: String = body.into();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = app
            .notification()
            .builder()
            .title("Siegu")
            .body(body)
            .show();
    });
}

/// Record that a sync/upload just made progress. Called on every sync progress
/// event so the stalled monitor can tell "still transferring" from "hung".
pub fn mark_sync_activity() {
    STALL_ALERTED.store(false, Ordering::SeqCst);
    let now = crate::common::unix_now_ms();
    LAST_SYNC_ACTIVITY_MS.store(now, Ordering::Relaxed);
}

/// Background monitor: while a live peer connection exists, if no sync/upload
/// makes progress for `STALL_THRESHOLD`, fire a "backup stalled" critical alert
/// (once per stalled stretch). Resets as soon as activity or state resumes.
pub fn spawn_backup_stall_monitor(app: &AppHandle, connected: Arc<AtomicBool>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let threshold = Duration::from_secs(60);
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if !connected.load(Ordering::SeqCst) {
                STALL_ALERTED.store(false, Ordering::SeqCst);
                continue;
            }
            let last = LAST_SYNC_ACTIVITY_MS.load(Ordering::Relaxed);
            if last == 0 {
                // Session connected but no progress observed yet; not stale.
                continue;
            }
            let now = crate::common::unix_now_ms();
            let idle = now.saturating_sub(last);
            if idle >= threshold.as_millis() as u64 && !STALL_ALERTED.swap(true, Ordering::SeqCst) {
                notify_critical(
                    &app,
                    "A backup appears to be stalled — no progress for a while. ".to_string()
                        + "Check the device connection.",
                );
            }
        }
    });
}
