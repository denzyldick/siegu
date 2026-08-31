//! Single source of truth for the siegu-core RPC command contract.
//!
//! Every command a frontend can call through the shared facade
//! ([`crate::rpc::dispatch`]) is declared exactly once here. The runtime
//! allowlists in `rpc.rs`, the generated TypeScript contract
//! (`shared/generated/rpc-commands.ts`), and the drift-guard test all derive
//! from [`CATALOG`], so the Rust table is the only place a command's name,
//! mutability, capability tier, argument keys, and result-stringify flag are
//! defined.
//!
//! A frontend engineer never descends into ML/DB/worker internals; they call
//! the facade. Adding a command = add one row here, then build (regenerates
//! the TS) and implement the `Backend` method.

/// Capability tier controlling who may invoke a command.
///
/// Mirrors the ownership model: guests can read and (opt-in) mutate the
/// library, but only `Owner` principals may trigger heavy host work (ML
/// analysis/indexing, sync/session/device control).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tier {
    /// Read the library. Allowed for every principal.
    #[default]
    ReadOnly,
    /// Mutate library state (favorites, trash, albums, people, config).
    /// Allowed for `rw` and `Owner` principals.
    ReadWrite,
    /// Execute host work (ML, sync/session/device). `Owner` only.
    Owner,
}

/// Static description of a single command in the facade.
pub struct CommandSpec {
    /// Wire name as sent in `CommandRequest.name` / `POST /rpc {"name"}`.
    pub name: &'static str,
    /// Capability tier (gates `READ_ONLY`/`READ_WRITE`/`OWNER` allowlists).
    pub tier: Tier,
    /// Whether the resolved value must be JSON-stringified for the browser
    /// caller to re-parse (matches the Tauri return-vs-invoke-string contract).
    pub stringify: bool,
    /// Accepted payload key names (used for the generated TS argument types).
    pub args: &'static [&'static str],
}

