use std::collections::HashMap;
use std::fs;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Autocomplete suggestion returned by search queries.
#[derive(Debug, Clone, Serialize)]
pub struct SearchSuggestion {
    pub title: String,
    #[serde(rename = "type")]
    pub suggestion_type: String,
}

/// A named person shown in the search dropdown, with the number of photos
/// that contain them and a representative face thumbnail.
#[derive(Debug, Clone, Serialize)]
pub struct SearchPerson {
    pub id: String,
    pub name: String,
    pub representative_crop: Option<String>,
    pub encoded: Option<String>,
    pub photo_count: i64,
}

/// Library-wide counts shown in the search dropdown footer.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchStats {
    pub photos: i64,
    pub videos: i64,
    pub favorites: i64,
    pub ocr_photos: i64,
    pub faces: i64,
    pub named_people: i64,
    /// Photos containing at least one detected face.
    pub face_photos: i64,
    /// Photos classified as NSFW (nsfw score >= 0.8).
    pub nsfw_photos: i64,
}

/// A lightweight photo used by the discovery rails (best shots, favorites,
/// recent). Carries just enough to render a thumbnail and a badge.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchPhotoTile {
    pub id: String,
    pub location: String,
    pub encoded: String,
    pub created: String,
    pub aesthetics_score: Option<f64>,
    pub favorite: bool,
}

/// A resolved location with a representative photo thumbnail so the discovery
/// rail can show a real preview instead of a bare label.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LocationGroup {
    pub name: String,
    pub count: i64,
    pub photo_location: Option<String>,
    pub encoded: Option<String>,
}

/// Photo/video counts for a single calendar day (`YYYY-MM-DD`), powering the
/// date-range picker in the search dropdown.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DayCount {
    pub date: String,
    pub photos: i64,
    pub videos: i64,
}

/// CLIP zero-shot classes that correspond to documents, receipts and
/// screenshots, powering the "Papers & screenshots" section and filter.
pub const PAPER_CLASSES: &[&str] = &[
    "a passport",
    "a driver's license",
    "an id card",
    "a document",
    "a receipt",
    "a screenshot",
    "a meme",
    "a text message",
];

/// Builds a SQL-safe `IN (...)` clause from the paper class list, escaping
/// embedded quotes so labels like "a driver's license" stay valid literals.
fn paper_class_in_clause() -> String {
    PAPER_CLASSES
        .iter()
        .map(|c| format!("'{}'", c.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",")
}

/// Optional facet filters combined with AND against the media list.
#[derive(Debug, Clone, Default)]
pub struct PhotoFilter {
    pub person_id: Option<String>,
    pub location: Option<String>,
    pub tag: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    /// Only photos marked as favorites.
    pub favorite: bool,
    /// Only photos containing at least one detected face.
    pub has_faces: bool,
    /// Only photos whose aesthetics score is at least this value.
    pub aesthetics_min: Option<f64>,
    /// Only photos shot with a camera whose Make or Model contains this value.
    pub camera: Option<String>,
    /// Only photos whose object classes are documents/screenshots (CLIP).
    pub papers: bool,
    /// Only show photos classified as NSFW (nsfw score >= 0.8).
    pub nsfw_only: bool,
    /// Random order instead of newest-first (used by "Surprise me").
    pub random: bool,
    /// Sort order: "newest" (default), "oldest", "best" (aesthetics desc), "random".
    pub order_by: Option<String>,
    /// Only photos that belong to this album.
    pub album_id: Option<String>,
}

/// A user-created collection of photos. Albums are local and free for
/// everyone; sharing them is a separate (paid) feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub cover_photo_id: Option<String>,
    pub sort_order: i64,
    /// Number of photos currently in the album.
    pub item_count: i64,
}

/// Persisted sync session for auto-reconnect on app restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSession {
    pub room_id: String,
    pub signaling_url: String,
    pub port: u16,
    pub is_initiator: bool,
    pub passphrase: String,
}

impl SavedSession {
    /// Encrypt the passphrase using AES-256-GCM with a device-derived key.
    /// Returns a base64-encoded string of nonce + ciphertext.
    pub fn encrypt(&self) -> String {
        use aes_gcm::{
            aead::{Aead, KeyInit, OsRng},
            Aes256Gcm, Nonce,
        };
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        use rand::RngCore;

        let key = Self::derive_key();
        let cipher = Aes256Gcm::new(&key.into());

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = self.passphrase.as_bytes();
        match cipher.encrypt(nonce, plaintext) {
            Ok(ciphertext) => {
                let mut combined = nonce_bytes.to_vec();
                combined.extend_from_slice(&ciphertext);
                B64.encode(combined)
            }
            Err(_) => String::new(),
        }
    }

    /// Decrypt a base64-encoded nonce + ciphertext string back to the passphrase.
    pub fn decrypt(encrypted: &str) -> Result<String, Box<dyn std::error::Error>> {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };
        use base64::{engine::general_purpose::STANDARD as B64, Engine};

        let key = Self::derive_key();
        let cipher = Aes256Gcm::new(&key.into());

        let combined = B64.decode(encrypted)?;
        if combined.len() < 12 {
            return Err("Invalid encrypted data".into());
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = cipher.decrypt(nonce, ciphertext)?;
        Ok(String::from_utf8(plaintext)?)
    }

    /// Derive a 256-bit key from a machine-specific seed.
    /// Uses the hostname + a hardcoded salt for simplicity.
    fn derive_key() -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let hostname = sysinfo::System::name().unwrap_or_else(|| "siegu-default".to_string());
        let mut hasher = Sha256::new();
        hasher.update(format!("siegu-session-key-v1-{hostname}"));
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        key
    }
}

/// SQLite-backed photo, face, and sync metadata store.
pub struct Database {
    pub connection: Connection,
}

use crate::scanner;

/// Build a SQL WHERE clause fragment matching all known video file extensions.
fn video_sql_like() -> String {
    let parts: Vec<String> = scanner::VIDEO_EXTENSIONS
        .iter()
        .map(|ext| format!("location LIKE '%.{ext}'"))
        .collect();
    format!("({})", parts.join(" OR "))
}

/// Build a SQL WHERE clause fragment excluding all known video file extensions.
fn video_sql_not_like() -> String {
    format!(
        "NOT ({})",
        video_sql_like()
            .strip_prefix('(')
            .unwrap_or("")
            .strip_suffix(')')
            .unwrap_or("")
    )
}

/// Whether a file path is one of the known video extensions (case-insensitive).
pub fn is_video_path(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    scanner::VIDEO_EXTENSIONS.contains(&ext.as_str())
}

const MONTH_NAMES: &[(u8, &str, &str)] = &[
    (1, "january", "jan"),
    (2, "february", "feb"),
    (3, "march", "mar"),
    (4, "april", "apr"),
    (5, "may", "may"),
    (6, "june", "jun"),
    (7, "july", "jul"),
    (8, "august", "aug"),
    (9, "september", "sep"),
    (10, "october", "oct"),
    (11, "november", "nov"),
    (12, "december", "dec"),
];

/// Convert a month name (e.g. "january", "mar") to a SQL LIKE pattern for date matching.
fn month_name_to_like(query: &str) -> Option<String> {
    let q_lower = query.to_lowercase();
    for &(num, full, abbr) in MONTH_NAMES {
        if q_lower.contains(full) || q_lower.contains(abbr) {
            return Some(format!("%-{:02}-%", num));
        }
    }
    None
}
/// Photo data for syncing between devices.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PhotoSyncInfo {
    pub id: String,
    pub location: String,
    pub created: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub objects: String, // JSON array of {class, probability}
    pub faces: String,   // JSON array of {face_id, crop_path, encoded, person_id}
    pub caption: Option<String>,
    pub aesthetics_score: Option<f64>,
}

/// A detected object class with its confidence score, used in sync payloads.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SyncObject {
    pub class: String,
    pub probability: String,
}

/// Face crop data for syncing between devices.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SyncFace {
    pub face_id: String,
    pub crop_path: String,
    pub encoded: String,
    pub person_id: Option<String>,
}

/// Borrowed photo data for importing into the database within a transaction.
pub struct ImportedPhoto<'a> {
    pub id: &'a str,
    pub location: &'a str,
    pub created: &'a str,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub objects_json: &'a str,
    pub faces_json: &'a str,
    pub encoded: &'a str,
    pub caption: Option<&'a str>,
    pub aesthetics_score: Option<f64>,
    /// True when the photo was imported from a peer over sync (a backup copy).
    /// Originals scanned from a library folder set this to false.
    pub received: bool,
}

/// A detected face with its associated person name (if any).
#[derive(Debug, Clone, Serialize)]
pub struct FaceWithPerson {
    pub photo_id: String,
    pub face_id: String,
    pub crop_path: String,
    pub encoded: String,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
}

