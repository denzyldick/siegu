use std::time::Duration;

use crate::signal::SignalMessage;

/// Outcome of a signalling-server reachability check.
#[derive(Debug, Clone)]
pub struct PingOutcome {
    pub ok: bool,
    pub message: String,
}

/// Normalize a user-supplied signalling URL into a form the client can dial.
///
/// Rules:
/// - missing scheme defaults to `wss://`
/// - `http://` / `https://` are mapped to `ws://` / `wss://`
/// - trailing slashes are trimmed
/// - an empty path (or a bare `/`) becomes `/ws` so it hits the signalling socket
pub fn normalize_signaling_url(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return s;
    }
    if !s.contains("://") {
        s = if let Some(rest) = s.strip_prefix("wss:") {
            format!("wss://{rest}")
        } else if let Some(rest) = s.strip_prefix("ws:") {
            format!("ws://{rest}")
        } else {
            format!("wss://{s}")
        };
    }
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("http://") {
        s = format!("ws://{}", &s[7..]);
    } else if lower.starts_with("https://") {
        s = format!("wss://{}", &s[8..]);
    }

    match url::Url::parse(&s) {
        Ok(mut u) => {
            let path = u.path();
            if path.is_empty() || path == "/" {
                u.set_path("/ws");
            }
            u.to_string()
        }
        Err(_) => s,
    }
}

fn extract_token(raw: &str) -> Option<String> {
    match raw.split_once('?') {
        Some((_, query)) => url::form_urlencoded::parse(query.as_bytes())
            .find(|(k, _)| k == "token")
            .map(|(_, v)| v.into_owned()),
        None => None,
    }
}

/// Connect to a signalling server and verify it answers a `CreateRoom` request.
pub async fn ping_signaling(raw_url: &str, timeout: Duration) -> PingOutcome {
    use futures_util::SinkExt;
    use futures_util::StreamExt;

    let url = normalize_signaling_url(raw_url);
    if url.is_empty() {
        return PingOutcome {
            ok: false,
            message: "Signalling URL is empty".to_string(),
        };
    }

    let token = extract_token(&url);

    let connect = async {
        let (mut ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .map_err(|e| e.to_string())?;
        let create = SignalMessage::CreateRoom { token };
        let payload = serde_json::to_string(&create).map_err(|e| e.to_string())?;
        ws.send(tokio_tungstenite::tungstenite::Message::text(payload))
            .await
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(ws)
    };

    let mut ws = match tokio::time::timeout(timeout, connect).await {
        Ok(Ok(ws)) => ws,
        Ok(Err(e)) => {
            return PingOutcome {
                ok: false,
                message: format!("Connection failed: {e}"),
            }
        }
        Err(_) => {
            return PingOutcome {
                ok: false,
                message: format!("Connection timed out after {}s", timeout.as_secs()),
            }
        }
    };

    let reply = async {
        loop {
            match ws.next().await {
                Some(Ok(msg)) if msg.is_text() => {
                    let text = msg.to_text().unwrap_or("");
                    match serde_json::from_str::<SignalMessage>(text) {
                        Ok(SignalMessage::RoomCreated { .. }) => {
                            return Ok::<PingOutcome, String>(PingOutcome {
                                ok: true,
                                message: "Signalling server reachable".to_string(),
                            });
                        }
                        Ok(SignalMessage::Error { message }) => {
                            return Ok::<PingOutcome, String>(PingOutcome {
                                ok: false,
                                message: format!("Server responded: {message}"),
                            });
                        }
                        _ => {}
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    return Ok::<PingOutcome, String>(PingOutcome {
                        ok: false,
                        message: format!("Connection error: {e}"),
                    });
                }
                None => {
                    return Ok::<PingOutcome, String>(PingOutcome {
                        ok: false,
                        message: "Server closed the connection".to_string(),
                    });
                }
            }
        }
    };

    match tokio::time::timeout(timeout, reply).await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(e)) => PingOutcome {
            ok: false,
            message: e.to_string(),
        },
        Err(_) => PingOutcome {
            ok: false,
            message: format!("No response after {}s", timeout.as_secs()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_adds_scheme() {
        assert_eq!(
            normalize_signaling_url("signal.example.com:8080"),
            "wss://signal.example.com:8080/ws"
        );
    }

    #[test]
    fn test_normalize_http_to_ws() {
        assert_eq!(
            normalize_signaling_url("http://192.168.1.5:8080"),
            "ws://192.168.1.5:8080/ws"
        );
        assert_eq!(
            normalize_signaling_url("https://siegu.io"),
            "wss://siegu.io/ws"
        );
    }

    #[test]
    fn test_normalize_keeps_existing_path() {
        assert_eq!(
            normalize_signaling_url("wss://siegu.io/ws"),
            "wss://siegu.io/ws"
        );
        assert_eq!(
            normalize_signaling_url("wss://siegu.io/custom"),
            "wss://siegu.io/custom"
        );
    }

    #[test]
    fn test_normalize_keeps_query_token() {
        assert_eq!(
            normalize_signaling_url("wss://siegu.io/ws?token=abc"),
            "wss://siegu.io/ws?token=abc"
        );
    }

    #[tokio::test]
    async fn test_ping_rejects_garbage() {
        let outcome = ping_signaling("not a url", Duration::from_secs(2)).await;
        assert!(!outcome.ok);
    }

    #[tokio::test]
    async fn test_ping_success_against_local_server() {
        let server = crate::lan_server::start(0).await;
        let port = server.port;
        let url = format!("ws://127.0.0.1:{port}");
        let outcome = ping_signaling(&url, Duration::from_secs(5)).await;
        assert!(outcome.ok, "expected ok, got: {}", outcome.message);
    }

    #[tokio::test]
    async fn test_ping_token_required() {
        let server = crate::lan_server::start_with_config(crate::lan_server::ServerConfig {
            port: 0,
            token: Some("s3cret".to_string()),
            web_dist: None,
        })
        .await;
        let port = server.port;

        let bad = ping_signaling(&format!("ws://127.0.0.1:{port}"), Duration::from_secs(5)).await;
        assert!(!bad.ok, "expected rejection without token");

        let good = ping_signaling(
            &format!("ws://127.0.0.1:{port}?token=s3cret"),
            Duration::from_secs(5),
        )
        .await;
        assert!(
            good.ok,
            "expected success with token, got: {}",
            good.message
        );
    }
}