/// The complete, ordered command catalog. This is the contract.
pub const CATALOG: &[CommandSpec] = &[
    // ── library browsing (read-only) ──────────────────────────────────────
    spec(
        "list_files",
        Tier::ReadOnly,
        true,
        &[
            "query",
            "offset",
            "limit",
            "favorites_only",
            "videos_only",
            "person_ids",
            "person_match",
            "person_alone",
            "location",
            "tag",
            "date_from",
            "date_to",
            "has_faces",
            "aesthetics_min",
            "camera",
            "papers",
            "nsfw_only",
            "random",
            "order_by",
            "album_id",
        ],
    ),
    spec("get_photo_by_id", Tier::ReadOnly, true, &["id"]),
    spec("get_photos_by_ids", Tier::ReadOnly, true, &["ids"]),
    spec("get_photo_encoded_batch", Tier::ReadOnly, true, &["ids"]),
    spec("get_photo_ocr", Tier::ReadOnly, false, &["id"]),
    spec("get_heatmap_data", Tier::ReadOnly, true, &[]),
    spec("day_counts", Tier::ReadOnly, true, &["from", "to"]),
    // ── search, facets, tags, geo (read-only) ─────────────────────────────
    spec("get_search_facets", Tier::ReadOnly, true, &[]),
    spec("search_facets", Tier::ReadOnly, true, &[]),
    spec("get_top_tags", Tier::ReadOnly, true, &[]),
    spec("list_objects", Tier::ReadOnly, true, &["query"]),
    spec("get_location_names", Tier::ReadOnly, false, &[]),
    // ── albums (read-only) ────────────────────────────────────────────────
    spec("list_albums", Tier::ReadOnly, true, &[]),
    spec("get_album", Tier::ReadOnly, true, &["album_id"]),
    spec("get_album_sections", Tier::ReadOnly, true, &[]),
    spec(
        "get_album_contents",
        Tier::ReadOnly,
        true,
        &["album_id", "offset", "limit"],
    ),
    spec("get_clip_categories", Tier::ReadOnly, true, &[]),
    // ── people (read-only) ────────────────────────────────────────────────
    spec("get_people", Tier::ReadOnly, true, &[]),
    spec("get_unnamed_faces", Tier::ReadOnly, true, &[]),
    spec(
        "get_person_photos",
        Tier::ReadOnly,
        true,
        &["person_id", "offset", "limit"],
    ),
    spec("get_person_faces", Tier::ReadOnly, true, &["person_id"]),
    spec("get_faces_for_photo", Tier::ReadOnly, true, &["photo_id"]),
    // ── directories / config / status / models / storage (read-only) ──────
    spec("list_directories", Tier::ReadOnly, true, &[]),
    spec("is_initialized", Tier::ReadOnly, false, &[]),
    spec("get_config", Tier::ReadOnly, true, &[]),
    spec("get_last_scan_time", Tier::ReadOnly, false, &[]),
    spec("get_unindexed_count", Tier::ReadOnly, false, &[]),
    spec("get_max_photo_rowid", Tier::ReadOnly, false, &[]),
    spec("get_indexing_status", Tier::ReadOnly, false, &[]),
    spec("storage_usage", Tier::ReadOnly, false, &[]),
    spec("check_models", Tier::ReadOnly, true, &[]),
    spec("get_model_capabilities", Tier::ReadOnly, true, &[]),
    // ── trash (read-only) ─────────────────────────────────────────────────
    spec("count_trash", Tier::ReadOnly, false, &[]),
    spec("list_trash", Tier::ReadOnly, true, &["limit"]),
    // ── favorites / trash (read-write) ────────────────────────────────────
    spec("toggle_favorite", Tier::ReadWrite, false, &["id"]),
    spec(
        "set_favorites",
        Tier::ReadWrite,
        false,
        &["ids", "favorite"],
    ),
    spec("trash_photo", Tier::ReadWrite, false, &["id"]),
    spec("restore_photo", Tier::ReadWrite, false, &["id"]),
    spec("empty_trash", Tier::ReadWrite, false, &[]),
    spec("delete_photo_permanently", Tier::ReadWrite, false, &["id"]),
    // ── albums (read-write) ───────────────────────────────────────────────
    spec("create_album", Tier::ReadWrite, true, &["name"]),
    spec(
        "create_smart_album",
        Tier::ReadWrite,
        true,
        &["name", "rule", "kind"],
    ),
    spec(
        "update_smart_album_rule",
        Tier::ReadWrite,
        false,
        &["album_id", "rule"],
    ),
    spec(
        "rename_album",
        Tier::ReadWrite,
        false,
        &["album_id", "name"],
    ),
    spec("delete_album", Tier::ReadWrite, false, &["album_id"]),
    spec("clear_dismissed_trips", Tier::ReadWrite, false, &[]),
    spec("sync_trips", Tier::ReadWrite, false, &[]),
    spec(
        "add_album_items",
        Tier::ReadWrite,
        false,
        &["album_id", "photo_ids"],
    ),
    spec(
        "remove_album_items",
        Tier::ReadWrite,
        false,
        &["album_id", "photo_ids"],
    ),
    spec(
        "reorder_album",
        Tier::ReadWrite,
        false,
        &["album_id", "ordered_ids"],
    ),
    // ── people (read-write) ───────────────────────────────────────────────
    spec(
        "assign_name_to_face",
        Tier::ReadWrite,
        false,
        &["face_id", "name"],
    ),
    spec("delete_face", Tier::ReadWrite, false, &["face_id"]),
    spec(
        "merge_people",
        Tier::ReadWrite,
        false,
        &["from_id", "to_id"],
    ),
    spec("rename_person", Tier::ReadWrite, false, &["id", "new_name"]),
    // ── config / directories / housekeeping (read-write) ──────────────────
    spec("save_config", Tier::ReadWrite, false, &["key", "value"]),
    spec("add_directory", Tier::ReadWrite, false, &["path"]),
    spec("remove_directory", Tier::ReadWrite, false, &["path"]),
    spec("remove_directory_full", Tier::ReadWrite, false, &["path"]),
    spec("mark_onboarding_complete", Tier::ReadWrite, false, &[]),
    spec("cleanup_database", Tier::ReadWrite, false, &["confirm"]),
    // ── ML analysis / indexing (owner-only) ────────────────────────────────
    // These drive the host's live ML worker. They are `Owner`-tier: a guest
    // (WebRTC share) must never trigger heavy analysis/indexing on the host.
    spec("analyze_photo", Tier::Owner, false, &["id"]),
    spec(
        "analyze_photo_model",
        Tier::Owner,
        false,
        &["id", "model_id"],
    ),
    spec("analyze_model", Tier::Owner, false, &["model_id"]),
    spec("index_faces", Tier::Owner, false, &[]),
    spec("abort_indexing", Tier::Owner, false, &[]),
    spec("pause_indexing", Tier::Owner, false, &[]),
    spec("resume_indexing", Tier::Owner, false, &[]),
    spec("reload_models", Tier::Owner, false, &[]),
    spec("unload_models", Tier::Owner, false, &[]),
    spec("get_models_loaded", Tier::Owner, false, &[]),
];

