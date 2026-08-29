use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use siegu_core::database::Database;
use siegu_core::lan_server::{self, ServerConfig};
use siegu_core::mesh_transport::MeshTransport;
use siegu_core::rpc::{dispatch, RpcContext, ShareMode};
use siegu_core::signal::SignalMessage;
use siegu_core::thumbnail::generate_thumbnail_bytes;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::CliSyncEvent;

type BoxError = Box<dyn std::error::Error>;

/// Best-effort LAN address discovery: ask the OS which source address it
/// would use to reach the internet. No packet is actually sent.
fn lan_ip() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

/// Open a persistent "room keeper" connection that creates the session room
/// and keeps it alive while the CLI process runs. The code is chosen by the
/// signalling server; the browser needs it (plus the token) to join.
async fn create_room(signal_url: &str, token: &str) -> Result<String, BoxError> {
    let (ws, _resp) = tokio_tungstenite::connect_async(signal_url)
        .await
        .map_err(|e| format!("failed to reach embedded signalling server: {e}"))?;
    let (mut write, read) = ws.split();

    let create = SignalMessage::CreateRoom {
        token: Some(token.to_string()),
    };
    let json = serde_json::to_string(&create)?;
    write
        .send(WsMessage::Text(json.into()))
        .await
        .map_err(|e| format!("failed to send CreateRoom: {e}"))?;

    // Hand ownership to the keeper task: it holds the write half open (so the
    // socket — and therefore the room — survives) and scans replies for the
    // RoomCreated code, forwarding it over a oneshot channel.
    let (code_tx, code_rx) = tokio::sync::oneshot::channel::<String>();
    tokio::spawn(async move {
        let mut write_enabled = write;
        let mut stream = read;
        let _ = &mut write_enabled; // keep the sink alive for the session
        let mut code_tx = Some(code_tx);
        while let Some(Ok(msg)) = stream.next().await {
            if let WsMessage::Text(text) = msg {
                if let Ok(SignalMessage::RoomCreated { code }) =
                    serde_json::from_str::<SignalMessage>(&text)
                {
                    if let Some(tx) = code_tx.take() {
                        let _ = tx.send(code);
                    }
                    // Continue draining traffic so the TCP window never fills.
                }
            }
        }
    });

    let code = tokio::time::timeout(std::time::Duration::from_secs(5), code_rx)
        .await
        .map_err(|_| "timed out waiting for a room code")?
        .map_err(|_| "signalling connection closed before a room was created".to_string())?;
    Ok(code)
}

/// Bridge one browser WebSocket to the loopback signalling server: frames are
/// spliced verbatim in both directions so the browser speaks the exact same
/// `/ws` protocol without the signalling port being exposed separately.
async fn bridge_to_signal(
    browser: warp::ws::WebSocket,
    signal_url: String,
) -> Result<(), BoxError> {
    let (upstream, _resp) = tokio_tungstenite::connect_async(&signal_url)
        .await
        .map_err(|e| format!("bridge: signalling dial failed: {e}"))?;
    let (mut up_write, mut up_read) = upstream.split();
    let (mut down_write, mut down_read) = browser.split();

    let to_browser = tokio::spawn(async move {
        while let Some(Ok(frame)) = up_read.next().await {
            let out = match frame {
                WsMessage::Text(t) => warp::ws::Message::text(t.as_str()),
                WsMessage::Binary(b) => warp::ws::Message::binary(b.to_vec()),
                WsMessage::Close(_) => break,
                _ => continue,
            };
            if down_write.send(out).await.is_err() {
                break;
            }
        }
        let _ = down_write.close().await;
    });

    while let Some(Ok(frame)) = down_read.next().await {
        let out = if frame.is_text() {
            frame.to_str().ok().map(|s| WsMessage::Text(s.into()))
        } else if frame.is_binary() {
            Some(WsMessage::Binary(frame.as_bytes().to_vec().into()))
        } else if frame.is_close() {
            break;
        } else {
            continue;
        };
        if let Some(out) = out {
            if up_write.send(out).await.is_err() {
                break;
            }
        }
    }
    up_write.close().await.ok();
    to_browser.abort();
    Ok(())
}

