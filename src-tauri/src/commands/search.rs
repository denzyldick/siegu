use serde::Serialize;

use crate::common::get_config_path;
use crate::database;
use crate::database::Database;

/// A single option inside a search facet section, with the number of matching photos.
#[derive(Debug, Clone, Serialize)]
pub struct SearchFacetGroup {
    pub id: String,
    pub name: Option<String>,
    pub representative_crop: Option<String>,
    pub encoded: Option<String>,
    pub count: i64,
}

/// A counted facet value such as a city or an object tag.
#[derive(Debug, Clone, Serialize)]
pub struct SearchFacetCount {
    pub name: String,
    pub count: i64,
}

/// Everything the search dropdown needs to render its discovery sections.
#[derive(Debug, Clone, Serialize)]
pub struct SearchFacets {
    pub people: Vec<SearchFacetGroup>,
    pub unnamed_faces: Vec<SearchFacetGroup>,
    pub locations: Vec<database::LocationGroup>,
    pub tags: Vec<SearchFacetCount>,
    pub papers: Vec<SearchFacetCount>,
    pub cameras: Vec<SearchFacetCount>,
    pub months: Vec<SearchFacetCount>,
    pub best_photos: Vec<database::SearchPhotoTile>,
    pub favorite_photos: Vec<database::SearchPhotoTile>,
    pub recent_photos: Vec<database::SearchPhotoTile>,
    pub stats: database::SearchStats,
}

fn to_group(
    id: String,
    name: Option<String>,
    crop: Option<String>,
    encoded: Option<String>,
    count: i64,
) -> SearchFacetGroup {
    SearchFacetGroup {
        id,
        name,
        representative_crop: crop,
        encoded,
        count,
    }
}

/// Pure business logic — testable without Tauri.
pub fn do_get_search_facets(db: &Database) -> SearchFacets {
    let people = db
        .get_search_people(20)
        .into_iter()
        .map(|p| {
            to_group(
                p.id,
                Some(p.name),
                p.representative_crop,
                p.encoded,
                p.photo_count,
            )
        })
        .collect();
    let unnamed_faces = db
        .get_anonymous_people_groups()
        .into_iter()
        .take(12)
        .map(|g| {
            to_group(
                g.id,
                None,
                g.representative_crop,
                g.encoded,
                g.face_count as i64,
            )
        })
        .collect();
    let locations = db.get_location_groups(25);
    let tags = db
        .get_tag_counts(40)
        .into_iter()
        .map(|(name, count)| SearchFacetCount { name, count })
        .collect();
    let papers = db
        .get_paper_counts(8)
        .into_iter()
        .map(|(name, count)| SearchFacetCount { name, count })
        .collect();
    let cameras = db
        .get_camera_counts(12)
        .into_iter()
        .map(|(name, count)| SearchFacetCount { name, count })
        .collect();
    let months = db
        .get_month_counts(12)
        .into_iter()
        .map(|(name, count)| SearchFacetCount { name, count })
        .collect();

    SearchFacets {
        people,
        unnamed_faces,
        locations,
        tags,
        papers,
        cameras,
        months,
        best_photos: db.get_best_photos(8),
        favorite_photos: db.get_favorite_photos(8),
        recent_photos: db.get_recent_photos(8),
        stats: db.get_search_stats(),
    }
}

#[tauri::command]
pub async fn search_facets(app: tauri::AppHandle) -> Result<String, String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Ok("{}".to_string());
    }
    let db = database::Database::new(&path);
    Ok(serde_json::to_string(&do_get_search_facets(&db)).unwrap_or_else(|_| "{}".to_string()))
}

