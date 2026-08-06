use crate::common::get_config_path;
use crate::database;
use crate::database::{Database, Face, FaceWithPerson, PersonWithFace, SearchSuggestion};
use crate::ml;

/// Pure business logic — testable without Tauri.
pub fn do_get_people(db: &Database) -> Vec<PersonWithFace> {
    db.get_people()
}

/// Pure business logic — testable without Tauri.
pub fn do_get_unnamed_faces(db: &Database) -> Vec<PersonWithFace> {
    db.get_anonymous_people_groups()
}

/// Pure business logic — testable without Tauri.
pub fn do_assign_name_to_face(db: &Database, face_id: &str, name: &str) -> String {
    db.assign_name_to_face(face_id, name)
}

/// Pure business logic — testable without Tauri.
pub fn do_get_person_photos(
    db: &Database,
    person_id: &str,
    offset: usize,
    limit: usize,
) -> Vec<crate::database::Photo> {
    db.get_photos_for_person(person_id, offset, limit)
}

/// Pure business logic — testable without Tauri.
pub fn do_get_person_faces(db: &Database, person_id: &str) -> Vec<Face> {
    db.get_person_faces(person_id)
}

/// Pure business logic — testable without Tauri.
pub fn do_get_faces_for_photo(db: &Database, photo_id: &str) -> Vec<FaceWithPerson> {
    db.get_faces_for_photo(photo_id)
}

/// Pure business logic — testable without Tauri.
pub fn do_delete_face(db: &Database, face_id: &str) {
    let _ = db
        .connection
        .execute("DELETE FROM faces WHERE face_id = ?1", [face_id]);
}

/// Pure business logic — testable without Tauri.
pub fn do_get_top_tags(db: &Database) -> Vec<SearchSuggestion> {
    let mut suggestions: Vec<SearchSuggestion> = Vec::new();

    if let Ok(mut stmt) = db
        .connection
        .prepare("SELECT class FROM object GROUP BY class ORDER BY COUNT(*) DESC LIMIT 5")
    {
        if let Ok(iter) = stmt.query_map([], |row| {
            Ok(SearchSuggestion {
                title: row.get(0)?,
                suggestion_type: "tag".to_string(),
            })
        }) {
            for item in iter.flatten() {
                suggestions.push(item);
            }
        }
    }

    if let Ok(mut stmt) = db
        .connection
        .prepare("SELECT value FROM properties WHERE key = 'location_name' GROUP BY value ORDER BY COUNT(*) DESC LIMIT 5")
    {
        if let Ok(iter) = stmt.query_map([], |row| {
            Ok(SearchSuggestion {
                title: row.get(0)?,
                suggestion_type: "location".to_string(),
            })
        }) {
            for item in iter.flatten() {
                suggestions.push(item);
            }
        }
    }

    if let Ok(mut stmt) = db
        .connection
        .prepare("SELECT name FROM people WHERE name IS NOT NULL GROUP BY name ORDER BY COUNT(*) DESC LIMIT 5")
    {
        if let Ok(iter) = stmt.query_map([], |row| {
            Ok(SearchSuggestion {
                title: row.get(0)?,
                suggestion_type: "person".to_string(),
            })
        }) {
            for item in iter.flatten() {
                suggestions.push(item);
            }
        }
    }

    suggestions
}

/// Pure business logic — testable without Tauri.
pub fn do_merge_people(db: &Database, from_id: &str, to_id: &str) {
    db.merge_people(from_id, to_id);
}

/// Pure business logic — testable without Tauri.
pub fn do_rename_person(db: &Database, id: &str, new_name: &str) {
    db.rename_person(id, new_name);
}

#[tauri::command]
pub async fn get_people(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&do_get_people(&database)).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn get_unnamed_faces(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&do_get_unnamed_faces(&database)).unwrap_or("[]".to_string())
}

#[tauri::command]
pub fn assign_name_to_face(
    app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
    face_id: String,
    name: String,
) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "".to_string();
    }
    let database = database::Database::new(&path);
    let id = do_assign_name_to_face(&database, &face_id, &name);

    let _ = state.tx.blocking_send(ml::Job::ProcessAll);
    id
}

#[tauri::command]
pub async fn get_person_photos(
    app: tauri::AppHandle,
    person_id: String,
    offset: usize,
    limit: usize,
) -> Result<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Ok("[]".to_string());
    }
    let database = database::Database::new(&path);
    Ok(
        serde_json::to_string(&do_get_person_photos(&database, &person_id, offset, limit))
            .unwrap_or("[]".to_string()),
    )
}

#[tauri::command]
pub async fn get_person_faces(app: tauri::AppHandle, person_id: String) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&do_get_person_faces(&database, &person_id)).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn get_faces_for_photo(app: tauri::AppHandle, photo_id: String) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&do_get_faces_for_photo(&database, &photo_id)).unwrap_or("[]".to_string())
}

