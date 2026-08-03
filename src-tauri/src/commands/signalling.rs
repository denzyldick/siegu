/// Reachability check for a self-hosted or hosted signalling server.
///
/// Returns a JSON string `{"ok": bool, "message": string}` so callers can
/// render the outcome without depending on the exact error type.
#[tauri::command]
pub async fn ping_signaling(url: String) -> String {
    let outcome = siegu_core::ping_signaling(&url, std::time::Duration::from_secs(5)).await;
    serde_json::json!({ "ok": outcome.ok, "message": outcome.message }).to_string()
}
