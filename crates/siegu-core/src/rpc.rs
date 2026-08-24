//! RPC dispatch for browser guests (#19).
//!
//! A guest speaks `SyncMessage::CommandRequest` over the WebRTC data channel;
//! this module maps `name + JSON payload` onto the exact same business
//! functions the Tauri app calls (see [`crate::library`] and `Database`).
//! Payload field names mirror the Tauri `invoke()` arguments 1:1 so the
//! frontend can switch transports without renaming anything.
//!
//! Sessions are read-only unless the host opted into `--share-mode rw`;
//! mutating commands are rejected at the door in that case.

use serde::Serialize;
use serde_json::{json, Value};

use crate::database::Database;
use crate::library;

/// Permission level of a web-share session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShareMode {
    /// Guests may browse and read only. Default.
    #[default]
    ReadOnly,
    /// Guests may also mutate (favorites, trash). Opt-in via CLI flag.
    ReadWrite,
}

impl ShareMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "ro" | "read-only" | "readonly" => Some(Self::ReadOnly),
            "rw" | "read-write" | "readwrite" => Some(Self::ReadWrite),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "ro",
            Self::ReadWrite => "rw",
        }
    }
}

/// Everything a command handler needs. Cheap to build per request; the
/// database opens its own short-lived SQLite connection like the Tauri
/// commands do.
pub struct RpcContext<'a> {
    pub config_path: &'a str,
    pub mode: ShareMode,
}

/// Commands that never mutate: allowed in every share mode.
const READ_ONLY_COMMANDS: &[&str] = &[
    "list_files",
    "get_photo_by_id",
    "get_photos_by_ids",
    "get_photo_encoded_batch",
    "count_trash",
    "list_trash",
    "get_search_facets",
];

/// Commands that change host state: require `--share-mode rw`.
const READ_WRITE_COMMANDS: &[&str] = &[
    "toggle_favorite",
    "set_favorites",
    "trash_photo",
    "restore_photo",
    "empty_trash",
];

fn is_mutation(name: &str) -> bool {
    READ_WRITE_COMMANDS.contains(&name)
}

// ── payload extraction helpers ────────────────────────────────────────────
// Missing fields default like the Tauri command signatures do; wrong types
// become a per-command error instead of a panic.

trait PayloadExt {
    fn str_field(&self, key: &str) -> Result<Option<String>, String>;
    fn opt_str(&self, key: &str) -> Result<Option<String>, String>;
    fn opt_usize(&self, key: &str) -> Result<Option<usize>, String>;
    fn opt_i64(&self, key: &str) -> Result<Option<i64>, String>;
    fn opt_f64(&self, key: &str) -> Result<Option<f64>, String>;
    fn bool_or(&self, key: &str, default: bool) -> Result<bool, String>;
    fn string_vec(&self, key: &str) -> Result<Vec<String>, String>;
}

impl PayloadExt for Value {
    fn str_field(&self, key: &str) -> Result<Option<String>, String> {
        self.opt_str(key)
    }

    fn opt_str(&self, key: &str) -> Result<Option<String>, String> {
        match self.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(other) => Err(format!("field '{key}' must be a string, got {other}")),
        }
    }

    fn opt_usize(&self, key: &str) -> Result<Option<usize>, String> {
        match self.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => n
                .as_u64()
                .map(|v| v as usize)
                .map(Some)
                .ok_or_else(|| format!("field '{key}' must be a non-negative number")),
            Some(other) => Err(format!("field '{key}' must be a number, got {other}")),
        }
    }

    fn opt_i64(&self, key: &str) -> Result<Option<i64>, String> {
        match self.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => n
                .as_i64()
                .map(Some)
                .ok_or_else(|| format!("field '{key}' must be an integer")),
            Some(other) => Err(format!("field '{key}' must be a number, got {other}")),
        }
    }

    fn opt_f64(&self, key: &str) -> Result<Option<f64>, String> {
        match self.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => n
                .as_f64()
                .map(Some)
                .ok_or_else(|| format!("field '{key}' must be a number")),
            Some(other) => Err(format!("field '{key}' must be a number, got {other}")),
        }
    }

    fn bool_or(&self, key: &str, default: bool) -> Result<bool, String> {
        match self.get(key) {
            None | Some(Value::Null) => Ok(default),
            Some(Value::Bool(b)) => Ok(*b),
            Some(other) => Err(format!("field '{key}' must be a boolean, got {other}")),
        }
    }

    fn string_vec(&self, key: &str) -> Result<Vec<String>, String> {
        match self.get(key) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| format!("field '{key}' must be an array of strings"))
                })
                .collect(),
            Some(other) => Err(format!("field '{key}' must be an array, got {other}")),
        }
    }
}

