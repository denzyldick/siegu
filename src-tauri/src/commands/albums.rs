use crate::common::get_config_path;
use crate::database;
use crate::database::{Album, AlbumKind, Database, Photo, PhotoFilter};

/// Pure business logic — testable without Tauri.
pub fn do_create_album(db: &Database, name: &str) -> Result<Album, String> {
    db.create_album(name)
}

/// Pure business logic — testable without Tauri.
pub fn do_create_smart_album(
    db: &Database,
    name: &str,
    rule: &PhotoFilter,
    kind: &str,
) -> Result<Album, String> {
    db.create_smart_album(name, rule, AlbumKind::parse(kind))
}

/// Pure business logic — testable without Tauri.
pub fn do_update_smart_album_rule(
    db: &Database,
    album_id: &str,
    rule: &PhotoFilter,
) -> Result<(), String> {
    db.update_smart_album_rule(album_id, rule)
}

/// Pure business logic — testable without Tauri.
pub fn do_get_album_sections(db: &Database) -> Vec<database::AlbumSection> {
    db.get_album_sections()
}

/// Pure business logic — testable without Tauri.
pub fn do_rename_album(db: &Database, album_id: &str, name: &str) -> Result<(), String> {
    db.rename_album(album_id, name)
}

/// Pure business logic — testable without Tauri.
pub fn do_delete_album(db: &Database, album_id: &str) -> Result<(), String> {
    db.delete_album(album_id)
}

/// Pure business logic — testable without Tauri.
pub fn do_list_albums(db: &Database) -> Vec<Album> {
    db.list_albums()
}

/// Pure business logic — testable without Tauri.
pub fn do_get_album(db: &Database, album_id: &str) -> Option<Album> {
    db.get_album(album_id)
}

/// Pure business logic — testable without Tauri.
pub fn do_add_album_items(
    db: &Database,
    album_id: &str,
    photo_ids: &[String],
) -> Result<(), String> {
    db.add_album_items(album_id, photo_ids)
}

/// Pure business logic — testable without Tauri.
pub fn do_remove_album_items(
    db: &Database,
    album_id: &str,
    photo_ids: &[String],
) -> Result<(), String> {
    db.remove_album_items(album_id, photo_ids)
}

/// Pure business logic — testable without Tauri.
pub fn do_reorder_album(
    db: &Database,
    album_id: &str,
    ordered_ids: &[String],
) -> Result<(), String> {
    db.reorder_album(album_id, ordered_ids)
}

/// Pure business logic — testable without Tauri.
pub fn do_get_album_contents(
    db: &Database,
    album_id: &str,
    offset: usize,
    limit: usize,
) -> Vec<Photo> {
    db.get_album_contents(album_id, offset, limit)
}

#[tauri::command]
pub async fn create_album(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("No config path".to_string());
    }
    let db = database::Database::new(&path);
    let album = do_create_album(&db, &name)?;
    serde_json::to_string(&album).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_smart_album(
    app: tauri::AppHandle,
    name: String,
    rule: String,
    kind: String,
) -> Result<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("No config path".to_string());
    }
    let rule = serde_json::from_str::<PhotoFilter>(&rule).map_err(|e| e.to_string())?;
    let db = database::Database::new(&path);
    let album = do_create_smart_album(&db, &name, &rule, &kind)?;
    serde_json::to_string(&album).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_smart_album_rule(
    app: tauri::AppHandle,
    album_id: String,
    rule: String,
) -> Result<(), String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("No config path".to_string());
    }
    let rule = serde_json::from_str::<PhotoFilter>(&rule).map_err(|e| e.to_string())?;
    let db = database::Database::new(&path);
    do_update_smart_album_rule(&db, &album_id, &rule)
}

#[tauri::command]
pub async fn get_album_sections(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let db = database::Database::new(&path);
    serde_json::to_string(&do_get_album_sections(&db)).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn rename_album(
    app: tauri::AppHandle,
    album_id: String,
    name: String,
) -> Result<(), String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("No config path".to_string());
    }
    let db = database::Database::new(&path);
    do_rename_album(&db, &album_id, &name)
}