/// Root directories (monitored photo folders) the static file server is allowed
/// to read. Mirrors the media-server roots in `src-tauri/src/transport.rs`: only
/// scanned library directories are reachable, never the whole filesystem.
fn allowed_roots(config_path: &str) -> Vec<PathBuf> {
    let db = Database::new(config_path);
    db.list_directories()
        .into_iter()
        .map(PathBuf::from)
        // Keep only roots that actually exist so canonicalize can resolve them.
        .filter(|p| p.is_dir())
        .collect()
}

/// True when `candidate` canonicalizes to an existing file that lives inside one
/// of the allowed roots (defends against `etc/passwd`-style reads and symlink
/// escapes, mirroring `transport.rs::path_within_roots`).
fn path_within_roots(roots: &[PathBuf], candidate: &Path) -> bool {
    let Ok(canon) = std::fs::canonicalize(candidate) else {
        return false;
    };
    if !canon.is_file() {
        return false;
    }
    roots.iter().any(|root| {
        std::fs::canonicalize(root)
            .map(|r| canon.starts_with(&r))
            .unwrap_or(false)
    })
}

/// Map a photo id to a servable, root-scoped absolute file path (or None).
fn photo_path(config_path: &str, id: &str) -> Option<PathBuf> {
    let db = Database::new(config_path);
    let location = db.get_photo_location(id)?;
    let p = PathBuf::from(location);
    let roots = allowed_roots(config_path);
    // Refuse to serve when the location isn't inside a monitored directory.
    if !path_within_roots(&roots, &p) {
        return None;
    }
    Some(p)
}

/// Small extension→MIME map so originals get sane `Content-Type` without pulling
/// `mime_guess` into the CLI crate. Images/videos default to application/octet-stream.
fn mime_for(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" | "heif" => "image/heic",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "m4v" => "video/mp4",
        _ => "application/octet-stream",
    }
}

/// A uniform 404 reply for denied webHost data requests: never reveals that the
/// protected surface exists (#28). Body type `Vec<u8>` matches the media/thumb
/// handlers so the reply type unifies.
fn denied_response() -> warp::http::Response<Vec<u8>> {
    warp::http::Response::builder()
        .status(warp::http::StatusCode::NOT_FOUND)
        .body(Vec::new())
        .unwrap()
}

/// True when the caller presented the webHost session token. `Authorization:
/// Bearer <token>` is used by `POST /rpc`; media is served to `<img>` tags which
/// can't set headers, so those routes require `?token=<token>` instead.
fn token_matches_auth(header: &Option<String>, expected_token: &str) -> bool {
    header.as_deref() == Some(format!("Bearer {expected_token}").as_str())
}

fn token_matches_query(query: &HashMap<String, String>, expected_token: &str) -> bool {
    query.get("token").map(|s| s.as_str()) == Some(expected_token)
}

