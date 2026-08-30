//! Generates the TypeScript RPC contract from the Rust command catalog.
//!
//! `src/rpc_catalog.rs` is the single source of truth for every command in the
//! siegu-core facade. This build script extracts each `spec(...)` entry and
//! emits `shared/generated/rpc-commands.ts`, which the browser data plane
//! imports — so command names, capability tiers, argument keys and the
//! stringify set are never hand-duplicated in TypeScript.
//!
//! A Rust unit test (see `crate::rpc_catalog`) re-parses the committed TS and
//! fails when it drifts from `CATALOG`, so the generated file stays in sync
//! even though it is committed for contributor convenience.

use std::env;
use std::fs;
use std::path::PathBuf;

/// Parse the `spec(...)` entries out of `rpc_catalog.rs` source text.
/// Returns `(name, tier, stringify, args)`.
fn parse_specs(src: &str) -> Vec<(String, String, bool, Vec<String>)> {
    let mut out = Vec::new();
    let bytes: Vec<char> = src.chars().collect();
    let mut i = 0;

    while i < bytes.len() {
        // find "spec(" occurrences
        if bytes[i] == 's'
            && bytes.get(i + 1) == Some(&'p')
            && bytes.get(i + 2) == Some(&'e')
            && bytes.get(i + 3) == Some(&'c')
            && bytes.get(i + 4) == Some(&'(')
            && (i == 0 || !is_ident_char(bytes[i - 1]))
        {
            // consume to matching close paren
            let start = i + 5; // after "spec("
            let mut depth = 1usize;
            let mut j = start;
            while j < bytes.len() && depth > 0 {
                match bytes[j] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            let inner: String = bytes[start..j.saturating_sub(1)].iter().collect();
            if let Some(spec) = parse_one(&inner) {
                out.push(spec);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Parse the comma-separated contents of a single `spec(...)` call.
fn parse_one(inner: &str) -> Option<(String, String, bool, Vec<String>)> {
    let parts = split_top_level(inner);
    if parts.len() != 4 {
        return None;
    }
    let name = strip_quotes(parts[0].trim())?;
    let tier = parts[1].trim().trim_start_matches("Tier::").to_string();
    let stringify = parts[2].trim() == "true";
    let args = parse_arg_array(&parts[3]);
    Some((name, tier, stringify, args))
}

/// Split on commas at the top level of `inner` (respecting quotes/brackets).
fn split_top_level(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                cur.push(c);
                // handle escaped quotes
                if i > 0 && chars[i - 1] == '\\' {
                    // escaped, keep going
                } else {
                    in_string = !in_string;
                }
            }
            '[' if !in_string => {
                depth += 1;
                cur.push(c);
            }
            ']' if !in_string => {
                depth -= 1;
                cur.push(c);
            }
            ',' if !in_string && depth == 0 => {
                parts.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
        i += 1;
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

fn strip_quotes(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        Some(s[1..s.len() - 1].to_string())
    } else {
        None
    }
}

fn parse_arg_array(s: &str) -> Vec<String> {
    let s = s.trim();
    let inner = s
        .strip_prefix("&[")
        .and_then(|t| t.strip_suffix(']'))
        .unwrap_or("");
    split_top_level(inner)
        .into_iter()
        .filter_map(|a| strip_quotes(&a))
        .collect()
}

fn main() {
    println!("cargo:rerun-if-changed=src/rpc_catalog.rs");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = fs::read_to_string(manifest.join("src/rpc_catalog.rs")).expect("read rpc_catalog.rs");
    let specs = parse_specs(&src);

    // Locate repo-level shared/generated dir: workspace root is 3 levels up
    // from crates/siegu-core (manifest dir = .../crates/siegu-core).
    let workspace_root = manifest.parent().and_then(|p| p.parent()).unwrap();
    let out_dir = workspace_root.join("shared/generated");
    fs::create_dir_all(&out_dir).expect("create shared/generated");

    let mut ts = String::new();
    ts.push_str("/* eslint-disable */\n");
    ts.push_str("// AUTO-GENERATED by crates/siegu-core/build.rs — DO NOT EDIT BY HAND.\n");
    ts.push_str(
        "// Source of truth: crates/siegu-core/src/rpc_catalog.rs (the Rust command catalog).\n",
    );
    ts.push_str(
        "// Regenerate with `cargo build -p siegu-core`. A Rust unit test enforces sync.\n\n",
    );
    ts.push_str("export interface RpcCommandSpec {\n");
    ts.push_str("  name: string;\n");
    ts.push_str("  tier: 'read' | 'write' | 'owner';\n");
    ts.push_str("  stringify: boolean;\n");
    ts.push_str("  args: string[];\n");
    ts.push_str("}\n\n");
    ts.push_str("export const RPC_COMMANDS: RpcCommandSpec[] = [\n");

    for (name, tier, stringify, args) in &specs {
        let tier_ts = match tier.as_str() {
            "ReadOnly" => "read",
            "ReadWrite" => "write",
            "Owner" => "owner",
            other => other,
        };
        let args_ts: Vec<String> = args.iter().map(|a| format!("\"{a}\"")).collect();
        ts.push_str(&format!(
            "  {{ name: \"{name}\", tier: \"{tier_ts}\", stringify: {stringify}, args: [{}] }},\n",
            args_ts.join(", ")
        ));
    }
    ts.push_str("];\n\n");

    ts.push_str("/** Command names in each tier. */\n");
    ts.push_str("export const READ_ONLY_COMMANDS: ReadonlySet<string> = new Set(\n  RPC_COMMANDS.filter((c) => c.tier === 'read').map((c) => c.name),\n);\n");
    ts.push_str("export const READ_WRITE_COMMANDS: ReadonlySet<string> = new Set(\n  RPC_COMMANDS.filter((c) => c.tier === 'write').map((c) => c.name),\n);\n");
    ts.push_str("export const OWNER_COMMANDS: ReadonlySet<string> = new Set(\n  RPC_COMMANDS.filter((c) => c.tier === 'owner').map((c) => c.name),\n);\n\n");
    ts.push_str(
        "/** Commands whose resolved value must be JSON-stringified for the browser caller. */\n",
    );
    ts.push_str("export const STRINGIFY_RESULT: ReadonlySet<string> = new Set(\n  RPC_COMMANDS.filter((c) => c.stringify).map((c) => c.name),\n);\n");

    fs::write(out_dir.join("rpc-commands.ts"), ts).expect("write rpc-commands.ts");
    println!(
        "cargo:warning=generated shared/generated/rpc-commands.ts ({})",
        specs.len()
    );
}
