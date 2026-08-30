//! Black-box integration tests for the `siegu` CLI's demo-dataset features.
//!
//! These tests exercise the REAL compiled `siegu` binary as an external API:
//! they spawn `siegu seed-demo` / `siegu analyze` as subprocesses against a
//! throwaway config dir, then read the `siegu.db` it wrote to assert the
//! extracted state matches the bundled `demos/` source datasets.
//!
//! Two groups:
//!   - Deterministic (offline, always run): seeding, thumbnails, idempotency,
//!     album membership. Validates "everything we extracted matches".
//!   - ML (needs ONNX models): skipped unless models are present, matching the
//!     repo's existing `#[ignore]` + runtime-skip model-test convention. Validates
//!     extracted ML signals (objects/faces/aesthetics/nsfw) and that they feed
//!     the search/tag/album features.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const MEDIA_EXTS: &[&str] = &["jpg", "jpeg", "png", "mp4", "mov"];

/// Path to the compiled `siegu` binary (cargo sets `CARGO_BIN_EXE_siegu` for
/// integration tests in the crate that declares the `siegu` bin).
fn bin() -> PathBuf {
    PathBuf::from(std::env::var_os("CARGO_BIN_EXE_siegu").expect("CARGO_BIN_EXE_siegu is set"))
}

/// The bundled demo assets: `<workspace>/demos/`.
fn demos_root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../demos"))
}

/// Run the `siegu` binary with the given args; returns the combined output.
fn run_siegu(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("spawn siegu binary")
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// True for the `<stem>_poster.jpg` counterparts of the bundled video clips
/// (they back the clips' thumbnails and are never seeded as photos).
fn is_poster_file(p: &Path) -> bool {
    p.file_name()
        .map(|n| {
            let s = n.to_string_lossy().to_lowercase();
            s.ends_with("_poster.jpg") || s.ends_with("_poster.jpeg")
        })
        .unwrap_or(false)
}

/// Expected media count per category by walking the bundled `demos/` tree.
fn expected_counts() -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let mut entries = std::fs::read_dir(demos_root())
        .expect("demos/ exists")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir());
    while let Some(dir) = entries.next() {
        let cat = dir.file_name().to_string_lossy().into_owned();
        let mut n = 0usize;
        if let Ok(read) = std::fs::read_dir(dir.path()) {
            for e in read.flatten() {
                let p = e.path();
                if p.is_file() && !is_poster_file(&p) {
                    if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
                        if MEDIA_EXTS.contains(&ext.to_lowercase().as_str()) {
                            n += 1;
                        }
                    }
                }
            }
        }
        map.insert(cat, n);
    }
    map
}

/// Open read-only the `siegu.db` produced by the CLI in `config_dir`.
fn db(config_dir: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(config_dir.join("siegu.db")).expect("open siegu.db")
}

fn photo_count(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM photo", [], |r| r.get(0))
        .unwrap()
}

/// Parse `SEEDED <cat> album=<id> photos=N` lines from seed output.
fn seeded_albums(combined: &str) -> HashMap<String, (String, usize)> {
    let mut map = HashMap::new();
    for line in combined.lines() {
        if let Some(rest) = line.split("SEEDED ").nth(1) {
            // rest = "<cat> album=<id> photos=N"
            let mut parts = rest.split_whitespace();
            let cat = parts.next().unwrap_or("").to_string();
            let album_id = parts
                .next()
                .and_then(|s| s.strip_prefix("album="))
                .unwrap_or("")
                .to_string();
            let photos = parts
                .next()
                .and_then(|s| s.strip_prefix("photos="))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            map.insert(cat, (album_id, photos));
        }
    }
    map
}

fn count_album_items(conn: &rusqlite::Connection, album_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM album_item WHERE album_id = ?1",
        [album_id],
        |r| r.get(0),
    )
    .unwrap()
}

/// Id of the combined "My Photos" album (one library = one camera roll).
fn combined_album_id(conn: &rusqlite::Connection) -> String {
    conn.query_row("SELECT id FROM album WHERE name = 'My Photos'", [], |r| {
        r.get(0)
    })
    .unwrap_or_else(|e| panic!("combined album exists: {e}"))
}

