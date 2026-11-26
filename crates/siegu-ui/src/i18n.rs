//! Locale catalogs for the GPUI desktop client.
//!
//! The Vue client keeps its translations in `src/locales/*.json`. We embed
//! those exact files so the native client ships the same eight languages and
//! stays in sync without a second source of truth.

#[cfg(test)]
use std::collections::BTreeSet;
use std::{collections::HashMap, sync::OnceLock};

use gpui::{App, Global, SharedString};
use serde_json::Value;

/// The locale codes supported by Siegu, in the same order as the Vue switcher.
pub const CODES: [&str; 8] = ["en", "nl", "fr", "es", "pap", "de", "it", "pt"];

const SOURCES: [(&str, &str); 8] = [
    ("en", include_str!("../../../src/locales/en.json")),
    ("nl", include_str!("../../../src/locales/nl.json")),
    ("fr", include_str!("../../../src/locales/fr.json")),
    ("es", include_str!("../../../src/locales/es.json")),
    ("pap", include_str!("../../../src/locales/pap.json")),
    ("de", include_str!("../../../src/locales/de.json")),
    ("it", include_str!("../../../src/locales/it.json")),
    ("pt", include_str!("../../../src/locales/pt.json")),
];

/// The active language, kept as a GPUI global so every window refreshes when
/// the user changes it in Settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Language {
    code: String,
}

impl Global for Language {}

struct Catalog {
    code: &'static str,
    messages: HashMap<String, String>,
}

static CATALOGS: OnceLock<Vec<Catalog>> = OnceLock::new();

/// Normalize a stored config value or a locale tag from the picker.
pub fn normalize(code: &str) -> String {
    let code = code.trim().to_ascii_lowercase();
    match code.as_str() {
        "en" | "en-us" | "en-gb" => "en",
        "nl" | "nl-nl" | "nl-be" => "nl",
        "fr" | "fr-fr" | "fr-be" | "fr-ca" => "fr",
        "es" | "es-es" | "es-419" => "es",
        "pap" | "pap-aw" | "pap-cw" => "pap",
        "de" | "de-de" | "de-at" | "de-ch" => "de",
        "it" | "it-it" | "it-ch" => "it",
        "pt" | "pt-pt" | "pt-br" => "pt",
        _ => "en",
    }
    .to_string()
}

/// Install the global catalog before opening any windows.
pub fn install(cx: &mut App, code: &str) {
    cx.set_global(Language {
        code: normalize(code),
    });
}

/// The currently selected locale code.
pub fn current(cx: &App) -> String {
    cx.try_global::<Language>()
        .map(|language| language.code.clone())
        .unwrap_or_else(|| "en".to_string())
}

/// Change the active language and repaint every window.
pub fn set_language(code: &str, cx: &mut App) {
    let code = normalize(code);
    if current(cx) != code {
        cx.set_global(Language { code: code.clone() });
    }
    cx.refresh_windows();
}

/// Translate a key using the active catalog. Missing keys fall back to
/// English, then to the key itself, so a partially translated screen still
/// renders something useful.
pub fn text(cx: &App, key: &str, args: &[(&str, &str)]) -> SharedString {
    let code = cx
        .try_global::<Language>()
        .map(|language| language.code.as_str())
        .unwrap_or("en");
    let value = catalogs()
        .iter()
        .find(|catalog| catalog.code == code)
        .or_else(|| catalogs().iter().find(|catalog| catalog.code == "en"))
        .and_then(|catalog| catalog.messages.get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string());
    fill(&value, args).into()
}

/// The localized name of one of the supported languages.
pub fn language_label(cx: &App, code: &str) -> SharedString {
    text(cx, &format!("language.{code}"), &[])
}

fn catalogs() -> &'static [Catalog] {
    CATALOGS
        .get_or_init(|| {
            SOURCES
                .iter()
                .map(|(code, source)| Catalog {
                    code,
                    messages: parse(source),
                })
                .collect()
        })
        .as_slice()
}

fn parse(source: &str) -> HashMap<String, String> {
    let value: Value = serde_json::from_str(source).expect("embedded locale must be valid JSON");
    let mut messages = HashMap::new();
    flatten(value, "", &mut messages);
    messages
}

fn flatten(value: Value, prefix: &str, out: &mut HashMap<String, String>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                let key = if prefix.is_empty() {
                    key
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(value, &key, out);
            }
        }
        Value::String(text) => {
            out.insert(prefix.to_string(), text);
        }
        _ => {}
    }
}

fn fill(value: &str, args: &[(&str, &str)]) -> String {
    let mut filled = value.to_string();
    for (name, replacement) in args {
        filled = filled.replace(&format!("{{{name}}}"), replacement);
    }
    filled
}

#[cfg(test)]
fn sorted_keys(catalog: &Catalog) -> Vec<&String> {
    let mut keys: Vec<_> = catalog.messages.keys().collect();
    keys.sort();
    keys
}

#[cfg(test)]
fn placeholders(value: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = value;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            break;
        };
        found.insert(after[..end].to_string());
        rest = &after[end + 1..];
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_eight_catalogs_parse_and_match_english() {
        let catalogs = catalogs();
        assert_eq!(catalogs.len(), CODES.len());
        let english = catalogs
            .iter()
            .find(|catalog| catalog.code == "en")
            .unwrap();
        assert!(!english.messages.is_empty());

        for catalog in catalogs {
            assert_eq!(
                sorted_keys(catalog),
                sorted_keys(english),
                "{}",
                catalog.code
            );
            for (key, value) in &catalog.messages {
                let expected = english.messages.get(key).expect("key parity checked above");
                assert_eq!(
                    placeholders(value),
                    placeholders(expected),
                    "{}:{}",
                    catalog.code,
                    key
                );
            }
        }
    }

    #[test]
    fn normalize_handles_vue_codes_and_unknown_values() {
        assert_eq!(normalize("en"), "en");
        assert_eq!(normalize("pt-BR"), "pt");
        assert_eq!(normalize("PAP"), "pap");
        assert_eq!(normalize("zz"), "en");
    }

    #[test]
    fn fill_replaces_named_arguments() {
        assert_eq!(fill("hi {name}", &[("name", "Ana")]), "hi Ana");
        assert_eq!(fill("{count} files", &[("count", "3")]), "3 files");
        assert_eq!(fill("no args", &[]), "no args");
    }
}
