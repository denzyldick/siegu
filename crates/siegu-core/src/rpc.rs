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
//!
//! The authoritative list of commands, their capability tier, stringify flag
//! and argument keys lives in [`crate::rpc_catalog`]; this module derives its
//! allowlists from it and is the only consumer path that runs the handlers.

use std::path::Path;

use serde::Serialize;
use serde_json::{json, Value};

use crate::database::{Album, AlbumKind, Database, PhotoFilter};
use crate::library;
use crate::rpc_catalog::{self, Tier};

/// Permission level of a web-share session.
///
/// `ReadOnly` and `ReadWrite` map to the public `--share-mode ro|rw` flags a
/// self-hoster sets for guests. `Owner` is the configured-owner capability: a
/// client authenticated to its own host (docker-compose / CLI `web_token`)
/// may additionally run heavy host work (ML, sync/session/device) — see
/// [`Tier`] and issue #42.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShareMode {
    /// Guests may browse and read only. Default.
    #[default]
    ReadOnly,
    /// Guests may also mutate (favorites, trash). Opt-in via CLI flag.
    ReadWrite,
    /// Owner: full capability including ML + sync/session/device control.
    Owner,
}

impl ShareMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "ro" | "read-only" | "readonly" => Some(Self::ReadOnly),
            "rw" | "read-write" | "readwrite" => Some(Self::ReadWrite),
            "owner" => Some(Self::Owner),
            _ => None,
        }
    }

    /// The lowest `Tier` this share mode unlocks.
    pub fn tier(&self) -> Tier {
        match self {
            ShareMode::ReadOnly => Tier::ReadOnly,
            ShareMode::ReadWrite => Tier::ReadWrite,
            ShareMode::Owner => Tier::Owner,
        }
    }
}

/// A handle to the host's live ML worker, if one is running. Exists per-request
/// so a guest request (which must never run ML) simply carries `None`, and a
/// CLI/web host that hasn't spawned a worker also carries `None`.
///
/// Kept as a plain `Option<&MlContext>` so the [`RpcContext`] struct literal is
/// identical whether or not the `ml` feature is enabled (only the referent type
/// differs).
#[cfg(feature = "ml")]
pub type MlWorkerRef<'a> = Option<&'a crate::ml_worker::MlContext>;
#[cfg(not(feature = "ml"))]
pub type MlWorkerRef<'a> = Option<&'a ()>;

/// Everything a command handler needs. Cheap to build per request; the
/// database opens its own short-lived SQLite connection like the Tauri
/// commands do.
pub struct RpcContext<'a> {
    pub config_path: &'a str,
    pub mode: ShareMode,
    /// Live ML worker handle, if this request is allowed to drive analysis.
    /// `Owner`-only; a guest/read/rw share never carries one.
    pub ml: MlWorkerRef<'a>,
}

/// Commands allowed at a given capability level, derived from the catalog
/// (single source of truth: [`crate::rpc_catalog::CATALOG`]).
fn is_mutation(name: &str) -> bool {
    rpc_catalog::is_mutation(name)
}

fn is_owner_only(name: &str) -> bool {
    rpc_catalog::is_owner_only(name)
}

// ── payload extraction helpers ────────────────────────────────────────────
// Missing fields default like the Tauri command signatures do; wrong types
// become a per-command error instead of a panic.

trait PayloadExt {
    fn str_field(&self, key: &str) -> Result<Option<String>, String>;
    fn opt_str(&self, key: &str) -> Result<Option<String>, String>;
    fn must_str(&self, key: &str) -> Result<String, String>;
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