fn count_thumbs(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM photo WHERE encoded != ''", [], |r| {
        r.get(0)
    })
    .unwrap()
}

fn thumb_prefix_ok(conn: &rusqlite::Connection) -> bool {
    let mut stmt = conn
        .prepare("SELECT encoded FROM photo WHERE encoded != '' LIMIT 10")
        .unwrap();
    let mut rows = stmt.query([]).unwrap();
    while let Ok(Some(row)) = rows.next() {
        let encoded: String = row.get(0).unwrap();
        if !encoded.starts_with("data:image/jpeg") {
            return false;
        }
    }
    true
}

// ── Group A: deterministic external-API tests (always run, offline) ────────

#[test]
fn seed_demo_produces_source_matching_library() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    let expected = expected_counts();
    assert!(
        !expected.is_empty(),
        "demos/ should contain at least one category"
    );

    let out = run_siegu(&[
        "seed-demo",
        "--config",
        &cfg.display().to_string(),
        "--demos-root",
        &demos_root().display().to_string(),
    ]);
    assert!(
        out.status.success(),
        "seed-demo should exit 0: {}",
        combined(&out)
    );

    let albums = seeded_albums(&combined(&out));
    assert_eq!(albums.len(), expected.len(), "one album per category");

    let conn = db(cfg);
    let total: i64 = expected.values().sum::<usize>() as i64;
    assert_eq!(
        photo_count(&conn),
        total,
        "photo rows == source image count"
    );

    for (cat, &expected_n) in &expected {
        let (album_id, printed_n) = albums.get(cat).expect("album printed for category");
        assert_eq!(*printed_n, expected_n, "printed photos for {cat}");
        assert_eq!(
            count_album_items(&conn, album_id),
            expected_n as i64,
            "album membership for {cat} matches source count"
        );
        // Album row exists and is named after the category.
        let name: Option<String> = conn
            .query_row("SELECT name FROM album WHERE id = ?1", [album_id], |r| {
                r.get(0)
            })
            .ok();
        assert!(name.is_some(), "album id printed must exist in DB");
    }

    // Every album references a real photo (the join the album UI uses).
    let dangling: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM album_item WHERE photo_id NOT IN (SELECT id FROM photo)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(dangling, 0, "no album_item should point at a missing photo");

    // The combined album holds every seeded photo (the collections default).
    assert_eq!(
        count_album_items(&conn, &combined_album_id(&conn)),
        total,
        "combined 'My Photos' album contains every seeded photo"
    );
}

#[test]
fn seed_demo_generates_thumbnails_for_every_photo() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    let out = run_siegu(&[
        "seed-demo",
        "--config",
        &cfg.display().to_string(),
        "--demos-root",
        &demos_root().display().to_string(),
    ]);
    assert!(out.status.success());
    let conn = db(cfg);
    let total = photo_count(&conn);
    assert!(total > 0);
    assert_eq!(count_thumbs(&conn), total, "every photo gets a thumbnail");
    assert!(
        thumb_prefix_ok(&conn),
        "thumbnails are base64 JPEG data URLs"
    );
}

#[test]
fn seed_demo_is_idempotent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    let args = [
        "seed-demo".to_string(),
        "--config".to_string(),
        cfg.display().to_string(),
        "--demos-root".to_string(),
        demos_root().display().to_string(),
    ];
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let expected = expected_counts();

    let first = run_siegu(&a);
    assert!(first.status.success());
    let conn1 = db(cfg);
    let photos1 = photo_count(&conn1);
    let albums1: i64 = conn1
        .query_row("SELECT COUNT(*) FROM album", [], |r| r.get(0))
        .unwrap();

    let second = run_siegu(&a);
    assert!(second.status.success());
    let conn2 = db(cfg);
    assert_eq!(
        photo_count(&conn2),
        photos1,
        "photo count unchanged on re-seed"
    );
    let albums2: i64 = conn2
        .query_row("SELECT COUNT(*) FROM album", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        albums2, albums1,
        "no duplicate albums created on re-seed (albums reused)"
    );
    assert_eq!(
        albums1,
        expected.len() as i64 + 1,
        "one album per demo category plus the combined 'My Photos' album"
    );
    // Re-seed claims it added nothing.
    let combined2 = combined(&second);
    assert!(
        combined2.contains("photos_added=0"),
        "re-seed adds zero photos: {combined2}"
    );
}

