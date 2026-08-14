//! One-time repair pass for photo dates and Google Takeout metadata.
//!
//! The original scanner stored dates from the file mtime whenever a file had no
//! EXIF — which is every video and most HEIC/HEIF files. Google Takeout exports
//! set mtime to the download date, so an entire migrated library sorted as if
//! every old video was taken today. EXIF dates were also stored in the raw
//! `2024:01:15 10:30:00` format, which corrupts lexical `ORDER BY created`.
//!
//! This pass:
//! 1. Normalizes every stored date to `YYYY-MM-DD HH:MM:SS` (no file I/O).
//! 2. Re-derives dates for video/HEIC rows and recently-dated rows from Google
//!    Takeout JSON sidecars, then from filename conventions, and updates rows
//!    whose date actually differs.
//! 3. Restores GPS, captions, and favorites from Takeout sidecars.
//!
//! It is idempotent and gated by the `date_backfill_done` config key, so it
//! runs at most once per library.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::database::Database;
use crate::scanner::{
    created_from_filename, is_heic_file, is_video_file, normalize_created, parse_sidecar,
    SidecarMeta,
};

/// Config key that marks the backfill as completed (value "1").
pub const BACKFILL_FLAG_KEY: &str = "date_backfill_done";

/// Rows dated within this window of "now" are treated as likely Takeout
/// migration victims and probed for a sidecar, even when they have EXIF.
const RECENT_WINDOW_SECS: i64 = 45 * 86400;

#[derive(Debug, Clone, Copy, Default)]
pub struct BackfillStats {
    /// Rows examined.
    pub scanned: usize,
    /// Rows whose `created` value changed.
    pub created_updated: usize,
    /// Rows whose latitude/longitude/caption changed.
    pub metadata_updated: usize,
    /// Rows left untouched.
    pub unchanged: usize,
}

/// Whether the one-time backfill still needs to run for this library.
pub fn is_backfill_pending(config_path: &str) -> bool {
    !Database::new(config_path)
        .get_state()
        .contains_key(BACKFILL_FLAG_KEY)
}

/// Mark the backfill as done (used by tests; `run_backfill` does this itself).
pub fn mark_backfill_done(config_path: &str) {
    Database::new(config_path).set_state_value(BACKFILL_FLAG_KEY, "1");
}

/// Run the repair pass. Safe to call repeatedly: it is skipped once the
/// `date_backfill_done` flag is present. All writes happen in a single
/// transaction.
pub fn run_backfill(config_path: &str) -> BackfillStats {
    let mut stats = BackfillStats::default();
    let mut db = Database::new(config_path);
    if db.get_state().contains_key(BACKFILL_FLAG_KEY) {
        return stats;
    }

    #[derive(Debug)]
    struct Row {
        id: String,
        location: String,
        created: String,
    }

    let rows: Vec<Row> = {
        let mut list = Vec::new();
        if let Ok(mut stmt) = db
            .connection
            .prepare("SELECT id, location, created FROM photo")
        {
            if let Ok(iter) = stmt.query_map([], |r| {
                Ok(Row {
                    id: r.get(0)?,
                    location: r.get(1)?,
                    created: r.get(2).unwrap_or_default(),
                })
            }) {
                list.extend(iter.flatten());
            }
        }
        list
    };

    let now = chrono::Local::now().timestamp();
    let mut dir_sidecars: HashMap<PathBuf, Vec<(String, PathBuf)>> = HashMap::new();

    let Ok(tx) = db.connection.transaction() else {
        tracing::warn!("backfill: could not open transaction");
        return stats;
    };

    {
        let mut update_stmt = match tx.prepare(
            "UPDATE photo SET created = ?, latitude = COALESCE(?, latitude), \
             longitude = COALESCE(?, longitude), caption = COALESCE(?, caption) WHERE id = ?",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("backfill: prepare update failed: {e}");
                return stats;
            }
        };
        let mut fav_stmt = match tx.prepare(
            "INSERT INTO properties (photo_id, key, value) SELECT ?1, 'favorite', 'true' \
             WHERE NOT EXISTS (SELECT 1 FROM properties WHERE photo_id = ?1 AND key = 'favorite')",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("backfill: prepare favorite failed: {e}");
                return stats;
            }
        };

        for row in &rows {
            stats.scanned += 1;
            let path = Path::new(&row.location);
            let normalized = normalize_created(&row.created);
            let is_video = is_video_file(path);
            let is_heic = is_heic_file(path);

            let mut created_new: Option<String> = None;
            let mut lat_new: Option<f64> = None;
            let mut lon_new: Option<f64> = None;
            let mut caption_new: Option<String> = None;
            let mut sidecar: Option<SidecarMeta> = None;

            // 1. Normalize the stored format (no file I/O needed).
            if normalized != row.created {
                created_new = Some(normalized.clone());
            }

            // 2. Sidecar: videos/HEIC lack EXIF (mtime dates are the migration
            //    damage) and any recent-dated row may be an EXIF-stripped photo.
            if is_video || is_heic || stored_is_recent(&row.created, now) {
                if let Some(sidecar_path) = find_sidecar_cached(path, &mut dir_sidecars) {
                    sidecar = parse_sidecar(&sidecar_path);
                }
                if let Some(s) = &sidecar {
                    if let Some(created) = &s.created {
                        if date_prefix(created) != date_prefix(&normalized) {
                            created_new = Some(created.clone());
                        }
                    }
                    if s.latitude.unwrap_or(0.0) != 0.0 || s.longitude.unwrap_or(0.0) != 0.0 {
                        lat_new = s.latitude;
                        lon_new = s.longitude;
                    }
                    caption_new = s.caption.clone();
                }
            }

            // 3. Filename conventions, when nothing better was found.
            if created_new.is_none() && (is_video || is_heic) {
                if let Some(filename_created) = created_from_filename(path) {
                    if date_prefix(&filename_created) != date_prefix(&normalized) {
                        created_new = Some(filename_created);
                    }
                }
            }

            let needs_update = created_new.is_some()
                || lat_new.is_some()
                || lon_new.is_some()
                || caption_new.is_some();

            if needs_update {
                let result = update_stmt.execute((
                    created_new.as_deref().unwrap_or(&row.created),
                    lat_new,
                    lon_new,
                    caption_new.as_deref(),
                    &row.id,
                ));
                match result {
                    Ok(_) => {
                        if created_new.is_some() {
                            stats.created_updated += 1;
                        }
                        if lat_new.is_some() || lon_new.is_some() || caption_new.is_some() {
                            stats.metadata_updated += 1;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("backfill: update failed for {}: {e}", row.id);
                    }
                }
            } else {
                stats.unchanged += 1;
            }

            if sidecar.as_ref().map(|s| s.favorite).unwrap_or(false) {
                if let Err(e) = fav_stmt.execute((&row.id,)) {
                    tracing::warn!("backfill: favorite insert failed for {}: {e}", row.id);
                }
            }
        }
    }

    if let Err(e) = tx.commit() {
        tracing::warn!("backfill: commit failed: {e}");
        return stats;
    }

    db.set_state_value(BACKFILL_FLAG_KEY, "1");
    stats
}

