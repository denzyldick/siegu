use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use siegu_core::lan_server::{self, ServerConfig};
use siegu_core::mesh_transport::MeshTransport;
use siegu_core::signal::SignalMessage;
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

async fn serve_static(
    port: u16,
    dist: PathBuf,
    code: String,
    signal_url: String,
) -> Result<SocketAddr, String> {
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
    let session = warp::path!("session")
        .map(move || warp::reply::json(&serde_json::json!({ "code": session_code })))
        .boxed();

    let assets = warp::get()
        .and(warp::path("assets"))
        .and(warp::fs::dir(dist))
        .boxed();

    let bridge_signal = signal_url.clone();
    let ws_bridge = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let url = bridge_signal.clone();
            ws.on_upgrade(move |browser| async move {
                if let Err(e) = bridge_to_signal(browser, url).await {
                    eprintln!("[siegu] ws bridge ended: {e}");
                }
            })
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
        warp::serve(index.or(session).or(assets).or(ws_bridge))
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

    // Distinct token for the signalling plane; the static plane serves no data.
    let token = uuid::Uuid::new_v4().to_string();
    let signal = lan_server::start_with_config(ServerConfig {
        port: 0,
        token: Some(token.clone()),
    })
    .await;
    println!("Signalling server on port {}", signal.port);

    let signal_url = format!("ws://127.0.0.1:{}/ws?token={}", signal.port, token);
    let code = create_room(&signal_url, &token).await?;
    println!("Session code: {code}");
    // Greppable handle for CLI guests/e2e drivers: they need the full
    // ws://…/ws?token=… URL to join a token-secured session.
    println!("Signalling token: {token}");

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
            eprintln!("[siegu] transport stopped: {e}");
        }
    });

    // CLI/e2e guests speak WebRTC only; SIEGU_WEB_NO_HTTP skips the static
    // bundle requirement so a session can be hosted without building the
    // browser client.
    let http_addr = match std::env::var_os("SIEGU_WEB_NO_HTTP") {
        Some(_) => {
            println!("SIEGU_WEB_NO_HTTP is set - skipping static file server");
            None
        }
        None => Some(
            serve_static(
                opts.http_port,
                web_dist_dir(),
                code.clone(),
                signal_url.clone(),
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
    println!("\nThe link expires when this command stops. Press Ctrl+C to end the session.");

    let _ = tokio::signal::ctrl_c().await;
    println!("\nShutting down...");
    transport_handle.abort();
    signal.stop();
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
