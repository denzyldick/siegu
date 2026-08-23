//! In-memory state for view-only sessions (#9).
//!
//! A viewer connects to a peer to *browse* its library without writing
//! anything to disk or database. Media arrives over the existing data-channel
//! file protocol but is buffered here instead of being persisted, and the UI
//! fetches it from this cache through the local media server.
//!
//! The registry is process-global because exactly one mesh session can be
//! active at a time; it is bound when the local side enters view-only mode
//! and reset on disconnect/stop.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};

use crate::mesh::SyncMessage;

/// Completed remote media entries cached for the current view-only session.
#[derive(Clone)]
pub struct RemoteMedia {
    pub bytes: Arc<Vec<u8>>,
    pub mime: String,
}

struct PartialMedia {
    checksum: String,
    filename: String,
    buffer: Vec<u8>,
}

/// Bound for the media cache so an open grid cannot grow memory forever.
/// FIFO eviction: oldest fetched items fall out first; thumbnails are small
/// so they dominate the steady-state hit rate anyway.
const MAX_CACHED_ITEMS: usize = 48;

pub struct ViewOnlyState {
    /// Sender into the active session's outbound message queue, used by the
    /// media-server handler to request missing media on demand.
    sender: Mutex<Option<tokio::sync::mpsc::UnboundedSender<SyncMessage>>>,
    completed: Mutex<HashMap<String, RemoteMedia>>,
    order: Mutex<VecDeque<String>>,
    partial: Mutex<HashMap<String, PartialMedia>>,
    notify: tokio::sync::Notify,
    /// True while THIS side is only viewing the peer's library: every
    /// persistence-touching handler must be skipped.
    pub viewing: AtomicBool,
    /// True while the PEER is viewing OUR library via this connection: we
    /// serve manifest/media but ignore any message that could mutate us.
    pub serving_view_only: AtomicBool,
}

static STATE: OnceLock<ViewOnlyState> = OnceLock::new();

pub fn state() -> &'static ViewOnlyState {
    STATE.get_or_init(|| ViewOnlyState {
        sender: Mutex::new(None),
        completed: Mutex::new(HashMap::new()),
        order: Mutex::new(VecDeque::new()),
        partial: Mutex::new(HashMap::new()),
        notify: tokio::sync::Notify::new(),
        viewing: AtomicBool::new(false),
        serving_view_only: AtomicBool::new(false),
    })
}

impl ViewOnlyState {
    /// Bind the outbound queue used to issue FetchMediaRequest messages.
    pub fn bind_session(&self, tx: tokio::sync::mpsc::UnboundedSender<SyncMessage>) {
        *self.sender.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    }

