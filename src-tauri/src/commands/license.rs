/// Siegu Pro license verification.
///
/// The paid (Pro) tier is unlocked in the app by entering the email used at
/// checkout. The entitlement is recorded server-side by the Cloudflare Worker
/// (`workers/pro-license`) when Stripe reports `checkout.session.completed`.
/// This command asks that worker whether the given email is paid, returning a
/// JSON string so the UI can render the outcome without depending on the exact
/// error type.
///
/// The worker URL and shared token are read from the app config
/// (`pro_license_url` / `pro_license_token`) so they are user-overridable and
/// not baked into the binary.
use std::collections::HashMap;
use std::time::Duration;

/// Pure business logic — testable without Tauri.
pub async fn do_verify_pro_email(
    email: &str,
    worker_url: &str,
    token: &str,
) -> Result<serde_json::Value, String> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        return Ok(
            serde_json::json!({ "ok": false, "paid": false, "verified": false, "error": "invalid_email" }),
        );
    }
    let url = format!(
        "{}/verify?email={}",
        worker_url.trim().trim_end_matches('/'),
        urlencoding(email)
    );
    if token.trim().is_empty() {
        return Ok(
            serde_json::json!({ "ok": false, "paid": false, "verified": false, "error": "missing_token" }),
        );
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&url)
        .header("x-siegu-token", token.trim())
        .send()
        .await
        .map_err(|e| format!("network_error: {e}"))?;

    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::json!({
        "ok": false,
        "paid": false,
        "verified": false,
        "error": "bad_response",
    }));

    Ok(serde_json::json!({
        "ok": parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        "paid": parsed.get("paid").and_then(|v| v.as_bool()).unwrap_or(false),
        "verified": parsed.get("verified").and_then(|v| v.as_bool()).unwrap_or(false),
        "email": parsed.get("email").and_then(|v| v.as_str()).unwrap_or(email),
        "plan": parsed.get("plan").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "error": parsed.get("error").and_then(|v| v.as_str()).unwrap_or(""),
        "status": status,
    }))
}

/// Ask the worker to email a validation link to the buyer. Only proceeds when
/// the worker says the email already paid. Returns the worker's JSON envelope.
pub async fn do_send_pro_verification(
    email: &str,
    worker_url: &str,
    token: &str,
) -> Result<serde_json::Value, String> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        return Ok(serde_json::json!({ "ok": false, "sent": false, "error": "invalid_email" }));
    }
    let url = format!(
        "{}/send-verify?email={}",
        worker_url.trim().trim_end_matches('/'),
        urlencoding(email)
    );
    if token.trim().is_empty() {
        return Ok(serde_json::json!({ "ok": false, "sent": false, "error": "missing_token" }));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(&url)
        .header("x-siegu-token", token.trim())
        .send()
        .await
        .map_err(|e| format!("network_error: {e}"))?;

    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::json!({
        "ok": false,
        "sent": false,
        "error": "bad_response",
    }));

    Ok(serde_json::json!({
        "ok": parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        "sent": parsed.get("sent").and_then(|v| v.as_bool()).unwrap_or(false),
        "paid": parsed.get("paid").and_then(|v| v.as_bool()).unwrap_or(parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)),
        "email": parsed.get("email").and_then(|v| v.as_str()).unwrap_or(email),
        "error": parsed.get("error").and_then(|v| v.as_str()).unwrap_or(""),
        "status": status,
    }))
}

/// Helper to extract configured Pro settings from a config map.
pub fn pro_config_from(config: &HashMap<String, String>) -> (String, String) {
    let url = config.get("pro_license_url").cloned().unwrap_or_default();
    let token = config.get("pro_license_token").cloned().unwrap_or_default();
    (url, token)
}

/// Simple percent-encoding for the email query param (no external dep needed).
fn urlencoding(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' | b'~' | b'@' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[tauri::command]
pub async fn verify_pro_email(
    app: tauri::AppHandle,
    email: String,
    worker_url: String,
    token: String,
) -> String {
    use crate::common::get_config_path;
    use crate::database::Database;

    let path = get_config_path(&app);
    // Fall back to the explicitly-passed URL/token; otherwise the caller
    // should have resolved them from config already.
    let (url, tok) = if worker_url.trim().is_empty() && token.trim().is_empty() && !path.is_empty()
    {
        let db = Database::new(&path);
        let cfg = db.get_state();
        pro_config_from(&cfg)
    } else {
        (worker_url, token)
    };

    match do_verify_pro_email(&email, &url, &tok).await {
        Ok(json) => json.to_string(),
        Err(e) => serde_json::json!({ "ok": false, "paid": false, "verified": false, "error": e })
            .to_string(),
    }
}

#[tauri::command]
pub async fn send_pro_verification(
    app: tauri::AppHandle,
    email: String,
    worker_url: String,
    token: String,
) -> String {
    use crate::common::get_config_path;
    use crate::database::Database;

    let path = get_config_path(&app);
    let (url, tok) = if worker_url.trim().is_empty() && token.trim().is_empty() && !path.is_empty()
    {
        let db = Database::new(&path);
        let cfg = db.get_state();
        pro_config_from(&cfg)
    } else {
        (worker_url, token)
    };

    match do_send_pro_verification(&email, &url, &tok).await {
        Ok(json) => json.to_string(),
        Err(e) => serde_json::json!({ "ok": false, "sent": false, "error": e }).to_string(),
    }
}