    fn must_str(&self, key: &str) -> Result<String, String> {
        self.opt_str(key)?
            .ok_or_else(|| format!("missing field '{key}'"))
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
    let mode_tier = ctx.mode.tier();

    if !rpc_catalog::is_known(name) {
        return Err(format!("unknown command '{name}'"));
    }

    if is_owner_only(name) && mode_tier != Tier::Owner {
        return Err(format!(
            "command '{name}' is owner-only; authenticate as the owner to run it"
        ));
    }

    if is_mutation(name) && mode_tier == Tier::ReadOnly {
        return Err(format!(
            "command '{name}' mutates the host library; restart with --share-mode rw to allow it"
        ));
    }

    let mut db = Database::new(ctx.config_path);

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

        // ── albums (read) ────────────────────────────────────────────────
        "list_albums" => Ok(to_json(&db.list_albums(), json!([]))),
        "get_album" => {
            let album_id = payload.must_str("album_id")?;
            Ok(to_json(&db.get_album(&album_id), json!(null)))
        }
        "get_album_sections" => Ok(to_json(&db.get_album_sections(), json!([]))),
        "get_album_contents" => {
            let album_id = payload.must_str("album_id")?;
            let offset = payload.opt_usize("offset")?.unwrap_or(0);
            let limit = payload.opt_usize("limit")?.unwrap_or(200);
            Ok(to_json(
                &db.get_album_contents(&album_id, offset, limit),
                json!([]),
            ))
        }
        "get_clip_categories" => Ok(to_json(&db.get_clip_auto_categories(), json!([]))),

        // ── people (read) ────────────────────────────────────────────────
        "get_people" => Ok(to_json(&db.get_people(), json!([]))),
        "get_unnamed_faces" => Ok(to_json(&db.get_anonymous_people_groups(), json!([]))),
        "get_person_photos" => {
            let person_id = payload.must_str("person_id")?;
            let offset = payload.opt_usize("offset")?.unwrap_or(0);
            let limit = payload.opt_usize("limit")?.unwrap_or(200);
            Ok(to_json(
                &db.get_photos_for_person(&person_id, offset, limit),
                json!([]),
            ))
        }
        "get_person_faces" => {
            let person_id = payload.must_str("person_id")?;
            Ok(to_json(&db.get_person_faces(&person_id), json!([])))
        }
        "get_faces_for_photo" => {
            let photo_id = payload.must_str("photo_id")?;
            Ok(to_json(&db.get_faces_for_photo(&photo_id), json!([])))
        }
        "get_top_tags" => Ok(to_json(&library::do_get_top_tags(&db), json!([]))),

        // ── search / geo / extraction extras (read) ──────────────────────
        "get_photo_ocr" => {
            let id = payload.must_str("id")?;
            Ok(json!(db.get_photo_ocr(&id)))
        }
        "get_heatmap_data" => Ok(to_json(&db.get_heatmap_points(), json!([]))),
        "day_counts" => {
            let from = payload.opt_str("from")?.unwrap_or_default();
            let to = payload.opt_str("to")?.unwrap_or_default();
            Ok(to_json(&db.get_day_counts(&from, &to), json!([])))
        }
        "list_objects" => {
            let query = payload.opt_str("query")?.unwrap_or_default();
            Ok(to_json(&db.list_objects(&query), json!([])))
        }
        "get_location_names" => Ok(to_json(&library::do_get_location_names(&db), json!([]))),
        // The Tauri command is `search_facets`; the host dispatch also accepts
        // it under the `get_search_facets` name above.
        "search_facets" => Ok(to_json(&library::do_get_search_facets(&db), json!({}))),

        // ── directories / config / status / models / storage (read) ──────
        "list_directories" => Ok(to_json(&db.list_directories(), json!([]))),
        "is_initialized" => Ok(json!(
            !db.list_directories().is_empty() || db.is_onboarding_complete() || db.has_any_photos()
        )),
        "get_config" => Ok(to_json(&db.get_state(), json!({}))),
        "get_last_scan_time" => Ok(to_json(&db.get_last_scan_time(), json!(null))),
        "get_unindexed_count" => Ok(json!(library::do_get_unindexed_count(&db))),
        "get_max_photo_rowid" => Ok(json!(db.max_photo_rowid())),
        // When the host runs a live ML worker (webHost `--owner-mode`) the
        // pending-work metric is the worker's in-flight queue depth, matching
        // the desktop command. Otherwise (read/rw guest, no worker) fall back to
        // the uncapped library count of photos still needing analysis.
        "get_indexing_status" => {
            #[cfg(feature = "ml")]
            if let Some(ml) = ctx.ml {
                return Ok(json!(crate::ml_commands::do_get_indexing_status(
                    &ml.pending_count
                )));
            }
            Ok(json!(library::do_get_unindexed_count(&db)))
        }
        "storage_usage" => {
            let quota = crate::mesh::MeshManager::get_storage_quota(ctx.config_path);
            let used = crate::mesh::MeshManager::get_storage_used(ctx.config_path);
            Ok(json!({ "used": used, "quota": quota }))
        }
        "check_models" => {
            let models_dir = Path::new(ctx.config_path).join("models");
            Ok(to_json(
                &crate::model_manager::check_models_downloaded(&models_dir),
                json!([]),
            ))
        }
        "get_model_capabilities" => {
            #[cfg(feature = "ml")]
            {
                let models_dir = Path::new(ctx.config_path).join("models");
                let config = db.get_state();
                let feas =
                    crate::ml_engine::models::model_feasibility(&models_dir, &config, &|_| {});
                Ok(to_json(&feas, json!([])))
            }
            #[cfg(not(feature = "ml"))]
            {
                Ok(json!([]))
            }
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
        "delete_photo_permanently" => {
            let id = payload
                .str_field("id")?
                .ok_or_else(|| "missing field 'id'".to_string())?;
            Ok(json!(db.purge_photo(&id).is_ok()))
        }

        // ── albums (rw) ──────────────────────────────────────────────────
        "create_album" => {
            let name = payload.must_str("name")?;
            let album: Album = db.create_album(&name)?;
            Ok(to_json(&album, json!(null)))
        }
        "create_smart_album" => {
            let name = payload.must_str("name")?;
            let rule = parse_photo_filter(payload)?;
            let kind = payload
                .opt_str("kind")?
                .unwrap_or_else(|| "smart".to_string());
            let album = db.create_smart_album(&name, &rule, AlbumKind::parse(&kind))?;
            Ok(to_json(&album, json!(null)))
        }
        "update_smart_album_rule" => {
            let album_id = payload.must_str("album_id")?;
            let rule = parse_photo_filter(payload)?;
            db.update_smart_album_rule(&album_id, &rule)?;
            Ok(Value::Null)
        }
        "rename_album" => {
            let album_id = payload.must_str("album_id")?;
            let name = payload.must_str("name")?;
            db.rename_album(&album_id, &name)?;
            Ok(Value::Null)
        }
        "delete_album" => {
            let album_id = payload.must_str("album_id")?;
            db.delete_album(&album_id)?;
            Ok(Value::Null)
        }
        "clear_dismissed_trips" => Ok(json!(db.clear_dismissed_trips())),
        "sync_trips" => Ok(json!(db.sync_trips())),
        "add_album_items" => {
            let album_id = payload.must_str("album_id")?;
            let photo_ids = payload.string_vec("photo_ids")?;
            db.add_album_items(&album_id, &photo_ids)?;
            Ok(Value::Null)
        }
        "remove_album_items" => {
            let album_id = payload.must_str("album_id")?;
            let photo_ids = payload.string_vec("photo_ids")?;
            db.remove_album_items(&album_id, &photo_ids)?;
            Ok(Value::Null)
        }
        "reorder_album" => {
            let album_id = payload.must_str("album_id")?;
            let ordered_ids = payload.string_vec("ordered_ids")?;
            db.reorder_album(&album_id, &ordered_ids)?;
            Ok(Value::Null)
        }

        // ── people (rw) ──────────────────────────────────────────────────
        "assign_name_to_face" => {
            let face_id = payload.must_str("face_id")?;
            let name = payload.must_str("name")?;
            Ok(json!(db.assign_name_to_face(&face_id, &name)))
        }
        "delete_face" => {
            let face_id = payload.must_str("face_id")?;
            library::do_delete_face(&db, &face_id);
            Ok(Value::Null)
        }
        "merge_people" => {
            let from_id = payload.must_str("from_id")?;
            let to_id = payload.must_str("to_id")?;
            db.merge_people(&from_id, &to_id);
            Ok(Value::Null)
        }
        "rename_person" => {
            let id = payload.must_str("id")?;
            let new_name = payload.must_str("new_name")?;
            db.rename_person(&id, &new_name);
            Ok(Value::Null)
        }

        // ── config / directories / housekeeping (rw) ─────────────────────
        "save_config" => {
            let key = payload.must_str("key")?;
            let value = payload.must_str("value")?;
            db.set_state_value(&key, &value);
            Ok(Value::Null)
        }
        "add_directory" => {
            let path = payload.must_str("path")?;
            db.add_directory(&path);
            Ok(Value::Null)
        }
        "remove_directory" => {
            let path = payload.must_str("path")?;
            db.remove_directory(path.clone());
            Ok(Value::Null)
        }
        "remove_directory_full" => {
            let path = payload.must_str("path")?;
            db.remove_directory_full(&path);
            Ok(Value::Null)
        }
        "mark_onboarding_complete" => {
            db.set_onboarding_complete();
            Ok(Value::Null)
        }
        "cleanup_database" => {
            let confirm = payload.bool_or("confirm", false)?;
            if !confirm {
                return Err("cleanup_database requires confirm=true".to_string());
            }
            db.wipe_all_data().map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }

        // ── ML analysis / indexing (owner-only) ───────────────────────────
        // Reached only when `ctx.mode == Owner` (owner-only gate above). The
        // host must also be running an ML worker; a guest/read/rw RpcContext
        // never carries one, so these resolve to a clear error.
        #[cfg(feature = "ml")]
        "analyze_photo" => {
            let ml = crate::ml_commands::require_worker(ctx.ml, name)?;
            let id = payload.must_str("id")?;
            crate::ml_commands::do_analyze_photo_sync(&ml.abort, &ml.tx, &id)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "ml")]
        "analyze_photo_model" => {
            let ml = crate::ml_commands::require_worker(ctx.ml, name)?;
            let id = payload.must_str("id")?;
            let model_id = payload.must_str("model_id")?;
            crate::ml_commands::do_analyze_photo_model_sync(&ml.abort, &ml.tx, &id, &model_id)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "ml")]
        "analyze_model" => {
            let ml = crate::ml_commands::require_worker(ctx.ml, name)?;
            let model_id = payload.must_str("model_id")?;
            crate::ml_commands::do_analyze_model_sync(&ml.tx, &model_id)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "ml")]
        "index_faces" => {
            let ml = crate::ml_commands::require_worker(ctx.ml, name)?;
            crate::ml_commands::do_index_faces(&db)?;
            crate::ml_commands::do_index_faces_sync(&ml.tx)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "ml")]
        "abort_indexing" => {
            let ml = crate::ml_commands::require_worker(ctx.ml, name)?;
            crate::ml_commands::do_abort_indexing(&ml.abort, &ml.pending_count, &ml.paused)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "ml")]
        "pause_indexing" => {
            let ml = crate::ml_commands::require_worker(ctx.ml, name)?;
            crate::ml_commands::do_pause_indexing(&ml.paused)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "ml")]
        "resume_indexing" => {
            let ml = crate::ml_commands::require_worker(ctx.ml, name)?;
            crate::ml_commands::do_resume_indexing(&ml.paused)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "ml")]
        "reload_models" => {
            let ml = crate::ml_commands::require_worker(ctx.ml, name)?;
            crate::ml_commands::do_reload_models_sync(&ml.tx)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "ml")]
        "unload_models" => {
            let ml = crate::ml_commands::require_worker(ctx.ml, name)?;
            crate::ml_commands::do_unload_models(&ml.models)?;
            Ok(Value::Null)
        }
        #[cfg(feature = "ml")]
        "get_models_loaded" => {
            let ml = crate::ml_commands::require_worker(ctx.ml, name)?;
            Ok(json!(crate::ml_commands::do_get_models_loaded(&ml.models)))
        }

        _ => Err(format!("unknown command '{name}'")),
    }
}