#[tauri::command]
pub async fn delete_album(app: tauri::AppHandle, album_id: String) -> Result<(), String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("No config path".to_string());
    }
    let db = database::Database::new(&path);
    do_delete_album(&db, &album_id)
}

#[tauri::command]
pub async fn list_albums(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let db = database::Database::new(&path);
    serde_json::to_string(&do_list_albums(&db)).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn get_album(app: tauri::AppHandle, album_id: String) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "null".to_string();
    }
    let db = database::Database::new(&path);
    match do_get_album(&db, &album_id) {
        Some(album) => serde_json::to_string(&album).unwrap_or("null".to_string()),
        None => "null".to_string(),
    }
}

#[tauri::command]
pub async fn add_album_items(
    app: tauri::AppHandle,
    album_id: String,
    photo_ids: Vec<String>,
) -> Result<(), String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("No config path".to_string());
    }
    let db = database::Database::new(&path);
    do_add_album_items(&db, &album_id, &photo_ids)
}

#[tauri::command]
pub async fn remove_album_items(
    app: tauri::AppHandle,
    album_id: String,
    photo_ids: Vec<String>,
) -> Result<(), String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("No config path".to_string());
    }
    let db = database::Database::new(&path);
    do_remove_album_items(&db, &album_id, &photo_ids)
}

#[tauri::command]
pub async fn reorder_album(
    app: tauri::AppHandle,
    album_id: String,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("No config path".to_string());
    }
    let db = database::Database::new(&path);
    do_reorder_album(&db, &album_id, &ordered_ids)
}

