use std::{
    collections::HashMap,
    fs::{self},
};

use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SearchSuggestion {
    pub title: String,
    #[serde(rename = "type")]
    pub suggestion_type: String,
}

pub struct Database {
    pub connection: Connection,
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

fn month_name_to_like(query: &str) -> Option<String> {
    let q_lower = query.to_lowercase();
    for &(num, full, abbr) in MONTH_NAMES {
        if q_lower.contains(full) || q_lower.contains(abbr) {
            return Some(format!("%-{:02}-%", num));
        }
    }
    None
}
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

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SyncObject {
    pub class: String,
    pub probability: String,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SyncFace {
    pub face_id: String,
    pub crop_path: String,
    pub encoded: String,
    pub person_id: Option<String>,
}

pub struct ImportedPhoto<'a> {
    pub id: &'a str,
    pub location: &'a str,
    pub created: &'a str,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub objects_json: &'a str,
    pub faces_json: &'a str,
    pub encoded: &'a str,
}

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
    pub fn get_photo_sync_info(&self) -> Vec<PhotoSyncInfo> {
        let mut results = Vec::new();
        // Only select photos that have been indexed (have at least one entry in object or faces table)
        // AND are NOT inside a 'siegu' folder (to prevent re-syncing synced files)
        let sql = "SELECT id, location, created, latitude, longitude, caption, aesthetics_score FROM photo p 
                   WHERE (EXISTS (SELECT 1 FROM object WHERE photo_id = p.id) 
                   OR EXISTS (SELECT 1 FROM faces WHERE photo_id = p.id))
                   AND p.location NOT LIKE '%/siegu/%'
                   AND p.location NOT LIKE '%\\siegu\\%'";
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
            "CREATE INDEX IF NOT EXISTS idx_photo_location ON photo(location);",
            (),
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photo_created ON photo(created);",
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
            "CREATE TABLE IF NOT EXISTS device(ip STRING, name STRING, offer STRING);",
            (),
        );
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS faces (photo_id STRING, face_id STRING PRIMARY KEY, crop_path STRING, encoded STRING, embedding BLOB, person_id STRING);", ());
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS people (id STRING PRIMARY KEY, name STRING, embedding BLOB);", ());
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS config(key STRING, value STRING);",
            (),
        );
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS logs (timestamp DATETIME DEFAULT CURRENT_TIMESTAMP, level STRING, message TEXT);", ());

        Self { connection: conn }
    }

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

    pub fn get_state(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Ok(mut stmt) = self.connection.prepare("SELECT key, value FROM config") {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }) {
                for row in rows.flatten() {
                    map.insert(row.0, row.1);
                }
            }
        }
        map
    }

    pub fn store_log(&self, level: &str, message: &str) {
        if let Err(e) = self.connection.execute(
            "INSERT INTO logs (level, message) VALUES (?1, ?2)",
            (level, message),
        ) {
            eprintln!("store_log: failed to insert log entry: {e}");
        }
    }

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

    pub fn clear_logs(&self) {
        if let Err(e) = self.connection.execute("DELETE FROM logs", ()) {
            eprintln!("clear_logs: {e}");
        }
    }

    pub fn set_state(&self, state: HashMap<String, String>) {
        for (key, value) in state {
            let _ = self.connection.execute(
                "INSERT OR REPLACE INTO config (key, value) VALUES(?1, ?2)",
                (&key, &value),
            );
        }
    }

    pub fn get_last_scan_time(&self) -> Option<String> {
        self.get_state().get("last_scan_time").cloned()
    }

    pub fn set_last_scan_time(&self, timestamp: String) {
        let _ = self.connection.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES('last_scan_time', ?1)",
            [&timestamp],
        );
    }

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

    pub fn list_photos(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
        favorites_only: bool,
        videos_only: bool,
    ) -> Vec<Photo> {
        let mut photos = Vec::new();
        let fav_filter = if favorites_only {
            "AND EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite')"
        } else {
            ""
        };
        let video_filter = if videos_only {
            "AND (p.location LIKE '%.mp4' OR p.location LIKE '%.mkv' OR p.location LIKE '%.mov' OR p.location LIKE '%.avi' OR p.location LIKE '%.webm')"
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
                    OR p.created LIKE ?3 OR p.created LIKE ?4)".to_string()
            } else {
                "AND (p.location LIKE ?3 OR p.id LIKE ?3 OR p.caption LIKE ?3 \
                    OR EXISTS(SELECT 1 FROM object WHERE photo_id=p.id AND class LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM ocr WHERE photo_id=p.id AND text LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM faces f JOIN people p_name ON f.person_id = p_name.id WHERE f.photo_id=p.id AND p_name.name LIKE ?3) \
                    OR EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='location_name' AND value LIKE ?3) \
                    OR p.created LIKE ?3)".to_string()
            }
        } else {
            String::new()
        };

        let sql = format!("SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, p.caption, p.aesthetics_score, 
            s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres 
            FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE 1=1 {fav_filter} {video_filter} {q_filter} ORDER BY p.created DESC LIMIT ?1, ?2");
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
             s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres \
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

    pub fn get_photo_by_id(&self, photo_id: &str) -> Option<Photo> {
        let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, EXISTS(SELECT 1 FROM properties WHERE photo_id=p.id AND key='favorite'), p.indexed, p.caption, p.aesthetics_score, \
             s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres \
             FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.id = ?1";
        let mut stmt = self.connection.prepare(&sql).ok()?;
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
                })
            })
            .ok()?;
        let mut photo = rows.next()?.ok()?;
        self.enrich_objects(std::slice::from_mut(&mut photo));
        self.enrich_properties(std::slice::from_mut(&mut photo));
        Some(photo)
    }

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

    pub fn store_face(&self, face: Face) {
        let embedding_bytes: Vec<u8> = face
            .embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        if let Err(e) = self.connection.execute("INSERT OR REPLACE INTO faces(photo_id, face_id, crop_path, encoded, embedding, person_id) VALUES(?1, ?2, ?3, ?4, ?5, ?6)", (&face.photo_id, &face.face_id, &face.crop_path, &face.encoded, &embedding_bytes, &face.person_id)) {
            eprintln!("store_face: failed to store face {}: {e}", face.face_id);
        }
    }

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

    pub fn create_anonymous_person(&self, embedding: &[f32]) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let embedding_bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        let _ = self.connection.execute(
            "INSERT INTO people (id, name, embedding) VALUES (?1, NULL, ?2)",
            (&id, &embedding_bytes),
        );
        id
    }

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

    pub fn get_anonymous_people_groups(&self) -> Vec<PersonWithFace> {
        let mut results = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare("SELECT p.id, f.crop_path, f.face_id, f.encoded, p.embedding, (SELECT COUNT(*) FROM faces WHERE person_id = p.id) FROM people p JOIN faces f ON p.id = f.person_id WHERE p.name IS NULL GROUP BY p.id ORDER BY (SELECT COUNT(*) FROM faces WHERE person_id = p.id) DESC") {
            if let Ok(iter) = stmt.query_map([], |row| {
                let embedding: Option<Vec<f32>> = row.get::<_, Option<Vec<u8>>>(4).ok().flatten().map(|bytes| bytes.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect());
                Ok(PersonWithFace {
                    id: row.get(0)?,
                    name: "Unnamed Person".to_string(),
                    representative_crop: row.get(1).ok(),
                    representative_face_id: row.get(2).ok(),
                    encoded: row.get(3).ok(),
                    embedding,
                    face_count: row.get(5)?
                })
            }) {
                for p in iter.flatten() { results.push(p); }
            }
        }
        results
    }

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
                })
            }) {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        photos
    }

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

    pub fn remove_directory(&self, path: String) {
        if let Err(e) = self
            .connection
            .execute("DELETE FROM directory WHERE name = ?1", [&path])
        {
            eprintln!("remove_directory: failed to remove '{path}': {e}");
        }
    }

    pub fn add_directory(&self, path: &str) {
        if let Err(e) = self
            .connection
            .execute("INSERT INTO directory (name) VALUES(?1)", [&path])
        {
            eprintln!("add_directory: failed to add '{path}': {e}");
        }
    }

    pub fn merge_people(&self, from_id: &str, to_id: &str) {
        if let Err(e) = self.connection.execute(
            "UPDATE faces SET person_id = ?1 WHERE person_id = ?2",
            (to_id, from_id),
        ) {
            eprintln!("merge_people: failed to update faces from {from_id} to {to_id}: {e}");
            return;
        }
        if let Err(e) = self
            .connection
            .execute("DELETE FROM people WHERE id = ?1", [from_id])
        {
            eprintln!("merge_people: failed to delete {from_id}: {e}");
            return;
        }
        self.update_person_centroid(to_id);
    }

    pub fn rename_person(&self, id: &str, new_name: &str) {
        if let Err(e) = self
            .connection
            .execute("UPDATE people SET name = ?1 WHERE id = ?2", (new_name, id))
        {
            eprintln!("rename_person: failed to rename {id} to {new_name}: {e}");
        }
    }

    pub fn remove_directory_full(&mut self, path: &str) {
        let tx = match self.connection.transaction() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("remove_directory_full: failed to start transaction: {e}");
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
            eprintln!("remove_directory_full: failed to commit: {e}");
        }
    }

    pub fn import_photo(&mut self, photo: ImportedPhoto<'_>) {
        let tx = match self.connection.transaction() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("import_photo: failed to start transaction: {e}");
                return;
            }
        };

        if let Err(e) = tx.execute(
            "INSERT OR REPLACE INTO photo (id, location, created, latitude, longitude, encoded) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (photo.id, photo.location, photo.created, photo.latitude, photo.longitude, photo.encoded),
        ) {
            eprintln!("import_photo: failed to upsert photo {}: {e}", photo.id);
            return;
        }

        if let Err(e) = tx.execute("DELETE FROM object WHERE photo_id = ?1", [photo.id]) {
            eprintln!(
                "import_photo: failed to clear objects for {}: {e}",
                photo.id
            );
        }
        if let Err(e) = tx.execute("DELETE FROM faces WHERE photo_id = ?1", [photo.id]) {
            eprintln!("import_photo: failed to clear faces for {}: {e}", photo.id);
        }

        if let Ok(objects) = serde_json::from_str::<Vec<SyncObject>>(photo.objects_json) {
            for obj in &objects {
                if let Err(e) = tx.execute(
                    "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                    (photo.id, &obj.class, &obj.probability),
                ) {
                    eprintln!(
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
                    eprintln!("import_photo: failed to insert face for {}: {e}", photo.id);
                }
            }
        }

        if let Err(e) = tx.commit() {
            eprintln!(
                "import_photo: failed to commit transaction for {}: {e}",
                photo.id
            );
        }
    }

    pub fn get_media_counts(&self) -> (i64, i64) {
        let photo_count: i64 = self.connection.query_row("SELECT COUNT(*) FROM photo WHERE NOT (location LIKE '%.mp4' OR location LIKE '%.mkv' OR location LIKE '%.mov' OR location LIKE '%.avi' OR location LIKE '%.webm')", [], |r| r.get(0)).unwrap_or(0);
        let video_count: i64 = self.connection.query_row("SELECT COUNT(*) FROM photo WHERE (location LIKE '%.mp4' OR location LIKE '%.mkv' OR location LIKE '%.mov' OR location LIKE '%.avi' OR location LIKE '%.webm')", [], |r| r.get(0)).unwrap_or(0);
        (photo_count, video_count)
    }

    pub fn list_devices(&self) -> Vec<DeviceInfo> {
        let mut results = Vec::new();
        if let Ok(mut stmt) = self.connection.prepare("SELECT ip, name FROM device") {
            if let Ok(iter) = stmt.query_map([], |row| {
                Ok(DeviceInfo {
                    id: row.get::<_, String>(0)?, // Using IP as ID for now or it could be UUID
                    title: row.get::<_, String>(1)?,
                    icon: "mdi-cellphone".to_string(), // Default icon
                    up_to_date: true,
                    host: false,
                    photo_count: 0,
                    video_count: 0,
                    os: "unknown".to_string(),
                })
            }) {
                for d in iter.flatten() {
                    results.push(d);
                }
            }
        }
        results
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

    pub fn store_photo_batch(&mut self, photos: &[Photo]) -> Result<(), String> {
        let tx = self.connection.transaction().map_err(|e| e.to_string())?;
        {
            let mut stmt = tx.prepare_cached("INSERT OR REPLACE INTO photo(id, location, encoded, created, latitude, longitude, indexed) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 1)").map_err(|e| e.to_string())?;
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

    pub fn update_photo_indexed(&self, id: &str, indexed: i32) {
        if let Err(e) = self
            .connection
            .execute("UPDATE photo SET indexed = ?1 WHERE id = ?2", (indexed, id))
        {
            eprintln!("update_photo_indexed: failed for {id}: {e}");
        }
    }

    pub fn update_ai_status(&self, photo_id: &str, model: &str, status: i32) {
        match model {
            "clip" | "face" | "ocr" | "nsfw" | "aesthetics" | "yolo" | "blip" | "arcface"
            | "midas" | "whisper" | "sam" | "superres" => {}
            _ => {
                eprintln!("Invalid model name: {model}");
                return;
            }
        }
        let sql = format!("INSERT INTO ai_status (photo_id, {model}) VALUES (?1, ?2) ON CONFLICT(photo_id) DO UPDATE SET {model} = ?2");
        let _ = self.connection.execute(&sql, (photo_id, status));
    }

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
                })
            }) {
                for p in iter.flatten() {
                    photos.push(p);
                }
            }
        }
        photos
    }

    pub fn get_photos_missing_model(&self, model: &str) -> Vec<String> {
        let mut ids = Vec::new();
        match model {
            "clip" | "face" | "ocr" | "nsfw" | "aesthetics" | "yolo" | "blip" | "arcface"
            | "midas" | "whisper" | "sam" | "superres" => {}
            _ => {
                eprintln!("Invalid model name: {model}");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let dir = std::env::temp_dir().join(format!("siegu_test_{}", std::process::id()));
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
            location: "/tmp/test.jpg".to_string(),
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
            location: "/tmp/test.jpg".to_string(),
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
            location: "/tmp/test.jpg".to_string(),
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
        };
        let _ = db.store_photo_batch(&[photo]);
        let points = db.get_heatmap_points();
        assert!(points.iter().any(|p| p.id == "test_heat_1"));
    }

    #[test]
    fn test_device_list() {
        let db = test_db();
        db.connection
            .execute(
                "INSERT OR REPLACE INTO device(ip, name) VALUES(?1, ?2)",
                ("192.168.1.1", "test-device"),
            )
            .unwrap();
        let devices = db.list_devices();
        assert!(devices.iter().any(|d| d.title == "test-device"));
    }

    #[test]
    fn test_last_scan_time() {
        let db = test_db();
        assert!(db.get_last_scan_time().is_none());

        db.set_last_scan_time("2024-06-01T12:00:00Z".to_string());
        assert_eq!(db.get_last_scan_time().unwrap(), "2024-06-01T12:00:00Z");
    }
}
