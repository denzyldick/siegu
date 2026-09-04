use crate::common::get_config_path;
use crate::database;

// Business logic lives in siegu-core (#19) so CLI hosts and RPC guests run
// the exact same functions as this app.
#[allow(unused_imports)] // some are only used by the unit tests below
pub use siegu_core::library::{
    do_get_heatmap_data, do_get_photo_by_id, do_get_photo_encoded_batch, do_get_photos_by_ids,
    do_list_files, do_list_files_filtered, do_set_favorites, do_toggle_favorite,
};

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn list_files(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::WebRtcState>,
    offset: usize,
    limit: usize,
    query: String,
    scan: bool,
    favorites_only: bool,
    videos_only: bool,
    person_ids: Option<Vec<String>>,
    person_match: Option<String>,
    person_alone: bool,
    location: Option<String>,
    tag: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    has_faces: bool,
    aesthetics_min: Option<f64>,
    camera: Option<String>,
    papers: bool,
    nsfw_only: bool,
    stored_only: bool,
    not_stored_only: bool,
    random: bool,
    order_by: Option<String>,
    album_id: Option<String>,
) -> Result<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Ok("[]".to_string());
    }
    if scan {
        crate::commands::scan::scan_files(app.clone());
    }
    if let Ok(tx_lock) = state.sync_tx.try_lock() {
        if tx_lock.is_some() {
            // A live peer session is active — mirror the peer's full library
            // over the data channel (#mirror). Falls back to local on error.
            return list_files_via_rpc(
                &state,
                &path,
                &query,
                offset,
                limit,
                favorites_only,
                videos_only,
                person_ids,
                person_match,
                person_alone,
                location,
                tag,
                date_from,
                date_to,
                has_faces,
                aesthetics_min,
                camera,
                papers,
                nsfw_only,
                stored_only,
                not_stored_only,
                random,
                order_by,
                album_id,
            )
            .await;
        }
    }
    let database = database::Database::new(&path);
    Ok(serde_json::to_string(&do_list_files_filtered(
        &database,
        &query,
        offset,
        limit,
        favorites_only,
        videos_only,
        person_ids.unwrap_or_default(),
        person_match,
        person_alone,
        location,
        tag,
        date_from,
        date_to,
        has_faces,
        aesthetics_min,
        camera,
        papers,
        nsfw_only,
        stored_only,
        not_stored_only,
        random,
        order_by,
        album_id,
    ))
    .unwrap_or("[]".to_string()))
}

/// Send a read-only `list_files` RPC to the connected peer and await the reply,
/// returning the peer's (host's) full library as the same JSON string the local
/// `list_files` command returns. (#mirror)
#[allow(clippy::too_many_arguments)]
async fn list_files_via_rpc(
    state: &crate::WebRtcState,
    config_path: &str,
    query: &str,
    offset: usize,
    limit: usize,
    favorites_only: bool,
    videos_only: bool,
    person_ids: Option<Vec<String>>,
    person_match: Option<String>,
    person_alone: bool,
    location: Option<String>,
    tag: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    has_faces: bool,
    aesthetics_min: Option<f64>,
    camera: Option<String>,
    papers: bool,
    nsfw_only: bool,
    stored_only: bool,
    not_stored_only: bool,
    random: bool,
    order_by: Option<String>,
    album_id: Option<String>,
) -> Result<String, String> {
    let mut tx_lock = state.sync_tx.lock().await;
    let Some(tx) = tx_lock.as_mut() else {
        return Err("Not connected to a device".to_string());
    };

    let id = state
        .rpc_next_id
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let (send, recv) = tokio::sync::oneshot::channel();
    state.rpc_pending.lock().await.insert(id, send);

    let payload = serde_json::json!({
        "query": query,
        "offset": offset,
        "limit": limit,
        "favorites_only": favorites_only,
        "videos_only": videos_only,
        "person_ids": person_ids.unwrap_or_default(),
        "person_match": person_match,
        "person_alone": person_alone,
        "location": location,
        "tag": tag,
        "date_from": date_from,
        "date_to": date_to,
        "has_faces": has_faces,
        "aesthetics_min": aesthetics_min,
        "camera": camera,
        "papers": papers,
        "nsfw_only": nsfw_only,
        "stored_only": stored_only,
        "not_stored_only": not_stored_only,
        "random": random,
        "order_by": order_by,
        "album_id": album_id,
    });

    if tx
        .send(crate::transport::SyncMessage::CommandRequest {
            id,
            name: "list_files".to_string(),
            payload,
        })
        .is_err()
    {
        state.rpc_pending.lock().await.remove(&id);
        return Err("Failed to send mirror request".to_string());
    }
    drop(tx_lock);

    match tokio::time::timeout(std::time::Duration::from_secs(60), recv).await {
        Ok(Ok((true, Some(result), _))) => {
            // Merge: mark items not stored locally as view_only so the
            // frontend streams them via /remote instead of 404-ing on the
            // host's file path. (#mirror)
            let merged = merge_mirror_result(config_path, result);
            Ok(serde_json::to_string(&merged).unwrap_or_else(|_| "[]".to_string()))
        }
        Ok(Ok((false, _, Some(err)))) => Err(err),
        Ok(_) => Err("Mirror request failed".to_string()),
        Err(_) => Err("Mirror request timed out".to_string()),
    }
}

