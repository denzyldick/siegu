use crate::common::{emit_log, get_config_path};
use crate::database;
use crate::database::{Database, LogEntry};

/// Pure business logic — testable without Tauri.
pub fn do_get_logs(db: &Database, limit: usize) -> Vec<LogEntry> {
    db.get_logs(limit)
}

/// Pure business logic — testable without Tauri.
pub fn do_clear_logs(db: &Database) {
    db.clear_logs();
}

/// Pure business logic — testable without Tauri.
pub fn do_get_last_scan_time(db: &Database) -> String {
    db.get_last_scan_time().unwrap_or("Never".to_string())
}

/// Pure business logic — testable without Tauri.
pub fn do_cleanup_database(path: &str, confirm: bool) -> bool {
    if !confirm {
        return false;
    }
    let db_path = std::path::Path::new(path).join("siegu.db");
    if db_path.exists() {
        let _ = std::fs::remove_file(&db_path);
        true
    } else {
        false
    }
}

/// Pure business logic — testable without Tauri.
pub fn do_resolve_photo_locations(db: &Database) -> usize {
    let sql = "SELECT id, latitude, longitude FROM photo WHERE (latitude != 0.0 OR longitude != 0.0) AND id NOT IN (SELECT photo_id FROM properties WHERE key = 'location_name')";
    let mut resolved = 0usize;
    if let Ok(mut stmt) = db.connection.prepare(sql) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0),
            ))
        }) {
            for row in rows.flatten() {
                let (id, lat, lon) = row;
                if let Some((city, country)) = siegu_core::geocode::find_nearest_city(lat, lon) {
                    let location_name = format!("{}, {}", city, country);
                    let _ = db.connection.execute(
                        "INSERT OR REPLACE INTO properties (photo_id, key, value) VALUES (?1, 'location_name', ?2)",
                        rusqlite::params![id, location_name],
                    );
                    resolved += 1;
                }
            }
        }
    }
    resolved
}

/// Pure business logic — testable without Tauri.
pub fn do_get_location_names(db: &Database) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(mut stmt) = db
        .connection
        .prepare("SELECT DISTINCT value FROM properties WHERE key = 'location_name' ORDER BY value")
    {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                names.push(row);
            }
        }
    }
    names
}

#[tauri::command]
pub async fn get_last_scan_time(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "Never".to_string();
    }
    let database = database::Database::new(&path);
    do_get_last_scan_time(&database)
}

#[tauri::command]
pub async fn cleanup_database(app: tauri::AppHandle, confirm: bool) {
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    do_cleanup_database(&path, confirm);
}

#[tauri::command]
pub async fn get_logs(app: tauri::AppHandle, limit: usize) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&do_get_logs(&database, limit)).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn clear_logs(app: tauri::AppHandle) {
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let database = database::Database::new(&path);
    do_clear_logs(&database);
}

#[tauri::command]
pub async fn resolve_photo_locations(app: tauri::AppHandle) -> Result<(), String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Err("Config path empty".to_string());
    }
    let db = database::Database::new(&path);
    let resolved = do_resolve_photo_locations(&db);
    emit_log(&app, format!("Resolved {} photo locations", resolved));
    Ok(())
}

#[tauri::command]
pub async fn get_location_names(app: tauri::AppHandle) -> Vec<String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Vec::new();
    }
    let db = database::Database::new(&path);
    do_get_location_names(&db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn get_logs_empty() {
        let (db, _dir) = test_db();
        assert!(do_get_logs(&db, 100).is_empty());
    }

    #[test]
    fn get_logs_with_entries() {
        let (db, _dir) = test_db();
        db.store_log("info", "Hello world");
        db.store_log("error", "Something failed");
        let logs = do_get_logs(&db, 100);
        assert_eq!(logs.len(), 2);
        assert!(logs.iter().any(|l| l.message == "Hello world"));
        assert!(logs.iter().any(|l| l.level == "error"));
    }

    #[test]
    fn get_logs_respects_limit() {
        let (db, _dir) = test_db();
        for i in 0..10 {
            db.store_log("info", &format!("log {i}"));
        }
        let logs = do_get_logs(&db, 3);
        assert_eq!(logs.len(), 3);
    }

    #[test]
    fn clear_logs_removes_all() {
        let (db, _dir) = test_db();
        db.store_log("info", "msg1");
        db.store_log("info", "msg2");
        do_clear_logs(&db);
        assert!(do_get_logs(&db, 100).is_empty());
    }

    #[test]
    fn get_last_scan_time_never() {
        let (db, _dir) = test_db();
        assert_eq!(do_get_last_scan_time(&db), "Never");
    }

    #[test]
    fn get_last_scan_time_after_set() {
        let (db, _dir) = test_db();
        db.set_last_scan_time("2026-01-15 10:00:00".to_string());
        assert_eq!(do_get_last_scan_time(&db), "2026-01-15 10:00:00");
    }

    #[test]
    fn cleanup_database_confirm_false_noop() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("siegu.db");
        std::fs::write(&db_path, "fake").unwrap();
        assert!(!do_cleanup_database(
            &dir.path().display().to_string(),
            false
        ));
        assert!(db_path.exists());
    }

    #[test]
    fn cleanup_database_confirm_true_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("siegu.db");
        std::fs::write(&db_path, "fake").unwrap();
        assert!(do_cleanup_database(&dir.path().display().to_string(), true));
        assert!(!db_path.exists());
    }

    #[test]
    fn cleanup_database_no_file_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!do_cleanup_database(
            &dir.path().display().to_string(),
            true
        ));
    }

    #[test]
    fn get_location_names_empty() {
        let (db, _dir) = test_db();
        assert!(do_get_location_names(&db).is_empty());
    }

    #[test]
    fn get_location_names_with_data() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES (?1, 'location_name', ?2)",
                ("p1", "Paris, France"),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES (?1, 'location_name', ?2)",
                ("p2", "Tokyo, Japan"),
            )
            .unwrap();
        let names = do_get_location_names(&db);
        assert_eq!(names.len(), 2);
        assert_eq!(names[0], "Paris, France");
        assert_eq!(names[1], "Tokyo, Japan");
    }

    #[test]
    fn get_location_names_distinct() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES (?1, 'location_name', ?2)",
                ("p1", "Paris, France"),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES (?1, 'location_name', ?2)",
                ("p2", "Paris, France"),
            )
            .unwrap();
        let names = do_get_location_names(&db);
        assert_eq!(names.len(), 1);
    }
}