/// Build a [`CommandSpec`] from a name + args, using a derived stringify flag
/// membership set (kept in sync with the browser `STRINGIFY_RESULT` set today).
const fn spec(
    name: &'static str,
    tier: Tier,
    stringify: bool,
    args: &'static [&'static str],
) -> CommandSpec {
    CommandSpec {
        name,
        tier,
        stringify,
        args,
    }
}

/// Command names whose tier is at least the given tier.
pub fn commands_at_or_above(tier: Tier) -> Vec<&'static str> {
    CATALOG
        .iter()
        .filter(|c| tier_rank(c.tier) >= tier_rank(tier))
        .map(|c| c.name)
        .collect()
}

/// Names that require exact tier match (used for the allowlists).
pub fn command_names(tier: Tier) -> Vec<&'static str> {
    CATALOG
        .iter()
        .filter(|c| c.tier == tier)
        .map(|c| c.name)
        .collect()
}

/// Names allowed for the tier, i.e. everything at or above it.
pub fn allowed_names(tier: Tier) -> Vec<&'static str> {
    CATALOG
        .iter()
        .filter(|c| tier_rank(c.tier) >= tier_rank(tier))
        .map(|c| c.name)
        .collect()
}

const fn tier_rank(t: Tier) -> u8 {
    match t {
        Tier::ReadOnly => 0,
        Tier::ReadWrite => 1,
        Tier::Owner => 2,
    }
}

/// Whether a command exists in the catalog.
pub fn is_known(name: &str) -> bool {
    CATALOG.iter().any(|c| c.name == name)
}

/// Whether a command is a mutation (rw or owner tier).
pub fn is_mutation(name: &str) -> bool {
    CATALOG
        .iter()
        .any(|c| c.name == name && c.tier != Tier::ReadOnly)
}

/// Whether a command requires `Owner` capability.
pub fn is_owner_only(name: &str) -> bool {
    CATALOG
        .iter()
        .any(|c| c.name == name && c.tier == Tier::Owner)
}

/// Whether a command's result should be stringified for browser callers.
pub fn should_stringify(name: &str) -> bool {
    CATALOG.iter().any(|c| c.name == name && c.stringify)
}