/// Merge a host's `list_files` JSON response with the local database: items
/// that exist locally keep their original shape; items NOT stored locally get
/// `view_only: true` so the frontend streams them via /remote. (#mirror)
fn merge_mirror_result(config_path: &str, result: serde_json::Value) -> serde_json::Value {
    let Some(arr) = result.as_array() else {
        return result;
    };
    if arr.is_empty() {
        return result;
    }
    // Batch-fetch all IDs that exist in the local database.
    let ids: Vec<String> = arr
        .iter()
        .filter_map(|v| v.get("id").and_then(|id| id.as_str()).map(String::from))
        .collect();
    let local_db = database::Database::new(config_path);
    let local_photos = local_db.get_photos_by_ids(&ids);
    let local_ids: std::collections::HashSet<String> =
        local_photos.iter().map(|p| p.id.clone()).collect();
    // Mark unstored items as view-only.
    let mut merged = arr.clone();
    for item in merged.iter_mut() {
        if let Some(id) = item.get("id").and_then(|id| id.as_str()) {
            if !local_ids.contains(id) {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("view_only".to_string(), serde_json::Value::Bool(true));
                }
            }
        }
    }
    serde_json::Value::Array(merged)
}

/// Explicit mirror listing command: return the connected peer's full library.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn remote_list_files(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::WebRtcState>,
    offset: usize,
    limit: usize,
    query: String,
    favorites_only: bool,
    videos_only: bool,
    person_ids: Option<Vec<String>>,
    person_match: Option<String>,
    person_alone: bool,
    location: Option<String>,
    tag: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    has_faces: bool,
    aesthetics_min: Option<f64>,
    camera: Option<String>,
    papers: bool,
    nsfw_only: bool,
    stored_only: bool,
    not_stored_only: bool,
    random: bool,
    order_by: Option<String>,
    album_id: Option<String>,
) -> Result<String, String> {
    list_files_via_rpc(
        &state,
        &get_config_path(&app),
        &query,
        offset,
        limit,
        favorites_only,
        videos_only,
        person_ids,
        person_match,
        person_alone,
        location,
        tag,
        date_from,
        date_to,
        has_faces,
        aesthetics_min,
        camera,
        papers,
        nsfw_only,
        stored_only,
        not_stored_only,
        random,
        order_by,
        album_id,
    )
    .await
}

#[tauri::command]
pub async fn toggle_favorite(app: tauri::AppHandle, id: String) -> bool {
    let path = get_config_path(&app);
    if path.is_empty() {
        return false;
    }
    let database = database::Database::new(&path);
    do_toggle_favorite(&database, &id)
}

#[tauri::command]
pub async fn set_favorites(app: tauri::AppHandle, ids: Vec<String>, favorite: bool) -> usize {
    let path = get_config_path(&app);
    if path.is_empty() {
        return 0;
    }
    let database = database::Database::new(&path);
    do_set_favorites(&database, &ids, favorite)
}

#[tauri::command]
pub async fn get_photo_ocr(app: tauri::AppHandle, id: String) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return String::new();
    }
    let database = database::Database::new(&path);
    database.get_photo_ocr(&id)
}

#[tauri::command]
pub async fn get_photo_by_id(app: tauri::AppHandle, id: String) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "null".to_string();
    }
    let database = database::Database::new(&path);
    match do_get_photo_by_id(&database, &id) {
        Some(photo) => serde_json::to_string(&photo).unwrap_or("null".to_string()),
        None => "null".to_string(),
    }
}