/// Parse the JSON payload's `rule` string into a [`PhotoFilter`] (the Tauri
/// command passes it as a serialized string, so the RPC payload does too).
fn parse_photo_filter(payload: &Value) -> Result<PhotoFilter, String> {
    let rule = payload.opt_str("rule")?.unwrap_or_else(|| "{}".to_string());
    serde_json::from_str(&rule).map_err(|e| format!("invalid album rule: {e}"))
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
            ml: None,
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

    #[test]
    fn albums_readable_in_read_only_mode() {
        let c = test_ctx(ShareMode::ReadOnly);
        assert_eq!(
            dispatch(&c, "list_albums", &json!({})).unwrap(),
            json!([]),
            "empty library lists no albums"
        );
        let sections = dispatch(&c, "get_album_sections", &json!({})).unwrap();
        assert!(sections.is_array());
        assert!(dispatch(&c, "get_album", &json!({"album_id": "nope"}))
            .unwrap()
            .is_null());
    }

    #[test]
    fn album_management_round_trip() {
        let c = test_ctx(ShareMode::ReadWrite);
        // Read-only mode must reject the mutation itself.
        let ro = dispatch(
            &test_ctx(ShareMode::ReadOnly),
            "create_album",
            &json!({"name": "Blocked"}),
        )
        .unwrap_err();
        assert!(ro.contains("--share-mode rw"));

        Database::new(c.config_path)
            .store_photo_batch(&[make_photo("p1", "/a.jpg"), make_photo("p2", "/b.jpg")])
            .unwrap();
        let created = dispatch(&c, "create_album", &json!({"name": "Holiday"})).unwrap();
        let album_id = created["id"].as_str().unwrap();

        dispatch(
            &c,
            "add_album_items",
            &json!({"album_id": album_id, "photo_ids": ["p1", "p2"]}),
        )
        .unwrap();
        let album = dispatch(&c, "get_album", &json!({"album_id": album_id})).unwrap();
        assert_eq!(album["item_count"], 2);

        let contents = dispatch(
            &c,
            "get_album_contents",
            &json!({"album_id": album_id, "offset": 0, "limit": 10}),
        )
        .unwrap();
        assert_eq!(contents.as_array().unwrap().len(), 2);

        dispatch(
            &c,
            "rename_album",
            &json!({"album_id": album_id, "name": "Trip"}),
        )
        .unwrap();
        assert_eq!(
            dispatch(&c, "get_album", &json!({"album_id": album_id})).unwrap()["name"],
            "Trip"
        );

        let albums = dispatch(&c, "list_albums", &json!({})).unwrap();
        assert_eq!(albums.as_array().unwrap().len(), 1);
    }

    #[test]
    fn people_and_faces_round_trip() {
        let c = test_ctx(ShareMode::ReadWrite);
        let db = Database::new(c.config_path);
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES ('p1','/p1.jpg','2026-01-01','')",
                (),
            )
            .unwrap();
        db.store_face(crate::database::Face {
            photo_id: "p1".to_string(),
            face_id: "f1".to_string(),
            crop_path: "/f1.jpg".to_string(),
            encoded: String::new(),
            embedding: vec![0.1; 512],
            person_id: None,
        });

        let person_id = dispatch(
            &c,
            "assign_name_to_face",
            &json!({"face_id": "f1", "name": "Alice"}),
        )
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
        assert!(!person_id.is_empty());

        let people = dispatch(&c, "get_people", &json!({})).unwrap();
        assert_eq!(people.as_array().unwrap().len(), 1);
        assert_eq!(people[0]["name"], "Alice");

        let photos = dispatch(
            &c,
            "get_person_photos",
            &json!({"person_id": &person_id, "offset": 0, "limit": 10}),
        )
        .unwrap();
        assert_eq!(photos.as_array().unwrap().len(), 1);
        assert_eq!(photos[0]["id"], "p1");

        let faces = dispatch(&c, "get_person_faces", &json!({"person_id": &person_id})).unwrap();
        assert_eq!(faces.as_array().unwrap().len(), 1);
        assert_eq!(faces[0]["face_id"], "f1");

        dispatch(
            &c,
            "rename_person",
            &json!({"id": &person_id, "new_name": "Alicia"}),
        )
        .unwrap();
        let people = dispatch(&c, "get_people", &json!({})).unwrap();
        assert_eq!(people.as_array().unwrap()[0]["name"], "Alicia");

        dispatch(&c, "delete_face", &json!({"face_id": "f1"})).unwrap();
        let remaining_faces =
            dispatch(&c, "get_person_faces", &json!({"person_id": &person_id})).unwrap();
        assert_eq!(
            remaining_faces.as_array().unwrap().len(),
            0,
            "face deleted but person still named"
        );
    }

    #[test]
    fn trash_permanent_delete_purges_photo() {
        let c = test_ctx(ShareMode::ReadWrite);
        Database::new(c.config_path)
            .store_photo_batch(&[make_photo("p1", "/a.jpg")])
            .unwrap();
        dispatch(&c, "trash_photo", &json!({"id": "p1"})).unwrap();
        assert_eq!(
            dispatch(&c, "delete_photo_permanently", &json!({"id": "p1"})).unwrap(),
            json!(true)
        );
        assert_eq!(
            dispatch(&c, "list_files", &json!({"limit": 10}))
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            0
        );
        // In read-only mode the permanent delete is rejected up front.
        let ro = test_ctx(ShareMode::ReadOnly);
        let err = dispatch(&ro, "delete_photo_permanently", &json!({"id": "p1"})).unwrap_err();
        assert!(err.contains("--share-mode rw"));
    }

    #[test]
    fn search_facets_alias_and_helpers() {
        let c = test_ctx(ShareMode::ReadOnly);
        assert_eq!(
            dispatch(&c, "search_facets", &json!({})).unwrap()["stats"].is_object(),
            true
        );
        assert_eq!(dispatch(&c, "get_top_tags", &json!({})).unwrap(), json!([]));
        assert_eq!(
            dispatch(&c, "get_location_names", &json!({})).unwrap(),
            json!([])
        );
        assert_eq!(
            dispatch(
                &c,
                "day_counts",
                &json!({"from": "2026-01-01", "to": "2026-12-31"})
            )
            .unwrap(),
            json!([])
        );
        assert_eq!(
            dispatch(&c, "get_unindexed_count", &json!({})).unwrap(),
            json!(0)
        );
        assert!(dispatch(&c, "check_models", &json!({})).unwrap().is_array());
    }

    fn make_unindexed_photo(id: &str, location: &str) -> Photo {
        let mut p = make_photo(id, location);
        p.indexed = 0;
        p
    }

    // Part D contract freeze: the RPC facade must report the library-wide
    // unindexed count (uncapped), not the LIMIT-50 snapshot used internally.
    #[test]
    fn unindexed_count_and_status_report_full_library() {
        let c = test_ctx(ShareMode::ReadOnly);
        let photos: Vec<Photo> = (0..55)
            .map(|i| make_unindexed_photo(&format!("u{i:03}"), &format!("/a/{i:03}.jpg")))
            .collect();
        Database::new(c.config_path)
            .store_photo_batch(&photos)
            .unwrap();
        assert_eq!(
            dispatch(&c, "get_unindexed_count", &json!({})).unwrap(),
            json!(55)
        );
        // With no live worker (read-only principal) get_indexing_status falls
        // back to the same uncapped library count.
        assert_eq!(
            dispatch(&c, "get_indexing_status", &json!({})).unwrap(),
            json!(55)
        );
    }

    // Part D contract freeze: get_top_tags / get_location_names surface the
    // library's real tags and locations over the facade, not just empty arrays.
    #[test]
    fn top_tags_and_location_names_report_seeded_library() {
        let c = test_ctx(ShareMode::ReadOnly);
        let mut photo = make_photo("p1", "/a.jpg");
        photo
            .properties
            .insert("location_name".to_string(), "Paris".to_string());
        Database::new(c.config_path)
            .store_photo_batch(&[photo])
            .unwrap();
        {
            let seed = Database::new(c.config_path);
            seed.connection
                .execute(
                    "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                    ["p1", "beach", "0.9"],
                )
                .unwrap();
        }
        let tags = dispatch(&c, "get_top_tags", &json!({})).unwrap();
        let names = tags
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(names.contains(&"beach"), "tag 'beach' missing: {tags}");
        assert_eq!(
            dispatch(&c, "get_location_names", &json!({})).unwrap(),
            json!(["Paris"])
        );
    }
}