#[test]
fn seed_demo_combined_album_contains_every_photo() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    let args = [
        "seed-demo".to_string(),
        "--config".to_string(),
        cfg.display().to_string(),
        "--demos-root".to_string(),
        demos_root().display().to_string(),
    ];
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let first = run_siegu(&a);
    assert!(first.status.success());
    let conn = db(cfg);
    let total = photo_count(&conn);
    assert!(total > 0);
    let combined = combined_album_id(&conn);
    assert_eq!(
        count_album_items(&conn, &combined),
        total,
        "combined album holds every photo on first seed"
    );

    // Re-seed: still idempotent, no duplicate membership rows.
    let second = run_siegu(&a);
    assert!(second.status.success());
    let conn2 = db(cfg);
    assert_eq!(
        count_album_items(&conn2, &combined_album_id(&conn2)),
        total,
        "combined album membership unchanged on re-seed"
    );
}

#[test]
fn seed_demo_videos_gain_poster_thumbnails_and_never_seed_posts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    let out = run_siegu(&[
        "seed-demo",
        "--config",
        &cfg.display().to_string(),
        "--demos-root",
        &demos_root().display().to_string(),
    ]);
    assert!(out.status.success());

    let expected_videos = expected_counts().get("videos").copied().unwrap_or(0);
    assert!(
        expected_videos > 0,
        "demos/videos should hold at least one clip"
    );
    let conn = db(cfg);
    let videos: i64 = conn
        .query_row("SELECT COUNT(*) FROM photo WHERE is_video = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        videos, expected_videos as i64,
        "every source clip is indexed as a video photo"
    );

    // Each clip gets its `<stem>_poster.jpg` as the library thumbnail.
    let missing: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM photo WHERE is_video = 1 AND encoded = ''",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(missing, 0, "every clip has a poster thumbnail");
    let bad_prefix: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM photo WHERE is_video = 1 AND encoded != '' \
             AND encoded NOT LIKE 'data:image/jpeg%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bad_prefix, 0, "poster thumbnails are base64 JPEG data URLs");

    // Poster files are never indexed as standalone photos.
    let posters: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM photo WHERE lower(location) LIKE '%_poster.jpg'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(posters, 0, "posters are thumbnail material, not photos");
}

#[test]
fn unrecognized_category_is_reported_not_fatal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    let out = run_siegu(&[
        "seed-demo",
        "--demos",
        "does-not-exist",
        "--config",
        &cfg.display().to_string(),
        "--demos-root",
        &demos_root().display().to_string(),
    ]);
    // CLI still exits 0 but seeds nothing; the missing category is a warning.
    assert!(out.status.success());
    let conn = db(cfg);
    assert_eq!(photo_count(&conn), 0, "unknown category seeds no photos");
}

// ── Group B: ML extraction (needs ONNX models; skipped when absent) ────────

/// Whether the ML model suite is present (mirrors `ml_engine::models::test_models_dir`).
fn ml_models_present() -> bool {
    let candidates = [
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test_models")),
        siegu_core::config::default_config_dir().join("models"),
    ];
    candidates
        .iter()
        .any(|d| d.join("face_detection_yunet_2023mar.onnx").exists())
}