#[tauri::command]
pub async fn get_photo_encoded_batch(
    app: tauri::AppHandle,
    ids: Vec<String>,
) -> std::collections::HashMap<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() || ids.is_empty() {
        return std::collections::HashMap::new();
    }
    let database = database::Database::new(&path);
    do_get_photo_encoded_batch(&database, &ids)
}

#[tauri::command]
pub async fn get_photos_by_ids(app: tauri::AppHandle, ids: Vec<String>) -> String {
    let path = get_config_path(&app);
    if path.is_empty() || ids.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    let photos = do_get_photos_by_ids(&database, &ids);
    serde_json::to_string(&photos).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn get_heatmap_data(app: tauri::AppHandle) -> String {
    use crate::common::emit_log;
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    let points = do_get_heatmap_data(&database);
    emit_log(
        &app,
        format!("DEBUG: Found {} photos with GPS for heatmap", points.len()),
    );
    serde_json::to_string(&points).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn trash_photo(app: tauri::AppHandle, id: String) -> bool {
    let path = get_config_path(&app);
    if path.is_empty() {
        return false;
    }
    let database = database::Database::new(&path);
    database.trash_photo(&id).is_ok()
}

#[tauri::command]
pub async fn restore_photo(app: tauri::AppHandle, id: String) -> bool {
    let path = get_config_path(&app);
    if path.is_empty() {
        return false;
    }
    let database = database::Database::new(&path);
    database.restore_photo(&id).is_ok()
}

#[tauri::command]
pub async fn empty_trash(app: tauri::AppHandle) -> i64 {
    let path = get_config_path(&app);
    if path.is_empty() {
        return 0;
    }
    let database = database::Database::new(&path);
    database.empty_trash()
}

#[tauri::command]
pub async fn count_trash(app: tauri::AppHandle) -> i64 {
    let path = get_config_path(&app);
    if path.is_empty() {
        return 0;
    }
    let database = database::Database::new(&path);
    database.count_trash()
}

#[tauri::command]
pub async fn list_trash(app: tauri::AppHandle, limit: i64) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    let photos = database.list_trash(limit);
    serde_json::to_string(&photos).unwrap_or("[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::test_helpers::*;

    #[test]
    fn list_files_empty_db() {
        let (db, _dir) = test_db();
        let result = do_list_files(&db, "", 0, 10, false, false);
        assert!(result.is_empty());
    }

    #[test]
    fn list_files_with_photos() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("p1", "/a.jpg"), make_photo("p2", "/b.jpg")])
            .unwrap();
        let result = do_list_files(&db, "", 0, 10, false, false);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn list_files_pagination() {
        let (mut db, _dir) = test_db();
        let photos: Vec<_> = (0..5)
            .map(|i| make_photo(&format!("p{i}"), &format!("/{i}.jpg")))
            .collect();
        db.store_photo_batch(&photos).unwrap();
        assert_eq!(do_list_files(&db, "", 0, 2, false, false).len(), 2);
        assert_eq!(do_list_files(&db, "", 2, 10, false, false).len(), 3);
    }

    #[test]
    fn list_files_favorites_only() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("a", "/a.jpg"), make_photo("b", "/b.jpg")])
            .unwrap();
        db.toggle_favorite("a");
        let favs = do_list_files(&db, "", 0, 10, true, false);
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].id, "a");
        let all = do_list_files(&db, "", 0, 10, false, false);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn list_files_query_search() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[
            make_photo("ph_beach", "/beach/sunset.jpg"),
            make_photo("ph_city", "/city/street.jpg"),
        ])
        .unwrap();
        let result = do_list_files(&db, "beach", 0, 10, false, false);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "ph_beach");
    }

    #[test]
    fn toggle_favorite_adds_and_removes() {
        let (db, _dir) = test_db();
        assert!(do_toggle_favorite(&db, "photo1"));
        assert!(!do_toggle_favorite(&db, "photo1"));
    }

    #[test]
    fn get_photo_by_id_found() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("p1", "/img.jpg")])
            .unwrap();
        let photo = do_get_photo_by_id(&db, "p1");
        assert!(photo.is_some());
        assert_eq!(photo.unwrap().location, "/img.jpg");
    }

    #[test]
    fn get_photo_by_id_not_found() {
        let (db, _dir) = test_db();
        assert!(do_get_photo_by_id(&db, "nonexistent").is_none());
    }

    #[test]
    fn get_photo_encoded_batch_basic() {
        let (mut db, _dir) = test_db();
        let mut p = make_photo("x1", "/x.jpg");
        p.encoded = "base64data".to_string();
        db.store_photo_batch(&[p]).unwrap();
        let result = do_get_photo_encoded_batch(&db, &["x1".to_string()]);
        assert_eq!(result.get("x1").unwrap(), "base64data");
    }

    #[test]
    fn get_photo_encoded_batch_empty_ids() {
        let (db, _dir) = test_db();
        let result = do_get_photo_encoded_batch(&db, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn get_photo_encoded_batch_missing() {
        let (db, _dir) = test_db();
        let result = do_get_photo_encoded_batch(&db, &["nope".to_string()]);
        assert!(result.is_empty());
    }

    #[test]
    fn get_photos_by_ids_basic() {
        let (mut db, _dir) = test_db();
        let mut p = make_photo("m1", "/map.jpg");
        p.latitude = 40.7;
        p.longitude = -74.0;
        db.store_photo_batch(&[p]).unwrap();
        let result = do_get_photos_by_ids(&db, &["m1".to_string()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].latitude, 40.7);
    }

    #[test]
    fn get_photos_by_ids_empty_ids() {
        let (db, _dir) = test_db();
        let result = do_get_photos_by_ids(&db, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn get_heatmap_data_empty() {
        let (db, _dir) = test_db();
        let result = do_get_heatmap_data(&db);
        assert!(result.is_empty());
    }

    #[test]
    fn get_heatmap_data_with_gps() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[
            make_photo_gps("g1", 40.7, -74.0),
            make_photo_gps("g2", 34.0, -118.2),
            make_photo("g3", "/no-gps.jpg"),
        ])
        .unwrap();
        let result = do_get_heatmap_data(&db);
        assert_eq!(result.len(), 2);
        let ids: Vec<_> = result.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"g1"));
        assert!(ids.contains(&"g2"));
    }

    #[test]
    fn get_heatmap_data_zero_gps_excluded() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("z1", "/zero.jpg")])
            .unwrap();
        let result = do_get_heatmap_data(&db);
        assert!(result.is_empty());
    }

    #[test]
    fn list_files_ordered_by_created_desc() {
        let (mut db, _dir) = test_db();
        let mut p1 = make_photo("ph1", "/a.jpg");
        p1.created = "2026-01-01 10:00:00".to_string();
        let mut p2 = make_photo("ph2", "/b.jpg");
        p2.created = "2026-01-02 10:00:00".to_string();
        db.store_photo_batch(&[p1, p2]).unwrap();
        let result = do_list_files(&db, "", 0, 10, false, false);
        assert_eq!(result[0].id, "ph2");
        assert_eq!(result[1].id, "ph1");
    }

    #[test]
    fn list_files_ordered_by_aesthetics() {
        let (mut db, _dir) = test_db();
        let mut p1 = make_photo("ph1", "/a.jpg");
        p1.created = "2026-01-03 10:00:00".to_string();
        let mut p2 = make_photo("ph2", "/b.jpg");
        p2.created = "2026-01-02 10:00:00".to_string();
        let mut p3 = make_photo("ph3", "/c.jpg");
        p3.created = "2026-01-01 10:00:00".to_string();
        db.store_photo_batch(&[p1, p2, p3]).unwrap();
        db.update_photo_metadata("ph1", None, Some(3.0), 2);
        db.update_photo_metadata("ph2", None, Some(8.0), 2);
        let result = do_list_files_filtered(
            &db,
            "",
            0,
            10,
            false,
            false,
            vec![],
            None,
            false,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            Some("best".to_string()),
            None,
        );
        assert_eq!(result[0].id, "ph2");
        assert_eq!(result[1].id, "ph1");
        assert_eq!(result[2].id, "ph3");
    }

    #[test]
    fn list_files_ordered_by_oldest() {
        let (mut db, _dir) = test_db();
        let mut p1 = make_photo("ph1", "/a.jpg");
        p1.created = "2026-01-01 10:00:00".to_string();
        let mut p2 = make_photo("ph2", "/b.jpg");
        p2.created = "2026-01-02 10:00:00".to_string();
        db.store_photo_batch(&[p1, p2]).unwrap();
        let result = do_list_files_filtered(
            &db,
            "",
            0,
            10,
            false,
            false,
            vec![],
            None,
            false,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            Some("oldest".to_string()),
            None,
        );
        assert_eq!(result[0].id, "ph1");
        assert_eq!(result[1].id, "ph2");
    }

    #[test]
    fn list_files_nsfw_only() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[
            make_photo("safe", "/safe.jpg"),
            make_photo("risky", "/risky.jpg"),
            make_photo("clean", "/clean.jpg"),
        ])
        .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES('risky', 'nsfw', '0.95')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES('clean', 'nsfw', '0.20')",
                (),
            )
            .unwrap();
        let result = do_list_files_filtered(
            &db,
            "",
            0,
            10,
            false,
            false,
            vec![],
            None,
            false,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            true,
            false,
            false,
            false,
            None,
            None,
        );
        let ids: Vec<_> = result.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"risky"));
        assert!(!ids.contains(&"safe"));
        assert!(!ids.contains(&"clean"));
    }

    #[test]
    fn list_files_offset_beyond_total() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("ph_a", "/a.jpg")])
            .unwrap();
        let result = do_list_files(&db, "", 100, 10, false, false);
        assert!(result.is_empty());
    }

    #[test]
    fn list_files_scoped_to_album() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("ph1", "/a.jpg"), make_photo("ph2", "/b.jpg")])
            .unwrap();
        let album = db.create_album("Trip").unwrap();
        db.add_album_items(&album.id, &["ph2".to_string()]).unwrap();

        let in_album = do_list_files_filtered(
            &db,
            "",
            0,
            10,
            false,
            false,
            vec![],
            None,
            false,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            None,
            Some(album.id.clone()),
        );
        assert_eq!(in_album.len(), 1);
        assert_eq!(in_album[0].id, "ph2");

        let all = do_list_files_filtered(
            &db,
            "",
            0,
            10,
            false,
            false,
            vec![],
            None,
            false,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            None,
            None,
        );
        assert_eq!(all.len(), 2);
    }

    fn seed_faces(db: &Database) {
        for (photo, face, person) in [
            ("both", "f1", "a"),
            ("both", "f2", "b"),
            ("alice", "f3", "a"),
            ("bob", "f4", "b"),
        ] {
            db.connection
                .execute(
                    "INSERT INTO faces (photo_id, face_id, crop_path, encoded, person_id) \
                     VALUES (?1, ?2, '', 'enc', ?3)",
                    (photo, face, person),
                )
                .unwrap();
        }
    }

    fn person_ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn list_files_multi_person_and() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[
            make_photo("both", "/both.jpg"),
            make_photo("alice", "/alice.jpg"),
            make_photo("bob", "/bob.jpg"),
            make_photo("none", "/none.jpg"),
        ])
        .unwrap();
        seed_faces(&db);
        let result = do_list_files_filtered(
            &db,
            "",
            0,
            10,
            false,
            false,
            person_ids(&["a", "b"]),
            None,
            false,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            None,
            None,
        );
        let ids: Vec<_> = result.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["both"]);
    }

    #[test]
    fn list_files_multi_person_or() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[
            make_photo("both", "/both.jpg"),
            make_photo("alice", "/alice.jpg"),
            make_photo("bob", "/bob.jpg"),
            make_photo("none", "/none.jpg"),
        ])
        .unwrap();
        seed_faces(&db);
        let result = do_list_files_filtered(
            &db,
            "",
            0,
            10,
            false,
            false,
            person_ids(&["a", "b"]),
            Some("or".to_string()),
            false,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            None,
            None,
        );
        let mut ids: Vec<_> = result.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["alice", "bob", "both"]);
    }

    #[test]
    fn list_files_person_alone_single() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[
            make_photo("both", "/both.jpg"),
            make_photo("alice", "/alice.jpg"),
            make_photo("bob", "/bob.jpg"),
        ])
        .unwrap();
        seed_faces(&db);
        let result = do_list_files_filtered(
            &db,
            "",
            0,
            10,
            false,
            false,
            person_ids(&["a"]),
            None,
            true,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            None,
            None,
        );
        let ids: Vec<_> = result.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["alice"]);
    }

    #[test]
    fn list_files_person_alone_group() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[
            make_photo("both", "/both.jpg"),
            make_photo("alice", "/alice.jpg"),
            make_photo("bob", "/bob.jpg"),
        ])
        .unwrap();
        seed_faces(&db);
        let result = do_list_files_filtered(
            &db,
            "",
            0,
            10,
            false,
            false,
            person_ids(&["a", "b"]),
            None,
            true,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            None,
            None,
        );
        let ids: Vec<_> = result.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["both"]);
    }
}