/// Security tests for the ML capability boundary (issue #19 / #42): a guest
/// principal (read-only or rw share, `ml: None`) must never run owner/ML
/// commands, while the owner principal (with a live worker) can.
#[cfg(all(test, feature = "ml"))]
mod ml_sec_tests {
    use super::*;
    use crate::ml_worker::MlContext;
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    const GUEST_ERR: &str = "authenticate as the owner";
    const NO_WORKER_ERR: &str = "needs a live ML worker";

    static LOCAL_COUNTER: AtomicUsize = AtomicUsize::new(9000);

    fn guest(mode: ShareMode) -> RpcContext<'static> {
        let id = LOCAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("siegu_rpc_mlsec_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let path: &'static String = Box::leak(Box::new(dir.display().to_string()));
        RpcContext {
            config_path: path.as_str(),
            mode,
            ml: None,
        }
    }

    fn owner_with_worker() -> RpcContext<'static> {
        let id = LOCAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "siegu_rpc_mlsec_owner_{}_{}",
            std::process::id(),
            id
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let path: &'static String = Box::leak(Box::new(dir.display().to_string()));
        let (tx, rx) = tokio::sync::mpsc::channel(crate::ml_worker::JOB_CHANNEL_CAPACITY);
        // Keep the receiver alive (as a real worker task would); otherwise the
        // channel closes and `try_send` in the facade reports a dead worker.
        Box::leak(Box::new(rx));
        let worker = Box::leak(Box::new(MlContext {
            tx,
            pending_count: std::sync::Arc::new(AtomicUsize::new(0)),
            abort: std::sync::Arc::new(AtomicBool::new(false)),
            paused: std::sync::Arc::new(AtomicBool::new(false)),
            models: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }));
        RpcContext {
            config_path: path.as_str(),
            mode: ShareMode::Owner,
            ml: Some(worker),
        }
    }

    #[test]
    fn guest_cannot_run_any_ml_command() {
        for mode in [ShareMode::ReadOnly, ShareMode::ReadWrite] {
            let c = guest(mode);
            for cmd in [
                "analyze_photo",
                "analyze_photo_model",
                "analyze_model",
                "index_faces",
                "abort_indexing",
                "reload_models",
                "unload_models",
                "get_models_loaded",
            ] {
                let err = dispatch(&c, cmd, &json!({})).unwrap_err();
                assert!(
                    err.contains(GUEST_ERR),
                    "guest mode {mode:?} should be blocked for {cmd}: {err}"
                );
            }
        }
    }

    #[test]
    fn owner_without_worker_gets_clear_error() {
        // An RpcContext carrying no worker handle must fail with an explicit
        // "no live ML worker" message rather than a confusing unknown command.
        let mut c = owner_with_worker();
        c.ml = None;
        let err = dispatch(&c, "analyze_photo", &json!({"id": "p1"})).unwrap_err();
        assert!(err.contains(NO_WORKER_ERR), "{err}");
    }

    #[test]
    fn owner_with_worker_can_run_analyze() {
        let c = owner_with_worker();
        // No DB write is needed: a local photo id is accepted by the facade.
        let res = dispatch(&c, "analyze_photo", &json!({"id": "p1"})).unwrap();
        assert!(res.is_null());
    }

    #[test]
    fn owner_can_pause_and_resume_worker() {
        let c = owner_with_worker();
        dispatch(&c, "pause_indexing", &json!({})).unwrap();
        dispatch(&c, "resume_indexing", &json!({})).unwrap();
        assert!(dispatch(&c, "get_models_loaded", &json!({})).unwrap() == json!(false));
    }
}