/// Photo/video counts per calendar day inside a `YYYY-MM-DD` range, for the
/// date-range picker cells in the search dropdown.
#[tauri::command]
pub async fn day_counts(
    app: tauri::AppHandle,
    from: String,
    to: String,
) -> Result<Vec<database::DayCount>, String> {
    let path = get_config_path(&app);
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let db = database::Database::new(&path);
    Ok(db.get_day_counts(&from, &to))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn search_facets_empty_db() {
        let (db, _dir) = test_db();
        let facets = do_get_search_facets(&db);
        assert!(facets.people.is_empty());
        assert!(facets.locations.is_empty());
        assert!(facets.tags.is_empty());
        assert_eq!(facets.stats.photos, 0);
    }

    #[test]
    fn search_facets_with_data() {
        let (db, _dir) = test_db();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                ("p1", "cat", "0.9"),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                ("p1", "dog", "0.8"),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES (?1, 'location_name', ?2)",
                ("p1", "Paris, France"),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES ('p1', '/x.jpg', '2026-03-01', '')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO people (id, name) VALUES (?1, ?2)",
                ("person-1", "Alice"),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO faces (photo_id, face_id, crop_path, encoded, person_id) VALUES ('p1', 'f1', 'crop.jpg', 'enc', 'person-1')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded, aesthetics_score) VALUES ('p2', '/b.jpg', '2026-03-02', 'thumb', 0.95)",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO properties (photo_id, key, value) VALUES ('p2', 'Make', 'Sony')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO object (photo_id, class, probability) VALUES ('p1', 'a receipt', '0.9')",
                (),
            )
            .unwrap();

        let facets = do_get_search_facets(&db);
        assert_eq!(facets.stats.photos, 2);
        assert_eq!(facets.locations.len(), 1);
        assert_eq!(facets.locations[0].name, "Paris, France");
        assert_eq!(facets.locations[0].count, 1);
        assert_eq!(facets.tags.len(), 3);
        assert_eq!(facets.people.len(), 1);
        assert_eq!(facets.people[0].name.as_deref(), Some("Alice"));
        assert_eq!(facets.people[0].count, 1);
        assert_eq!(facets.papers.len(), 1);
        assert_eq!(facets.papers[0].name, "a receipt");
        assert_eq!(facets.cameras.len(), 1);
        assert_eq!(facets.cameras[0].name, "Sony");
        assert_eq!(facets.best_photos.len(), 1);
        assert_eq!(facets.best_photos[0].id, "p2");
        assert_eq!(facets.best_photos[0].aesthetics_score, Some(0.95));
        assert_eq!(facets.recent_photos.len(), 2);
    }

    #[test]
    fn search_facets_named_people_first_with_counts() {
        let (db, _dir) = test_db();
        for (id, name) in [("a", "Alice"), ("b", "Bob"), ("c", "Cara")] {
            db.connection
                .execute("INSERT INTO people (id, name) VALUES (?1, ?2)", (id, name))
                .unwrap();
        }
        // Bob appears in 3 photos, Alice in 1.
        for pid in ["p1", "p2", "p3"] {
            db.connection
                .execute(
                    "INSERT INTO photo (id, location, created, encoded) VALUES (?1, ?2, '2026-01-01', '')",
                    (pid, format!("/{pid}.jpg")),
                )
                .unwrap();
            db.connection
                .execute(
                    "INSERT INTO faces (photo_id, face_id, crop_path, encoded, person_id) VALUES (?1, ?2, '', 'enc', 'b')",
                    (pid, format!("f-{pid}")),
                )
                .unwrap();
        }
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created, encoded) VALUES ('p4', '/p4.jpg', '2026-01-01', '')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO faces (photo_id, face_id, crop_path, encoded, person_id) VALUES ('p4', 'f4', '', 'enc', 'a')",
                (),
            )
            .unwrap();

        let facets = do_get_search_facets(&db);
        let names: Vec<&str> = facets
            .people
            .iter()
            .map(|p| p.name.as_deref().unwrap())
            .collect();
        assert_eq!(names, vec!["Bob", "Alice", "Cara"]);
        assert_eq!(facets.people[0].count, 3);
        assert_eq!(facets.people[1].count, 1);
        assert_eq!(facets.people[2].count, 0);
    }
}
