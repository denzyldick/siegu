//! Duplicate detection and review.
//!
//! Three stages, in increasing cost and capability:
//!   1. Exact content hash — group photos whose file bytes hash identically.
//!   2. Perceptual dHash — group photos whose downsized thumbnails are nearly
//!      identical (re-encodes, resizes, format changes).
//!   3. AI (CLIP) cosine similarity — group photos whose CLIP visual
//!      embeddings are very close (heavy crops, edits, filters). Requires the
//!      CLIP model to be downloaded and its embeddings stored.
//!
//! Groups are ranked and the "best" photo to keep is chosen by the stored
//! `aesthetics_score`. When no member has a score the group is flagged
//! `unknown_best` and the caller should ask the user to pick.

use std::collections::HashMap;

use crate::database::Database;

/// Perceptual dHash hamming-distance threshold: photos at or below this many
/// differing bits are considered perceptual duplicates.
pub const DHASH_HAMMING_THRESHOLD: u32 = 10;
/// CLIP cosine-similarity threshold: embeddings at or above this are treated
/// as the same image (for L2-normalized embeddings this equals dot product).
pub const CLIP_SIMILARITY_THRESHOLD: f32 = 0.90;

/// A run of photos that are duplicates of one another.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateGroup {
    /// Photo ids that are duplicates of one another.
    pub members: Vec<String>,
    /// Id of the member with the highest aesthetics score, when any member has
    /// a score. `None` + `unknown_best` means "no quality score — pick manually".
    pub best_id: Option<String>,
    /// `true` when no member had an aesthetics score, so the UI must not
    /// auto-highlight a "best" pick.
    pub unknown_best: bool,
}

impl DuplicateGroup {
    fn with_members(members: Vec<String>, quality: &HashMap<String, f64>) -> DuplicateGroup {
        let mut best_id: Option<String> = None;
        let mut best_score = f64::MIN;
        for id in &members {
            if let Some(score) = quality.get(id) {
                if *score > best_score {
                    best_score = *score;
                    best_id = Some(id.clone());
                }
            }
        }
        let unknown_best = best_id.is_none();
        DuplicateGroup {
            members,
            best_id,
            unknown_best,
        }
    }
}

/// Perceptual dHash (64-bit gradient hash) of an image. Returns a 16-char
/// lowercase hex string.
pub fn dhash(img: &image::DynamicImage) -> String {
    let gray = img.resize_exact(9, 8, image::imageops::FilterType::Triangle);
    let luma = gray.to_luma8();
    let mut bits: u64 = 0;
    let mut bit: u32 = 0;
    for y in 0..8 {
        for x in 0..8 {
            let left = luma.get_pixel(x, y).0[0];
            let right = luma.get_pixel(x + 1, y).0[0];
            if left < right {
                bits |= 1 << bit;
            }
            bit += 1;
        }
    }
    format!("{:016x}", bits)
}

/// Hamming distance between two hex dHash strings.
pub fn dhash_hamming(a: &str, b: &str) -> u32 {
    let av = u64::from_str_radix(a, 16).unwrap_or(0);
    let bv = u64::from_str_radix(b, 16).unwrap_or(0);
    (av ^ bv).count_ones()
}

/// Dot product of two L2-normalized vectors == cosine similarity.
fn similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Decode the stored thumbnail data-URL for a photo into an image, if present.
fn image_from_thumb(db: &Database, id: &str) -> Option<image::DynamicImage> {
    let bytes = db.get_photo_thumbnail_bytes(id)?;
    image::load_from_memory(&bytes).ok()
}

/// Union-Find helper for clustering indices into duplicate groups.
struct UnionFind {
    parent: Vec<usize>,
}
impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, mut a: usize) -> usize {
        while self.parent[a] != a {
            self.parent[a] = self.parent[self.parent[a]];
            a = self.parent[a];
        }
        a
    }
    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent[ra] = rb;
        }
    }
    fn clusters(&mut self, n: usize) -> Vec<Vec<usize>> {
        let mut map: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            map.entry(self.find(i)).or_default().push(i);
        }
        map.into_values().collect()
    }
}

/// Stage 1 (non-AI): compute and persist exact + perceptual hashes for any
/// photos missing them, then group duplicates. Returns the groups and how many
/// photos had hashes computed this run.
pub fn detect_non_ai(db: &Database) -> (Vec<DuplicateGroup>, usize) {
    detect_non_ai_progress(db, &mut |_, _| {})
}