fn to_json<T: Serialize>(value: &T, fallback: Value) -> Value {
    serde_json::to_value(value).unwrap_or(fallback)
}

/// Run one command. Returns the JSON result or a plain error string safe to
/// send back as `CommandResponse.error` and to log.
pub fn dispatch(ctx: &RpcContext, name: &str, payload: &Value) -> Result<Value, String> {
    if is_mutation(name) && ctx.mode == ShareMode::ReadOnly {
        return Err(format!(
            "command '{name}' mutates the host library; restart with --share-mode rw to allow it"
        ));
    }

    let db = Database::new(ctx.config_path);
    if !READ_ONLY_COMMANDS.contains(&name) && !is_mutation(name) {
        return Err(format!("unknown command '{name}'"));
    }

    match name {
        // ── library browsing ──────────────────────────────────────────────
        "list_files" => {
            let photos = library::do_list_files_filtered(
                &db,
                &payload.opt_str("query")?.unwrap_or_default(),
                payload.opt_usize("offset")?.unwrap_or(0),
                payload.opt_usize("limit")?.unwrap_or(1000),
                payload.bool_or("favorites_only", false)?,
                payload.bool_or("videos_only", false)?,
                payload.string_vec("person_ids")?,
                payload.opt_str("person_match")?,
                payload.bool_or("person_alone", false)?,
                payload.opt_str("location")?,
                payload.opt_str("tag")?,
                payload.opt_str("date_from")?,
                payload.opt_str("date_to")?,
                payload.bool_or("has_faces", false)?,
                payload.opt_f64("aesthetics_min")?,
                payload.opt_str("camera")?,
                payload.bool_or("papers", false)?,
                payload.bool_or("nsfw_only", false)?,
                payload.bool_or("random", false)?,
                payload.opt_str("order_by")?,
                payload.opt_str("album_id")?,
            );
            Ok(to_json(&photos, json!([])))
        }
        "get_photo_by_id" => {
            let id = payload
                .str_field("id")?
                .ok_or_else(|| "missing field 'id'".to_string())?;
            Ok(to_json(&library::do_get_photo_by_id(&db, &id), json!(null)))
        }
        "get_photos_by_ids" => {
            let ids = payload.string_vec("ids")?;
            Ok(to_json(
                &library::do_get_photos_by_ids(&db, &ids),
                json!([]),
            ))
        }
        "get_photo_encoded_batch" => {
            let ids = payload.string_vec("ids")?;
            Ok(to_json(
                &library::do_get_photo_encoded_batch(&db, &ids),
                json!({}),
            ))
        }

        // ── search facets ────────────────────────────────────────────────
        "get_search_facets" => Ok(to_json(&library::do_get_search_facets(&db), json!({}))),

        // ── trash state ──────────────────────────────────────────────────
        "count_trash" => Ok(json!(db.count_trash())),
        "list_trash" => {
            let limit = payload.opt_i64("limit")?.unwrap_or(100);
            let photos = db.list_trash(limit);
            Ok(to_json(&photos, json!([])))
        }

        // ── mutations (rw sessions only) ─────────────────────────────────
        "toggle_favorite" => {
            let id = payload
                .str_field("id")?
                .ok_or_else(|| "missing field 'id'".to_string())?;
            Ok(json!(library::do_toggle_favorite(&db, &id)))
        }
        "set_favorites" => {
            let ids = payload.string_vec("ids")?;
            let favorite = payload.bool_or("favorite", true)?;
            Ok(json!(library::do_set_favorites(&db, &ids, favorite)))
        }
        "trash_photo" => {
            let id = payload
                .str_field("id")?
                .ok_or_else(|| "missing field 'id'".to_string())?;
            Ok(json!(db.trash_photo(&id).is_ok()))
        }
        "restore_photo" => {
            let id = payload
                .str_field("id")?
                .ok_or_else(|| "missing field 'id'".to_string())?;
            Ok(json!(db.restore_photo(&id).is_ok()))
        }
        "empty_trash" => Ok(json!(db.empty_trash())),

        _ => Err(format!("unknown command '{name}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{AiStatus, Photo};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_ctx(mode: ShareMode) -> RpcContext<'static> {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("siegu_rpc_test_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let path: &'static String = Box::leak(Box::new(dir.display().to_string()));
        RpcContext {
            config_path: path.as_str(),
            mode,
        }
    }

    fn make_photo(id: &str, location: &str) -> Photo {
        Photo {
            id: id.to_string(),
            location: location.to_string(),
            encoded: String::new(),
            created: "2026-01-01".to_string(),
            objects: HashMap::new(),
            properties: HashMap::new(),
            latitude: 0.0,
            longitude: 0.0,
            favorite: false,
            indexed: 2,
            caption: None,
            aesthetics_score: None,
            ai_status: AiStatus::default(),
            sync_needed: false,
            received: false,
            view_only: false,
            last_opened: 0,
        }
    }

    #[test]
    fn share_mode_parse() {
        assert_eq!(ShareMode::parse("ro"), Some(ShareMode::ReadOnly));
        assert_eq!(ShareMode::parse("RW"), Some(ShareMode::ReadWrite));
        assert_eq!(ShareMode::parse("bogus"), None);
        assert_eq!(ShareMode::default(), ShareMode::ReadOnly);
    }

    #[test]
    fn list_files_returns_photos() {
        let c = test_ctx(ShareMode::ReadOnly);
        Database::new(c.config_path)
            .store_photo_batch(&[make_photo("p1", "/a.jpg")])
            .unwrap();
        let result = dispatch(&c, "list_files", &json!({ "limit": 10 })).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "p1");
    }

    #[test]
    fn unknown_command_is_rejected() {
        let c = test_ctx(ShareMode::ReadOnly);
        let err = dispatch(&c, "definitely_not_a_command", &json!({})).unwrap_err();
        assert!(err.contains("unknown command"));
    }

    #[test]
    fn mutation_blocked_in_read_only_mode() {
        let c = test_ctx(ShareMode::ReadOnly);
        let err = dispatch(&c, "toggle_favorite", &json!({"id": "p1"})).unwrap_err();
        assert!(err.contains("--share-mode rw"));
    }

    #[test]
    fn mutation_allowed_in_read_write_mode() {
        let c = test_ctx(ShareMode::ReadWrite);
        Database::new(c.config_path)
            .store_photo_batch(&[make_photo("p1", "/a.jpg")])
            .unwrap();
        let result = dispatch(
            &c,
            "set_favorites",
            &json!({"ids": ["p1"], "favorite": true}),
        )
        .unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn favorites_flow_round_trip() {
        let c = test_ctx(ShareMode::ReadWrite);
        Database::new(c.config_path)
            .store_photo_batch(&[make_photo("p1", "/a.jpg"), make_photo("p2", "/b.jpg")])
            .unwrap();

        assert_eq!(
            dispatch(&c, "toggle_favorite", &json!({"id": "p1"})).unwrap(),
            json!(true)
        );
        let favs = dispatch(&c, "list_files", &json!({"favorites_only": true})).unwrap();
        assert_eq!(favs.as_array().unwrap().len(), 1);
        assert_eq!(favs.as_array().unwrap()[0]["id"], "p1");
    }

    #[test]
    fn trash_flow_round_trip() {
        let c = test_ctx(ShareMode::ReadWrite);
        Database::new(c.config_path)
            .store_photo_batch(&[make_photo("p1", "/a.jpg")])
            .unwrap();

        assert_eq!(dispatch(&c, "count_trash", &json!({})).unwrap(), json!(0));
        assert_eq!(
            dispatch(&c, "trash_photo", &json!({"id": "p1"})).unwrap(),
            json!(true)
        );
        assert_eq!(dispatch(&c, "count_trash", &json!({})).unwrap(), json!(1));
        let trash = dispatch(&c, "list_trash", &json!({"limit": 50})).unwrap();
        assert_eq!(trash.as_array().unwrap()[0]["id"], "p1");
        assert_eq!(
            dispatch(&c, "restore_photo", &json!({"id": "p1"})).unwrap(),
            json!(true)
        );
        assert_eq!(dispatch(&c, "count_trash", &json!({})).unwrap(), json!(0));
    }

    #[test]
    fn get_photo_by_id_and_batch() {
        let c = test_ctx(ShareMode::ReadOnly);
        Database::new(c.config_path)
            .store_photo_batch(&[make_photo("p1", "/a.jpg"), make_photo("p2", "/b.jpg")])
            .unwrap();

        let one = dispatch(&c, "get_photo_by_id", &json!({"id": "p1"})).unwrap();
        assert_eq!(one["location"], "/a.jpg");

        let missing = dispatch(&c, "get_photo_by_id", &json!({"id": "nope"})).unwrap();
        assert_eq!(missing, json!(null));

        let many = dispatch(&c, "get_photos_by_ids", &json!({"ids": ["p1", "p2"]})).unwrap();
        assert_eq!(many.as_array().unwrap().len(), 2);
    }

    #[test]
    fn search_facets_shape() {
        let c = test_ctx(ShareMode::ReadOnly);
        let result = dispatch(&c, "get_search_facets", &json!({})).unwrap();
        assert!(result["people"].is_array());
        assert!(result["stats"].is_object());
    }

    #[test]
    fn bad_payload_types_error_not_panic() {
        let c = test_ctx(ShareMode::ReadOnly);
        let err = dispatch(&c, "list_files", &json!({"limit": "lots"})).unwrap_err();
        assert!(err.contains("must be"));
    }
}
