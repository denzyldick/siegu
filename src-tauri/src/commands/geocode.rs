use crate::common::get_config_path;
use crate::database;
use crate::database::{Database, SearchSuggestion};

/// Pure business logic — testable without Tauri.
pub fn do_list_objects(db: &Database, query: &str) -> Vec<SearchSuggestion> {
    db.list_objects(query)
}

#[tauri::command]
pub async fn list_objects(app: tauri::AppHandle, query: String) -> Result<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Ok("[]".to_string());
    }
    let db = database::Database::new(&path);
    Ok(serde_json::to_string(&do_list_objects(&db, &query)).unwrap_or("[]".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn list_objects_empty() {
        let (db, _dir) = test_db();
        let result = do_list_objects(&db, "anything");
        assert!(result.is_empty());
    }

    #[test]
    fn list_objects_searches_tags() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO photo (id, location) VALUES ('p1', '/a.jpg')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo (id, location) VALUES ('p2', '/b.jpg')",
                (),
            )
            .unwrap();
        db.connection
            .execute("INSERT INTO tag (name) VALUES ('cat')", ())
            .unwrap();
        db.connection
            .execute("INSERT INTO tag (name) VALUES ('dog')", ())
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo_tag (photo_id, tag_id, probability) VALUES ('p1', (SELECT id FROM tag WHERE name = 'cat'), 0.95)",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo_tag (photo_id, tag_id, probability) VALUES ('p2', (SELECT id FROM tag WHERE name = 'dog'), 0.88)",
                (),
            )
            .unwrap();
        let result = do_list_objects(&db, "cat");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "cat");
        assert_eq!(result[0].suggestion_type, "tag");
    }

    #[test]
    fn list_objects_searches_people() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO people (id, name) VALUES (?1, ?2)",
                ("p1", "Alice"),
            )
            .unwrap();
        let result = do_list_objects(&db, "Ali");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Alice");
        assert_eq!(result[0].suggestion_type, "person");
    }

    #[test]
    fn list_objects_searches_locations() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO photo (id, location) VALUES ('p1', '/a.jpg')",
                (),
            )
            .unwrap();
        db.connection
            .execute("INSERT INTO place (name) VALUES ('Paris, France')", ())
            .unwrap();
        db.connection
            .execute(
                "UPDATE photo SET place_id = (SELECT id FROM place WHERE name = 'Paris, France') WHERE id = 'p1'",
                (),
            )
            .unwrap();
        let result = do_list_objects(&db, "Paris");
        assert!(result
            .iter()
            .any(|r| r.title == "Paris, France" && r.suggestion_type == "location"));
    }

    #[test]
    fn list_objects_combined_results() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO photo (id, location) VALUES ('p1', '/a.jpg')",
                (),
            )
            .unwrap();
        db.connection
            .execute("INSERT INTO tag (name) VALUES ('mountain')", ())
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo_tag (photo_id, tag_id, probability) VALUES ('p1', (SELECT id FROM tag WHERE name = 'mountain'), 0.9)",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO people (id, name) VALUES (?1, ?2)",
                ("p1", "Mountain Mike"),
            )
            .unwrap();
        let result = do_list_objects(&db, "Mountain");
        assert!(result.len() >= 2);
    }
}