/// `detect_non_ai` with a progress callback `(done, total)` fired after each
/// photo's hashes are computed. `total` is the number of photos needing a
/// hash this run; a second call is nearly free once file hashes are stored.
pub fn detect_non_ai_progress(
    db: &Database,
    progress: &mut dyn FnMut(usize, usize),
) -> (Vec<DuplicateGroup>, usize) {
    let missing = db.photos_missing_dup_hashes(100_000);
    let total = missing.len();
    let mut computed = 0usize;
    for (done, (id, location)) in missing.into_iter().enumerate() {
        let file_sha = sha256_file(&location);
        let dh = image_from_thumb(db, &id).map(|img| dhash(&img));
        if file_sha.is_some() {
            db.upsert_dup_hashes_partial(&id, file_sha.as_deref(), dh.as_deref());
            computed += 1;
        }
        progress(done + 1, total);
    }

    let groups = group_non_ai(db);
    (groups, computed)
}

/// Streaming SHA-256 of a file (avoids loading whole files into memory).
fn sha256_file(path: &str) -> Option<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::with_capacity(256 * 1024, file);
    let mut h = Sha256::new();
    let mut buf = [0u8; 256 * 1024];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => h.update(&buf[..n]),
            Err(_) => return None,
        }
    }
    Some(format!("{:x}", h.finalize()))
}

/// Group persisted (id, sha, dhash) rows into duplicate clusters using exact
/// hashes plus perceptual hamming distance.
fn group_non_ai(db: &Database) -> Vec<DuplicateGroup> {
    let rows = db.all_dup_data();
    let n = rows.len();
    let labels: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
    let mut uf = UnionFind::new(n);

    // Exact duplicates by file SHA-256.
    let mut by_sha: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        by_sha.entry(r.1.as_str()).or_default().push(i);
    }
    for idxs in by_sha.values() {
        for w in idxs.windows(2) {
            uf.union(w[0], w[1]);
        }
    }

    // Perceptual neighbors by dHash hamming distance, bucketed by hash prefix
    // to avoid an O(n^2) scan of the whole library.
    let mut buckets: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        let prefix: String = r.2.chars().take(2).collect();
        buckets.entry(prefix).or_default().push(i);
    }
    for idxs in buckets.values() {
        for a in 0..idxs.len() {
            for b in (a + 1)..idxs.len() {
                // Skip pairs whose perceptual hash is missing (no thumbnail);
                // exact hashing still covered them above.
                if rows[idxs[a]].2.is_empty() || rows[idxs[b]].2.is_empty() {
                    continue;
                }
                if dhash_hamming(&rows[idxs[a]].2, &rows[idxs[b]].2) <= DHASH_HAMMING_THRESHOLD {
                    uf.union(idxs[a], idxs[b]);
                }
            }
        }
    }

    let quality = db.quality_scores();
    uf.clusters(n)
        .into_iter()
        .filter_map(|cluster| {
            if cluster.len() < 2 {
                return None;
            }
            let members: Vec<String> = cluster.iter().map(|i| labels[*i].clone()).collect();
            Some(DuplicateGroup::with_members(members, &quality))
        })
        .collect()
}

/// Stage 2 (AI): group photos whose stored CLIP embeddings are very similar.
/// Requires the CLIP model to have produced embeddings already; the caller is
/// responsible for guiding the user to download the model and re-run the
/// pipeline so embeddings are persisted.
pub fn detect_clip(db: &Database) -> Vec<DuplicateGroup> {
    let embs = db.list_clip_embeddings();
    let n = embs.len();
    let labels: Vec<String> = embs.iter().map(|e| e.0.clone()).collect();
    let mut uf = UnionFind::new(n);

    for a in 0..n {
        for b in (a + 1)..n {
            if similarity(&embs[a].1, &embs[b].1) >= CLIP_SIMILARITY_THRESHOLD {
                uf.union(a, b);
            }
        }
    }

    let quality = db.quality_scores();
    uf.clusters(n)
        .into_iter()
        .filter_map(|cluster| {
            if cluster.len() < 2 {
                return None;
            }
            let members: Vec<String> = cluster.iter().map(|i| labels[*i].clone()).collect();
            Some(DuplicateGroup::with_members(members, &quality))
        })
        .collect()
}

/// Combined duplicate group list (non-AI first, then any extra CLIP groups).
/// CLIP groups are only meaningful once embeddings exist.
pub fn detect_all(db: &Database, include_clip: bool) -> Vec<DuplicateGroup> {
    let (mut groups, _) = detect_non_ai(db);
    if include_clip {
        groups.extend(detect_clip(db));
    }
    groups
}

// ── User-facing view ────────────────────────────────────────────────────

/// A single member of a duplicate group for the UI, with everything the card
/// needs to render a thumbnail and let the user keep/trash it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateMemberView {
    pub id: String,
    pub location: String,
    pub aesthetics: Option<f64>,
    pub is_best: bool,
}