async fn serve_static(
    port: u16,
    dist: PathBuf,
    code: String,
    signal_url: String,
    config_path: String,
    share_mode: ShareMode,
    web_token: String,
) -> Result<SocketAddr, String> {
    use warp::reply::Reply as _;
    use warp::Filter;

    if !dist.join("index.html").exists() {
        return Err(format!(
            "web client bundle not found at {}.\nBuild it first: cd webclient && npm install && npm run build",
            dist.display()
        ));
    }

    let index_dist = dist.clone();
    let index = warp::get()
        .and(warp::path::end())
        .and_then(move || {
            let path = index_dist.join("index.html");
            async move {
                match std::fs::read(&path) {
                    Ok(bytes) => Ok(warp::http::Response::builder()
                        .header("content-type", "text/html; charset=utf-8")
                        .body(bytes)
                        .unwrap_or_else(|_| warp::http::Response::new("internal error".into()))),
                    Err(_) => Err(warp::reject::not_found()),
                }
            }
        })
        .boxed();

    let session_code = code.clone();
    let session_web_token = web_token.clone();
    let session = warp::path!("session")
        .map(move || {
            warp::reply::json(&serde_json::json!({
                "code": session_code,
                "webToken": session_web_token,
            }))
        })
        .boxed();

    // After matching the "assets" URL prefix the remaining path is the bare
    // filename, so the fs root must be the bundle's assets directory.
    let assets = warp::get()
        .and(warp::path("assets"))
        .and(warp::fs::dir(dist.join("assets")))
        .boxed();

    let bridge_signal = signal_url.clone();
    let ws_bridge = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let url = bridge_signal.clone();
            ws.on_upgrade(move |browser| async move {
                if let Err(e) = bridge_to_signal(browser, url).await {
                    crate::cli_warn!("[siegu] ws bridge ended: {e}");
                }
            })
        })
        .boxed();

    // ── webHost (Mode A) HTTP surface ──────────────────────────────────────
    // Owner of a mounted library reaches the same business functions + media
    // over HTTP instead of WebRTC (#26). `POST /rpc` mirrors the RPC dispatch;
    // `/thumb/{id}` and `/media/{id}` serve bytes by photo id, root-scoped.

    let rpc_config = config_path.clone();
    let rpc_mode = share_mode;
    let rpc_web_token = web_token.clone();
    let rpc = warp::path("rpc")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .then(move |auth: Option<String>, body: serde_json::Value| {
            let config = rpc_config.clone();
            let mode = rpc_mode;
            let expected_token = rpc_web_token.clone();
            async move {
                if !token_matches_auth(&auth, &expected_token) {
                    return denied_response().into_response();
                }
                let name = body
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let payload = body.get("payload").cloned().unwrap_or_default();
                let cfg = config.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let ctx = RpcContext {
                        config_path: &cfg,
                        mode,
                    };
                    dispatch(&ctx, &name, &payload)
                })
                .await;
                let reply = match result {
                    Ok(Ok(value)) => warp::reply::json(&serde_json::json!({
                        "ok": true, "result": value
                    })),
                    Ok(Err(error)) => warp::reply::json(&serde_json::json!({
                        "ok": false, "error": error
                    })),
                    Err(_) => warp::reply::json(&serde_json::json!({
                        "ok": false, "error": "rpc task panicked"
                    })),
                };
                reply.into_response()
            }
        })
        .boxed();

    let thumb_config = config_path.clone();
    let thumb_web_token = web_token.clone();
    let thumb = warp::get()
        .and(warp::path("thumb"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::query::<HashMap<String, String>>())
        .then(move |id: String, query: HashMap<String, String>| {
            let config = thumb_config.clone();
            let expected_token = thumb_web_token.clone();
            async move {
                if !token_matches_query(&query, &expected_token) {
                    return denied_response();
                }
                // Prefer the DB-cached thumbnail; else generate from the file.
                let bytes = {
                    let db = Database::new(&config);
                    let cached = db.get_photo_thumbnail_bytes(&id);
                    if let Some(b) = cached.filter(|b| !b.is_empty()) {
                        Some(b)
                    } else {
                        let path = photo_path(&config, &id);
                        let p = path.clone();
                        tokio::task::spawn_blocking(move || {
                            p.and_then(|p| p.to_str().and_then(|s| generate_thumbnail_bytes(s)))
                        })
                        .await
                        .ok()
                        .flatten()
                    }
                };
                match bytes {
                    Some(body) => warp::http::Response::builder()
                        .status(warp::http::StatusCode::OK)
                        .header("content-type", "image/jpeg")
                        .header("cache-control", "public, max-age=31536000, immutable")
                        .body(body)
                        .unwrap_or_else(|_| {
                            warp::http::Response::new(Vec::new()) // empty 200 on builder err
                        }),
                    None => {
                        let mut res = warp::http::Response::new(Vec::<u8>::new());
                        *res.status_mut() = warp::http::StatusCode::NOT_FOUND;
                        res
                    }
                }
            }
        })
        .boxed();

    let media_config = config_path.clone();
    let media_web_token = web_token.clone();
    let media = warp::get()
        .and(warp::path("media"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::query::<HashMap<String, String>>())
        .then(move |id: String, query: HashMap<String, String>| {
            let config = media_config.clone();
            let expected_token = media_web_token.clone();
            async move {
                let response_for = |status: warp::http::StatusCode, body: Vec<u8>| {
                    warp::http::Response::builder()
                        .status(status)
                        .body(body)
                        .unwrap_or_else(|_| warp::http::Response::new(Vec::new()))
                };
                if !token_matches_query(&query, &expected_token) {
                    return denied_response();
                }
                let Some(path) = photo_path(&config, &id) else {
                    return response_for(warp::http::StatusCode::NOT_FOUND, Vec::new());
                };
                let mime = mime_for(&path);
                match tokio::fs::read(&path).await {
                    Ok(body) => {
                        let mut res = response_for(warp::http::StatusCode::OK, body);
                        if let Ok(mime) = mime.parse() {
                            res.headers_mut().insert("content-type", mime);
                        }
                        res
                    }
                    Err(_) => response_for(warp::http::StatusCode::NOT_FOUND, Vec::new()),
                }
            }
        })
        .boxed();

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("failed to bind http port {port}: {e}"))?;
    let bound = listener
        .local_addr()
        .map_err(|e| format!("failed to read bound address: {e}"))?;
    tokio::spawn(async move {
        warp::serve(
            index
                .or(session)
                .or(assets)
                .or(ws_bridge)
                .or(rpc)
                .or(thumb)
                .or(media),
        )
        .incoming(listener)
        .run()
        .await;
    });
    Ok(bound)
}