    pub fn is_bound(&self) -> bool {
        self.sender
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Request `id` from the peer over the data channel (fire and forget;
    /// completion is observed via [`Self::wait_for`]).
    pub fn request_media(&self, id: &str, thumbnail: bool) -> bool {
        let tx = self
            .sender
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        match tx {
            Some(tx) => tx
                .send(SyncMessage::FetchMediaRequest {
                    id: id.to_string(),
                    thumbnail,
                    restore: false,
                })
                .is_ok(),
            None => false,
        }
    }

    pub fn insert_partial(&self, id: &str, checksum: String, filename: String) {
        self.partial
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id.to_string(),
                PartialMedia {
                    checksum,
                    filename,
                    buffer: Vec::new(),
                },
            );
    }

    pub fn append_chunk(&self, id: &str, data: &[u8]) {
        if let Some(partial) = self
            .partial
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(id)
        {
            partial.buffer.extend_from_slice(data);
        }
    }

    /// Finish a transfer: verify the checksum, move bytes into the completed
    /// cache and wake any waiters. Returns false on checksum mismatch or an
    /// unknown transfer.
    pub fn complete(&self, id: &str, checksum_fn: impl Fn(&[u8]) -> String) -> bool {
        let partial = self
            .partial
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        let Some(partial) = partial else { return false };
        if checksum_fn(&partial.buffer) != partial.checksum {
            return false;
        }
        self.insert_completed(
            id.to_string(),
            RemoteMedia {
                bytes: Arc::new(partial.buffer),
                mime: mime_for(&partial.filename),
            },
        );
        true
    }

    /// Insert directly-served bytes (e.g. sharer-generated thumbnails).
    pub fn insert_completed(&self, key: String, media: RemoteMedia) {
        {
            let mut completed = self.completed.lock().unwrap_or_else(|e| e.into_inner());
            let mut order = self.order.lock().unwrap_or_else(|e| e.into_inner());
            while order.len() >= MAX_CACHED_ITEMS {
                if let Some(evict) = order.pop_front() {
                    completed.remove(&evict);
                }
            }
            if !order.contains(&key) {
                order.push_back(key.clone());
            }
            completed.insert(key, media);
        }
        self.notify.notify_waiters();
    }

    pub fn get(&self, key: &str) -> Option<RemoteMedia> {
        self.completed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .cloned()
    }

    /// Wait until `key` shows up in the completed cache (woken by
    /// [`Self::insert_completed`]) or the timeout elapses.
    pub async fn wait_for(&self, key: &str, timeout: std::time::Duration) -> Option<RemoteMedia> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(media) = self.get(key) {
                return Some(media);
            }
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return None;
            }
            let _ = tokio::time::timeout_at(deadline, self.notify.notified()).await;
        }
    }

    /// Drop all per-session state (buffers, cache, sender, flags).
    pub fn reset_session(&self) {
        self.sender.lock().unwrap_or_else(|e| e.into_inner()).take();
        self.completed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.order.lock().unwrap_or_else(|e| e.into_inner()).clear();
        self.partial
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.viewing
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.serving_view_only
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

/// Best-effort MIME type from a filename extension; defaults to JPEG since
/// nearly everything flowing through here is a photo or video poster frame.
pub fn mime_for(filename: &str) -> String {
    let lower = filename.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "heic" | "heif" => "image/heic",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "3gp" => "video/3gpp",
        _ => "image/jpeg",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_verifies_checksum_and_caches() {
        let state = ViewOnlyState {
            sender: Mutex::new(None),
            completed: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
            partial: Mutex::new(HashMap::new()),
            notify: tokio::sync::Notify::new(),
            viewing: AtomicBool::new(false),
            serving_view_only: AtomicBool::new(false),
        };
        state.insert_partial("p1", "bad".into(), "a.jpg".into());
        state.append_chunk("p1", b"hello".as_slice());
        assert!(!state.complete("p1", |_| "mismatch".to_string()));

        state.insert_partial("p2", md5_like(b"hello").to_string(), "b.png".into());
        state.append_chunk("p2", b"hello".as_slice());
        assert!(state.complete("p2", |bytes| md5_like(bytes).to_string()));
        let media = state.get("p2").expect("cached");
        assert_eq!(media.mime, "image/png");
        assert_eq!(*media.bytes, b"hello".to_vec());
    }

    fn md5_like(bytes: &[u8]) -> u32 {
        bytes
            .iter()
            .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(*b as u32))
    }

    #[test]
    fn cache_evicts_oldest_beyond_cap() {
        let state = ViewOnlyState {
            sender: Mutex::new(None),
            completed: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
            partial: Mutex::new(HashMap::new()),
            notify: tokio::sync::Notify::new(),
            viewing: AtomicBool::new(false),
            serving_view_only: AtomicBool::new(false),
        };
        for i in 0..(MAX_CACHED_ITEMS + 4) {
            state.insert_completed(
                format!("k{i}"),
                RemoteMedia {
                    bytes: Arc::new(vec![i as u8]),
                    mime: "image/jpeg".into(),
                },
            );
        }
        assert!(state.get("k0").is_none(), "oldest evicted");
        assert!(state.get(&format!("k{}", MAX_CACHED_ITEMS + 3)).is_some());
    }

    #[test]
    fn mime_detection_defaults_to_jpeg() {
        assert_eq!(mime_for("IMG.mp4"), "video/mp4");
        assert_eq!(mime_for("photo.PNG"), "image/png");
        assert_eq!(mime_for("noext"), "image/jpeg");
    }
}