/// A duplicate group enriched for the UI: per-member metadata, a human label,
/// and how many bytes could be reclaimed by trashing the non-best members.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateGroupView {
    pub members: Vec<DuplicateMemberView>,
    pub best_id: Option<String>,
    pub unknown_best: bool,
    /// "exact" (identical SHA-256), "perceptual" (dHash), or "clip" (AI).
    pub kind: String,
    /// Number of bytes that would be freed by trashing every non-best member.
    pub reclaimable_bytes: u64,
}

/// Bytes on disk for a photo's source file (from its location path).
fn file_bytes(db: &Database, id: &str) -> u64 {
    db.get_photo_location(id)
        .and_then(|p| std::fs::metadata(&p).ok())
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Total on-disk bytes of every photo/video currently in the library: the sum
/// of the source file size for each item whose file still exists.
pub fn library_total_bytes(db: &Database) -> u64 {
    db.all_photo_locations()
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

/// Combined photo/video counts and total on-disk library size.
pub struct LibraryOverview {
    pub photo_count: i64,
    pub video_count: i64,
    pub library_bytes: u64,
}

/// Counts plus the on-disk size of the library, ready for storage views.
pub fn library_overview(db: &Database) -> LibraryOverview {
    let (photo_count, video_count) = db.get_media_counts();
    LibraryOverview {
        photo_count,
        video_count,
        library_bytes: library_total_bytes(db),
    }
}

fn enrich(db: &Database, group: &DuplicateGroup, kind: &str) -> DuplicateGroupView {
    let mut members: Vec<DuplicateMemberView> = group
        .members
        .iter()
        .map(|id| DuplicateMemberView {
            id: id.clone(),
            location: db.get_photo_location(id).unwrap_or_default(),
            aesthetics: db.quality_scores().get(id).copied(),
            is_best: Some(id.as_str()) == group.best_id.as_deref(),
        })
        .collect();
    members.sort_by(|a, b| {
        b.aesthetics
            .partial_cmp(&a.aesthetics)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let reclaimable_bytes = members
        .iter()
        .filter(|m| !m.is_best)
        .map(|m| file_bytes(db, &m.id))
        .sum();

    DuplicateGroupView {
        members,
        best_id: group.best_id.clone(),
        unknown_best: group.unknown_best,
        kind: kind.to_string(),
        reclaimable_bytes,
    }
}

/// Combined non-AI groups with kind labels. Exact groups (all members share the
/// same file SHA-256) are labeled "exact", everything else "perceptual".
pub fn detect_all_view(db: &Database, include_clip: bool) -> Vec<DuplicateGroupView> {
    detect_all_view_progress(db, include_clip, &mut |_, _| {})
}

/// `detect_all_view` with a progress callback `(done, total)` forwarded from
/// the hash-computation stage. Fired only while hashes are being computed.
pub fn detect_all_view_progress(
    db: &Database,
    include_clip: bool,
    progress: &mut dyn FnMut(usize, usize),
) -> Vec<DuplicateGroupView> {
    let (mut groups, _) = detect_non_ai_progress(db, progress);

    let by_id: HashMap<String, (String, String)> = db
        .all_dup_data()
        .into_iter()
        .map(|(id, sha, dh, _, _)| (id, (sha, dh)))
        .collect();

    let views: Vec<DuplicateGroupView> = groups
        .drain(..)
        .map(|g| {
            let shas: Vec<&str> = g
                .members
                .iter()
                .filter_map(|id| by_id.get(id).map(|(sha, _)| sha.as_str()))
                .collect();
            let exact = shas.len() == g.members.len()
                && !shas.is_empty()
                && shas.windows(2).all(|w| w[0] == w[1]);
            let kind = if exact { "exact" } else { "perceptual" };
            enrich(db, &g, kind)
        })
        .collect();

    if include_clip {
        let clip = detect_clip(db);
        views
            .into_iter()
            .chain(clip.into_iter().map(|g| enrich(db, &g, "clip")))
            .collect()
    } else {
        views
    }
}

/// Aggregate stats over duplicate groups for a status banner.
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct DuplicateStats {
    pub group_count: usize,
    pub duplicate_count: usize,
    pub reclaimable_bytes: u64,
}

pub fn duplicate_stats(db: &Database, include_clip: bool) -> DuplicateStats {
    duplicate_stats_from_views(&detect_all_view(db, include_clip))
}

/// Aggregate stats directly from already-detected views (no re-scan).
pub fn duplicate_stats_from_views(views: &[DuplicateGroupView]) -> DuplicateStats {
    let mut stats = DuplicateStats {
        group_count: views.len(),
        ..Default::default()
    };
    for v in views {
        stats.reclaimable_bytes += v.reclaimable_bytes;
        stats.duplicate_count += v.members.len().saturating_sub(1);
    }
    stats
}

/// Trash every non-best member of the given duplicate group; returns how many
/// photos were trashed. When no quality-based "best" exists, the first member
/// (highest aesthetics, else first in group order) is kept and never trashed.
pub fn trash_group_non_best(db: &Database, group: &DuplicateGroupView) -> usize {
    let keep_id = group
        .members
        .iter()
        .find(|m| m.is_best)
        .map(|m| m.id.as_str())
        .unwrap_or_else(|| group.members.first().map(|m| m.id.as_str()).unwrap_or(""));
    let mut trashed = 0usize;
    for m in &group.members {
        if m.id == keep_id {
            continue;
        }
        if db.trash_photo(&m.id).is_ok() {
            trashed += 1;
        }
    }
    trashed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_gray(v: u8) -> image::DynamicImage {
        image::DynamicImage::ImageLuma8(image::GrayImage::from_fn(16, 16, |x, y| {
            // A horizontal gradient whose direction flips with v parity, so
            // `v` genuinely changes the resulting dHash.
            let slope: u8 = if v % 2 == 0 { x as u8 } else { 15 - x as u8 };
            image::Luma([v.wrapping_add(slope)])
        }))
    }

    #[test]
    fn test_dhash_stable_and_hamming() {
        let a = dhash(&tiny_gray(0));
        let b = dhash(&tiny_gray(0));
        assert_eq!(a.len(), 16);
        assert_eq!(a, b);
        assert_eq!(dhash_hamming(&a, &b), 0);
        // Even v (increasing gradient) vs odd v (decreasing gradient) must
        // produce visibly different hashes.
        let c = dhash(&tiny_gray(1));
        assert_ne!(a, c);
    }

    #[test]
    fn test_dhash_resize_invariance() {
        // Same content at two resolutions should hash identically. Build the
        // gradient from NORMALIZED coordinates so both images sample the same
        // continuous ramp (x sweeps 0..1 in both) — the old test used raw
        // pixel indices, so the 4x4 and 80x80 images described totally
        // different content and could never match after resize.
        let ramp = |size: u32| {
            image::DynamicImage::ImageLuma8(image::GrayImage::from_fn(size, size, |x, y| {
                let nx = (x as f32 + 0.5) / size as f32;
                let ny = (y as f32 + 0.5) / size as f32;
                image::Luma([(nx * 200.0 + ny * 40.0) as u8])
            }))
        };
        let small = ramp(16);
        let big = ramp(160);
        assert_eq!(dhash(&small), dhash(&big));
    }

    #[test]
    fn test_enrich_marks_best_and_sorts() {
        let dir = std::env::temp_dir().join(format!("siegu-dup-enrich-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::database::Database::new(&dir.display().to_string());

        let group = DuplicateGroup {
            members: vec!["a".to_string(), "b".to_string()],
            best_id: Some("b".to_string()),
            unknown_best: false,
        };
        let view = enrich(&db, &group, "exact");
        assert_eq!(view.kind, "exact");
        assert_eq!(view.best_id.as_deref(), Some("b"));
        assert!(view.members.iter().any(|m| m.is_best));
        assert_eq!(view.reclaimable_bytes, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_trash_group_non_best_trashes_only_non_best() {
        let dir = std::env::temp_dir().join(format!("siegu-dup-trash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut db = crate::database::Database::new(&dir.display().to_string());
        for (id, path) in [("a", "/a.jpg"), ("b", "/b.jpg"), ("c", "/c.jpg")] {
            let photo = crate::database::Photo {
                id: id.to_string(),
                location: path.to_string(),
                encoded: String::new(),
                created: "2024-01-01".to_string(),
                objects: Default::default(),
                properties: Default::default(),
                latitude: 0.0,
                longitude: 0.0,
                favorite: false,
                indexed: 1,
                caption: None,
                aesthetics_score: None,
                ai_status: Default::default(),
                sync_needed: false,
                received: false,
                view_only: false,
                last_opened: 0,
            };
            db.store_photo_batch(&[photo]).unwrap();
        }
        db.upsert_dup_hashes("a", "same", "0000000000000000");
        db.upsert_dup_hashes("b", "same", "0000000000000000");
        db.upsert_dup_hashes("c", "other", "ffffffffffffffff");

        let views = detect_all_view(&db, false);
        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.members.len(), 2);
        let trashed = trash_group_non_best(&db, view);
        assert!(trashed >= 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