#[tauri::command]
pub async fn delete_face(app: tauri::AppHandle, face_id: String) {
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let database = database::Database::new(&path);
    do_delete_face(&database, &face_id);
}

#[tauri::command]
pub async fn get_top_tags(app: tauri::AppHandle) -> String {
    let path = get_config_path(&app);
    if path.is_empty() {
        return "[]".to_string();
    }
    let database = database::Database::new(&path);
    serde_json::to_string(&do_get_top_tags(&database)).unwrap_or("[]".to_string())
}

#[tauri::command]
pub fn merge_people(
    app: tauri::AppHandle,
    state: tauri::State<'_, ml::MlContext>,
    from_id: String,
    to_id: String,
) {
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let db = database::Database::new(&path);
    do_merge_people(&db, &from_id, &to_id);

    let _ = state.tx.blocking_send(ml::Job::ProcessAll);
}

#[tauri::command]
pub async fn rename_person(app: tauri::AppHandle, id: String, new_name: String) {
    let path = get_config_path(&app);
    if path.is_empty() {
        return;
    }
    let db = database::Database::new(&path);
    do_rename_person(&db, &id, &new_name);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use siegu_core::database::Face;

    fn make_face(photo_id: &str, face_id: &str, person_id: Option<&str>) -> Face {
        Face {
            photo_id: photo_id.to_string(),
            face_id: face_id.to_string(),
            crop_path: format!("/crops/{face_id}.jpg"),
            encoded: String::new(),
            embedding: vec![0.0; 512],
            person_id: person_id.map(|s| s.to_string()),
        }
    }

    #[test]
    fn get_people_empty() {
        let (db, _dir) = test_db();
        assert!(do_get_people(&db).is_empty());
    }

    #[test]
    fn get_people_with_named() {
        let (db, _dir) = test_db();
        let person_id = db.assign_name_to_face("face1", "Alice");
        let people = do_get_people(&db);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].name, "Alice");
        assert_eq!(people[0].id, person_id);
    }

    #[test]
    fn assign_name_to_face_new_person() {
        let (db, _dir) = test_db();
        db.store_face(make_face("p1", "f1", None));
        let person_id = do_assign_name_to_face(&db, "f1", "Bob");
        assert!(!person_id.is_empty());
        let people = do_get_people(&db);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].name, "Bob");
    }

    #[test]
    fn assign_name_to_face_existing_person_merges() {
        let (db, _dir) = test_db();
        db.store_face(make_face("p1", "f1", None));
        let alice_id = do_assign_name_to_face(&db, "f1", "Alice");
        db.store_face(make_face("p2", "f2", None));
        let alice_id2 = do_assign_name_to_face(&db, "f2", "Alice");
        assert_eq!(alice_id, alice_id2);
        let people = do_get_people(&db);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].face_count, 2);
    }

    #[test]
    fn assign_name_to_face_uses_existing_anon_person() {
        let (db, _dir) = test_db();
        let anon_id = db.create_anonymous_person(&vec![0.1; 512]);
        db.store_face(make_face("p1", "f1", Some(&anon_id)));
        let returned_id = do_assign_name_to_face(&db, "f1", "Charlie");
        assert_eq!(returned_id, anon_id);
        assert_eq!(do_get_people(&db).len(), 1);
        assert!(do_get_unnamed_faces(&db).is_empty());
    }

    #[test]
    fn get_person_photos_empty() {
        let (db, _dir) = test_db();
        assert!(do_get_person_photos(&db, "nonexistent", 0, 50).is_empty());
    }

    #[test]
    fn get_person_photos_with_photos() {
        let (mut db, _dir) = test_db();
        db.store_photo_batch(&[make_photo("ph1", "/a.jpg")])
            .unwrap();
        db.store_face(make_face("ph1", "f1", None));
        let person_id = do_assign_name_to_face(&db, "f1", "Dave");
        let photos = do_get_person_photos(&db, &person_id, 0, 50);
        assert_eq!(photos.len(), 1);
        assert_eq!(photos[0].id, "ph1");
    }

    #[test]
    fn get_person_photos_paginated() {
        let (mut db, _dir) = test_db();
        let photos: Vec<_> = (0..5)
            .map(|i| make_photo(&format!("pp{i}"), &format!("/{i}.jpg")))
            .collect();
        db.store_photo_batch(&photos).unwrap();
        for i in 0..5 {
            db.store_face(make_face(&format!("pp{i}"), &format!("f{i}"), None));
        }
        let person_id = do_assign_name_to_face(&db, "f0", "Paged");
        for i in 1..5 {
            do_assign_name_to_face(&db, &format!("f{i}"), "Paged");
        }
        let first = do_get_person_photos(&db, &person_id, 0, 2);
        let second = do_get_person_photos(&db, &person_id, 2, 2);
        let tail = do_get_person_photos(&db, &person_id, 4, 2);
        assert_eq!(first.len(), 2);
        assert_eq!(second.len(), 2);
        assert_eq!(tail.len(), 1);
        let mut all: Vec<_> = first.into_iter().chain(second).chain(tail).collect();
        all.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn get_person_faces_empty() {
        let (db, _dir) = test_db();
        assert!(do_get_person_faces(&db, "nonexistent").is_empty());
    }

    #[test]
    fn get_person_faces_with_faces() {
        let (db, _dir) = test_db();
        db.store_face(make_face("p1", "f1", None));
        let person_id = do_assign_name_to_face(&db, "f1", "Eve");
        let faces = do_get_person_faces(&db, &person_id);
        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].face_id, "f1");
    }

    #[test]
    fn get_faces_for_photo_empty() {
        let (db, _dir) = test_db();
        assert!(do_get_faces_for_photo(&db, "nope").is_empty());
    }

    #[test]
    fn get_faces_for_photo_with_faces() {
        let (db, _dir) = test_db();
        db.store_face(make_face("p1", "f1", None));
        db.store_face(make_face("p1", "f2", None));
        let faces = do_get_faces_for_photo(&db, "p1");
        assert_eq!(faces.len(), 2);
        assert!(faces.iter().all(|f| f.photo_id == "p1"));
    }

    #[test]
    fn delete_face_removes_face() {
        let (db, _dir) = test_db();
        db.store_face(make_face("p1", "f1", None));
        assert_eq!(do_get_faces_for_photo(&db, "p1").len(), 1);
        do_delete_face(&db, "f1");
        assert!(do_get_faces_for_photo(&db, "p1").is_empty());
    }

    #[test]
    fn delete_face_nonexistent_no_panic() {
        let (db, _dir) = test_db();
        do_delete_face(&db, "does_not_exist");
    }

    #[test]
    fn merge_people_moves_faces() {
        let (db, _dir) = test_db();
        db.store_face(make_face("p1", "f1", None));
        let from_id = do_assign_name_to_face(&db, "f1", "OldName");
        db.store_face(make_face("p2", "f2", None));
        let to_id = do_assign_name_to_face(&db, "f2", "NewName");
        do_merge_people(&db, &from_id, &to_id);
        let people = do_get_people(&db);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].name, "NewName");
        assert_eq!(people[0].face_count, 2);
    }

    #[test]
    fn rename_person() {
        let (db, _dir) = test_db();
        db.store_face(make_face("p1", "f1", None));
        let person_id = do_assign_name_to_face(&db, "f1", "OldName");
        do_rename_person(&db, &person_id, "NewName");
        let people = do_get_people(&db);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].name, "NewName");
    }

    #[test]
    fn get_top_tags_empty() {
        let (db, _dir) = test_db();
        assert!(do_get_top_tags(&db).is_empty());
    }

    #[test]
    fn get_top_tags_with_objects() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                ("p1", "cat", "0.95"),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                ("p2", "dog", "0.88"),
            )
            .unwrap();
        let tags = do_get_top_tags(&db);
        let tag_titles: Vec<_> = tags.iter().map(|t| t.title.as_str()).collect();
        assert!(tag_titles.contains(&"cat"));
        assert!(tag_titles.contains(&"dog"));
        assert!(tags.iter().all(|t| t.suggestion_type == "tag"));
    }

    #[test]
    fn get_top_tags_with_people() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO people (id, name) VALUES (?1, ?2)",
                ("p1", "Alice"),
            )
            .unwrap();
        let tags = do_get_top_tags(&db);
        assert!(tags
            .iter()
            .any(|t| t.title == "Alice" && t.suggestion_type == "person"));
    }

    #[test]
    fn assign_name_to_face_mlc_context_sends_process_all() {
        let (db, _dir) = test_db();
        let (ml_ctx, mut rx) = mock_ml_context();
        db.store_face(make_face("p1", "f1", None));
        db.assign_name_to_face("f1", "TestPerson");
        let _ = ml_ctx.tx.blocking_send(ml::Job::ProcessAll);
        let job = rx.try_recv().unwrap();
        assert!(matches!(job, ml::Job::ProcessAll));
    }

    #[test]
    fn get_unnamed_faces_empty() {
        let (db, _dir) = test_db();
        assert!(do_get_unnamed_faces(&db).is_empty());
    }

    #[test]
    fn get_unnamed_faces_with_anon() {
        let (db, _dir) = test_db();
        let anon_id = db.create_anonymous_person(&vec![0.1; 512]);
        db.store_face(make_face("p1", "f1", Some(&anon_id)));
        let unnamed = do_get_unnamed_faces(&db);
        assert_eq!(unnamed.len(), 1);
        assert_eq!(unnamed[0].name, "Unnamed Person");
    }
}
