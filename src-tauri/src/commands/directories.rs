use crate::common::get_config_path;
use crate::database;
use crate::database::Database;

/// Pure business logic — testable without Tauri.
pub fn do_add_directory(db: &Database, path: &str) {
    db.add_directory(path);
}

/// Pure business logic — testable without Tauri.
pub fn do_list_directories(db: &Database) -> Vec<String> {
    db.list_directories()
}

/// Pure business logic — testable without Tauri.
pub fn do_remove_directory(db: &Database, path: &str) {
    db.remove_directory(path.to_string());
}

/// Pure business logic — testable without Tauri.
pub fn do_remove_directory_full(db: &mut Database, path: &str) {
    db.remove_directory_full(path);
}

/// Pure business logic — testable without Tauri.
pub fn do_is_initialized(db: &Database) -> bool {
    !db.list_directories().is_empty() || db.is_onboarding_complete() || db.has_any_photos()
}

#[tauri::command]
pub async fn add_directory(app: tauri::AppHandle, path: String) {
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return;
    }
    let database = database::Database::new(&config_path);
    do_add_directory(&database, &path);
}

#[tauri::command]
pub async fn list_directories(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&do_list_directories(&database)).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn remove_directory(app: tauri::AppHandle, path: String) {
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return;
    }
    let database = database::Database::new(&config_path);
    do_remove_directory(&database, &path);
}

#[tauri::command]
pub async fn remove_directory_full(app: tauri::AppHandle, path: String) {
    let config_path = get_config_path(&app);
    if config_path.is_empty() {
        return;
    }
    let mut db = database::Database::new(&config_path);
    do_remove_directory_full(&mut db, &path);
}

#[tauri::command]
pub async fn is_initialized(app: tauri::AppHandle) -> bool {
    let path = get_config_path(&app);
    if path.is_empty() {
        return false;
    }
    let database = database::Database::new(&path);
    do_is_initialized(&database)
}

#[tauri::command]
pub async fn mark_onboarding_complete(app: tauri::AppHandle) {
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let database = database::Database::new(&path);
    database.set_onboarding_complete();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn add_directory_and_list() {
        let (db, _dir) = test_db();
        do_add_directory(&db, "/home/photos");
        do_add_directory(&db, "/home/videos");
        let dirs = do_list_directories(&db);
        assert!(dirs.contains(&"/home/photos".to_string()));
        assert!(dirs.contains(&"/home/videos".to_string()));
    }

    #[test]
    fn list_directories_empty() {
        let (db, _dir) = test_db();
        assert!(do_list_directories(&db).is_empty());
    }

    #[test]
    fn remove_directory() {
        let (db, _dir) = test_db();
        do_add_directory(&db, "/tmp/photos");
        do_remove_directory(&db, "/tmp/photos");
        assert!(do_list_directories(&db).is_empty());
    }

    #[test]
    fn remove_nonexistent_directory_no_panic() {
        let (db, _dir) = test_db();
        do_remove_directory(&db, "/does/not/exist");
    }

    #[test]
    fn is_initialized_false() {
        let (db, _dir) = test_db();
        assert!(!do_is_initialized(&db));
    }

    #[test]
    fn is_initialized_true() {
        let (db, _dir) = test_db();
        do_add_directory(&db, "/photos");
        assert!(do_is_initialized(&db));
    }

    #[test]
    fn is_initialized_true_when_onboarding_complete() {
        let (db, _dir) = test_db();
        db.set_onboarding_complete();
        assert!(do_is_initialized(&db));
    }

    #[test]
    fn is_initialized_true_when_photos_exist() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("p1", "/tmp/p1.jpg")])
            .unwrap();
        assert!(do_is_initialized(&db));
    }

    #[test]
    fn add_directory_duplicate_not_deduped() {
        let (db, _dir) = test_db();
        do_add_directory(&db, "/photos");
        do_add_directory(&db, "/photos");
        let dirs = do_list_directories(&db);
        assert_eq!(dirs.len(), 2);
    }

    #[test]
    fn remove_directory_full_removes_photos() {
        let (mut db, _dir) = test_db();
        do_add_directory(&mut db, "/photos");
        db.store_photo_batch(&[make_photo("ph1", "/photos/a.jpg")])
            .unwrap();
        do_remove_directory_full(&mut db, "/photos");
        assert!(do_list_directories(&db).is_empty());
        assert!(do_list_files(&db, "", 0, 10, false, false).is_empty());
    }

    #[test]
    fn remove_directory_does_not_remove_photos() {
        let (mut db, _dir) = test_db();
        do_add_directory(&db, "/photos");
        db.store_photo_batch(&[make_photo("ph1", "/photos/a.jpg")])
            .unwrap();
        do_remove_directory(&db, "/photos");
        assert!(do_list_directories(&db).is_empty());
        assert!(!do_list_files(&db, "", 0, 10, false, false).is_empty());
    }

    fn do_list_files(
        db: &crate::database::Database,
        query: &str,
        offset: usize,
        limit: usize,
        favorites_only: bool,
        videos_only: bool,
    ) -> Vec<crate::database::Photo> {
        db.list_photos(query, offset, limit, favorites_only, videos_only)
    }
}