/// Locate the Takeout sidecar for a media file, caching the `.json` listing of
/// each directory so large folders are only read once.
fn find_sidecar_cached(
    media_path: &Path,
    cache: &mut HashMap<PathBuf, Vec<(String, PathBuf)>>,
) -> Option<PathBuf> {
    let parent = media_path.parent()?;
    let media_name = media_path.file_name()?.to_str()?.to_string();
    let media_stem = media_name.split('.').next().unwrap_or(&media_name);

    let exact = parent.join(format!("{media_name}.json"));
    if exact.is_file() {
        return Some(exact);
    }
    let exact = parent.join(format!("{media_name}.supplemental-metadata.json"));
    if exact.is_file() {
        return Some(exact);
    }

    let entries = cache.entry(parent.to_path_buf()).or_insert_with(|| {
        let mut v = Vec::new();
        if let Ok(rd) = std::fs::read_dir(parent) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
                {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()).map(str::to_string) {
                        v.push((stem, p));
                    }
                }
            }
        }
        v
    });

    for (stem, p) in entries.iter() {
        if stem.as_str() == media_name
            || stem.as_str() == media_stem
            || (stem.len() > media_name.len() && stem.starts_with(&media_name))
        {
            return Some(p.clone());
        }
    }
    None
}

/// First 10 chars (`YYYY-MM-DD`) of a created string.
fn date_prefix(s: &str) -> &str {
    if s.len() >= 10 {
        &s[..10]
    } else {
        s
    }
}

