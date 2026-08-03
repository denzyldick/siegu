use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::database;
use siegu_core::mesh_transport::MeshTransport;
use warp::Filter;

pub use siegu_core::mesh::SyncMessage;

pub fn get_or_create_device_id(config_path: &str) -> String {
    use crate::database;
    let db = database::Database::new(config_path);
    let state = db.get_state();
    if let Some(id) = state.get("device_id") {
        return id.clone();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let mut new_state = std::collections::HashMap::new();
    new_state.insert("device_id".to_string(), id.clone());
    db.set_state(new_state);
    id
}

pub struct MediaServerState {
    pub port: u16,
}

/// Cache of the monitored photo directories, refreshed on a short TTL so newly
/// added folders become servable without re-opening the database per request.
type DirCache = Arc<std::sync::Mutex<(std::time::Instant, Vec<String>)>>;

fn load_allowed_roots(config_path: &str, cache: &DirCache) -> Vec<PathBuf> {
    const MAX_AGE: std::time::Duration = std::time::Duration::from_secs(5);
    let mut guard = cache.lock().unwrap();
    if guard.0.elapsed() > MAX_AGE {
        guard.1 = database::Database::new(config_path).list_directories();
        guard.0 = std::time::Instant::now();
    }
    guard.1.iter().map(PathBuf::from).collect()
}

/// The requested file must resolve (after symlink resolution) to a regular file
/// inside one of the monitored photo directories. `Path::starts_with` compares
/// component-wise, so a sibling like `/a/bc` cannot pass the `/a/b` check.
fn path_within_roots(roots: &[PathBuf], candidate: &Path) -> bool {
    let canon = match std::fs::canonicalize(candidate) {
        Ok(c) => c,
        Err(_) => return false,
    };
    if !canon.is_file() {
        return false;
    }
    roots.iter().any(|root| {
        std::fs::canonicalize(root)
            .map(|root_canon| canon.starts_with(root_canon))
            .unwrap_or(false)
    })
}

fn parse_single_range(range: &str, len: u64) -> Option<(u64, u64)> {
    let spec = range.strip_prefix("bytes=")?;
    let (a, b) = spec.split_once('-')?;
    let a = a.trim().parse::<u64>().ok();
    let b = b.trim().parse::<u64>().ok();
    if a.is_none() && b.is_none() {
        return None;
    }
    let (start, end) = match (a, b) {
        (Some(s), Some(e)) => (s, e.min(len.saturating_sub(1))),
        (Some(s), None) => (s, len.saturating_sub(1)),
        (None, Some(suffix)) => (len.saturating_sub(suffix), len.saturating_sub(1)),
        (None, None) => return None,
    };
    if start > end || start >= len {
        return None;
    }
    Some((start, end))
}

/// Serve one already-validated media file with optional single-range support so
/// the webview can seek in videos.
async fn serve_media_file(
    path: &Path,
    range: Option<String>,
    is_head: bool,
) -> Result<impl warp::Reply, warp::Rejection> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let meta = match tokio::fs::metadata(path).await {
        Ok(m) if m.is_file() => m,
        _ => return Err(warp::reject::not_found()),
    };
    let len = meta.len();
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut builder = warp::http::Response::builder()
        .header("Content-Type", mime.as_ref())
        .header("Accept-Ranges", "bytes");

    let bytes = if let Some(range) = range {
        match parse_single_range(&range, len) {
            Some((start, end)) => {
                let mut file = tokio::fs::File::open(path)
                    .await
                    .map_err(|_| warp::reject::not_found())?;
                file.seek(std::io::SeekFrom::Start(start))
                    .await
                    .map_err(|_| warp::reject::not_found())?;
                let count = (end - start + 1) as usize;
                let mut buf = Vec::with_capacity(count);
                file.take(end - start + 1)
                    .read_to_end(&mut buf)
                    .await
                    .map_err(|_| warp::reject::not_found())?;
                builder = builder
                    .status(warp::http::StatusCode::PARTIAL_CONTENT)
                    .header("Content-Range", format!("bytes {start}-{end}/{len}"))
                    .header("Content-Length", buf.len().to_string());
                Some(bytes::Bytes::from(buf))
            }
            None => {
                builder = builder
                    .status(warp::http::StatusCode::RANGE_NOT_SATISFIABLE)
                    .header("Content-Range", format!("bytes */{len}"))
                    .header("Content-Length", "0");
                Some(bytes::Bytes::new())
            }
        }
    } else {
        let data = tokio::fs::read(path)
            .await
            .map_err(|_| warp::reject::not_found())?;
        builder = builder.header("Content-Length", data.len().to_string());
        Some(bytes::Bytes::from(data))
    };

    let body = if is_head {
        bytes::Bytes::new()
    } else {
        bytes.unwrap_or_default()
    };
    builder.body(body).map_err(|_| warp::reject::not_found())
}