pub struct WebOptions {
    pub http_port: u16,
    pub config: Option<String>,
    /// RPC permission for connected browsers (#19). ReadOnly by default;
    /// `--share-mode rw` lets guests mutate favorites/trash.
    pub share_mode: siegu_core::ShareMode,
    /// Optional hosted signalling server (`wss://…`) to pair through instead of
    /// the embedded loopback one (#27, Phase 4). Lets guests connect from any
    /// network by code + token.
    pub server: Option<String>,
}

/// `siegu web`: share this machine's library as a view-only gallery in any
/// browser (#11). Runs an embedded signalling server plus a small static file
/// server; possession of the printed URL+code grants one-off viewing access.
pub async fn run(opts: WebOptions) -> Result<(), BoxError> {
    let config_path = opts
        .config
        .map(PathBuf::from)
        .unwrap_or_else(siegu_core::config::default_config_dir);
    std::fs::create_dir_all(&config_path)?;
    let config_path = config_path.display().to_string();

    // Distinct token for the signalling plane (WebRTC room join).
    let token = uuid::Uuid::new_v4().to_string();
    // Distinct token for the webHost (Mode A) HTTP data plane: `/rpc`, `/thumb`,
    // `/media` are gated behind it so serving media now (since #26) doesn't make
    // the static plane read the library to anyone who can reach the port.
    let web_token = uuid::Uuid::new_v4().to_string();

    // Signalling: either embed our own loopback server (default) or, with
    // `--server <wss://…>`, connect to a hosted signaler shared across devices
    // so a guest can pair by code + token from anywhere (Phase 4).
    let mut signal: Option<_> = None;
    let signal_url = if let Some(server) = opts.server.clone() {
        let url = server.trim_end_matches('/').to_string();
        crate::cli_info!("Connecting to hosted signaling server: {url}");
        url
    } else {
        let s = lan_server::start_with_config(ServerConfig {
            port: 0,
            token: Some(token.clone()),
            web_dist: None,
        })
        .await;
        let port = s.port;
        crate::cli_line!("Signalling server on port {port}");
        signal = Some(s);
        format!("ws://127.0.0.1:{port}/ws?token={token}")
    };

    let code = create_room(&signal_url, &token).await?;
    crate::cli_line!("Session code: {code}");
    // Greppable handle for CLI guests/e2e drivers: they need the full
    // ws://…/ws?token=… URL to join a token-secured session.
    crate::cli_line!("Signalling token: {token}");

    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "siegu-host".to_string());

    let sync_tx = Arc::new(tokio::sync::Mutex::new(None));
    let event = Arc::new(CliSyncEvent {
        config_path: config_path.clone(),
        sync_tx: Arc::clone(&sync_tx),
        ready: Arc::new(tokio::sync::Notify::new()),
        view_manifest: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        view_notify: Arc::new(tokio::sync::Notify::new()),
        rpc_slot: Arc::new(tokio::sync::Mutex::new(None)),
        rpc_notify: Arc::new(tokio::sync::Notify::new()),
    });

    let transport = MeshTransport::new(
        code.clone(),
        true,
        signal_url.clone(),
        config_path.clone(),
        uuid::Uuid::new_v4().to_string(),
        hostname,
        Vec::new(),
        event,
    )
    .with_share_mode(opts.share_mode);
    let transport_handle = tokio::spawn(async move {
        if let Err(e) = transport.start().await {
            crate::cli_warn!("[siegu] transport stopped: {e}");
        }
    });

    // CLI/e2e guests speak WebRTC only; SIEGU_WEB_NO_HTTP skips the static
    // bundle requirement so a session can be hosted without building the
    // browser client.
    let http_addr = match std::env::var_os("SIEGU_WEB_NO_HTTP") {
        Some(_) => {
            crate::cli_info!("SIEGU_WEB_NO_HTTP is set - skipping static file server");
            None
        }
        None => Some(
            serve_static(
                opts.http_port,
                web_dist_dir(),
                code.clone(),
                signal_url.clone(),
                config_path.clone(),
                opts.share_mode,
                web_token.clone(),
            )
            .await?,
        ),
    };
    if let Some(http_addr) = http_addr {
        println!(
            "\nOpen in a browser on this machine:\n  http://127.0.0.1:{}/#{}.{}",
            http_addr.port(),
            code,
            token
        );
        if let Some(ip) = lan_ip() {
            println!(
                "\nOr from another device on this network:\n  http://{ip}:{}/#{}.{}",
                http_addr.port(),
                code,
                token
            );
        }
    }
    crate::cli_info!("The link expires when this command stops. Press Ctrl+C to end the session.");

    let _ = tokio::signal::ctrl_c().await;
    crate::cli_info!("Shutting down...");
    transport_handle.abort();
    if let Some(s) = signal {
        s.stop();
    }
    Ok(())
}