impl Database {
    /// Get all photos (with their detected objects and faces) for device sync.
    /// Includes every scanned photo so sync works immediately after a scan, plus
    /// photos received over sync (stored under a 'siegu' folder). The manifest is
    /// "what this device has", so it must include received files: otherwise a
    /// sync-only device reports an empty library and re-requests everything it
    /// already received on every reconnect.
    pub fn get_photo_sync_info(&self) -> Vec<PhotoSyncInfo> {
        let mut results = Vec::new();
        let sql = "SELECT id, location, created, latitude, longitude, caption, aesthetics_score FROM photo p";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            let iter = stmt.query_map([], |row| {
                let id: String = row.get(0)?;

                // Fetch objects for this photo
                let mut objects = Vec::new();
                if let Ok(mut obj_stmt) = self
                    .connection
                    .prepare("SELECT class, probability FROM object WHERE photo_id = ?1")
                {
                    if let Ok(obj_rows) = obj_stmt.query_map([&id], |r| {
                        Ok(SyncObject {
                            class: r.get(0)?,
                            probability: r.get(1)?,
                        })
                    }) {
                        for obj in obj_rows.flatten() {
                            objects.push(obj);
                        }
                    }
                }

                // Fetch faces for this photo
                let mut faces = Vec::new();
                if let Ok(mut face_stmt) = self.connection.prepare(
                    "SELECT face_id, crop_path, encoded, person_id FROM faces WHERE photo_id = ?1",
                ) {
                    if let Ok(face_rows) = face_stmt.query_map([&id], |r| {
                        Ok(SyncFace {
                            face_id: r.get(0)?,
                            crop_path: r.get(1)?,
                            encoded: r.get(2)?,
                            person_id: r.get(3)?,
                        })
                    }) {
                        for face in face_rows.flatten() {
                            faces.push(face);
                        }
                    }
                }

                Ok(PhotoSyncInfo {
                    id,
                    location: row.get(1)?,
                    created: row.get(2).unwrap_or_default(),
                    latitude: row.get(3).ok(),
                    longitude: row.get(4).ok(),
                    objects: serde_json::to_string(&objects).unwrap_or("[]".to_string()),
                    faces: serde_json::to_string(&faces).unwrap_or("[]".to_string()),
                    caption: row.get(5).ok(),
                    aesthetics_score: row.get(6).ok(),
                })
            });
            if let Ok(iter) = iter {
                for p in iter.flatten() {
                    results.push(p);
                }
            }
        }
        results
    }

    /// Get sync info for a single photo by its ID.
    pub fn get_photo_sync_info_by_id(&self, photo_id: &str) -> Result<PhotoSyncInfo, String> {
        let sql = "SELECT id, location, created, latitude, longitude, caption, aesthetics_score FROM photo WHERE id = ?1";
        self.connection
            .query_row(sql, [photo_id], |row| {
                let id: String = row.get(0)?;

                // Fetch objects for this photo
                let mut objects = Vec::new();
                if let Ok(mut obj_stmt) = self
                    .connection
                    .prepare("SELECT class, probability FROM object WHERE photo_id = ?1")
                {
                    if let Ok(obj_rows) = obj_stmt.query_map([&id], |r| {
                        Ok(SyncObject {
                            class: r.get(0)?,
                            probability: r.get(1)?,
                        })
                    }) {
                        for obj in obj_rows.flatten() {
                            objects.push(obj);
                        }
                    }
                }

                // Fetch faces for this photo
                let mut faces = Vec::new();
                if let Ok(mut face_stmt) = self.connection.prepare(
                    "SELECT face_id, crop_path, encoded, person_id FROM faces WHERE photo_id = ?1",
                ) {
                    if let Ok(face_rows) = face_stmt.query_map([&id], |r| {
                        Ok(SyncFace {
                            face_id: r.get(0)?,
                            crop_path: r.get(1)?,
                            encoded: r.get(2)?,
                            person_id: r.get(3)?,
                        })
                    }) {
                        for face in face_rows.flatten() {
                            faces.push(face);
                        }
                    }
                }

                Ok(PhotoSyncInfo {
                    id,
                    location: row.get(1)?,
                    created: row.get(2).unwrap_or_default(),
                    latitude: row.get(3).ok(),
                    longitude: row.get(4).ok(),
                    objects: serde_json::to_string(&objects).unwrap_or("[]".to_string()),
                    faces: serde_json::to_string(&faces).unwrap_or("[]".to_string()),
                    caption: row.get(5).ok(),
                    aesthetics_score: row.get(6).ok(),
                })
            })
            .map_err(|e| e.to_string())
    }

    /// Open or create the database at the given config directory, running schema migrations.
    pub fn new(config_path: &str) -> Self {
        let path = format!("{config_path}/siegu.db");
        let _ = fs::create_dir_all(config_path);
        let conn = Connection::open(&path).expect("Failed to open database connection");

        // Enable WAL mode for better concurrency and set a busy timeout
        let _ = conn.execute("PRAGMA journal_mode=WAL;", ());
        let _ = conn.busy_timeout(std::time::Duration::from_secs(5));

        let _ = conn.execute("CREATE TABLE IF NOT EXISTS photo (id STRING PRIMARY KEY, location STRING, encoded STRING, created DATE_TIME, latitude REAL, longitude REAL, indexed INTEGER DEFAULT 0, caption TEXT, aesthetics_score REAL);", ());
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS ai_status (photo_id STRING PRIMARY KEY, clip INTEGER DEFAULT 0, face INTEGER DEFAULT 0, ocr INTEGER DEFAULT 0, nsfw INTEGER DEFAULT 0, aesthetics INTEGER DEFAULT 0, yolo INTEGER DEFAULT 0, blip INTEGER DEFAULT 0, arcface INTEGER DEFAULT 0, midas INTEGER DEFAULT 0, whisper INTEGER DEFAULT 0, sam INTEGER DEFAULT 0, superres INTEGER DEFAULT 0);", ());
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS ocr (photo_id STRING, text TEXT);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_ocr_photo_id ON ocr(photo_id);",
            (),
        );

        // Simple migration: try to add columns if they don't exist (ignore errors if they do)
        let _ = conn.execute("ALTER TABLE photo ADD COLUMN latitude REAL;", ());
        let _ = conn.execute("ALTER TABLE photo ADD COLUMN longitude REAL;", ());
        let _ = conn.execute("ALTER TABLE photo ADD COLUMN created DATE_TIME;", ());
        let _ = conn.execute("ALTER TABLE photo ADD COLUMN caption TEXT;", ());
        let _ = conn.execute("ALTER TABLE photo ADD COLUMN aesthetics_score REAL;", ());
        let _ = conn.execute(
            "ALTER TABLE photo ADD COLUMN sync_needed INTEGER DEFAULT 0;",
            (),
        );
        let _ = conn.execute(
            "ALTER TABLE photo ADD COLUMN indexed INTEGER DEFAULT 0;",
            (),
        );
        let _ = conn.execute(
            "ALTER TABLE photo ADD COLUMN received INTEGER DEFAULT 0;",
            (),
        );

        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_location ON photo(location);",
            (),
        );

        // Deduplicate existing rows by location, then enforce uniqueness
        let _ = conn.execute(
            "DELETE FROM photo WHERE rowid NOT IN (SELECT MIN(rowid) FROM photo GROUP BY location)",
            (),
        );
        let _ = conn.execute("DROP INDEX IF EXISTS idx_photo_location;", ());
        let _ = conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_photo_location_unique ON photo(location);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_created ON photo(created);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_aesthetics ON photo(aesthetics_score);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_indexed ON photo(indexed);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_coords ON photo(latitude, longitude);",
            (),
        );
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS directory (name TEXT);", ());
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS object(photo_id TEXT, class TEXT, probability TEXT);",
            (),
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS properties (photo_id TEXT, key TEXT, value TEXT);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_object_photo_id ON object(photo_id);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_properties_photo_id ON properties(photo_id);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_faces_person_id ON faces(person_id);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_faces_photo_id ON faces(photo_id);",
            (),
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS peer_device(\
             device_id TEXT PRIMARY KEY, name TEXT, ip TEXT, port INTEGER DEFAULT 0, \
             device_type TEXT DEFAULT '', os TEXT DEFAULT '', \
             models_enabled TEXT DEFAULT '[]', protocol_version INTEGER DEFAULT 1, \
             storage_used INTEGER DEFAULT 0, storage_capacity INTEGER DEFAULT 0, \
             last_seen TEXT DEFAULT (datetime('now'))\
             );",
            (),
        );
        // Migrate legacy device table → peer_device, then drop it.
        let _ = conn.execute(
            "INSERT OR IGNORE INTO peer_device(device_id, name, ip, port, last_seen) \
             SELECT lower(hex(randomblob(16))), name, ip, 0, datetime('now') FROM device",
            (),
        );
        let _ = conn.execute("DROP TABLE IF EXISTS device", ());
        let _ = conn.execute(
            "ALTER TABLE peer_device ADD COLUMN photo_count INTEGER DEFAULT 0;",
            (),
        );
        let _ = conn.execute(
            "ALTER TABLE peer_device ADD COLUMN video_count INTEGER DEFAULT 0;",
            (),
        );
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS faces (photo_id STRING, face_id STRING PRIMARY KEY, crop_path STRING, encoded STRING, embedding BLOB, person_id STRING);", ());
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS people (id STRING PRIMARY KEY, name STRING, embedding BLOB);", ());
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS config(key TEXT, value TEXT);",
            (),
        );
        // Deduplicate config rows (old schema had no UNIQUE constraint)
        let _ = conn.execute(
            "DELETE FROM config WHERE rowid NOT IN (SELECT MIN(rowid) FROM config GROUP BY key)",
            (),
        );
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS logs (timestamp DATETIME DEFAULT CURRENT_TIMESTAMP, level STRING, message TEXT);", ());

        // Albums (local, free tier): user-created collections of photos.
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS album(\
             id TEXT PRIMARY KEY, name TEXT NOT NULL, \
             created_at TEXT DEFAULT (datetime('now')), \
             cover_photo_id TEXT, sort_order INTEGER DEFAULT 0\
             );",
            (),
        );
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS album_item(\
             album_id TEXT NOT NULL, photo_id TEXT NOT NULL, \
             added_at TEXT DEFAULT (datetime('now')), \
             position INTEGER DEFAULT 0, \
             PRIMARY KEY (album_id, photo_id)\
             );",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_album_item_album ON album_item(album_id, position);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_album_item_photo ON album_item(photo_id);",
            (),
        );

        Self { connection: conn }
    }

    /// Run `PRAGMA integrity_check` and return true if the database is healthy.
    pub fn check_integrity(&self) -> crate::Result<bool> {
        let result: String = self
            .connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if result == "ok" {
            Ok(true)
        } else {
            tracing::error!(result = %result, "Database integrity check failed");
            Ok(false)
        }
    }

    /// Read all config key-value pairs from the config table.
    pub fn get_state(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Ok(mut stmt) = self.connection.prepare("SELECT key, value FROM config") {
            if let Ok(rows) = stmt.query_map([], |row| {
                let key = Self::sql_value_to_string(row.get::<_, rusqlite::types::Value>(0)?);
                let value = Self::sql_value_to_string(row.get::<_, rusqlite::types::Value>(1)?);
                Ok((key, value))
            }) {
                for row in rows.flatten() {
                    map.insert(row.0, row.1);
                }
            }
        }
        map
    }

    /// Insert a log entry with the given level and message.
    pub fn store_log(&self, level: &str, message: &str) {
        if let Err(e) = self.connection.execute(
            "INSERT INTO logs (level, message) VALUES (?1, ?2)",
            (level, message),
        ) {
            tracing::warn!("store_log: failed to insert log entry: {e}");
        }
    }

    /// Convert any SQLite storage class to its string representation.
    fn sql_value_to_string(value: rusqlite::types::Value) -> String {
        match value {
            rusqlite::types::Value::Null => String::new(),
            rusqlite::types::Value::Integer(i) => i.to_string(),
            rusqlite::types::Value::Real(f) => f.to_string(),
            rusqlite::types::Value::Text(s) => s,
            rusqlite::types::Value::Blob(b) => String::from_utf8_lossy(&b).into_owned(),
        }
    }

    /// Retrieve the most recent log entries, up to `limit`.
    pub fn get_logs(&self, limit: usize) -> Vec<LogEntry> {
        let mut logs = Vec::new();
        let sql = "SELECT timestamp, level, message FROM logs ORDER BY timestamp DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit as i64], |row| {
                Ok(LogEntry {
                    timestamp: row.get(0)?,
                    level: row.get(1)?,
                    message: row.get(2)?,
                })
            }) {
                for log in iter.flatten() {
                    logs.push(log);
                }
            }
        }
        logs
    }

    /// Delete all log entries from the logs table.
    pub fn clear_logs(&self) {
        if let Err(e) = self.connection.execute("DELETE FROM logs", ()) {
            tracing::warn!("clear_logs: {e}");
        }
    }

    /// Write config key-value pairs, replacing any existing keys.
    pub fn set_state(&self, state: HashMap<String, String>) {
        for (key, value) in state {
            let _ = self
                .connection
                .execute("DELETE FROM config WHERE key = ?1", [&key]);
            let _ = self.connection.execute(
                "INSERT INTO config (key, value) VALUES(?1, ?2)",
                (&key, &value),
            );
        }
    }

    /// Get the timestamp of the last directory scan.
    pub fn get_last_scan_time(&self) -> Option<String> {
        self.get_state().get("last_scan_time").cloned()
    }

    /// Set the timestamp of the last directory scan.
    pub fn set_last_scan_time(&self, timestamp: String) {
        let _ = self.connection.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES('last_scan_time', ?1)",
            [&timestamp],
        );
    }

    /// Save a sync session for auto-reconnect on next startup.
    /// The passphrase is encrypted with AES-256-GCM using a device-derived key.
    pub fn save_session(&self, session: &SavedSession) {
        let encrypted = session.encrypt();
        let mut state = self.get_state();
        state.insert("session_room_id".to_string(), session.room_id.clone());
        state.insert(
            "session_signaling_url".to_string(),
            session.signaling_url.clone(),
        );
        state.insert("session_port".to_string(), session.port.to_string());
        state.insert(
            "session_is_initiator".to_string(),
            session.is_initiator.to_string(),
        );
        state.insert("session_encrypted_passphrase".to_string(), encrypted);
        self.set_state(state);
    }

    /// Load the saved session, if any. Returns None if no session exists or decryption fails.
    pub fn load_session(&self) -> Option<SavedSession> {
        let state = self.get_state();
        let room_id = state.get("session_room_id")?.clone();
        let signaling_url = state.get("session_signaling_url")?.clone();
        let port = state
            .get("session_port")
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(0);
        let is_initiator = state
            .get("session_is_initiator")
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);
        let encrypted = state.get("session_encrypted_passphrase")?.clone();
        let passphrase = SavedSession::decrypt(&encrypted).ok()?;
        Some(SavedSession {
            room_id,
            signaling_url,
            port,
            is_initiator,
            passphrase,
        })
    }

    /// Clear any saved session.
    pub fn clear_session(&self) {
        let mut state = self.get_state();
        state.remove("session_room_id");
        state.remove("session_signaling_url");
        state.remove("session_port");
        state.remove("session_is_initiator");
        state.remove("session_encrypted_passphrase");
        self.set_state(state);
    }

    /// Search for object tags, location names, people, and month suggestions matching the query.
    pub fn list_objects(&self, query: &str) -> Vec<SearchSuggestion> {
        let mut objects = Vec::new();
        let sql = "SELECT class, 'tag' FROM object WHERE class LIKE ?1 \
            UNION SELECT value, 'location' FROM properties WHERE key = 'location_name' AND value LIKE ?1 \
            UNION SELECT name, 'person' FROM people WHERE name IS NOT NULL AND name LIKE ?1 \
            LIMIT 20";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([format!("%{query}%")], |row| {
                Ok(SearchSuggestion {
                    title: row.get(0)?,
                    suggestion_type: row.get(1)?,
                })
            }) {
                for item in iter.flatten() {
                    objects.push(item);
                }
            }
        }

        let q = query.trim();
        if q.len() >= 2 {
            let q_lower = q.to_lowercase();
            for &(num, full, abbr) in MONTH_NAMES {
                if full.starts_with(&q_lower) || abbr.starts_with(&q_lower) {
                    objects.push(SearchSuggestion {
                        title: full.to_string(),
                        suggestion_type: "date".to_string(),
                    });
                }

                if q_lower.contains(full) || q_lower.contains(abbr) {
                    let ym_sql = "SELECT DISTINCT substr(created, 1, 7) FROM photo WHERE created LIKE ?1 ORDER BY 1 DESC LIMIT 5";
                    if let Ok(mut stmt) = self.connection.prepare(ym_sql) {
                        let pat = format!("%-{:02}-%", num);
                        if let Ok(iter) = stmt.query_map([&pat], |row| row.get::<_, String>(0)) {
                            for ym in iter.flatten() {
                                if let Some((year, _)) = ym.split_once('-') {
                                    objects.push(SearchSuggestion {
                                        title: format!("{} {}", full, year),
                                        suggestion_type: "date".to_string(),
                                    });
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }

        objects
    }

    fn enrich_objects(&self, photos: &mut [Photo]) {
        if photos.is_empty() {
            return;
        }
        let placeholders: Vec<String> = photos
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT photo_id, class, probability FROM object WHERE photo_id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = photos
            .iter()
            .map(|p| &p.id as &dyn rusqlite::types::ToSql)
            .collect();
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(params.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)
                        .ok()
                        .or_else(|| row.get::<_, String>(2).ok().and_then(|s| s.parse().ok()))
                        .unwrap_or(0.0),
                ))
            }) {
                for row in iter.flatten() {
                    if let Some(p) = photos.iter_mut().find(|p| p.id == row.0) {
                        p.objects.insert(row.1, row.2);
                    }
                }
            }
        }
    }

    fn enrich_properties(&self, photos: &mut [Photo]) {
        if photos.is_empty() {
            return;
        }
        let placeholders: Vec<String> = photos
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT photo_id, key, value FROM properties WHERE photo_id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = photos
            .iter()
            .map(|p| &p.id as &dyn rusqlite::types::ToSql)
            .collect();
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(params.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)
                        .ok()
                        .or_else(|| row.get::<_, f64>(2).ok().map(|v| v.to_string()))
                        .or_else(|| row.get::<_, i64>(2).ok().map(|v| v.to_string()))
                        .unwrap_or_default(),
                ))
            }) {
                for row in iter.flatten() {
                    if let Some(p) = photos.iter_mut().find(|p| p.id == row.0) {
                        p.properties.insert(row.1, row.2);
                    }
                }
            }
        }
    }

    /// List photos with search, pagination, and optional favorite/video filters.
    pub fn list_photos(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
        favorites_only: bool,
        videos_only: bool,
    ) -> Vec<Photo> {
        self.list_photos_filtered(
            query,
            offset,
            limit,
            favorites_only,
            videos_only,
            &PhotoFilter::default(),
        )
    }

    /// List photos with search, pagination, favorite/video filters, and facet
    /// filters (person, location, tag, date range) combined with AND.
    pub fn list_photos_filtered(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
        favorites_only: bool,
        videos_only: bool,
        filter: &PhotoFilter,
    ) -> Vec<Photo> {
        let mut photos = Vec::new();
        let fav_filter = if favorites_only {
            "AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite')"
        } else {
            ""
        };
        let video_filter = if videos_only {
            &format!("AND {}", video_sql_like())
        } else {
            ""
        };

        let is_uuid = query.len() == 36 && query.chars().all(|c| c.is_alphanumeric() || c == '-');

        let month_like = month_name_to_like(query);
        let month_param = month_like.as_ref().map(|p| p.to_string());

        let q_filter = if !query.is_empty() {
            if is_uuid {
                "AND (p.id = ?3 OR EXISTS(SELECT 1 FROM faces WHERE photo_id=p.id AND person_id = ?3))".to_string()
            } else if month_param.is_some() {
                "AND (p.location LIKE ?3 OR p.id LIKE ?3 OR p.caption LIKE ?3 \
                    OR EXISTS(SELECT 1 FROM object WHERE photo_id=p.id AND class LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM ocr WHERE photo_id=p.id AND text LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM faces f JOIN people p_name ON f.person_id = p_name.id WHERE f.photo_id=p.id AND p_name.name LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='location_name' AND value LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='transcript' AND value LIKE ?3) \
                    OR p.created LIKE ?3 OR p.created LIKE ?4)".to_string()
            } else {
                "AND (p.location LIKE ?3 OR p.id LIKE ?3 OR p.caption LIKE ?3 \
                    OR EXISTS(SELECT 1 FROM object WHERE photo_id=p.id AND class LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM ocr WHERE photo_id=p.id AND text LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM faces f JOIN people p_name ON f.person_id = p_name.id WHERE f.photo_id=p.id AND p_name.name LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='location_name' AND value LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='transcript' AND value LIKE ?3) \
                    OR p.created LIKE ?3)".to_string()
            }
        } else {
            String::new()
        };

        // Build facet filters using the next available parameter slots.
        let mut facet_filters = String::new();
        let mut extra_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut slot = 2usize; // ?1 offset, ?2 limit
        if !query.is_empty() {
            slot += 1; // ?3 query
            if month_param.is_some() {
                slot += 1; // ?4 month
            }
        }
        if let Some(ref person_id) = filter.person_id {
            slot += 1;
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM faces WHERE photo_id=p.id AND person_id = ?{slot})"
            ));
            extra_params.push(Box::new(person_id.clone()));
        }
        if let Some(ref location) = filter.location {
            slot += 1;
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='location_name' AND value = ?{slot})"
            ));
            extra_params.push(Box::new(location.clone()));
        }
        if let Some(ref tag) = filter.tag {
            slot += 1;
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM object WHERE photo_id=p.id AND class = ?{slot})"
            ));
            extra_params.push(Box::new(tag.clone()));
        }
        if let Some(ref date_from) = filter.date_from {
            slot += 1;
            facet_filters.push_str(&format!(" AND p.created >= ?{slot}"));
            extra_params.push(Box::new(date_from.clone()));
        }
        if let Some(ref date_to) = filter.date_to {
            slot += 1;
            facet_filters.push_str(&format!(" AND p.created <= ?{slot}"));
            extra_params.push(Box::new(date_to.clone()));
        }
        if filter.favorite {
            facet_filters.push_str(
                " AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite')",
            );
        }
        if filter.has_faces {
            facet_filters.push_str(" AND EXISTS(SELECT 1 FROM faces WHERE photo_id=p.id)");
        }
        if let Some(ref aesthetics_min) = filter.aesthetics_min {
            slot += 1;
            facet_filters.push_str(&format!(" AND p.aesthetics_score >= ?{slot}"));
            extra_params.push(Box::new(*aesthetics_min));
        }
        if let Some(ref camera) = filter.camera {
            slot += 1;
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key IN ('Make','Model') AND value LIKE ?{slot})"
            ));
            extra_params.push(Box::new(format!("%{camera}%")));
        }
        if filter.papers {
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM object WHERE photo_id=p.id AND class IN ({paper_in}))",
                paper_in = paper_class_in_clause()
            ));
        }
        if filter.nsfw_only {
            facet_filters.push_str(
                " AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='nsfw' AND CAST(value AS REAL) >= 0.8)",
            );
        }
        if let Some(ref album_id) = filter.album_id {
            slot += 1;
            facet_filters.push_str(&format!(
                " AND EXISTS(SELECT 1 FROM album_item WHERE album_id=?{slot} AND photo_id=p.id)"
            ));
            extra_params.push(Box::new(album_id.clone()));
        }

        let order_by = if let Some(order) = filter.order_by.as_deref() {
            match order {
                "oldest" => "ORDER BY p.created ASC",
                "best" => "ORDER BY p.aesthetics_score DESC NULLS LAST, p.created DESC",
                "random" => "ORDER BY RANDOM()",
                _ => "ORDER BY p.created DESC",
            }
        } else if filter.random {
            "ORDER BY RANDOM()"
        } else {
            "ORDER BY p.created DESC"
        };

        let sql = format!("SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, p.caption, p.aesthetics_score, 
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres, p.sync_needed, p.received 
            FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE 1=1 {fav_filter} {video_filter} {q_filter} {facet_filters} {order_by} LIMIT ?1, ?2");
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            let q_param = if is_uuid {
                query.to_string()
            } else {
                format!("%{query}%")
            };
            let mut params: Vec<Box<dyn rusqlite::ToSql>> =
                vec![Box::new(offset as i64), Box::new(limit as i64)];
            if !query.is_empty() {
                params.push(Box::new(q_param));
                if let Some(ref mp) = month_param {
                    params.push(Box::new(mp.clone()));
                }
            }
            params.extend(extra_params);
            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            if let Ok(iter) = stmt.query_map(param_refs.as_slice(), |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2)?,
                    created: row.get(5).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: row.get(3).unwrap_or(0.0),
                    longitude: row.get(4).unwrap_or(0.0),
                    favorite: row.get(6).unwrap_or(false),
                    indexed: row.get(7).unwrap_or(0),
                    caption: row.get(8).ok(),
                    aesthetics_score: row.get(9).ok(),
                    ai_status: AiStatus {
                        clip: row.get(10).unwrap_or(0),
                        face: row.get(11).unwrap_or(0),
                        ocr: row.get(12).unwrap_or(0),
                        nsfw: row.get(13).unwrap_or(0),
                        aesthetics: row.get(14).unwrap_or(0),
                        yolo: row.get(15).unwrap_or(0),
                        blip: row.get(16).unwrap_or(0),
                        arcface: row.get(17).unwrap_or(0),
                        midas: row.get(18).unwrap_or(0),
                        whisper: row.get(19).unwrap_or(0),
                        sam: row.get(20).unwrap_or(0),
                        superres: row.get(21).unwrap_or(0),
                    },
                    sync_needed: row.get(22).unwrap_or(false),
                    received: row.get(23).unwrap_or(false),
                })
            }) {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        self.enrich_objects(&mut photos);
        self.enrich_properties(&mut photos);
        photos
    }

    /// Named people with a representative face thumbnail, ordered by the number
    /// of distinct photos that contain them.
    pub fn get_search_people(&self, limit: i64) -> Vec<SearchPerson> {
        let mut people = Vec::new();
        let sql = "SELECT p.id, p.name, f.crop_path, f.encoded, \
            (SELECT COUNT(DISTINCT photo_id) FROM faces WHERE person_id = p.id) \
            FROM people p LEFT JOIN faces f ON p.id = f.person_id \
            WHERE p.name IS NOT NULL AND TRIM(p.name) != '' \
            GROUP BY p.id ORDER BY 5 DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok(SearchPerson {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    representative_crop: row.get(2).ok(),
                    encoded: row.get(3).ok(),
                    photo_count: row.get(4)?,
                })
            }) {
                for p in iter.flatten() {
                    people.push(p);
                }
            }
        }
        people
    }

    /// Distinct resolved location names with photo counts, most common first.
    pub fn get_location_counts(&self, limit: i64) -> Vec<(String, i64)> {
        let mut counts = Vec::new();
        let sql = "SELECT value, COUNT(*) FROM properties WHERE key = 'location_name' \
            GROUP BY value ORDER BY COUNT(*) DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            }) {
                for c in iter.flatten() {
                    counts.push(c);
                }
            }
        }
        counts
    }

    /// Detected object classes with photo counts, most common first.
    pub fn get_tag_counts(&self, limit: i64) -> Vec<(String, i64)> {
        let mut counts = Vec::new();
        let sql =
            "SELECT class, COUNT(*) FROM object GROUP BY class ORDER BY COUNT(*) DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            }) {
                for c in iter.flatten() {
                    counts.push(c);
                }
            }
        }
        counts
    }

    /// Year-month buckets ("2026-03") with photo counts, newest first.
    pub fn get_month_counts(&self, limit: i64) -> Vec<(String, i64)> {
        let mut counts = Vec::new();
        let sql = "SELECT substr(created, 1, 7) AS ym, COUNT(*) FROM photo \
            WHERE created IS NOT NULL AND created != '' GROUP BY ym ORDER BY ym DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            }) {
                for c in iter.flatten() {
                    counts.push(c);
                }
            }
        }
        counts
    }

    /// Library-wide counts used by the search dropdown footer.
    pub fn get_search_stats(&self) -> SearchStats {
        let query_count = |sql: &str| -> i64 {
            self.connection
                .query_row(sql, [], |r| r.get(0))
                .unwrap_or(0)
        };
        let video_like = video_sql_like();
        SearchStats {
            photos: query_count("SELECT COUNT(*) FROM photo"),
            videos: query_count(&format!("SELECT COUNT(*) FROM photo WHERE {video_like}")),
            favorites: query_count("SELECT COUNT(*) FROM properties WHERE key = 'favorite'"),
            ocr_photos: query_count("SELECT COUNT(DISTINCT photo_id) FROM ocr"),
            faces: query_count("SELECT COUNT(*) FROM faces"),
            named_people: query_count("SELECT COUNT(*) FROM people WHERE name IS NOT NULL"),
            face_photos: query_count("SELECT COUNT(DISTINCT photo_id) FROM faces"),
            nsfw_photos: query_count(
                "SELECT COUNT(DISTINCT photo_id) FROM properties WHERE key = 'nsfw' AND CAST(value AS REAL) >= 0.8",
            ),
        }
    }

    /// Highest-rated photos by aesthetics score, for the "Best of your library" rail.
    /// Best-scored photo of each day, most recent day first, for the Best Shots rail.
    pub fn get_best_photos(&self, limit: i64) -> Vec<SearchPhotoTile> {
        let mut tiles = Vec::new();
        let sql = "SELECT id, location, encoded, created, aesthetics_score, favorite FROM ( \
            SELECT p.id, p.location, p.encoded, p.created, p.aesthetics_score, \
                EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite') AS favorite, \
                ROW_NUMBER() OVER ( \
                    PARTITION BY substr(p.created, 1, 10) \
                    ORDER BY p.aesthetics_score DESC, p.created ASC \
                ) AS rn \
            FROM photo p \
            WHERE p.aesthetics_score IS NOT NULL AND p.aesthetics_score > 0 \
        ) WHERE rn = 1 \
        ORDER BY created DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok(SearchPhotoTile {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2).unwrap_or_default(),
                    created: row.get(3).unwrap_or_default(),
                    aesthetics_score: row.get(4).ok(),
                    favorite: row.get(5).unwrap_or(false),
                })
            }) {
                for t in iter.flatten() {
                    tiles.push(t);
                }
            }
        }
        tiles
    }

    /// Favorited photos, newest first, for the Favorites rail.
    pub fn get_favorite_photos(&self, limit: i64) -> Vec<SearchPhotoTile> {
        let mut tiles = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.created, p.aesthetics_score, 1 \
            FROM photo p WHERE EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite') \
            ORDER BY p.created DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok(SearchPhotoTile {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2).unwrap_or_default(),
                    created: row.get(3).unwrap_or_default(),
                    aesthetics_score: row.get(4).ok(),
                    favorite: row.get(5).unwrap_or(false),
                })
            }) {
                for t in iter.flatten() {
                    tiles.push(t);
                }
            }
        }
        tiles
    }

    /// Most recently added photos for the Recent rail.
    pub fn get_recent_photos(&self, limit: i64) -> Vec<SearchPhotoTile> {
        let mut tiles = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.created, p.aesthetics_score, \
            EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite') \
            FROM photo p ORDER BY p.created DESC, p.id DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok(SearchPhotoTile {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2).unwrap_or_default(),
                    created: row.get(3).unwrap_or_default(),
                    aesthetics_score: row.get(4).ok(),
                    favorite: row.get(5).unwrap_or(false),
                })
            }) {
                for t in iter.flatten() {
                    tiles.push(t);
                }
            }
        }
        tiles
    }

    /// Document/screenshot classes (CLIP zero-shot) with photo counts.
    pub fn get_paper_counts(&self, limit: i64) -> Vec<(String, i64)> {
        let mut counts = Vec::new();
        let sql = format!(
            "SELECT class, COUNT(*) FROM object WHERE class IN ({paper_in}) \
        GROUP BY class ORDER BY COUNT(*) DESC LIMIT ?1",
            paper_in = paper_class_in_clause()
        );
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            }) {
                for c in iter.flatten() {
                    counts.push(c);
                }
            }
        }
        counts
    }

    /// Camera identifiers (Make + Model from EXIF) with photo counts, most common first.
    pub fn get_camera_counts(&self, limit: i64) -> Vec<(String, i64)> {
        let mut by_photo: std::collections::HashMap<String, (Option<String>, Option<String>)> =
            std::collections::HashMap::new();
        let sql = "SELECT photo_id, key, value FROM properties WHERE key IN ('Make','Model')";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            }) {
                for r in iter.flatten() {
                    let (pid, key, value) = r;
                    let entry = by_photo.entry(pid).or_default();
                    if key == "Make" {
                        entry.0 = Some(value);
                    } else {
                        entry.1 = Some(value);
                    }
                }
            }
        }
        let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for (_, (make, model)) in by_photo {
            let brand = match (make, model) {
                (Some(m), _) if !m.trim().is_empty() => m.trim().to_lowercase(),
                (_, Some(md)) if !md.trim().is_empty() => {
                    md.split_whitespace().next().unwrap_or("").to_lowercase()
                }
                _ => continue,
            };
            if brand.is_empty() {
                continue;
            }
            *counts.entry(brand).or_insert(0) += 1;
        }
        let mut sorted: Vec<(String, i64)> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        sorted.truncate(limit as usize);
        sorted
    }

    /// Locations with counts and a representative photo thumbnail for the rail.
    pub fn get_location_groups(&self, limit: i64) -> Vec<LocationGroup> {
        let mut groups = Vec::new();
        let sql = "SELECT pr.value AS name, COUNT(*) AS cnt, \
            (SELECT p2.location FROM photo p2 JOIN properties pr2 ON pr2.photo_id=p2.id \
                WHERE pr2.key='location_name' AND pr2.value=pr.value \
                ORDER BY (p2.encoded != '') DESC, p2.created DESC LIMIT 1) AS rep_loc, \
            (SELECT p2.encoded FROM photo p2 JOIN properties pr2 ON pr2.photo_id=p2.id \
                WHERE pr2.key='location_name' AND pr2.value=pr.value \
                ORDER BY (p2.encoded != '') DESC, p2.created DESC LIMIT 1) AS rep_enc \
            FROM properties pr WHERE pr.key='location_name' \
            GROUP BY pr.value ORDER BY cnt DESC LIMIT ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([limit], |row| {
                Ok(LocationGroup {
                    name: row.get(0)?,
                    count: row.get(1)?,
                    photo_location: row.get(2).ok(),
                    encoded: row.get(3).ok(),
                })
            }) {
                for g in iter.flatten() {
                    groups.push(g);
                }
            }
        }
        groups
    }

    /// Day-level photo/video counts within a date range (inclusive, `YYYY-MM-DD`).
    pub fn get_day_counts(&self, from: &str, to: &str) -> Vec<DayCount> {
        let mut counts = Vec::new();
        let sql = format!(
            "SELECT substr(created, 1, 10) AS day, COUNT(*) AS cnt, \
             SUM(CASE WHEN {} THEN 1 ELSE 0 END) AS videos \
             FROM photo WHERE substr(created, 1, 10) BETWEEN ?1 AND ?2 \
             GROUP BY day ORDER BY day",
            video_sql_like()
        );
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map([from, to], |row| {
                Ok(DayCount {
                    date: row.get(0)?,
                    photos: row.get(1)?,
                    videos: row.get(2).unwrap_or(0),
                })
            }) {
                for d in iter.flatten() {
                    counts.push(d);
                }
            }
        }
        counts
    }

    /// Toggle the favorite status of a photo. Returns true if now favorited.
    pub fn toggle_favorite(&self, photo_id: &str) -> bool {
        let exists = self
            .connection
            .query_row(
                "SELECT 1 FROM properties WHERE photo_id = ?1 AND key = 'favorite'",
                [photo_id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if exists {
            let _ = self.connection.execute(
                "DELETE FROM properties WHERE photo_id = ?1 AND key = 'favorite'",
                [photo_id],
            );
            false
        } else {
            let _ = self.connection.execute(
                "INSERT INTO properties (photo_id, key, value) VALUES(?1, 'favorite', 'true')",
                [photo_id],
            );
            true
        }
    }

    /// Get all photos that have non-zero GPS coordinates, for map heatmap display.
    pub fn get_heatmap_points(&self) -> Vec<MapPoint> {
        let mut points = Vec::new();
        let sql =
            "SELECT id, latitude, longitude FROM photo WHERE (latitude != 0.0 OR longitude != 0.0)";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], |row| {
                Ok(MapPoint {
                    id: row.get(0)?,
                    latitude: row.get(1).unwrap_or(0.0),
                    longitude: row.get(2).unwrap_or(0.0),
                })
            }) {
                for p in iter.flatten() {
                    points.push(p);
                }
            }
        }
        points
    }

    /// Fetch multiple photos by their IDs in a single query.
    pub fn get_photos_by_ids(&self, photo_ids: &[String]) -> Vec<Photo> {
        if photo_ids.is_empty() {
            return Vec::new();
        }
        let mut photos = Vec::new();
        let placeholders: Vec<String> = photo_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, p.caption, p.aesthetics_score, \
             s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres, p.sync_needed, p.received \
             FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = photo_ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(params.as_slice(), |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2)?,
                    created: row.get(5).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: row.get(3).unwrap_or(0.0),
                    longitude: row.get(4).unwrap_or(0.0),
                    favorite: row.get(6).unwrap_or(false),
                    indexed: row.get(7).unwrap_or(0),
                    caption: row.get(8).ok(),
                    aesthetics_score: row.get(9).ok(),
                    ai_status: AiStatus {
                        clip: row.get(10).unwrap_or(0),
                        face: row.get(11).unwrap_or(0),
                        ocr: row.get(12).unwrap_or(0),
                        nsfw: row.get(13).unwrap_or(0),
                        aesthetics: row.get(14).unwrap_or(0),
                        yolo: row.get(15).unwrap_or(0),
                        blip: row.get(16).unwrap_or(0),
                        arcface: row.get(17).unwrap_or(0),
                        midas: row.get(18).unwrap_or(0),
                        whisper: row.get(19).unwrap_or(0),
                        sam: row.get(20).unwrap_or(0),
                        superres: row.get(21).unwrap_or(0),
                    },
                    sync_needed: row.get(22).unwrap_or(false),
                    received: row.get(23).unwrap_or(false),
                })
            }) {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        self.enrich_objects(&mut photos);
        self.enrich_properties(&mut photos);
        photos
    }

    /// Fetch a single photo by its ID, with objects and properties populated.
    /// Concatenated OCR text for a photo (all detected text rows).
    pub fn get_photo_ocr(&self, photo_id: &str) -> String {
        let mut parts = Vec::new();
        if let Ok(mut stmt) = self
            .connection
            .prepare("SELECT text FROM ocr WHERE photo_id = ?1 ORDER BY rowid")
        {
            if let Ok(rows) = stmt.query_map([photo_id], |r| r.get::<_, String>(0)) {
                for row in rows.flatten() {
                    let t = row.trim();
                    if !t.is_empty() {
                        parts.push(t.to_string());
                    }
                }
            }
        }
        parts.join(" ")
    }

    pub fn get_photo_by_id(&self, photo_id: &str) -> Option<Photo> {
        let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, p.caption, p.aesthetics_score, \
             s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres, p.sync_needed, p.received \
             FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.id = ?1";
        let mut stmt = self.connection.prepare(sql).ok()?;
        let mut rows = stmt
            .query_map([photo_id], |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2)?,
                    created: row.get(5).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: row.get(3).unwrap_or(0.0),
                    longitude: row.get(4).unwrap_or(0.0),
                    favorite: row.get(6).unwrap_or(false),
                    indexed: row.get(7).unwrap_or(0),
                    caption: row.get(8).ok(),
                    aesthetics_score: row.get(9).ok(),
                    ai_status: AiStatus {
                        clip: row.get(10).unwrap_or(0),
                        face: row.get(11).unwrap_or(0),
                        ocr: row.get(12).unwrap_or(0),
                        nsfw: row.get(13).unwrap_or(0),
                        aesthetics: row.get(14).unwrap_or(0),
                        yolo: row.get(15).unwrap_or(0),
                        blip: row.get(16).unwrap_or(0),
                        arcface: row.get(17).unwrap_or(0),
                        midas: row.get(18).unwrap_or(0),
                        whisper: row.get(19).unwrap_or(0),
                        sam: row.get(20).unwrap_or(0),
                        superres: row.get(21).unwrap_or(0),
                    },
                    sync_needed: row.get(22).unwrap_or(false),
                    received: row.get(23).unwrap_or(false),
                })
            })
            .ok()?;
        let mut photo = rows.next()?.ok()?;
        self.enrich_objects(std::slice::from_mut(&mut photo));
        self.enrich_properties(std::slice::from_mut(&mut photo));
        Some(photo)
    }

    /// Get base64-encoded thumbnails for a batch of photo IDs.
    pub fn get_photo_encoded_batch(&self, photo_ids: &[String]) -> HashMap<String, String> {
        let mut result = HashMap::new();
        if photo_ids.is_empty() {
            return result;
        }
        let placeholders: Vec<String> = photo_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT id, encoded FROM photo WHERE id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = photo_ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(params.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }) {
                for row in iter.flatten() {
                    result.insert(row.0, row.1);
                }
            }
        }
        result
    }

    /// Store or replace a detected face with its embedding and optional person assignment.
    pub fn store_face(&self, face: Face) {
        let embedding_bytes: Vec<u8> = face
            .embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        if let Err(e) = self.connection.execute("INSERT OR REPLACE INTO faces(photo_id, face_id, crop_path, encoded, embedding, person_id) VALUES(?1, ?2, ?3, ?4, ?5, ?6)", (&face.photo_id, &face.face_id, &face.crop_path, &face.encoded, &embedding_bytes, &face.person_id)) {
            tracing::warn!("store_face: failed to store face {}: {e}", face.face_id);
        }
    }

    /// List all named people with a representative face and face count.
    pub fn get_people(&self) -> Vec<PersonWithFace> {
        let mut people = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare("SELECT p.id, p.name, f.crop_path, f.face_id, f.encoded, p.embedding, (SELECT COUNT(*) FROM faces WHERE person_id = p.id) FROM people p LEFT JOIN faces f ON p.id = f.person_id WHERE p.name IS NOT NULL GROUP BY p.id") {
            if let Ok(iter) = stmt.query_map([], |row| {
                let embedding: Option<Vec<f32>> = row.get::<_, Option<Vec<u8>>>(5).ok().flatten().map(|bytes| bytes.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect());
                Ok(PersonWithFace { id: row.get(0)?, name: row.get(1)?, representative_crop: row.get(2).ok(), representative_face_id: row.get(3).ok(), encoded: row.get(4).ok(), embedding, face_count: row.get(6)? })
            }) {
                for p in iter.flatten() { people.push(p); }
            }
        }
        people
    }

    /// Assign a name to a face, creating or merging people as needed. Returns the person ID.
    pub fn assign_name_to_face(&self, face_id: &str, name: &str) -> String {
        let existing_id: Option<String> = self
            .connection
            .query_row("SELECT id FROM people WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .ok();
        let current_person_id: Option<String> = self
            .connection
            .query_row(
                "SELECT person_id FROM faces WHERE face_id = ?1",
                [face_id],
                |row| row.get(0),
            )
            .ok();

        let target_id = match existing_id {
            Some(id) => {
                if let Some(anon_id) = current_person_id {
                    if anon_id != id {
                        let _ = self.connection.execute(
                            "UPDATE faces SET person_id = ?1 WHERE person_id = ?2",
                            (&id, &anon_id),
                        );
                        let _ = self
                            .connection
                            .execute("DELETE FROM people WHERE id = ?1", [&anon_id]);
                    }
                } else {
                    let _ = self.connection.execute(
                        "UPDATE faces SET person_id = ?1 WHERE face_id = ?2",
                        (&id, face_id),
                    );
                }
                id
            }
            None => {
                if let Some(id) = current_person_id {
                    let _ = self
                        .connection
                        .execute("UPDATE people SET name = ?1 WHERE id = ?2", (name, &id));
                    id
                } else {
                    let new_id = uuid::Uuid::new_v4().to_string();
                    let _ = self.connection.execute(
                        "INSERT INTO people (id, name) VALUES (?1, ?2)",
                        (&new_id, name),
                    );
                    let _ = self.connection.execute(
                        "UPDATE faces SET person_id = ?1 WHERE face_id = ?2",
                        (&new_id, face_id),
                    );
                    new_id
                }
            }
        };

        self.update_person_centroid(&target_id);
        target_id
    }

    /// Create an unnamed person record from a face embedding. Returns the new person ID.
    pub fn create_anonymous_person(&self, embedding: &[f32]) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let embedding_bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        let _ = self.connection.execute(
            "INSERT INTO people (id, name, embedding) VALUES (?1, NULL, ?2)",
            (&id, &embedding_bytes),
        );
        id
    }

    /// Recompute the average face embedding (centroid) for a person.
    pub fn update_person_centroid(&self, person_id: &str) {
        let mut embeddings = Vec::new();
        if let Ok(mut stmt) = self
            .connection
            .prepare("SELECT embedding FROM faces WHERE person_id = ?1")
        {
            if let Ok(rows) = stmt.query_map([person_id], |row| row.get::<_, Vec<u8>>(0)) {
                for row in rows.flatten() {
                    let emb: Vec<f32> = row
                        .chunks_exact(4)
                        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                        .collect();
                    if emb.len() == 512 {
                        embeddings.push(emb);
                    }
                }
            }
        }

        if embeddings.is_empty() {
            return;
        }

        let count = embeddings.len() as f32;
        let mut centroid = vec![0.0f32; 512];
        for emb in embeddings {
            for (sum, value) in centroid.iter_mut().zip(emb.iter()).take(512) {
                *sum += value;
            }
        }
        for value in centroid.iter_mut().take(512) {
            *value /= count;
        }

        let norm: f32 = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in centroid.iter_mut() {
                *v /= norm;
            }
        }

        let centroid_bytes: Vec<u8> = centroid.iter().flat_map(|f| f.to_le_bytes()).collect();
        let _ = self.connection.execute(
            "UPDATE people SET embedding = ?1 WHERE id = ?2",
            (centroid_bytes, person_id),
        );
    }

    /// List all unnamed people grouped by face similarity, ordered by face count.
    pub fn get_anonymous_people_groups(&self) -> Vec<PersonWithFace> {
        let mut results = Vec::new();
        let sql = "SELECT p.id, f.crop_path, f.face_id, f.encoded, p.embedding, COUNT(*) \
            FROM people p JOIN faces f ON p.id = f.person_id \
            WHERE p.name IS NULL GROUP BY p.id ORDER BY COUNT(*) DESC";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], |row| {
                let embedding: Option<Vec<f32>> = row
                    .get::<_, Option<Vec<u8>>>(4)
                    .ok()
                    .flatten()
                    .map(|bytes| {
                        bytes
                            .chunks_exact(4)
                            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                            .collect()
                    });
                Ok(PersonWithFace {
                    id: row.get(0)?,
                    name: "Unnamed Person".to_string(),
                    representative_crop: row.get(1).ok(),
                    representative_face_id: row.get(2).ok(),
                    encoded: row.get(3).ok(),
                    embedding,
                    face_count: row.get(5)?,
                })
            }) {
                for p in iter.flatten() {
                    results.push(p);
                }
            }
        }
        results
    }

    /// Get all detected faces in a photo, with person names if assigned.
    pub fn get_faces_for_photo(&self, photo_id: &str) -> Vec<FaceWithPerson> {
        let mut faces = Vec::new();
        let sql = "SELECT f.photo_id, f.face_id, f.crop_path, f.encoded, f.person_id, p.name FROM faces f LEFT JOIN people p ON f.person_id = p.id WHERE f.photo_id = ?1";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([photo_id], |row| {
                Ok(FaceWithPerson {
                    photo_id: row.get(0)?,
                    face_id: row.get(1)?,
                    crop_path: row.get(2)?,
                    encoded: row.get(3)?,
                    person_id: row.get(4)?,
                    person_name: row.get(5)?,
                })
            }) {
                for f in iter.flatten() {
                    faces.push(f);
                }
            }
        }
        faces
    }

    /// Get all face records belonging to a person.
    pub fn get_person_faces(&self, person_id: &str) -> Vec<Face> {
        let mut faces = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare("SELECT photo_id, face_id, crop_path, encoded, person_id FROM faces WHERE person_id = ?1") {
            if let Ok(iter) = stmt.query_map([person_id], |row| {
                Ok(Face {
                    photo_id: row.get(0)?,
                    face_id: row.get(1)?,
                    crop_path: row.get(2)?,
                    encoded: row.get(3)?,
                    embedding: Vec::new(), // Not needed for UI
                    person_id: row.get(4)?,
                })
            }) {
                for f in iter.flatten() { faces.push(f); }
            }
        }
        faces
    }

    /// Get all photos that contain a given person.
    pub fn get_photos_for_person(&self, person_id: &str) -> Vec<Photo> {
        let mut photos = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, p.caption, p.aesthetics_score, 
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres 
            FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id JOIN faces f ON p.id = f.photo_id WHERE f.person_id = ?1 GROUP BY p.id";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([person_id], |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2)?,
                    created: row.get(5).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: row.get(3).unwrap_or(0.0),
                    longitude: row.get(4).unwrap_or(0.0),
                    favorite: row.get(6).unwrap_or(false),
                    indexed: row.get(7).unwrap_or(2),
                    caption: row.get(8).ok(),
                    aesthetics_score: row.get(9).ok(),
                    ai_status: AiStatus {
                        clip: row.get(10).unwrap_or(0),
                        face: row.get(11).unwrap_or(0),
                        ocr: row.get(12).unwrap_or(0),
                        nsfw: row.get(13).unwrap_or(0),
                        aesthetics: row.get(14).unwrap_or(0),
                        yolo: row.get(15).unwrap_or(0),
                        blip: row.get(16).unwrap_or(0),
                        arcface: row.get(17).unwrap_or(0),
                        midas: row.get(18).unwrap_or(0),
                        whisper: row.get(19).unwrap_or(0),
                        sam: row.get(20).unwrap_or(0),
                        superres: row.get(21).unwrap_or(0),
                    },
                    sync_needed: false,
                    received: false,
                })
            }) {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        photos
    }

    /// List all monitored directory paths.
    pub fn list_directories(&self) -> Vec<String> {
        let mut results = Vec::new();
        if let Ok(mut stm) = self.connection.prepare("SELECT name FROM directory") {
            if let Ok(iter) = stm.query_map((), |row| row.get(0)) {
                for val in iter.flatten() {
                    results.push(val);
                }
            }
        }
        results
    }

    /// Remove a monitored directory path from the config.
    pub fn remove_directory(&self, path: String) {
        if let Err(e) = self
            .connection
            .execute("DELETE FROM directory WHERE name = ?1", [&path])
        {
            tracing::warn!("remove_directory: failed to remove '{path}': {e}");
        }
    }

    /// Add a directory path to the monitored directories list.
    pub fn add_directory(&self, path: &str) {
        if let Err(e) = self
            .connection
            .execute("INSERT INTO directory (name) VALUES(?1)", [&path])
        {
            tracing::warn!("add_directory: failed to add '{path}': {e}");
        }
    }

    /// Mark onboarding as complete so the app does not show first-run setup again
    /// even when no scan directory was configured (e.g. an Android device that only
    /// receives photos over sync).
    pub fn set_onboarding_complete(&self) {
        let mut state = std::collections::HashMap::new();
        state.insert("onboarding_complete".to_string(), "1".to_string());
        self.set_state(state);
    }

    /// Whether onboarding has been completed.
    pub fn is_onboarding_complete(&self) -> bool {
        self.get_state()
            .get("onboarding_complete")
            .map(|v| v == "1")
            .unwrap_or(false)
    }

    /// Whether any photo exists in the library at all.
    pub fn has_any_photos(&self) -> bool {
        self.connection
            .query_row("SELECT EXISTS(SELECT 1 FROM photo)", [], |r| r.get(0))
            .unwrap_or(false)
    }

    /// Merge all faces from `from_id` into `to_id`, then delete the source person.
    pub fn merge_people(&self, from_id: &str, to_id: &str) {
        if let Err(e) = self.connection.execute(
            "UPDATE faces SET person_id = ?1 WHERE person_id = ?2",
            (to_id, from_id),
        ) {
            tracing::warn!("merge_people: failed to update faces from {from_id} to {to_id}: {e}");
            return;
        }
        if let Err(e) = self
            .connection
            .execute("DELETE FROM people WHERE id = ?1", [from_id])
        {
            tracing::warn!("merge_people: failed to delete {from_id}: {e}");
            return;
        }
        self.update_person_centroid(to_id);
    }

    /// Rename a person record.
    pub fn rename_person(&self, id: &str, new_name: &str) {
        if let Err(e) = self
            .connection
            .execute("UPDATE people SET name = ?1 WHERE id = ?2", (new_name, id))
        {
            tracing::warn!("rename_person: failed to rename {id} to {new_name}: {e}");
        }
    }

    /// Remove a directory and all its photos, objects, faces, and properties in a transaction.
    pub fn remove_directory_full(&mut self, path: &str) {
        let tx = match self.connection.transaction() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("remove_directory_full: failed to start transaction: {e}");
                return;
            }
        };

        let mut photo_ids = Vec::new();
        if let Ok(mut stmt) = tx.prepare("SELECT id FROM photo WHERE location LIKE ?1") {
            if let Ok(rows) = stmt.query_map([format!("{path}%")], |row| row.get::<_, String>(0)) {
                for id in rows.flatten() {
                    photo_ids.push(id);
                }
            }
        }
        for id in &photo_ids {
            let _ = tx.execute("DELETE FROM object WHERE photo_id = ?1", [&id]);
            let _ = tx.execute("DELETE FROM faces WHERE photo_id = ?1", [&id]);
            let _ = tx.execute("DELETE FROM properties WHERE photo_id = ?1", [&id]);
            let _ = tx.execute("DELETE FROM photo WHERE id = ?1", [&id]);
        }
        let _ = tx.execute("DELETE FROM directory WHERE name = ?1", [path]);
        if let Err(e) = tx.commit() {
            tracing::warn!("remove_directory_full: failed to commit: {e}");
        }
    }

    /// Delete rows whose media file no longer exists on disk (immediate prune).
    /// Only rows rooted at `dir` are considered. Removing the row drops the file
    /// from the sync manifest, so the next manifest exchange re-requests and
    /// restores it from any peer that still holds a copy. Returns the number of
    /// rows removed.
    pub fn prune_missing_files(&mut self, dir: &str) -> usize {
        let mut gone: Vec<String> = Vec::new();
        let prefix = format!("{dir}{}", std::path::MAIN_SEPARATOR);
        if let Ok(mut stmt) = self
            .connection
            .prepare("SELECT id, location FROM photo WHERE location = ?1 OR location LIKE ?2")
        {
            if let Ok(rows) = stmt.query_map(rusqlite::params![dir, format!("{prefix}%")], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }) {
                for r in rows.flatten() {
                    if !std::path::Path::new(&r.1).exists() {
                        gone.push(r.0);
                    }
                }
            }
        }
        if gone.is_empty() {
            return 0;
        }
        let tx = match self.connection.transaction() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("prune_missing_files: failed to start transaction: {e}");
                return 0;
            }
        };
        let mut removed = 0;
        for id in &gone {
            let _ = tx.execute("DELETE FROM object WHERE photo_id = ?1", [id]);
            let _ = tx.execute("DELETE FROM faces WHERE photo_id = ?1", [id]);
            let _ = tx.execute("DELETE FROM properties WHERE photo_id = ?1", [id]);
            if tx
                .execute("DELETE FROM photo WHERE id = ?1", [id])
                .map(|n| n > 0)
                .unwrap_or(false)
            {
                removed += 1;
            }
        }
        if let Err(e) = tx.commit() {
            tracing::warn!("prune_missing_files: failed to commit: {e}");
        }
        removed
    }

    /// Import a photo with its objects and faces within a transaction (for device sync).
    pub fn import_photo(&mut self, photo: ImportedPhoto<'_>) {
        let tx = match self.connection.transaction() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("import_photo: failed to start transaction: {e}");
                return;
            }
        };

        if let Err(e) = tx.execute(
            "INSERT OR REPLACE INTO photo (id, location, created, latitude, longitude, encoded, caption, aesthetics_score, received) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (photo.id, photo.location, photo.created, photo.latitude, photo.longitude, photo.encoded, photo.caption, photo.aesthetics_score, photo.received),
        ) {
            tracing::warn!("import_photo: failed to upsert photo {}: {e}", photo.id);
            return;
        }

        if let Err(e) = tx.execute("DELETE FROM object WHERE photo_id = ?1", [photo.id]) {
            tracing::warn!(
                "import_photo: failed to clear objects for {}: {e}",
                photo.id
            );
        }
        if let Err(e) = tx.execute("DELETE FROM faces WHERE photo_id = ?1", [photo.id]) {
            tracing::warn!("import_photo: failed to clear faces for {}: {e}", photo.id);
        }

        if let Ok(objects) = serde_json::from_str::<Vec<SyncObject>>(photo.objects_json) {
            for obj in &objects {
                if let Err(e) = tx.execute(
                    "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                    (photo.id, &obj.class, &obj.probability),
                ) {
                    tracing::warn!(
                        "import_photo: failed to insert object for {}: {e}",
                        photo.id
                    );
                }
            }
        }

        if let Ok(faces) = serde_json::from_str::<Vec<SyncFace>>(photo.faces_json) {
            for face in &faces {
                if let Err(e) = tx.execute(
                    "INSERT OR REPLACE INTO faces (photo_id, face_id, crop_path, encoded, person_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (photo.id, &face.face_id, &face.crop_path, &face.encoded, &face.person_id),
                ) {
                    tracing::warn!("import_photo: failed to insert face for {}: {e}", photo.id);
                }
            }
        }

        if let Err(e) = tx.commit() {
            tracing::warn!(
                "import_photo: failed to commit transaction for {}: {e}",
                photo.id
            );
        }
    }

    /// Return (photo_count, video_count) based on file extensions.
    pub fn get_media_counts(&self) -> (i64, i64) {
        let not_video = video_sql_not_like();
        let is_video = video_sql_like();
        let photo_count: i64 = self
            .connection
            .query_row(
                &format!("SELECT COUNT(*) FROM photo WHERE {not_video}"),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let video_count: i64 = self
            .connection
            .query_row(
                &format!("SELECT COUNT(*) FROM photo WHERE {is_video}"),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        (photo_count, video_count)
    }

    /// Return (pending_photo_count, pending_video_count) of items that still need
    /// to be backed up to another device (originals not yet received by a peer).
    pub fn get_pending_sync_counts(&self) -> (i64, i64) {
        let not_video = video_sql_not_like();
        let is_video = video_sql_like();
        let photo_count: i64 = self
            .connection
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM photo WHERE {not_video} AND sync_needed = 1 AND received = 0"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let video_count: i64 = self
            .connection
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM photo WHERE {is_video} AND sync_needed = 1 AND received = 0"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        (photo_count, video_count)
    }
    pub fn get_all_people_with_embeddings(&self) -> Vec<(String, Vec<f32>)> {
        let mut results = Vec::new();
        if let Ok(mut stmt) = self
            .connection
            .prepare("SELECT id, embedding FROM people WHERE embedding IS NOT NULL")
        {
            if let Ok(iter) = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let bytes: Vec<u8> = row.get(1)?;
                let emb: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                    .collect();
                Ok((id, emb))
            }) {
                for p in iter.flatten() {
                    results.push(p);
                }
            }
        }
        results
    }

    /// Batch-insert photos with their properties, ignoring duplicates.
    pub fn store_photo_batch(&mut self, photos: &[Photo]) -> Result<(), String> {
        let tx = self.connection.transaction().map_err(|e| e.to_string())?;
        {
            let mut stmt = tx.prepare_cached("INSERT OR IGNORE INTO photo(id, location, encoded, created, latitude, longitude, indexed, sync_needed) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 1, 1)").map_err(|e| e.to_string())?;
            for p in photos {
                let _ = stmt.execute((
                    &p.id,
                    &p.location,
                    &p.encoded,
                    &p.created,
                    &p.latitude,
                    &p.longitude,
                ));
            }
        }
        {
            let mut prop_stmt = tx
                .prepare_cached(
                    "INSERT OR REPLACE INTO properties(photo_id, key, value) VALUES(?1, ?2, ?3)",
                )
                .map_err(|e| e.to_string())?;
            for p in photos {
                for (key, value) in &p.properties {
                    let _ = prop_stmt.execute((&p.id, key, value));
                }
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }

    /// Set the indexed level for a photo (0=new, 1=metadata, 2=fully processed).
    pub fn update_photo_indexed(&self, id: &str, indexed: i32) {
        if let Err(e) = self
            .connection
            .execute("UPDATE photo SET indexed = ?1 WHERE id = ?2", (indexed, id))
        {
            tracing::warn!("update_photo_indexed: failed for {id}: {e}");
        }
    }

    /// Whether the photo already has a stored thumbnail (non-empty `encoded`).
    pub fn has_thumbnail(&self, id: &str) -> bool {
        self.connection
            .query_row("SELECT encoded FROM photo WHERE id = ?1", [id], |r| {
                r.get::<_, String>(0)
            })
            .map(|encoded| !encoded.is_empty())
            .unwrap_or(false)
    }

    /// Store a generated thumbnail for a photo. Only writes when the photo has no
    /// thumbnail yet. Returns true if the thumbnail was actually stored.
    pub fn update_photo_thumbnail(&self, id: &str, encoded: &str) -> bool {
        self.connection
            .execute(
                "UPDATE photo SET encoded = ?1 WHERE id = ?2 AND (encoded IS NULL OR encoded = '')",
                (encoded, id),
            )
            .map(|affected| affected > 0)
            .unwrap_or(false)
    }

    /// Update caption, aesthetics_score, and indexed level for a photo.
    pub fn clear_sync_needed(&self, id: &str) {
        if let Err(e) = self
            .connection
            .execute("UPDATE photo SET sync_needed = 0 WHERE id = ?1", [id])
        {
            tracing::warn!("clear_sync_needed: failed for {id}: {e}");
        }
    }

    pub fn update_photo_metadata(
        &self,
        id: &str,
        caption: Option<&str>,
        aesthetics_score: Option<f64>,
        indexed: i32,
    ) {
        if let Err(e) = self.connection.execute(
            "UPDATE photo SET caption = ?1, aesthetics_score = ?2, indexed = ?3 WHERE id = ?4",
            (caption, aesthetics_score, indexed, id),
        ) {
            tracing::warn!("update_photo_metadata: failed for {id}: {e}");
        }
    }

    /// Record that a specific ML model has processed a photo.
    pub fn update_ai_status(&self, photo_id: &str, model: &str, status: i32) {
        match model {
            "clip" | "face" | "ocr" | "nsfw" | "aesthetics" | "yolo" | "blip" | "arcface"
            | "midas" | "whisper" | "sam" | "superres" => {}
            _ => {
                tracing::warn!("Invalid model name: {model}");
                return;
            }
        }
        let sql = format!("INSERT INTO ai_status (photo_id, {model}) VALUES (?1, ?2) ON CONFLICT(photo_id) DO UPDATE SET {model} = ?2");
        let _ = self.connection.execute(&sql, (photo_id, status));
    }

    /// Get up to 50 photos that have not yet been fully AI-processed.
    pub fn get_unindexed_photos(&self) -> Vec<Photo> {
        let mut photos = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, p.indexed, p.caption, p.aesthetics_score, 
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres 
            FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.indexed < 2 LIMIT 50";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], |row| {
                Ok(Photo {
                    id: row.get(0)?,
                    location: row.get(1)?,
                    encoded: row.get(2)?,
                    created: row.get(5).unwrap_or_default(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: row.get(3).unwrap_or(0.0),
                    longitude: row.get(4).unwrap_or(0.0),
                    favorite: false,
                    indexed: row.get(6).unwrap_or(0),
                    caption: row.get(7).ok(),
                    aesthetics_score: row.get(8).ok(),
                    ai_status: AiStatus {
                        clip: row.get(9).unwrap_or(0),
                        face: row.get(10).unwrap_or(0),
                        ocr: row.get(11).unwrap_or(0),
                        nsfw: row.get(12).unwrap_or(0),
                        aesthetics: row.get(13).unwrap_or(0),
                        yolo: row.get(14).unwrap_or(0),
                        blip: row.get(15).unwrap_or(0),
                        arcface: row.get(16).unwrap_or(0),
                        midas: row.get(17).unwrap_or(0),
                        whisper: row.get(18).unwrap_or(0),
                        sam: row.get(19).unwrap_or(0),
                        superres: row.get(20).unwrap_or(0),
                    },
                    sync_needed: false,
                    received: false,
                })
            }) {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        photos
    }

    /// Get a paginated batch of unindexed photos.
    pub fn get_unindexed_photos_batch(&self, offset: usize, limit: usize) -> Vec<Photo> {
        let mut photos = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, p.indexed, p.caption, p.aesthetics_score, 
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres 
            FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.indexed < 2 LIMIT ?1 OFFSET ?2";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) =
                stmt.query_map(rusqlite::params![limit as i64, offset as i64], |row| {
                    Ok(Photo {
                        id: row.get(0)?,
                        location: row.get(1)?,
                        encoded: row.get(2)?,
                        created: row.get(5).unwrap_or_default(),
                        objects: HashMap::new(),
                        properties: HashMap::new(),
                        latitude: row.get(3).unwrap_or(0.0),
                        longitude: row.get(4).unwrap_or(0.0),
                        favorite: false,
                        indexed: row.get(6).unwrap_or(0),
                        caption: row.get(7).ok(),
                        aesthetics_score: row.get(8).ok(),
                        ai_status: AiStatus {
                            clip: row.get(9).unwrap_or(0),
                            face: row.get(10).unwrap_or(0),
                            ocr: row.get(11).unwrap_or(0),
                            nsfw: row.get(12).unwrap_or(0),
                            aesthetics: row.get(13).unwrap_or(0),
                            yolo: row.get(14).unwrap_or(0),
                            blip: row.get(15).unwrap_or(0),
                            arcface: row.get(16).unwrap_or(0),
                            midas: row.get(17).unwrap_or(0),
                            whisper: row.get(18).unwrap_or(0),
                            sam: row.get(19).unwrap_or(0),
                            superres: row.get(20).unwrap_or(0),
                        },
                        sync_needed: false,
                        received: false,
                    })
                })
            {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        photos
    }

    /// Get photo IDs that have not been processed by the given model.
    pub fn get_photos_missing_model(&self, model: &str) -> Vec<String> {
        let mut ids = Vec::new();
        match model {
            "clip" | "face" | "ocr" | "nsfw" | "aesthetics" | "yolo" | "blip" | "arcface"
            | "midas" | "whisper" | "sam" | "superres" => {}
            _ => {
                tracing::warn!("Invalid model name: {model}");
                return ids;
            }
        }
        let sql = format!("SELECT p.id FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE s.{model} = 0 OR s.{model} IS NULL");
        if let Ok(mut stmt) = self.connection.prepare(&sql) {
            if let Ok(iter) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for id in iter.flatten() {
                    ids.push(id);
                }
            }
        }
        ids
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
pub struct AiStatus {
    pub clip: i32,
    pub face: i32,
    pub ocr: i32,
    pub nsfw: i32,
    pub aesthetics: i32,
    pub yolo: i32,
    pub blip: i32,
    pub arcface: i32,
    pub midas: i32,
    pub whisper: i32,
    pub sam: i32,
    pub superres: i32,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Photo {
    pub id: String,
    pub location: String,
    pub encoded: String,
    pub created: String,
    pub objects: HashMap<String, f64>,
    pub properties: HashMap<String, String>,
    pub latitude: f64,
    pub longitude: f64,
    pub favorite: bool,
    pub indexed: i32, // 0: new, 1: metadata only, 2: fully processed
    pub caption: Option<String>,
    pub aesthetics_score: Option<f64>,
    pub ai_status: AiStatus,
    /// True until any peer has received the file (drives the "not backed up" badge).
    #[serde(default)]
    pub sync_needed: bool,
    /// True when this row was imported from a peer (a backup copy).
    #[serde(default)]
    pub received: bool,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct MapPoint {
    pub id: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Face {
    pub photo_id: String,
    pub face_id: String,
    pub crop_path: String,
    pub encoded: String,
    pub embedding: Vec<f32>,
    pub person_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonWithFace {
    pub id: String,
    pub name: String,
    pub representative_crop: Option<String>,
    pub representative_face_id: Option<String>,
    pub encoded: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub face_count: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub up_to_date: bool,
    pub host: bool,
    pub photo_count: i64,
    pub video_count: i64,
    pub os: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDevice {
    pub device_id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub device_type: String,
    pub os: String,
    pub models_enabled: Vec<String>,
    pub protocol_version: u8,
    pub storage_used: i64,
    pub storage_capacity: i64,
    pub last_seen: String,
    /// Media received from this peer (running total).
    pub photo_count: i64,
    pub video_count: i64,
}

impl Database {
    pub fn upsert_peer_device(&self, device: &PeerDevice) {
        let _ = self.connection.execute(
            "INSERT INTO peer_device(device_id, name, ip, port, device_type, os, models_enabled, protocol_version, storage_used, storage_capacity, last_seen, photo_count, video_count) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'), ?11, ?12) \
             ON CONFLICT(device_id) DO UPDATE SET \
             name=excluded.name, ip=excluded.ip, port=excluded.port, \
             device_type=excluded.device_type, os=excluded.os, \
             models_enabled=excluded.models_enabled, protocol_version=excluded.protocol_version, \
             storage_used=excluded.storage_used, storage_capacity=excluded.storage_capacity, \
             photo_count=excluded.photo_count, video_count=excluded.video_count, \
             last_seen=datetime('now')",
            rusqlite::params![
                device.device_id, device.name, device.ip, device.port as i32,
                device.device_type, device.os,
                serde_json::to_string(&device.models_enabled).unwrap_or_else(|_| "[]".to_string()),
                device.protocol_version as i32, device.storage_used, device.storage_capacity,
                device.photo_count, device.video_count,
            ],
        );
    }

    pub fn update_peer_device_seen(&self, device_id: &str) {
        let _ = self.connection.execute(
            "UPDATE peer_device SET last_seen = datetime('now') WHERE device_id = ?1",
            rusqlite::params![device_id],
        );
    }

    pub fn update_peer_device_storage(&self, device_id: &str, used: i64, capacity: i64) {
        let _ = self.connection.execute(
            "UPDATE peer_device SET storage_used = ?2, storage_capacity = ?3 WHERE device_id = ?1",
            rusqlite::params![device_id, used, capacity],
        );
    }

    pub fn list_peer_devices(&self) -> Vec<PeerDevice> {
        let mut results = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare(
            "SELECT device_id, name, ip, port, device_type, os, models_enabled, protocol_version, storage_used, storage_capacity, last_seen, photo_count, video_count \
             FROM peer_device ORDER BY last_seen DESC"
        ) {
            if let Ok(iter) = stmt.query_map([], |row| {
                let models_str: String = row.get(6).unwrap_or_else(|_| "[]".to_string());
                let models: Vec<String> = serde_json::from_str(&models_str).unwrap_or_default();
                Ok(PeerDevice {
                    device_id: row.get(0)?,
                    name: row.get(1)?,
                    ip: row.get(2)?,
                    port: row.get::<_, i32>(3).unwrap_or(0) as u16,
                    device_type: row.get(4)?,
                    os: row.get(5)?,
                    models_enabled: models,
                    protocol_version: row.get::<_, i32>(7).unwrap_or(1) as u8,
                    storage_used: row.get(8)?,
                    storage_capacity: row.get(9)?,
                    last_seen: row.get(10)?,
                    photo_count: row.get(11)?,
                    video_count: row.get(12)?,
                })
            }) {
                for d in iter.flatten() {
                    results.push(d);
                }
            }
        }
        results
    }

    pub fn get_peer_device(&self, device_id: &str) -> Option<PeerDevice> {
        self.connection.query_row(
            "SELECT device_id, name, ip, port, device_type, os, models_enabled, protocol_version, storage_used, storage_capacity, last_seen, photo_count, video_count \
             FROM peer_device WHERE device_id = ?1",
            rusqlite::params![device_id],
            |row| {
                let models_str: String = row.get(6).unwrap_or_else(|_| "[]".to_string());
                let models: Vec<String> = serde_json::from_str(&models_str).unwrap_or_default();
                Ok(PeerDevice {
                    device_id: row.get(0)?,
                    name: row.get(1)?,
                    ip: row.get(2)?,
                    port: row.get::<_, i32>(3).unwrap_or(0) as u16,
                    device_type: row.get(4)?,
                    os: row.get(5)?,
                    models_enabled: models,
                    protocol_version: row.get::<_, i32>(7).unwrap_or(1) as u8,
                    storage_used: row.get(8)?,
                    storage_capacity: row.get(9)?,
                    last_seen: row.get(10)?,
                    photo_count: row.get(11)?,
                    video_count: row.get(12)?,
                })
            },
        ).ok()
    }

    /// Rename a peer device by its id. Leaves every other field untouched.
    pub fn rename_peer_device(&self, device_id: &str, new_name: &str) {
        let _ = self.connection.execute(
            "UPDATE peer_device SET name = ?2 WHERE device_id = ?1",
            rusqlite::params![device_id, new_name],
        );
    }

    /// Add to a peer's running photo/video totals.
    pub fn increment_peer_device_counts(&self, device_id: &str, photos: i64, videos: i64) {
        let _ = self.connection.execute(
            "UPDATE peer_device SET photo_count = photo_count + ?2, video_count = video_count + ?3 WHERE device_id = ?1",
            rusqlite::params![device_id, photos, videos],
        );
    }

    pub fn remove_peer_device(&self, device_id: &str) {
        let _ = self.connection.execute(
            "DELETE FROM peer_device WHERE device_id = ?1",
            rusqlite::params![device_id],
        );
    }

    // ---------- Albums (local, free tier) ----------

    /// Create a new empty album and return the persisted row.
    pub fn create_album(&self, name: &str) -> Result<Album, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Album name cannot be empty".to_string());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let sort_order: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM album",
                [],
                |r| r.get(0),
            )
            .unwrap_or(1);
        self.connection
            .execute(
                "INSERT INTO album(id, name, sort_order) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, name, sort_order],
            )
            .map_err(|e| e.to_string())?;
        self.get_album(&id)
            .ok_or_else(|| "Failed to create album".to_string())
    }

    /// Rename an existing album. Rejects blank names.
    pub fn rename_album(&self, album_id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Album name cannot be empty".to_string());
        }
        self.connection
            .execute(
                "UPDATE album SET name = ?1 WHERE id = ?2",
                rusqlite::params![name, album_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Delete an album and all of its items.
    pub fn delete_album(&self, album_id: &str) -> Result<(), String> {
        self.connection
            .execute(
                "DELETE FROM album_item WHERE album_id = ?1",
                rusqlite::params![album_id],
            )
            .map_err(|e| e.to_string())?;
        self.connection
            .execute(
                "DELETE FROM album WHERE id = ?1",
                rusqlite::params![album_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn album_from_row(row: &rusqlite::Row) -> rusqlite::Result<Album> {
        Ok(Album {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            cover_photo_id: row.get(3)?,
            sort_order: row.get(4)?,
            item_count: row.get(5)?,
        })
    }

    /// List all albums ordered by sort_order, with live item counts.
    pub fn list_albums(&self) -> Vec<Album> {
        let sql = "SELECT a.id, a.name, a.created_at, a.cover_photo_id, a.sort_order, \
            (SELECT COUNT(*) FROM album_item WHERE album_id = a.id) AS item_count \
            FROM album a ORDER BY a.sort_order ASC, a.created_at ASC";
        let mut albums = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map([], Self::album_from_row) {
                for album in iter.flatten() {
                    albums.push(album);
                }
            }
        }
        albums
    }

    /// Fetch a single album by id, or None if it does not exist.
    pub fn get_album(&self, album_id: &str) -> Option<Album> {
        let sql = "SELECT a.id, a.name, a.created_at, a.cover_photo_id, a.sort_order, \
            (SELECT COUNT(*) FROM album_item WHERE album_id = a.id) AS item_count \
            FROM album a WHERE a.id = ?1";
        self.connection
            .query_row(sql, rusqlite::params![album_id], Self::album_from_row)
            .ok()
    }

    /// Add photos to an album, appending them in position order. Duplicates are
    /// ignored (album_item PK is (album_id, photo_id)). The cover is updated to
    /// the last photo actually added.
    pub fn add_album_items(&self, album_id: &str, photo_ids: &[String]) -> Result<(), String> {
        let album_exists: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM album WHERE id = ?1)",
                rusqlite::params![album_id],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if !album_exists {
            return Err(format!("Album '{album_id}' does not exist"));
        }
        let mut max_position: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(position), 0) FROM album_item WHERE album_id = ?1",
                rusqlite::params![album_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let mut last_added: Option<String> = None;
        for photo_id in photo_ids {
            let inserted = self
                .connection
                .execute(
                    "INSERT OR IGNORE INTO album_item(album_id, photo_id, position) VALUES (?1, ?2, ?3)",
                    rusqlite::params![album_id, photo_id, max_position],
                )
                .map_err(|e| e.to_string())?;
            if inserted > 0 {
                max_position += 1;
                last_added = Some(photo_id.clone());
            }
        }
        if let Some(ref cover) = last_added {
            let _ = self.connection.execute(
                "UPDATE album SET cover_photo_id = ?1 WHERE id = ?2",
                rusqlite::params![cover, album_id],
            );
        }
        Ok(())
    }

    /// Remove photos from an album. If the current cover photo is removed, the
    /// most recently added remaining photo becomes the new cover.
    pub fn remove_album_items(&self, album_id: &str, photo_ids: &[String]) -> Result<(), String> {
        for photo_id in photo_ids {
            self.connection
                .execute(
                    "DELETE FROM album_item WHERE album_id = ?1 AND photo_id = ?2",
                    rusqlite::params![album_id, photo_id],
                )
                .map_err(|e| e.to_string())?;
        }
        let current_cover: Option<String> = self
            .connection
            .query_row(
                "SELECT cover_photo_id FROM album WHERE id = ?1",
                rusqlite::params![album_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        if let Some(ref cover) = current_cover {
            if photo_ids.contains(cover) {
                let new_cover: Option<String> = self
                    .connection
                    .query_row(
                        "SELECT photo_id FROM album_item WHERE album_id = ?1 \
                         ORDER BY position DESC, added_at DESC LIMIT 1",
                        rusqlite::params![album_id],
                        |r| r.get(0),
                    )
                    .ok()
                    .flatten();
                let _ = self.connection.execute(
                    "UPDATE album SET cover_photo_id = ?1 WHERE id = ?2",
                    rusqlite::params![new_cover, album_id],
                );
            }
        }
        Ok(())
    }

    /// Reorder photos within an album. `ordered_ids` should contain the album's
    /// photo ids in the new order; ids not present in the album are ignored.
    pub fn reorder_album(&self, album_id: &str, ordered_ids: &[String]) -> Result<(), String> {
        for (position, photo_id) in ordered_ids.iter().enumerate() {
            self.connection
                .execute(
                    "UPDATE album_item SET position = ?1 WHERE album_id = ?2 AND photo_id = ?3",
                    rusqlite::params![position as i64, album_id, photo_id],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Paginated album contents ordered by album position (manual order), with
    /// the newest-added photo first on ties.
    pub fn get_album_contents(&self, album_id: &str, offset: usize, limit: usize) -> Vec<Photo> {
        let mut photos = Vec::new();
        let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, \
            EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, \
            p.caption, p.aesthetics_score, \
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, \
            s.whisper, s.sam, s.superres, p.sync_needed, p.received \
            FROM photo p \
            JOIN album_item ai ON ai.photo_id = p.id \
            LEFT JOIN ai_status s ON p.id = s.photo_id \
            WHERE ai.album_id = ?3 \
            ORDER BY ai.position ASC, ai.added_at DESC \
            LIMIT ?1, ?2";
        if let Ok(mut stmt) = self.connection.prepare(sql) {
            if let Ok(iter) = stmt.query_map(
                rusqlite::params![offset as i64, limit as i64, album_id],
                |row| {
                    Ok(Photo {
                        id: row.get(0)?,
                        location: row.get(1)?,
                        encoded: row.get(2)?,
                        created: row.get(5).unwrap_or_default(),
                        objects: HashMap::new(),
                        properties: HashMap::new(),
                        latitude: row.get(3).unwrap_or(0.0),
                        longitude: row.get(4).unwrap_or(0.0),
                        favorite: row.get(6).unwrap_or(false),
                        indexed: row.get(7).unwrap_or(0),
                        caption: row.get(8).ok(),
                        aesthetics_score: row.get(9).ok(),
                        ai_status: AiStatus {
                            clip: row.get(10).unwrap_or(0),
                            face: row.get(11).unwrap_or(0),
                            ocr: row.get(12).unwrap_or(0),
                            nsfw: row.get(13).unwrap_or(0),
                            aesthetics: row.get(14).unwrap_or(0),
                            yolo: row.get(15).unwrap_or(0),
                            blip: row.get(16).unwrap_or(0),
                            arcface: row.get(17).unwrap_or(0),
                            midas: row.get(18).unwrap_or(0),
                            whisper: row.get(19).unwrap_or(0),
                            sam: row.get(20).unwrap_or(0),
                            superres: row.get(21).unwrap_or(0),
                        },
                        sync_needed: row.get(22).unwrap_or(false),
                        received: row.get(23).unwrap_or(false),
                    })
                },
            ) {
                for photo in iter.flatten() {
                    photos.push(photo);
                }
            }
        }
        self.enrich_objects(&mut photos);
        self.enrich_properties(&mut photos);
        photos
    }

    /// Number of items currently in an album (0 if the album does not exist).
    pub fn album_item_count(&self, album_id: &str) -> i64 {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM album_item WHERE album_id = ?1",
                rusqlite::params![album_id],
                |r| r.get(0),
            )
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};
    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_db() -> Database {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("siegu_test_{}_{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        Database::new(&dir.display().to_string())
    }

    #[test]
    fn test_database_creation() {
        let db = test_db();
        let (photos, videos) = db.get_media_counts();
        assert!(photos >= 0);
        assert!(videos >= 0);
    }

    #[test]
    fn test_state_set_get() {
        let db = test_db();
        let mut state = HashMap::new();
        state.insert("theme".to_string(), "dark".to_string());
        state.insert("tier".to_string(), "paid".to_string());
        db.set_state(state);

        let config = db.get_state();
        assert_eq!(config.get("theme").unwrap(), "dark");
        assert_eq!(config.get("tier").unwrap(), "paid");
    }

    #[test]
    fn test_state_overwrite() {
        let db = test_db();
        let mut state = HashMap::new();
        state.insert("theme".to_string(), "dark".to_string());
        db.set_state(state);

        let mut state2 = HashMap::new();
        state2.insert("theme".to_string(), "light".to_string());
        db.set_state(state2);

        let config = db.get_state();
        assert_eq!(config.get("theme").unwrap(), "light");
    }

    #[test]
    fn test_add_list_remove_directory() {
        let db = test_db();
        db.add_directory("/home/test/photos");
        db.add_directory("/home/test/videos");

        let dirs = db.list_directories();
        assert!(dirs.contains(&"/home/test/photos".to_string()));
        assert!(dirs.contains(&"/home/test/videos".to_string()));

        db.remove_directory("/home/test/photos".to_string());
        let dirs = db.list_directories();
        assert!(!dirs.contains(&"/home/test/photos".to_string()));
        assert!(dirs.contains(&"/home/test/videos".to_string()));
    }

    #[test]
    fn test_log_store_retrieve() {
        let db = test_db();
        db.store_log("info", "Test message 1");
        db.store_log("error", "Test error");
        db.store_log("info", "Test message 2");

        let logs = db.get_logs(10);
        assert!(logs.len() >= 3);
        assert!(logs.iter().any(|l| l.message.contains("Test error")));
    }

    #[test]
    fn test_clear_logs() {
        let db = test_db();
        db.store_log("info", "to be cleared");
        assert!(!db.get_logs(10).is_empty());

        db.clear_logs();
        assert!(db.get_logs(10).is_empty());
    }

    #[test]
    fn test_check_integrity() {
        let db = test_db();
        assert!(db.check_integrity().unwrap_or(false));
    }

    #[test]
    fn test_toggle_favorite() {
        let mut db = test_db();
        let photo = Photo {
            id: "test_fav_1".to_string(),
            location: "/tmp/test_fav.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[photo]);

        let result = db.toggle_favorite("test_fav_1");
        assert!(result);
        let p = db.get_photo_by_id("test_fav_1").unwrap();
        assert!(p.favorite);

        let result = db.toggle_favorite("test_fav_1");
        assert!(!result);
        let p = db.get_photo_by_id("test_fav_1").unwrap();
        assert!(!p.favorite);
    }

    #[test]
    fn test_store_and_get_photo_properties() {
        let mut db = test_db();
        let mut props = HashMap::new();
        props.insert("Make".to_string(), "Apple".to_string());
        props.insert("Model".to_string(), "iPhone 15".to_string());

        let photo = Photo {
            id: "test_props_1".to_string(),
            location: "/tmp/test_props.jpg".to_string(),
            encoded: String::new(),
            created: "2024-06-01".to_string(),
            objects: HashMap::new(),
            properties: props,
            latitude: 40.7128,
            longitude: -74.0060,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[photo]);

        let p = db.get_photo_by_id("test_props_1").unwrap();
        assert_eq!(p.latitude, 40.7128);
        assert_eq!(p.longitude, -74.0060);
    }

    #[test]
    fn test_get_heatmap_points() {
        let mut db = test_db();
        let photo = Photo {
            id: "test_heat_1".to_string(),
            location: "/tmp/test_heat.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 52.3676,
            longitude: 4.9041,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[photo]);
        let points = db.get_heatmap_points();
        assert!(points.iter().any(|p| p.id == "test_heat_1"));
    }

    #[test]
    fn test_peer_device_upsert_and_list() {
        let db = test_db();
        db.upsert_peer_device(&PeerDevice {
            device_id: "test-id".to_string(),
            name: "test-device".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 0,
            device_type: String::new(),
            os: "android".to_string(),
            models_enabled: vec![],
            protocol_version: 1,
            storage_used: 0,
            storage_capacity: 0,
            last_seen: String::new(),
            photo_count: 0,
            video_count: 0,
        });
        let peers = db.list_peer_devices();
        assert!(peers.iter().any(|p| p.name == "test-device"));
    }

    #[test]
    fn test_peer_device_rename_and_counts() {
        let db = test_db();
        db.upsert_peer_device(&PeerDevice {
            device_id: "test-id".to_string(),
            name: "old-name".to_string(),
            ip: "192.168.1.1".to_string(),
            port: 0,
            device_type: String::new(),
            os: "android".to_string(),
            models_enabled: vec![],
            protocol_version: 1,
            storage_used: 0,
            storage_capacity: 0,
            last_seen: String::new(),
            photo_count: 0,
            video_count: 0,
        });

        db.rename_peer_device("test-id", "new-name");
        let peer = db.get_peer_device("test-id").expect("peer exists");
        assert_eq!(peer.name, "new-name");
        assert_eq!(peer.os, "android");

        db.increment_peer_device_counts("test-id", 3, 2);
        let peer = db.get_peer_device("test-id").expect("peer exists");
        assert_eq!(peer.photo_count, 3);
        assert_eq!(peer.video_count, 2);

        db.increment_peer_device_counts("test-id", 1, 0);
        let peer = db.get_peer_device("test-id").expect("peer exists");
        assert_eq!(peer.photo_count, 4);
        assert_eq!(peer.video_count, 2);
    }

    #[test]
    fn test_is_video_path() {
        assert!(super::is_video_path("/DCIM/video.mp4"));
        assert!(super::is_video_path("/clip.MOV"));
        assert!(!super::is_video_path("/DCIM/photo.jpg"));
        assert!(!super::is_video_path("/photo.JPEG"));
    }

    #[test]
    fn test_last_scan_time() {
        let db = test_db();
        assert!(db.get_last_scan_time().is_none());

        db.set_last_scan_time("2024-06-01T12:00:00Z".to_string());
        assert_eq!(db.get_last_scan_time().unwrap(), "2024-06-01T12:00:00Z");
    }

    #[test]
    fn test_update_photo_indexed() {
        let mut db = test_db();
        let photo = Photo {
            id: "idx_1".to_string(),
            location: "/tmp/test_idx.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[photo]);

        db.update_photo_indexed("idx_1", 2);
        let loaded = db.get_photo_by_id("idx_1").expect("exists");
        assert_eq!(loaded.indexed, 2);
    }

    #[test]
    fn test_update_ai_status_and_missing_model() {
        let mut db = test_db();
        let photo = Photo {
            id: "ai_1".to_string(),
            location: "/tmp/test_ai.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[photo]);

        let missing = db.get_photos_missing_model("clip");
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "ai_1");

        db.update_ai_status("ai_1", "clip", 1);
        let missing = db.get_photos_missing_model("clip");
        assert!(missing.is_empty());
    }

    #[test]
    fn test_get_unindexed_photos() {
        let mut db = test_db();
        let mut p1 = Photo {
            id: "unidx_1".to_string(),
            location: "/tmp/test_unidx1.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let mut p2 = Photo {
            id: "unidx_2".to_string(),
            location: "/tmp/test_unidx2.jpg".to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 2,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        };
        let _ = db.store_photo_batch(&[p1, p2]);

        db.update_photo_indexed("unidx_1", 0);
        db.update_photo_indexed("unidx_2", 2);

        let unindexed = db.get_unindexed_photos();
        assert_eq!(unindexed.len(), 1);
        assert_eq!(unindexed[0].id, "unidx_1");
    }

    #[test]
    fn test_get_unindexed_photos_batch() {
        let mut db = test_db();
        let photos: Vec<Photo> = (0..10)
            .map(|i| Photo {
                id: format!("batch_{i}"),
                location: format!("/tmp/test_batch_{i}.jpg"),
                encoded: String::new(),
                created: "2024-01-01".to_string(),
                objects: HashMap::new(),
                properties: HashMap::new(),
                latitude: 0.0,
                longitude: 0.0,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: AiStatus::default(),
                sync_needed: true,
                received: false,
            })
            .collect();
        let _ = db.store_photo_batch(&photos);

        let batch1 = db.get_unindexed_photos_batch(0, 3);
        assert_eq!(batch1.len(), 3);

        let batch2 = db.get_unindexed_photos_batch(3, 3);
        assert_eq!(batch2.len(), 3);

        let batch_end = db.get_unindexed_photos_batch(9, 3);
        assert_eq!(batch_end.len(), 1);

        let batch_past = db.get_unindexed_photos_batch(20, 3);
        assert!(batch_past.is_empty());
    }

    #[test]
    fn test_update_ai_status_invalid_model() {
        let db = test_db();
        db.update_ai_status("photo_1", "invalid_model", 1);
    }

    #[test]
    fn test_video_sql_like_contains_all_extensions() {
        let sql = super::video_sql_like();
        for ext in super::scanner::VIDEO_EXTENSIONS {
            assert!(sql.contains(ext), "SQL should contain extension: {ext}");
        }
    }

    #[test]
    fn test_video_sql_not_like() {
        let sql = super::video_sql_not_like();
        assert!(sql.starts_with("NOT ("), "should start with NOT (");
        assert!(sql.ends_with(')'), "should end with )");
    }

    #[test]
    fn test_get_media_counts_includes_all_video_extensions() {
        let mut db = test_db();
        let extensions = [
            "mp4", "mkv", "mov", "avi", "webm", "flv", "wmv", "m4v", "3gp",
        ];
        let mut photos: Vec<Photo> = extensions
            .iter()
            .enumerate()
            .map(|(i, ext)| Photo {
                id: format!("vid_{i}"),
                location: format!("/tmp/video_{i}.{ext}"),
                encoded: String::new(),
                created: String::new(),
                objects: HashMap::new(),
                properties: HashMap::new(),
                latitude: 0.0,
                longitude: 0.0,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: AiStatus::default(),
                sync_needed: true,
                received: false,
            })
            .collect();
        photos.push(Photo {
            id: "img_1".to_string(),
            location: "/tmp/photo.jpg".to_string(),
            encoded: String::new(),
            created: String::new(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        });
        let _ = db.store_photo_batch(&photos);

        let (photo_count, video_count) = db.get_media_counts();
        assert_eq!(
            video_count, 9,
            "should count all 9 video extensions, got {video_count}"
        );
        assert_eq!(photo_count, 1, "should count 1 photo, got {photo_count}");
    }

    #[test]
    fn test_get_pending_sync_counts_filters_received() {
        let mut db = test_db();
        let photos: Vec<Photo> = (0..6)
            .map(|i| Photo {
                id: format!("pending_{i}"),
                location: if i % 2 == 0 {
                    format!("/tmp/p_{i}.jpg")
                } else {
                    format!("/tmp/p_{i}.mp4")
                },
                encoded: String::new(),
                created: String::new(),
                objects: HashMap::new(),
                properties: HashMap::new(),
                latitude: 0.0,
                longitude: 0.0,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: AiStatus::default(),
                sync_needed: true,
                received: false,
            })
            .collect();
        let _ = db.store_photo_batch(&photos);
        db.import_photo(ImportedPhoto {
            id: "pending_0",
            location: "/tmp/p_0.jpg",
            created: "2024-01-01",
            latitude: None,
            longitude: None,
            objects_json: "[]",
            faces_json: "[]",
            encoded: "",
            caption: None,
            aesthetics_score: None,
            received: true,
        });
        db.clear_sync_needed("pending_1");

        let (pending_photos, pending_videos) = db.get_pending_sync_counts();
        assert_eq!(pending_photos, 2, "should count 2 pending photos");
        assert_eq!(pending_videos, 2, "should count 2 pending videos");
    }

    #[test]
    fn test_list_photos_videos_only() {
        let mut db = test_db();
        let photos: Vec<Photo> = (0..5)
            .map(|i| {
                let ext = if i % 2 == 0 { "mp4" } else { "jpg" };
                Photo {
                    id: format!("media_{i}"),
                    location: format!("/tmp/file_{i}.{ext}"),
                    encoded: String::new(),
                    created: String::new(),
                    objects: HashMap::new(),
                    properties: HashMap::new(),
                    latitude: 0.0,
                    longitude: 0.0,
                    favorite: false,
                    indexed: 0,
                    caption: None,
                    aesthetics_score: None,
                    ai_status: AiStatus::default(),
                    sync_needed: true,
                    received: false,
                }
            })
            .collect();
        let _ = db.store_photo_batch(&photos);

        let all = db.list_photos("", 0, 100, false, false);
        assert_eq!(all.len(), 5);

        let videos = db.list_photos("", 0, 100, false, true);
        assert_eq!(videos.len(), 3, "should return only mp4 files");
        for v in &videos {
            assert!(v.location.ends_with(".mp4"), "expected mp4: {}", v.location);
        }
    }

    #[test]
    fn test_list_photos_facet_filters() {
        let mut db = test_db();
        db.connection
            .execute(
                "INSERT INTO people (id, name) VALUES ('person-1', 'Alice')",
                (),
            )
            .unwrap();
        for (id, created) in [
            ("p1", "2026-03-05"),
            ("p2", "2026-02-10"),
            ("p3", "2025-12-20"),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, '')",
                    (id, format!("/{id}.jpg"), created),
                )
                .unwrap();
        }
        db.connection
            .execute(
                "INSERT INTO faces (photo_id, face_id, crop_path, encoded, person_id) VALUES ('p1', 'f1', '', 'enc', 'person-1')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p2', 'beach', '0.9')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'location_name', 'Paris, France')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p3', 'location_name', 'Tokyo, Japan')",
                (),
            )
            .unwrap();

        let filter = |f: PhotoFilter| db.list_photos_filtered("", 0, 100, false, false, &f);

        let by_person = filter(PhotoFilter {
            person_id: Some("person-1".into()),
            ..Default::default()
        });
        assert_eq!(by_person.len(), 1);
        assert_eq!(by_person[0].id, "p1");

        let by_location = filter(PhotoFilter {
            location: Some("Tokyo, Japan".into()),
            ..Default::default()
        });
        assert_eq!(by_location.len(), 1);
        assert_eq!(by_location[0].id, "p3");

        let by_tag = filter(PhotoFilter {
            tag: Some("beach".into()),
            ..Default::default()
        });
        assert_eq!(by_tag.len(), 1);
        assert_eq!(by_tag[0].id, "p2");

        let by_date = filter(PhotoFilter {
            date_from: Some("2026-01-01".into()),
            date_to: Some("2026-03-31".into()),
            ..Default::default()
        });
        assert_eq!(by_date.len(), 2);

        let combined = filter(PhotoFilter {
            person_id: Some("person-1".into()),
            location: Some("Paris, France".into()),
            ..Default::default()
        });
        assert_eq!(combined.len(), 1);

        let empty = filter(PhotoFilter {
            person_id: Some("person-1".into()),
            tag: Some("beach".into()),
            ..Default::default()
        });
        assert!(empty.is_empty());
    }

    #[test]
    fn test_search_facet_counts() {
        let mut db = test_db();
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES ('p1', '/a.jpg', '2026-03-01', '')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES ('p2', '/b.jpg', '2026-03-15', '')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES ('p3', '/c.mp4', '2026-02-01', '')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'location_name', 'Paris, France')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p2', 'location_name', 'Paris, France')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'favorite', 'true')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p1', 'cat', '0.9')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p2', 'cat', '0.8')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO ocr (photo_id, text) VALUES ('p2', 'hello')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO people (id, name) VALUES ('person-1', 'Alice')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO faces (photo_id, face_id, person_id) VALUES ('p1', 'f1', 'person-1')",
                (),
            )
            .unwrap();

        let locations = db.get_location_counts(10);
        assert_eq!(locations, vec![("Paris, France".to_string(), 2)]);

        let tags = db.get_tag_counts(10);
        assert_eq!(tags, vec![("cat".to_string(), 2)]);

        let months = db.get_month_counts(10);
        assert_eq!(
            months,
            vec![("2026-03".to_string(), 2), ("2026-02".to_string(), 1)]
        );

        let people = db.get_search_people(10);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].name, "Alice");
        assert_eq!(people[0].photo_count, 1);

        let stats = db.get_search_stats();
        assert_eq!(stats.photos, 3);
        assert_eq!(stats.videos, 1);
        assert_eq!(stats.favorites, 1);
        assert_eq!(stats.ocr_photos, 1);
        assert_eq!(stats.faces, 1);
        assert_eq!(stats.named_people, 1);
        assert_eq!(stats.face_photos, 1);
    }

    #[test]
    fn test_get_photo_ocr_concatenates_rows() {
        let mut db = test_db();
        let _ = db.connection.execute(
            "INSERT INTO photo (id, location, created, encoded) VALUES ('p1', '/a.jpg', '2026-01-01', '')",
            (),
        );
        let _ = db.connection.execute(
            "INSERT INTO ocr (photo_id, text) VALUES ('p1', 'hello'), ('p1', 'world')",
            (),
        );
        let result = db.get_photo_ocr("p1");
        assert_eq!(result, "hello world");
        assert_eq!(db.get_photo_ocr("missing"), "");
    }

    #[test]
    fn test_discovery_photo_rails_and_groups() {
        let mut db = test_db();
        for (id, created, aesthetics) in [
            ("p1", "2026-03-01", Some(0.9)),
            ("p2", "2026-02-10", Some(0.5)),
            ("p3", "2026-01-05", None),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded, aesthetics_score) VALUES (?1, ?2, ?3, '', ?4)",
                    (id, format!("/{id}.jpg"), created, aesthetics),
                )
                .unwrap();
        }
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p2', 'favorite', 'true')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'location_name', 'Paris, France')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'Make', 'Apple')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'Model', 'iPhone 15 Pro')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p1', 'a receipt', '0.9')",
                (),
            )
            .unwrap();

        let best = db.get_best_photos(10);
        assert_eq!(best.len(), 2);
        assert_eq!(best[0].id, "p1");
        assert_eq!(best[0].aesthetics_score, Some(0.9));

        let favs = db.get_favorite_photos(10);
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].id, "p2");
        assert!(favs[0].favorite);

        let recent = db.get_recent_photos(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].id, "p1");

        let papers = db.get_paper_counts(10);
        assert_eq!(papers, vec![("a receipt".to_string(), 1)]);

        let cameras = db.get_camera_counts(10);
        assert_eq!(cameras, vec![("apple".to_string(), 1)]);

        let groups = db.get_location_groups(10);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Paris, France");
        assert_eq!(groups[0].count, 1);
    }

    #[test]
    fn test_get_best_photos_one_per_day() {
        let mut db = test_db();
        for (id, created, aesthetics) in [
            ("d1a", "2026-03-10 09:00:00", Some(0.7)),
            ("d1b", "2026-03-10 18:00:00", Some(0.95)),
            ("d1c", "2026-03-10 12:00:00", Some(0.8)),
            ("d2", "2026-03-09", Some(0.5)),
            ("d3", "2026-03-08", None),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded, aesthetics_score) VALUES (?1, ?2, ?3, '', ?4)",
                    (id, format!("/{id}.jpg"), created, aesthetics),
                )
                .unwrap();
        }

        let best = db.get_best_photos(10);
        assert_eq!(
            best.len(),
            2,
            "one best shot per day, skipping days without scores"
        );
        assert_eq!(
            best[0].id, "d1b",
            "best-scored photo of the most recent day"
        );
        assert_eq!(best[0].aesthetics_score, Some(0.95));
        assert_eq!(best[1].id, "d2");
    }

    #[test]
    fn test_get_day_counts() {
        let mut db = test_db();
        for (id, created, location) in [
            ("p1", "2026-03-01T10:00:00", "/a.jpg"),
            ("p2", "2026-03-01T18:00:00", "/b.jpg"),
            ("v1", "2026-03-02T09:00:00", "/c.MP4"),
            ("p3", "2026-04-10T09:00:00", "/d.jpg"),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, '')",
                    (id, location, created),
                )
                .unwrap();
        }

        let counts = db.get_day_counts("2026-03-01", "2026-03-31");
        assert_eq!(counts.len(), 2);
        assert_eq!(counts[0].date, "2026-03-01");
        assert_eq!(counts[0].photos, 2);
        assert_eq!(counts[0].videos, 0);
        assert_eq!(counts[1].date, "2026-03-02");
        assert_eq!(counts[1].photos, 1);
        assert_eq!(counts[1].videos, 1);

        let empty = db.get_day_counts("2025-01-01", "2025-01-31");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_list_photos_new_facet_filters() {
        let mut db = test_db();
        for (id, created, aesthetics) in [
            ("p1", "2026-03-01", Some(0.9)),
            ("p2", "2026-02-10", Some(0.4)),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded, aesthetics_score) VALUES (?1, ?2, ?3, '', ?4)",
                    (id, format!("/{id}.jpg"), created, aesthetics),
                )
                .unwrap();
        }
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'favorite', 'true')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'Make', 'Sony')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO faces (photo_id, face_id) VALUES ('p1', 'f1')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p1', 'a screenshot', '0.9')",
                (),
            )
            .unwrap();

        let filter = |f: PhotoFilter| db.list_photos_filtered("", 0, 100, false, false, &f);

        let favs = filter(PhotoFilter {
            favorite: true,
            ..Default::default()
        });
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].id, "p1");

        let faces = filter(PhotoFilter {
            has_faces: true,
            ..Default::default()
        });
        assert_eq!(faces.len(), 1);

        let quality = filter(PhotoFilter {
            aesthetics_min: Some(0.6),
            ..Default::default()
        });
        assert_eq!(quality.len(), 1);
        assert_eq!(quality[0].id, "p1");

        let sony = filter(PhotoFilter {
            camera: Some("Sony".into()),
            ..Default::default()
        });
        assert_eq!(sony.len(), 1);

        let papers = filter(PhotoFilter {
            papers: true,
            ..Default::default()
        });
        assert_eq!(papers.len(), 1);

        let random = filter(PhotoFilter {
            random: true,
            ..Default::default()
        });
        assert_eq!(random.len(), 2);
    }

    fn make_photo(id: &str, location: &str) -> Photo {
        Photo {
            id: id.to_string(),
            location: location.to_string(),
            encoded: String::new(),
            created: "2024-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 0,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: true,
            received: false,
        }
    }

    #[test]
    fn test_has_thumbnail_and_update_photo_thumbnail() {
        let mut db = test_db();
        let _ = db.store_photo_batch(&[make_photo("thumb_1", "/tmp/thumb.jpg")]);

        assert!(!db.has_thumbnail("thumb_1"), "fresh photo has no thumbnail");
        assert!(
            db.update_photo_thumbnail("thumb_1", "data:image/jpeg;base64,abc"),
            "first thumbnail write should succeed"
        );
        assert!(db.has_thumbnail("thumb_1"));

        let photo = db.get_photo_by_id("thumb_1").unwrap();
        assert_eq!(photo.encoded, "data:image/jpeg;base64,abc");

        assert!(
            !db.update_photo_thumbnail("thumb_1", "data:image/jpeg;base64,overwrite"),
            "existing thumbnail must not be overwritten"
        );
        let photo = db.get_photo_by_id("thumb_1").unwrap();
        assert_eq!(photo.encoded, "data:image/jpeg;base64,abc");
    }

    #[test]
    fn test_get_photo_sync_info_includes_unindexed_and_received() {
        let mut db = test_db();
        let _ = db.store_photo_batch(&[
            make_photo("sync_plain", "/home/test/plain.jpg"),
            make_photo("sync_indexed", "/home/test/indexed.jpg"),
            make_photo("sync_siegu", "/home/test/siegu/received.jpg"),
        ]);
        db.import_photo(ImportedPhoto {
            id: "sync_indexed",
            location: "/home/test/indexed.jpg",
            created: "2024-01-01",
            latitude: None,
            longitude: None,
            objects_json: r#"[{"class":"person","probability":0.9}]"#,
            faces_json: "[]",
            encoded: "",
            caption: None,
            aesthetics_score: None,
            received: true,
        });

        let info = db.get_photo_sync_info();
        let ids: Vec<&str> = info.iter().map(|p| p.id.as_str()).collect();

        assert!(
            ids.contains(&"sync_plain"),
            "unindexed photo must appear in sync info immediately"
        );
        assert!(ids.contains(&"sync_indexed"));
        assert!(
            ids.contains(&"sync_siegu"),
            "received files under /siegu/ are part of this device's library and must be in the manifest"
        );
    }

    #[test]
    fn test_prune_missing_files_removes_only_gone_rows() {
        let mut db = test_db();
        let dir = std::env::temp_dir().join(format!(
            "siegu_prune_{}_{}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        let existing = dir.join("still_here.jpg");
        fs::write(&existing, b"jpeg").expect("create existing file");

        let existing_path = existing.display().to_string();
        let gone_path = dir.join("deleted.jpg").display().to_string();
        let _ = db.store_photo_batch(&[
            make_photo("prune_existing", &existing_path),
            make_photo("prune_gone", &gone_path),
        ]);

        let removed = db.prune_missing_files(&dir.display().to_string());
        assert_eq!(removed, 1, "only the deleted file's row should be pruned");
        assert!(db.get_photo_by_id("prune_existing").is_some());
        assert!(db.get_photo_by_id("prune_gone").is_none());
    }

    #[test]
    fn test_ingest_marks_sync_needed_and_import_marks_received() {
        let mut db = test_db();
        let _ = db.store_photo_batch(&[make_photo("orig_1", "/tmp/orig.jpg")]);
        db.import_photo(ImportedPhoto {
            id: "recv_1",
            location: "/tmp/siegu/recv.jpg",
            created: "2024-01-01",
            latitude: None,
            longitude: None,
            objects_json: "[]",
            faces_json: "[]",
            encoded: "",
            caption: None,
            aesthetics_score: None,
            received: true,
        });

        let pending: i64 = db
            .connection
            .query_row(
                "SELECT COUNT(*) FROM photo WHERE sync_needed = 1 AND received = 0",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(pending, 1, "only the scanned original awaits sync");

        let received: i64 = db
            .connection
            .query_row("SELECT COUNT(*) FROM photo WHERE received = 1", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        assert_eq!(received, 1, "imported photo must be flagged as received");
    }

    #[test]
    fn test_onboarding_complete_flag() {
        let db = test_db();
        assert!(!db.is_onboarding_complete(), "fresh DB is not initialized");

        db.set_onboarding_complete();
        assert!(db.is_onboarding_complete());
    }

    #[test]
    fn test_legacy_config_numeric_value_reads_as_string() {
        let db = test_db();
        db.connection.execute("DROP TABLE config", ()).unwrap();
        db.connection
            .execute("CREATE TABLE config(key STRING, value STRING)", ())
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO config (key, value) VALUES('session_port', '35225')",
                (),
            )
            .unwrap();
        assert_eq!(
            db.get_state().get("session_port").map(String::as_str),
            Some("35225")
        );
    }

    #[test]
    fn test_has_any_photos() {
        let mut db = test_db();
        assert!(!db.has_any_photos(), "fresh DB has no photos");

        let _ = db.store_photo_batch(&[make_photo("p_any", "/tmp/any.jpg")]);
        assert!(db.has_any_photos());
    }
}