pub fn start_media_server(config_path: String) -> u16 {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let cache: DirCache = Arc::new(std::sync::Mutex::new((
                std::time::Instant::now(),
                Vec::new(),
            )));

            let media = warp::get()
                .or(warp::head())
                .unify()
                .and(warp::path("media"))
                .and(warp::path::tail())
                .and(warp::header::optional::<String>("range"))
                .and(warp::method())
                .and_then(
                    move |tail: warp::path::Tail, range: Option<String>, method| {
                        let cache = Arc::clone(&cache);
                        let config = config_path.clone();
                        async move {
                            // The frontend encodes the absolute file path as
                            // `/media/<percent-encoded path>`; anything outside the
                            // monitored photo directories is rejected.
                            let decoded = percent_encoding::percent_decode_str(tail.as_str())
                                .decode_utf8_lossy()
                                .into_owned();
                            let roots = load_allowed_roots(&config, &cache);
                            let path = PathBuf::from(decoded);
                            if !path_within_roots(&roots, &path) {
                                return Err(warp::reject::not_found());
                            }
                            let is_head = method == warp::http::Method::HEAD;
                            serve_media_file(&path, range, is_head).await
                        }
                    },
                );

            let addr: std::net::SocketAddr = ([127, 0, 0, 1], 0).into();
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            let addr = listener.local_addr().unwrap();
            let port = addr.port();
            let _ = tx.send(port);
            warp::serve(media).incoming(listener).run().await;
        });
    });

    rx.blocking_recv().unwrap_or(0)
}