/// Resolve the web client bundle location: `$SIEGU_WEB_DIST`, `./webclient/dist`,
/// or the workspace-relative path when running inside the repo.
fn web_dist_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SIEGU_WEB_DIST") {
        return PathBuf::from(dir);
    }
    let local = PathBuf::from("webclient/dist");
    if local.join("index.html").exists() {
        return local;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../webclient/dist")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_for_common_extensions() {
        assert_eq!(mime_for(Path::new("pic.JPG")), "image/jpeg");
        assert_eq!(mime_for(Path::new("x.png")), "image/png");
        assert_eq!(mime_for(Path::new("v.mp4")), "video/mp4");
        assert_eq!(mime_for(Path::new("a.heic")), "image/heic");
        assert_eq!(mime_for(Path::new("noext")), "application/octet-stream");
    }

    #[test]
    fn path_within_roots_allows_inside_and_rejects_outside() {
        let base = std::env::temp_dir().join(format!("siegu-web-test-{}", std::process::id()));
        let root = base.join("photos");
        let outside = base.join("other");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let in_file = root.join("a.jpg");
        std::fs::write(&in_file, b"x").unwrap();
        let out_file = outside.join("b.jpg");
        std::fs::write(&out_file, b"y").unwrap();

        let roots = vec![root.clone()];
        assert!(path_within_roots(&roots, &in_file));
        // A file outside the allowed root must be rejected (security).
        assert!(!path_within_roots(&roots, &out_file));
        // A non-existent path must be rejected.
        assert!(!path_within_roots(&roots, &root.join("missing.jpg")));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn path_within_roots_rejects_path_traversal_like_prefix() {
        let base = std::env::temp_dir().join(format!("siegu-web-prefix-{}", std::process::id()));
        let root = base.join("a");
        let sibling = base.join("ab");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        // `ab/x` must NOT be considered inside root `a` (component-wise compare).
        let trap = sibling.join("x.jpg");
        std::fs::write(&trap, b"z").unwrap();
        assert!(!path_within_roots(&[root.clone()], &trap));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn bearer_matches_only_exact_token() {
        let none = None;
        assert!(!token_matches_auth(&none, "tk"));
        assert!(!token_matches_auth(&Some("tk".into()), "tk")); // wrong scheme
        assert!(!token_matches_auth(&Some("Bearer wrong".into()), "tk"));
        assert!(token_matches_auth(&Some("Bearer tk".into()), "tk"));
    }

    #[test]
    fn query_token_matches_only_exact_token() {
        let empty = HashMap::new();
        assert!(!token_matches_query(&empty, "tk"));
        let bad = HashMap::from([("token".to_string(), "nope".to_string())]);
        assert!(!token_matches_query(&bad, "tk"));
        let good = HashMap::from([("token".to_string(), "tk".to_string())]);
        assert!(token_matches_query(&good, "tk"));
    }
}