#[tauri::command]
pub async fn get_album_contents(
    app: tauri::AppHandle,
    album_id: String,
    offset: usize,
    limit: usize,
) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let db = database::Database::new(&path);
    serde_json::to_string(&do_get_album_contents(&db, &album_id, offset, limit))
        .unwrap_or("[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    fn photo_ids(db: &Database) -> Vec<String> {
        for i in 0..5 {
            let _ = db.connection.execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, '')",
                rusqlite::params![
                    format!("p{i}"),
                    format!("/a{i}.jpg"),
                    format!("2026-01-0{}", i + 1)
                ],
            );
        }
        db.list_photos("", 0, 50, false, false)
            .into_iter()
            .map(|p| p.id)
            .collect()
    }

    #[test]
    fn create_and_list_albums() {
        let (db, _dir) = test_db();
        let album = do_create_album(&db, " Summer  2026 ").unwrap();
        assert_eq!(album.name, "Summer  2026".trim());
        assert_eq!(album.item_count, 0);
        let albums = do_list_albums(&db);
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].id, album.id);
    }

    #[test]
    fn create_album_rejects_blank_name() {
        let (db, _dir) = test_db();
        assert!(do_create_album(&db, "   ").is_err());
        assert!(do_create_album(&db, "").is_err());
    }

    #[test]
    fn rename_album_updates_name() {
        let (db, _dir) = test_db();
        let album = do_create_album(&db, "Holiday").unwrap();
        do_rename_album(&db, &album.id, "Trip").unwrap();
        assert_eq!(do_get_album(&db, &album.id).unwrap().name, "Trip");
    }

    #[test]
    fn rename_album_rejects_blank() {
        let (db, _dir) = test_db();
        let album = do_create_album(&db, "Holiday").unwrap();
        assert!(do_rename_album(&db, &album.id, " ").is_err());
        assert_eq!(do_get_album(&db, &album.id).unwrap().name, "Holiday");
    }

    #[test]
    fn delete_album_removes_items() {
        let (db, _dir) = test_db();
        let ids = photo_ids(&db);
        assert!(!ids.is_empty());
        let album = do_create_album(&db, "To Delete").unwrap();
        do_add_album_items(&db, &album.id, &ids).unwrap();
        assert_eq!(
            do_get_album(&db, &album.id).unwrap().item_count,
            ids.len() as i64
        );
        do_delete_album(&db, &album.id).unwrap();
        assert!(do_get_album(&db, &album.id).is_none());
        assert_eq!(do_list_albums(&db).len(), 0);
    }

    #[test]
    fn add_items_updates_count_and_cover() {
        let (db, _dir) = test_db();
        let ids = photo_ids(&db);
        let album = do_create_album(&db, "Covers").unwrap();
        do_add_album_items(&db, &album.id, &ids).unwrap();
        let album = do_get_album(&db, &album.id).unwrap();
        assert_eq!(album.item_count, ids.len() as i64);
        // Cover is the last photo added.
        assert_eq!(
            album.cover_photo_id.as_deref(),
            ids.last().map(|s| s.as_str())
        );
    }

    #[test]
    fn duplicate_photos_ignored() {
        let (db, _dir) = test_db();
        let ids = photo_ids(&db);
        let album = do_create_album(&db, "Dupes").unwrap();
        do_add_album_items(&db, &album.id, &ids).unwrap();
        do_add_album_items(&db, &album.id, &ids).unwrap();
        assert_eq!(
            do_get_album(&db, &album.id).unwrap().item_count,
            ids.len() as i64
        );
    }

    #[test]
    fn remove_items_updates_cover() {
        let (db, _dir) = test_db();
        let ids = photo_ids(&db);
        let album = do_create_album(&db, "Remove").unwrap();
        do_add_album_items(&db, &album.id, &ids).unwrap();
        do_remove_album_items(&db, &album.id, &ids[ids.len() - 1..]).unwrap();
        let album = do_get_album(&db, &album.id).unwrap();
        assert_eq!(album.item_count, ids.len() as i64 - 1);
        assert_eq!(
            album.cover_photo_id.as_deref(),
            ids[ids.len() - 2..].first().map(|s| s.as_str())
        );
    }

    #[test]
    fn reorder_changes_contents_order() {
        let (db, _dir) = test_db();
        let ids = photo_ids(&db);
        let album = do_create_album(&db, "Order").unwrap();
        do_add_album_items(&db, &album.id, &ids).unwrap();
        let mut reversed = ids.clone();
        reversed.reverse();
        do_reorder_album(&db, &album.id, &reversed).unwrap();
        let contents = do_get_album_contents(&db, &album.id, 0, 100);
        assert_eq!(contents.len(), ids.len());
        assert_eq!(contents[0].id, reversed[0]);
        assert_eq!(contents[ids.len() - 1].id, reversed[ids.len() - 1]);
    }

    #[test]
    fn contents_respects_pagination() {
        let (db, _dir) = test_db();
        let ids = photo_ids(&db);
        let album = do_create_album(&db, "Paged").unwrap();
        do_add_album_items(&db, &album.id, &ids).unwrap();
        let page = do_get_album_contents(&db, &album.id, 1, 2);
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].id, ids[1]);
        assert_eq!(page[1].id, ids[2]);
    }

    #[test]
    fn contents_rejects_missing_album() {
        let (db, _dir) = test_db();
        assert!(do_add_album_items(&db, "nope", &[]).is_err());
        assert!(do_get_album_contents(&db, "nope", 0, 50).is_empty());
    }

    #[test]
    fn empty_album_has_no_contents() {
        let (db, _dir) = test_db();
        let album = do_create_album(&db, "Empty").unwrap();
        assert_eq!(do_get_album_contents(&db, &album.id, 0, 50).len(), 0);
    }

    fn rule(ids: &[&str]) -> database::PhotoFilter {
        database::PhotoFilter {
            person_ids: ids.iter().map(|s| s.to_string()).collect(),
            ..database::PhotoFilter::default()
        }
    }

    #[test]
    fn create_smart_album_stores_rule_and_counts() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO people (id, name) VALUES ('person-1', 'Alice')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES ('p1', '/p1.jpg', '2026-01-01', 'enc')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO faces (photo_id, face_id, person_id) VALUES ('p1', 'f1', 'person-1')",
                (),
            )
            .unwrap();

        let album = do_create_smart_album(&db, "Alice", &rule(&["person-1"]), "smart").unwrap();
        assert_eq!(album.kind, AlbumKind::Smart);
        assert_eq!(album.item_count, 1);
        assert_eq!(do_get_album_contents(&db, &album.id, 0, 50).len(), 1);
    }

    #[test]
    fn smart_album_rule_can_be_updated() {
        let (db, _dir) = test_db();
        let album = do_create_smart_album(&db, "Empty", &rule(&[]), "smart").unwrap();
        assert_eq!(album.item_count, 0);
        assert!(do_update_smart_album_rule(&db, &album.id, &rule(&["person-1"])).is_ok());
        let updated = do_get_album(&db, &album.id).unwrap();
        assert_eq!(updated.kind, AlbumKind::Smart);
    }

    #[test]
    fn add_items_rejected_for_smart_album() {
        let (db, _dir) = test_db();
        let album = do_create_smart_album(&db, "Auto", &rule(&[]), "smart").unwrap();
        assert!(do_add_album_items(&db, &album.id, &["p1".to_string()]).is_err());
    }

    #[test]
    fn trips_are_detected_and_upserted() {
        let (db, _dir) = test_db();
        for (id, created) in [
            ("t1", "2026-05-01 09:00:00"),
            ("t2", "2026-05-02 10:00:00"),
            ("t3", "2026-05-03 11:00:00"),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, '')",
                    (id, format!("/{id}.jpg"), created),
                )
                .unwrap();
        }
        assert_eq!(db.sync_trips(), 1);
        let trips = db.list_albums_by_kind(AlbumKind::Trip);
        assert_eq!(trips.len(), 1);
        assert_eq!(trips[0].name, "Trip · 2026");
        assert_eq!(trips[0].item_count, 3);

        // Re-running is idempotent (no duplicate trip rows).
        assert_eq!(db.sync_trips(), 1);
        assert_eq!(db.list_albums_by_kind(AlbumKind::Trip).len(), 1);

        // A user rename survives resyncs.
        do_rename_album(&db, &trips[0].id, "My Holiday").unwrap();
        assert_eq!(db.sync_trips(), 1);
        assert_eq!(
            db.list_albums_by_kind(AlbumKind::Trip)[0].name,
            "My Holiday"
        );

        // Deleting a trip dismisses it; it does not come back.
        do_delete_album(&db, &trips[0].id).unwrap();
        assert_eq!(db.sync_trips(), 0);
        assert_eq!(db.list_albums_by_kind(AlbumKind::Trip).len(), 0);

        // Stale trips are cleaned up when their photos no longer match.
        db.connection
            .execute("DELETE FROM photo WHERE id IN ('t1','t2')", ())
            .unwrap();
        assert_eq!(db.sync_trips(), 0);
        assert_eq!(db.list_albums_by_kind(AlbumKind::Trip).len(), 0);
    }

    #[test]
    fn sections_include_people_places_trips_smart_manual() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO people (id, name) VALUES ('person-1', 'Alice')",
                (),
            )
            .unwrap();
        for (id, created) in [
            ("p1", "2026-05-01"),
            ("p2", "2026-05-02"),
            ("p3", "2026-05-03"),
        ] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, ?3, 'enc')",
                    (id, format!("/{id}.jpg"), created),
                )
                .unwrap();
        }
        db.connection
            .execute(
                "INSERT INTO faces (photo_id, face_id, person_id) VALUES ('p1', 'f1', 'person-1')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p1', 'location_name', 'Paris, France')",
                (),
            )
            .unwrap();
        do_create_album(&db, "Manual").unwrap();
        do_create_smart_album(&db, "Smart", &rule(&[]), "smart").unwrap();

        let sections = do_get_album_sections(&db);
        let ids: Vec<String> = sections.iter().map(|s| s.id.clone()).collect();
        assert_eq!(ids, vec!["people", "places", "trips", "smart", "albums"]);
        assert_eq!(sections[0].items.len(), 1);
        assert_eq!(sections[0].items[0].name, "Alice");
        assert_eq!(sections[1].items[0].name, "Paris, France");
        assert_eq!(sections[2].items.len(), 1);
        assert_eq!(sections[2].items[0].kind, "trip");
        assert_eq!(sections[3].items[0].kind, "smart");
        assert_eq!(sections[4].items[0].kind, "manual");
    }
}