/// Name ↔ spec lookup.
pub fn spec_for(name: &str) -> Option<&'static CommandSpec> {
    CATALOG.iter().find(|c| c.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Re-parse a single `{ name, tier, stringify, args }` line from the
    /// generated TS into the fields we care about for the sync check.
    fn parse_ts_line(line: &str) -> Option<(String, String, bool, Vec<String>)> {
        let line = line.trim();
        let line = line.strip_prefix('{')?.trim();
        // Drop a trailing `},` (the JSON-object closing brace + comma).
        let line = line.strip_suffix(',').map(|s| s.trim()).unwrap_or(line);
        // If it's the last line-adjacent entry (no trailing comma), drop `}`.
        let line = line.strip_suffix('}').map(|s| s.trim()).unwrap_or(line);

        let mut name = String::new();
        let mut tier = String::new();
        let mut stringify = false;
        let mut args: Vec<String> = Vec::new();

        for part in split_commas(line) {
            if let Some(rest) = part.strip_prefix("name:") {
                name = rest.trim().trim_matches('"').to_string();
            } else if let Some(rest) = part.strip_prefix("tier:") {
                let t = rest.trim().trim_matches('"');
                tier = t.trim_matches('\'').to_string();
            } else if let Some(rest) = part.strip_prefix("stringify:") {
                stringify = rest.trim() == "true";
            } else if let Some(rest) = part.strip_prefix("args:") {
                let inner = rest
                    .trim()
                    .strip_prefix('[')
                    .and_then(|t| t.strip_suffix(']'))
                    .unwrap_or("");
                args = inner
                    .split(',')
                    .map(|a| a.trim().trim_matches('"').to_string())
                    .filter(|a| !a.is_empty())
                    .collect();
            }
        }
        if name.is_empty() {
            None
        } else {
            Some((name, tier, stringify, args))
        }
    }

    fn split_commas(s: &str) -> Vec<String> {
        // Split on top-level commas only (respects the `args: [...]` brackets).
        let mut out = Vec::new();
        let mut depth = 0i32;
        let mut cur = String::new();
        for c in s.chars() {
            match c {
                '[' => {
                    depth += 1;
                    cur.push(c);
                }
                ']' => {
                    depth -= 1;
                    cur.push(c);
                }
                ',' if depth == 0 => {
                    out.push(cur.trim().to_string());
                    cur.clear();
                }
                _ => cur.push(c),
            }
        }
        if !cur.trim().is_empty() {
            out.push(cur.trim().to_string());
        }
        out
    }

    fn tier_to_ts(tier: Tier) -> &'static str {
        match tier {
            Tier::ReadOnly => "read",
            Tier::ReadWrite => "write",
            Tier::Owner => "owner",
        }
    }

    #[test]
    fn generated_ts_matches_catalog() {
        let manifest = match std::env::var("CARGO_MANIFEST_DIR") {
            Ok(v) => v,
            Err(_) => return,
        };
        let path = std::path::Path::new(&manifest)
            .parent()
            .and_then(|p| p.parent())
            .unwrap()
            .join("shared/generated/rpc-commands.ts");
        let ts = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read generated TS at {}: {e}", path.display()));

        let mut parsed: Vec<(String, String, bool, Vec<String>)> =
            ts.lines().filter_map(parse_ts_line).collect();
        parsed.sort_by(|a, b| a.0.cmp(&b.0));

        let mut catalog: Vec<(String, String, bool, Vec<String>)> = CATALOG
            .iter()
            .map(|c| {
                (
                    c.name.to_string(),
                    tier_to_ts(c.tier).to_string(),
                    c.stringify,
                    c.args.iter().map(|a| a.to_string()).collect(),
                )
            })
            .collect();
        catalog.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            parsed.len(),
            catalog.len(),
            "generated TS command count ({}) differs from catalog ({}). Regenerate with `cargo build -p siegu-core`.",
            parsed.len(),
            catalog.len()
        );

        for (p, c) in parsed.iter().zip(catalog.iter()) {
            assert_eq!(p, c, "generated TS entry for '{}' drifted from the Rust catalog. Regenerate with `cargo build -p siegu-core`.", c.0);
        }
    }

    #[test]
    fn ml_commands_are_owner_tier() {
        // The ML-trigger commands must be Owner-tier so a guest can never run
        // them (security guard at the dispatch + transport boundary).
        for name in [
            "analyze_photo",
            "analyze_photo_model",
            "analyze_model",
            "index_faces",
            "abort_indexing",
            "pause_indexing",
            "resume_indexing",
            "reload_models",
            "unload_models",
            "get_models_loaded",
        ] {
            assert!(
                is_owner_only(name),
                "{name} must be Owner-only (guest must never run ML)"
            );
            assert!(is_known(name));
        }
        // Doc reads stay accessible to read-only principals.
        assert!(!is_owner_only("get_indexing_status"));
        assert!(!is_owner_only("get_unindexed_count"));
    }

    #[test]
    fn catalog_unique_names() {
        let mut names: Vec<&str> = CATALOG.iter().map(|c| c.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(
            names.len(),
            CATALOG.len(),
            "catalog has duplicate command names"
        );
    }
}
