use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::types::AppInfo;
use super::{history, launchservices, listing, scanner, uti};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub associations: BTreeMap<String, String>,
    /// URL scheme handlers (e.g. "http" -> "org.mozilla.firefox").
    #[serde(default)]
    pub schemes: BTreeMap<String, String>,
}

pub struct ImportResult {
    /// (extension key, new app name, previous app name if any)
    pub applied: Vec<(String, String, Option<String>)>,
    pub unchanged: Vec<(String, String)>,
    pub skipped: Vec<(String, String, String)>,
}

/// Export current file associations to a Config.
/// Values are bundle IDs (canonical, lossless for round-tripping).
pub fn export_associations(apps: &[AppInfo]) -> Result<(Config, BTreeMap<String, String>)> {
    let mut associations = BTreeMap::new();
    let mut display_names = BTreeMap::new();

    for assoc in listing::query_all(apps, &|| {}) {
        let Some(bundle_id) = assoc.bundle_id else {
            continue;
        };
        let key = format!(".{}", assoc.ext);
        if let Some(name) = assoc.app_name
            && name != bundle_id
        {
            display_names.insert(key.clone(), name);
        }
        associations.insert(key, bundle_id);
    }

    let schemes = export_schemes(apps, &mut display_names);

    Ok((
        Config {
            associations,
            schemes,
        },
        display_names,
    ))
}

/// Export URL scheme handlers. Only schemes where a real choice exists are
/// included: those declared by more than one installed app, plus the always
/// contested web/mail schemes. Exporting every vanity scheme (slack://,
/// spotify://, ...) would just be noise — the declaring app is the only
/// possible handler.
fn export_schemes(
    apps: &[AppInfo],
    display_names: &mut BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut schemes = BTreeMap::new();
    for scheme in listing::contested_schemes(apps) {
        if let Some(bundle_id) = launchservices::query_default_scheme_handler(&scheme)
            .ok()
            .flatten()
        {
            let name = scanner::resolve_name(apps, &bundle_id);
            if name != bundle_id {
                display_names.insert(scheme.clone(), name);
            }
            schemes.insert(scheme, bundle_id);
        }
    }
    schemes
}

/// Import associations from a Config, applying each one.
/// Values can be bundle IDs or display names — both are accepted, and both
/// must resolve to an installed app. Associations already set correctly are
/// left untouched, so import is idempotent. With `dry_run`, nothing is
/// written and `applied` lists what would change.
pub fn import_associations(config: &Config, apps: &[AppInfo], dry_run: bool) -> ImportResult {
    let mut applied = Vec::new();
    let mut unchanged = Vec::new();
    let mut skipped = Vec::new();

    for (ext_key, value) in &config.associations {
        let ext = ext_key.trim_start_matches('.');

        let (bundle_id, display_name) = match scanner::resolve_app_or_bundle_id(apps, value) {
            Ok(pair) => pair,
            Err(reason) => {
                skipped.push((ext_key.clone(), value.clone(), reason.to_string()));
                continue;
            }
        };

        let uti_str = match uti::uti_for_extension(ext) {
            Ok(u) => u,
            Err(e) => {
                skipped.push((ext_key.clone(), display_name, e.to_string()));
                continue;
            }
        };

        // Leave associations that already match untouched.
        let current = launchservices::query_default_bundle_id(ext).ok().flatten();
        if current
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case(&bundle_id))
        {
            unchanged.push((ext_key.clone(), display_name));
            continue;
        }

        let previous = current.clone().map(|c| scanner::resolve_name(apps, &c));

        if dry_run {
            applied.push((ext_key.clone(), display_name, previous));
            continue;
        }

        match launchservices::set_default(&bundle_id, &uti_str) {
            Ok(_) => {
                // Best-effort: history must never fail the import itself.
                let _ = history::record(history::HistoryEvent {
                    kind: "set".into(),
                    key: ext_key.clone(),
                    old: current,
                    new: Some(bundle_id),
                    detail: None,
                    timestamp: history::now_secs(),
                    source: "import".into(),
                });
                applied.push((ext_key.clone(), display_name, previous));
            }
            Err(e) => {
                skipped.push((ext_key.clone(), display_name, e.to_string()));
            }
        }
    }

    for (scheme_key, value) in &config.schemes {
        let scheme = scheme_key.trim().to_lowercase();
        let display_key = format!("{}://", scheme);

        let (bundle_id, display_name) = match scanner::resolve_app_or_bundle_id(apps, value) {
            Ok(pair) => pair,
            Err(reason) => {
                skipped.push((display_key, value.clone(), reason.to_string()));
                continue;
            }
        };

        let current = launchservices::query_default_scheme_handler(&scheme)
            .ok()
            .flatten();
        if current
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case(&bundle_id))
        {
            unchanged.push((display_key, display_name));
            continue;
        }

        let previous = current.clone().map(|c| scanner::resolve_name(apps, &c));

        if dry_run {
            applied.push((display_key, display_name, previous));
            continue;
        }

        match launchservices::set_default_scheme_handler(&bundle_id, &scheme) {
            Ok(_) => {
                let _ = history::record(history::HistoryEvent {
                    kind: "set_scheme".into(),
                    key: display_key.clone(),
                    old: current,
                    new: Some(bundle_id),
                    detail: None,
                    timestamp: history::now_secs(),
                    source: "import".into(),
                });
                applied.push((display_key, display_name, previous));
            }
            Err(e) => {
                skipped.push((display_key, display_name, e.to_string()));
            }
        }
    }

    ImportResult {
        applied,
        unchanged,
        skipped,
    }
}