pub fn create_transport(
    room_id: String,
    is_initiator: bool,
    signaling_url: String,
    config_path: String,
    app: tauri::AppHandle,
    external_tx: Option<
        Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<SyncMessage>>>>,
    >,
    connected: Option<Arc<std::sync::atomic::AtomicBool>>,
) -> MeshTransport {
    let device_id = get_or_create_device_id(&config_path);
    let db = database::Database::new(&config_path);
    let state = db.get_state();
    let device_name = state
        .get("device_name")
        .cloned()
        .or_else(|| {
            std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("COMPUTERNAME"))
                .ok()
        })
        .unwrap_or_else(|| "siegu-device".to_string());
    let models_enabled = Vec::new();

    let sync_tx = external_tx
        .clone()
        .unwrap_or_else(|| Arc::new(tokio::sync::Mutex::new(None)));
    let event = Arc::new(super::tauri_sync_event::TauriSyncEvent {
        app: app.clone(),
        config_path: config_path.clone(),
        sync_tx,
        offline_notified: std::sync::atomic::AtomicBool::new(false),
        connected: connected.unwrap_or_default(),
        active_peer: Arc::new(tokio::sync::Mutex::new(None)),
    });

    let mut transport = MeshTransport::new(
        room_id,
        is_initiator,
        signaling_url,
        config_path,
        device_id,
        device_name,
        models_enabled,
        event,
    );

    if let Some(ext) = external_tx {
        transport = transport.with_external_tx(ext);
    }

    transport
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn path_within_roots_accepts_file_inside_photo_dir() {
        let root = tempfile::tempdir().unwrap();
        let photo_dir = root.path().join("photos");
        fs::create_dir(&photo_dir).unwrap();
        let file = photo_dir.join("IMG_0001.jpg");
        fs::write(&file, b"jpeg").unwrap();

        let roots = vec![photo_dir.clone()];
        assert!(path_within_roots(&roots, &file));
    }

    #[test]
    fn path_within_roots_rejects_outside_file() {
        let root = tempfile::tempdir().unwrap();
        let photo_dir = root.path().join("photos");
        let other_dir = root.path().join("other");
        fs::create_dir(&photo_dir).unwrap();
        fs::create_dir(&other_dir).unwrap();
        let outside = other_dir.join("shadow.txt");
        fs::write(&outside, b"x").unwrap();

        let roots = vec![photo_dir];
        assert!(!path_within_roots(&roots, &outside));
    }

    #[test]
    fn path_within_roots_rejects_sibling_prefix_collision() {
        let root = tempfile::tempdir().unwrap();
        let photo_dir = root.path().join("photo");
        let sibling = root.path().join("photo2");
        fs::create_dir(&photo_dir).unwrap();
        fs::create_dir(&sibling).unwrap();
        let file = sibling.join("a.jpg");
        fs::write(&file, b"jpeg").unwrap();

        let roots = vec![photo_dir];
        assert!(!path_within_roots(&roots, &file));
    }

    #[cfg(unix)]
    #[test]
    fn path_within_roots_rejects_symlink_escape() {
        let root = tempfile::tempdir().unwrap();
        let photo_dir = root.path().join("photos");
        fs::create_dir(&photo_dir).unwrap();
        let secret = root.path().join("secret.txt");
        fs::write(&secret, b"secret").unwrap();
        let link = photo_dir.join("link.jpg");
        std::os::unix::fs::symlink(&secret, &link).unwrap();

        let roots = vec![photo_dir];
        assert!(!path_within_roots(&roots, &link));
    }

    #[test]
    fn parse_single_range_handles_open_ended() {
        assert_eq!(parse_single_range("bytes=10-", 100), Some((10, 99)));
    }

    #[test]
    fn parse_single_range_handles_bounded_and_suffix() {
        assert_eq!(parse_single_range("bytes=10-20", 100), Some((10, 20)));
        assert_eq!(parse_single_range("bytes=-10", 100), Some((90, 99)));
    }

    #[test]
    fn parse_single_range_clamps_and_rejects_invalid() {
        assert_eq!(parse_single_range("bytes=90-", 100), Some((90, 99)));
        assert_eq!(parse_single_range("bytes=50-10", 100), None);
        assert_eq!(parse_single_range("bytes=200-", 100), None);
        assert_eq!(parse_single_range("bananas", 100), None);
    }

    #[test]
    fn serve_media_file_writes_expected_bytes() {
        // Exercise the full handler through a real loopback listener.
        let root = tempfile::tempdir().unwrap();
        let photo_dir = root.path().join("photos");
        fs::create_dir(&photo_dir).unwrap();
        let file = photo_dir.join("a.jpg");
        {
            let mut f = fs::File::create(&file).unwrap();
            let mut data = Vec::new();
            for i in 0..=255u8 {
                data.push(i);
            }
            f.write_all(&data).unwrap();
        }

        let cache: DirCache = Arc::new(std::sync::Mutex::new((
            std::time::Instant::now(),
            vec![photo_dir.display().to_string()],
        )));

        let rt = tokio::runtime::Runtime::new().unwrap();
        let port = rt.block_on(async {
            let media = warp::get()
                .or(warp::head())
                .unify()
                .and(warp::path("media"))
                .and(warp::path::tail())
                .and(warp::header::optional::<String>("range"))
                .and(warp::method())
                .and_then(
                    move |tail: warp::path::Tail, range: Option<String>, method| {
                        let cache = Arc::clone(&cache);
                        async move {
                            let decoded = percent_encoding::percent_decode_str(tail.as_str())
                                .decode_utf8_lossy()
                                .into_owned();
                            let roots: Vec<PathBuf> =
                                cache.lock().unwrap().1.iter().map(PathBuf::from).collect();
                            let path = PathBuf::from(decoded);
                            if !path_within_roots(&roots, &path) {
                                return Err(warp::reject::not_found());
                            }
                            let is_head = method == warp::http::Method::HEAD;
                            serve_media_file(&path, range, is_head).await
                        }
                    },
                );

            let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                .await
                .unwrap();
            let addr = listener.local_addr().unwrap();
            tokio::spawn(async move { warp::serve(media.boxed()).incoming(listener).run().await });
            addr.port()
        });

        rt.block_on(async {
            use percent_encoding::utf8_percent_encode;
            use percent_encoding::NON_ALPHANUMERIC;

            let encoded = utf8_percent_encode(file.to_str().unwrap(), NON_ALPHANUMERIC).to_string();
            let url = format!("http://127.0.0.1:{port}/media/{encoded}");
            let resp = reqwest::get(&url).await.unwrap();
            assert_eq!(resp.status(), reqwest::StatusCode::OK);
            let body = resp.bytes().await.unwrap();
            assert_eq!(body.len(), 256);
            assert_eq!(body[0], 0);
            assert_eq!(body[255], 255);

            let outside = format!("http://127.0.0.1:{port}/media/etc%2Fpasswd");
            let resp = reqwest::get(&outside).await.unwrap();
            assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);
        });
    }
}