/// Whether a stored `YYYY-MM-DD HH:MM:SS` (local wall time) falls within the
/// recent "download window", making it a likely Takeout mtime victim.
fn stored_is_recent(created: &str, now: i64) -> bool {
    if created.len() < 10 {
        return false;
    }
    let Ok(dt) = chrono::NaiveDateTime::parse_from_str(created, "%Y-%m-%d %H:%M:%S") else {
        return false;
    };
    use chrono::TimeZone;
    let Some(local) = chrono::Local.from_local_datetime(&dt).single() else {
        return false;
    };
    let ts = local.timestamp();
    ts <= now && now - ts <= RECENT_WINDOW_SECS
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn db_path(dir: &TempDir) -> String {
        dir.path().to_str().unwrap().to_string()
    }

    #[test]
    fn test_run_backfill_normalizes_and_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let db = Database::new(&db_path(&dir));

        // Colon-format EXIF date and an mtime-style video date.
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created) VALUES ('p1', '/tmp/a.jpg', '2024:01:15 10:30:00')",
                (),
            )
            .unwrap();
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created) VALUES ('p2', '/tmp/v.mp4', '2026-08-01 09:00:00')",
                (),
            )
            .unwrap();

        let stats = run_backfill(&db_path(&dir));
        assert!(stats.created_updated >= 1);

        let get = |id: &str| -> String {
            db.connection
                .query_row("SELECT created FROM photo WHERE id = ?1", [id], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        assert_eq!(get("p1"), "2024-01-15 10:30:00");

        // Idempotent: a second run does nothing and leaves the flag set.
        let db2 = Database::new(&db_path(&dir));
        assert!(db2.get_state().contains_key(BACKFILL_FLAG_KEY));
        let stats2 = run_backfill(&db_path(&dir));
        assert_eq!(stats2.scanned, 0);
    }

    #[test]
    fn test_backfill_uses_filename_date_for_video() {
        let dir = TempDir::new().unwrap();
        let db = Database::new(&db_path(&dir));

        db.connection
            .execute(
                "INSERT INTO photo (id, location, created) VALUES ('v1', '/tmp/VID_20230101_120000.mp4', '2026-08-01 09:00:00')",
                (),
            )
            .unwrap();

        let stats = run_backfill(&db_path(&dir));
        assert!(stats.created_updated >= 1);

        let created: String = db
            .connection
            .query_row("SELECT created FROM photo WHERE id = 'v1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(created, "2023-01-01 12:00:00");
    }

    #[test]
    fn test_backfill_uses_takeout_sidecar() {
        let dir = TempDir::new().unwrap();
        let media_dir = dir.path().join("photos");
        std::fs::create_dir_all(&media_dir).unwrap();
        let media = media_dir.join("IMG_0001.mp4");
        std::fs::write(&media, "fake-video").unwrap();
        let sidecar = media_dir.join("IMG_0001.mp4.json");
        std::fs::write(
            &sidecar,
            r#"{
                "title": "IMG_0001.mp4",
                "photoTakenTime": { "timestamp": "1692113136", "formatted": "Aug 15, 2023, 2:25:36 PM UTC" },
                "creationTime": { "timestamp": "1692120000", "formatted": "Aug 15, 2023, 4:40:00 PM UTC" },
                "geoData": { "latitude": 36.778259, "longitude": -119.417931 },
                "favorited": true,
                "description": "Beach sunset"
            }"#,
        )
        .unwrap();

        let db = Database::new(&db_path(&dir));
        db.connection
            .execute(
                "INSERT INTO photo (id, location, created) VALUES ('t1', ?1, '2026-08-01 09:00:00')",
                [media.to_str().unwrap()],
            )
            .unwrap();

        let stats = run_backfill(&db_path(&dir));
        assert!(stats.created_updated >= 1);
        assert!(stats.metadata_updated >= 1);

        let created: String = db
            .connection
            .query_row("SELECT created FROM photo WHERE id = 't1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        use chrono::TimeZone;
        let expected = chrono::Utc
            .timestamp_opt(1692113136, 0)
            .single()
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        assert_eq!(created, expected);

        let latitude: f64 = db
            .connection
            .query_row("SELECT latitude FROM photo WHERE id = 't1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(latitude, 36.778259);

        let caption: String = db
            .connection
            .query_row("SELECT caption FROM photo WHERE id = 't1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(caption, "Beach sunset");

        let favorite: i64 = db
            .connection
            .query_row(
                "SELECT COUNT(*) FROM properties WHERE photo_id = 't1' AND key = 'favorite'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(favorite, 1);
    }

    #[test]
    fn test_sidecar_matching_with_truncated_supplemental_metadata() {
        let dir = TempDir::new().unwrap();
        let media_dir = dir.path().join("photos");
        std::fs::create_dir_all(&media_dir).unwrap();
        // Google truncates the full sidecar name at 46 chars, but the media
        // filename (which comes first) is preserved: "....jpg.supplemental-m.json"
        let media = media_dir.join("SomeQuiteLongName20230101_1200001234567890.jpg");
        std::fs::write(&media, "fake").unwrap();
        let sidecar =
            media_dir.join("SomeQuiteLongName20230101_1200001234567890.jpg.supplemental-m.json");
        std::fs::write(
            &sidecar,
            r#"{"photoTakenTime": { "timestamp": "1672574400" }}"#,
        )
        .unwrap();

        let found = crate::scanner::sidecar_meta_for(&media);
        assert!(found.is_some(), "truncated sidecar should be matched");
        use chrono::TimeZone;
        let expected = chrono::Utc
            .timestamp_opt(1672574400, 0)
            .single()
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        assert_eq!(found.unwrap().created.unwrap(), expected);
    }
}
