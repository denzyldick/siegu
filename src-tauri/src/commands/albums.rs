use crate::common::get_config_path;
use crate::database;
use crate::database::{Album, Database, Photo};

/// Pure business logic — testable without Tauri.
pub fn do_create_album(db: &Database, name: &str) -> Result<Album, String> {
    db.create_album(name)
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
}