/// Serialize a Config to TOML string with display name comments.
pub fn to_toml(config: &Config, display_names: &BTreeMap<String, String>) -> Result<String> {
    let version = env!("CARGO_PKG_VERSION");
    // Build TOML manually so we can add per-line comments with display names
    let mut lines = vec![
        format!("# Generated by openwith v{}", version),
        String::new(),
        "[associations]".to_string(),
    ];

    for (ext, bundle_id) in &config.associations {
        lines.push(toml_line(ext, bundle_id, display_names.get(ext)));
    }

    if !config.schemes.is_empty() {
        lines.push(String::new());
        lines.push("[schemes]".to_string());
        for (scheme, bundle_id) in &config.schemes {
            lines.push(toml_line(scheme, bundle_id, display_names.get(scheme)));
        }
    }

    lines.push(String::new());
    Ok(lines.join("\n"))
}

fn toml_line(key: &str, value: &str, display_name: Option<&String>) -> String {
    let escaped_key = format!("\"{}\"", key);
    let escaped_val = format!("\"{}\"", value);
    match display_name {
        Some(name) => format!("{} = {}  # {}", escaped_key, escaped_val, name),
        None => format!("{} = {}", escaped_key, escaped_val),
    }
}

/// Deserialize a Config from a TOML string.
pub fn from_toml(content: &str) -> Result<Config> {
    toml::from_str(content).context("failed to parse TOML config")
}

#[cfg(test)]
mod tests {
    use super::{from_toml, to_toml};
    use std::collections::BTreeMap;

    #[test]
    fn toml_round_trip_preserves_associations_and_schemes() {
        let input = r#"
[associations]
".md" = "abnerworks.Typora"

[schemes]
"http" = "org.mozilla.firefox"
"#;

        let config = from_toml(input).unwrap();
        assert_eq!(
            config.associations.get(".md").map(String::as_str),
            Some("abnerworks.Typora")
        );
        assert_eq!(
            config.schemes.get("http").map(String::as_str),
            Some("org.mozilla.firefox")
        );

        let mut display_names = BTreeMap::new();
        display_names.insert(".md".to_string(), "Typora".to_string());
        display_names.insert("http".to_string(), "Firefox".to_string());

        let out = to_toml(&config, &display_names).unwrap();
        assert!(out.contains("[associations]"));
        assert!(out.contains("\".md\" = \"abnerworks.Typora\"  # Typora"));
        assert!(out.contains("[schemes]"));
        assert!(out.contains("\"http\" = \"org.mozilla.firefox\"  # Firefox"));

        let reparsed = from_toml(&out).unwrap();
        assert_eq!(reparsed.associations, config.associations);
        assert_eq!(reparsed.schemes, config.schemes);
    }

    #[test]
    fn missing_schemes_table_defaults_to_empty() {
        let config = from_toml("[associations]\n\".pdf\" = \"com.apple.Preview\"\n").unwrap();
        assert!(config.schemes.is_empty());
    }
}
