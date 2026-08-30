//! Library business logic shared by every frontend surface (#19).
//!
//! These functions used to live in `src-tauri/src/commands/{photos,search}.rs`
//! as `do_*` helpers. They are pure over [`Database`] so the Tauri app, CLI
//! hosts and RPC dispatch (#19) all run the exact same code. src-tauri
//! re-exports them; behavior is unchanged.

use serde::Serialize;

use crate::database::{Database, Photo, SearchSuggestion};

/// Pure business logic — testable without Tauri.
#[allow(dead_code)]
pub fn do_list_files(
    db: &Database,
    query: &str,
    offset: usize,
    limit: usize,
    favorites_only: bool,
    videos_only: bool,
) -> Vec<Photo> {
    do_list_files_filtered(
        db,
        query,
        offset,
        limit,
        favorites_only,
        videos_only,
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
        None,
        None,
    )
}

/// Pure business logic — testable without Tauri. Adds optional facet filters.
#[allow(clippy::too_many_arguments)]
pub fn do_list_files_filtered(
    db: &Database,
    query: &str,
    offset: usize,
    limit: usize,
    favorites_only: bool,
    videos_only: bool,
    person_ids: Vec<String>,
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
    random: bool,
    order_by: Option<String>,
    album_id: Option<String>,
) -> Vec<Photo> {
    let filter = crate::database::PhotoFilter {
        person_ids,
        person_match: match person_match.as_deref() {
            Some("or") => crate::database::PersonMatch::Or,
            _ => crate::database::PersonMatch::And,
        },
        person_alone,
        location,
        tag,
        date_from,
        date_to,
        query: None,
        videos: None,
        favorite: favorites_only,
        has_faces,
        aesthetics_min,
        camera,
        papers,
        nsfw_only,
        random,
        order_by,
        album_id,
    };
    db.list_photos_filtered(
        query,
        offset,
        limit,
        favorites_only,
        videos_only,
        &filter,
        false,
    )
}

/// Pure business logic — testable without Tauri.
pub fn do_toggle_favorite(db: &Database, id: &str) -> bool {
    db.toggle_favorite(id)
}

/// Pure business logic — testable without Tauri.
pub fn do_set_favorites(db: &Database, ids: &[String], favorite: bool) -> usize {
    db.set_favorites(ids, favorite)
}

/// Pure business logic — testable without Tauri.
pub fn do_get_photo_by_id(db: &Database, id: &str) -> Option<Photo> {
    db.get_photo_by_id(id)
}

/// Pure business logic — testable without Tauri.
pub fn do_get_photo_encoded_batch(
    db: &Database,
    ids: &[String],
) -> std::collections::HashMap<String, String> {
    db.get_photo_encoded_batch(ids)
}

/// Pure business logic — testable without Tauri.
pub fn do_get_photos_by_ids(db: &Database, ids: &[String]) -> Vec<Photo> {
    db.get_photos_by_ids(ids)
}

/// Pure business logic — testable without Tauri.
pub fn do_get_heatmap_data(db: &Database) -> Vec<crate::database::MapPoint> {
    db.get_heatmap_points()
}

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
    pub locations: Vec<crate::database::LocationGroup>,
    pub tags: Vec<SearchFacetCount>,
    pub papers: Vec<SearchFacetCount>,
    pub cameras: Vec<SearchFacetCount>,
    pub months: Vec<SearchFacetCount>,
    pub best_photos: Vec<crate::database::SearchPhotoTile>,
    pub favorite_photos: Vec<crate::database::SearchPhotoTile>,
    pub recent_photos: Vec<crate::database::SearchPhotoTile>,
    pub stats: crate::database::SearchStats,
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

/// Count of photos not yet fully indexed (`indexed < 2`). Uncapped — callers
/// must not use `get_unindexed_photos()` (which has an internal LIMIT 50) when
/// they want the true library-wide count for a status report.
pub fn do_get_unindexed_count(db: &Database) -> usize {
    let count: i64 = db
        .connection
        .query_row("SELECT COUNT(*) FROM photo WHERE indexed < 2", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    count as usize
}

/// Top tag/location/person search suggestions for the search box.
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

/// Distinct location names in the library, alphabetically (for the map filter).
pub fn do_get_location_names(db: &Database) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(mut stmt) = db
        .connection
        .prepare("SELECT DISTINCT value FROM properties WHERE key = 'location_name' ORDER BY value")
    {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            names.extend(rows.flatten());
        }
    }
    names
}

/// Delete a single face row by id. Silent on a nonexistent id (matches the
/// desktop behavior; the caller is expected to have looked the face up first).
pub fn do_delete_face(db: &Database, face_id: &str) {
    let _ = db
        .connection
        .execute("DELETE FROM faces WHERE face_id = ?1", [face_id]);
}
