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
    let missing = db.photos_missing_dup_hashes(100_000);
    let mut computed = 0usize;
    for (id, location) in &missing {
        let file_sha = std::fs::read(location).ok().map(|bytes| {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(&bytes);
            format!("{:x}", h.finalize())
        });
        let dh = image_from_thumb(db, id).map(|img| dhash(&img));
        if let (Some(fs), Some(dh)) = (file_sha, dh) {
            db.upsert_dup_hashes(id, &fs, &dh);
        }
        computed += 1;
    }

    let groups = group_non_ai(db);
    (groups, computed)
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
}