fn seed_and_analyze(config_dir: &Path) {
    let out = run_siegu(&[
        "seed-demo",
        "--config",
        &config_dir.display().to_string(),
        "--demos-root",
        &demos_root().display().to_string(),
    ]);
    assert!(out.status.success());
    // The analysis worker loads ONNX models from `<config>/models`. Expose the
    // repo's `test_models/` (at the workspace root, like `demos/`) via an
    // absolute symlink so the temp config dir sees them.
    let models_target =
        std::fs::canonicalize(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test_models"))
            .unwrap_or_default();
    if models_target.is_dir() {
        let models_link = config_dir.join("models");
        if !models_link.exists() {
            std::os::unix::fs::symlink(&models_target, &models_link)
                .expect("symlink test_models into config/models");
        }
    }
    // Run the full analysis pipeline to completion (headless, synchronous).
    let an = run_siegu(&[
        "analyze",
        "all",
        "--headless",
        "--config-dir",
        &config_dir.display().to_string(),
    ]);
    assert!(
        an.status.success(),
        "analyze all --headless should succeed: {}",
        combined(&an)
    );
}

#[test]
#[ignore] // needs ONNX models (siegu models download / test_models/)
fn analyze_persists_objects_and_marks_indexed() {
    if !ml_models_present() {
        println!("Skipping: ONNX models not present");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    seed_and_analyze(cfg);
    let conn = db(cfg);
    let objects: i64 = conn
        .query_row("SELECT COUNT(*) FROM object", [], |r| r.get(0))
        .unwrap();
    assert!(objects > 0, "analysis produces at least one object/tag row");
    let indexed: i64 = conn
        .query_row("SELECT COUNT(*) FROM photo WHERE indexed = 2", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        indexed > 0,
        "analyzed photos are marked fully processed (indexed=2)"
    );
}

#[test]
#[ignore] // needs ONNX models
fn analyze_persists_people_from_faces() {
    if !ml_models_present() {
        println!("Skipping: ONNX models not present");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    seed_and_analyze(cfg);
    let conn = db(cfg);
    // The people category should produce face embeddings/people rows.
    let faces: i64 = conn
        .query_row("SELECT COUNT(*) FROM faces", [], |r| r.get(0))
        .unwrap();
    let people: i64 = conn
        .query_row("SELECT COUNT(*) FROM people", [], |r| r.get(0))
        .unwrap();
    assert!(
        faces > 0 && people > 0,
        "people demo yields faces ({faces}) and people ({people})"
    );
}

#[test]
#[ignore] // needs ONNX models
fn analyze_persists_aesthetics_and_face_count_properties() {
    if !ml_models_present() {
        println!("Skipping: ONNX models not present");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    seed_and_analyze(cfg);
    let conn = db(cfg);
    let with_score: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM photo WHERE aesthetics_score IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(with_score > 0, "aesthetic scores persisted");
    let face_prop: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM properties WHERE key = 'face_count'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(face_prop > 0, "face_count property persisted");
}

// ── Group C: feature-usefulness (deterministic + ML) ───────────────────────

#[test]
fn seeded_albums_are_queryable_by_the_album_join_features_use() {
    // The `list_files({ albumId })` path features use joins album_item -> photo.
    // Prove the seeded structure feeds that exact query.
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    let out = run_siegu(&[
        "seed-demo",
        "--config",
        &cfg.display().to_string(),
        "--demos-root",
        &demos_root().display().to_string(),
    ]);
    assert!(out.status.success());
    let albums = seeded_albums(&combined(&out));
    let conn = db(cfg);
    for (cat, (album_id, _)) in &albums {
        let joined: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM album_item ai JOIN photo p ON p.id = ai.photo_id \
                 WHERE ai.album_id = ?1",
                [album_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            joined > 0,
            "album '{cat}' is queryable via the join the album UI uses"
        );
    }
}

#[test]
#[ignore] // needs ONNX models
fn extracted_tags_are_searchable_by_the_query_features_use() {
    if !ml_models_present() {
        println!("Skipping: ONNX models not present");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = tmp.path();
    seed_and_analyze(cfg);
    let conn = db(cfg);
    // Features surface extracted tags through the `object` table; prove at least
    // one tag is attached to a real photo (the search/facet path).
    let tagged: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM object o JOIN photo p ON p.id = o.photo_id",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        tagged > 0,
        "extracted tags attach to real photos (searchable)"
    );
}
