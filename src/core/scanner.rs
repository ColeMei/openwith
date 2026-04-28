use anyhow::{Result, bail};
use std::path::Path;
use std::process::Command;

use super::types::AppInfo;
use super::{plist, uti};

/// Scan the system for installed applications, returning their paths.
pub fn scan_app_paths() -> Result<Vec<String>> {
    let mut app_paths = Vec::new();

    if let Ok(output) = Command::new("mdfind")
        .arg("kMDItemContentType == 'com.apple.application-bundle'")
        .arg("-onlyin")
        .arg("/System/Applications")
        .arg("-onlyin")
        .arg("/Applications")
        .output()
    {
        let content = String::from_utf8_lossy(&output.stdout);
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && line.ends_with(".app") {
                app_paths.push(line.to_string());
            }
        }
    }

    collect_app_paths_from_dir(Path::new("/System/Applications"), 3, &mut app_paths);
    collect_app_paths_from_dir(Path::new("/Applications"), 3, &mut app_paths);

    if let Ok(home) = std::env::var("HOME") {
        let user_apps = format!("{}/Applications", home);
        collect_app_paths_from_dir(Path::new(&user_apps), 3, &mut app_paths);
    }

    app_paths.sort();
    app_paths.dedup();
    Ok(app_paths)
}

fn collect_app_paths_from_dir(dir: &Path, max_depth: usize, app_paths: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension() == Some(std::ffi::OsStr::new("app")) {
            if let Some(path_str) = path.to_str() {
                app_paths.push(path_str.to_string());
            }
            continue;
        }

        if max_depth > 0 && path.is_dir() {
            collect_app_paths_from_dir(&path, max_depth - 1, app_paths);
        }
    }
}

/// Read CFBundleIdentifier from an app's Info.plist via PlistBuddy.
fn read_bundle_id_from_plist(plist_path: &str) -> Option<String> {
    let output = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg("Print :CFBundleIdentifier")
        .arg(plist_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if id.is_empty() { None } else { Some(id) }
}

/// Scan all apps and build AppInfo structs with extensions and bundle IDs.
pub fn scan_all_apps() -> Result<Vec<AppInfo>> {
    let paths = scan_app_paths()?;
    let mut apps = Vec::new();

    for app_path in &paths {
        let name = match Path::new(app_path).file_stem().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let info_plist = format!("{}/Contents/Info.plist", app_path);
        let document_types = plist::parse_document_types(&info_plist).unwrap_or_default();
        let bundle_id = read_bundle_id_from_plist(&info_plist).unwrap_or_default();

        apps.push(AppInfo {
            name,
            bundle_id,
            extensions: document_types.extensions,
            content_types: document_types.content_types,
        });
    }

    Ok(apps)
}

pub fn app_supports_extension(app: &AppInfo, ext: &str) -> bool {
    let ext = ext.trim_start_matches('.');
    if app.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
        return true;
    }

    let Ok(ext_uti) = uti::uti_for_extension(ext) else {
        return false;
    };

    app.content_types
        .iter()
        .any(|content_type| uti::conforms_to(content_type, &ext_uti))
}

/// Resolve an app name using exact match first, then a unique fuzzy match.
pub fn resolve_app<'a>(apps: &'a [AppInfo], app_name: &str) -> Result<&'a AppInfo> {
    let search = app_name.trim();
    if search.is_empty() {
        bail!("app name cannot be empty");
    }

    let exact_matches: Vec<&AppInfo> = apps
        .iter()
        .filter(|app| app.name.eq_ignore_ascii_case(search))
        .collect();

    match exact_matches.len() {
        1 => return Ok(exact_matches[0]),
        n if n > 1 => bail!(ambiguous_app_message(search, &exact_matches)),
        _ => {}
    }

    let search_lower = search.to_lowercase();
    let fuzzy_matches: Vec<&AppInfo> = apps
        .iter()
        .filter(|app| app.name.to_lowercase().contains(&search_lower))
        .collect();

    match fuzzy_matches.len() {
        1 => Ok(fuzzy_matches[0]),
        0 => bail!("app '{}' not found", app_name),
        _ => bail!(ambiguous_app_message(search, &fuzzy_matches)),
    }
}

/// Resolve a bundle ID to an app name. Falls back to the raw bundle ID.
pub fn resolve_name(apps: &[AppInfo], bundle_id: &str) -> String {
    apps.iter()
        .find(|a| a.bundle_id.eq_ignore_ascii_case(bundle_id))
        .map(|a| a.name.clone())
        .unwrap_or_else(|| bundle_id.to_string())
}

fn ambiguous_app_message(search: &str, matches: &[&AppInfo]) -> String {
    let candidates = matches
        .iter()
        .map(|app| {
            if app.bundle_id.is_empty() {
                app.name.clone()
            } else {
                format!("{} ({})", app.name, app.bundle_id)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "app '{}' is ambiguous; matches: {}. Use a more specific app name.",
        search, candidates
    )
}

#[cfg(test)]
mod tests {
    use super::{collect_app_paths_from_dir, resolve_app};
    use crate::core::types::AppInfo;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn app(name: &str, bundle_id: &str) -> AppInfo {
        AppInfo {
            name: name.to_string(),
            bundle_id: bundle_id.to_string(),
            extensions: vec![],
            content_types: vec![],
        }
    }

    #[test]
    fn resolve_app_prefers_case_insensitive_exact_match() {
        let apps = vec![
            app("Google Chrome", "com.google.Chrome"),
            app("Chrome Canary", "com.google.Chrome.canary"),
        ];

        let resolved = resolve_app(&apps, "google chrome").unwrap();

        assert_eq!(resolved.bundle_id, "com.google.Chrome");
    }

    #[test]
    fn resolve_app_accepts_unique_fuzzy_match() {
        let apps = vec![
            app("Preview", "com.apple.Preview"),
            app("Skim", "net.sourceforge.skim-app.skim"),
        ];

        let resolved = resolve_app(&apps, "prev").unwrap();

        assert_eq!(resolved.bundle_id, "com.apple.Preview");
    }

    #[test]
    fn resolve_app_rejects_ambiguous_fuzzy_match() {
        let apps = vec![
            app("Visual Studio Code", "com.microsoft.VSCode"),
            app("CodeRunner", "com.krill.CodeRunner"),
        ];

        let err = resolve_app(&apps, "code").unwrap_err().to_string();

        assert!(err.contains("ambiguous"));
        assert!(err.contains("Visual Studio Code"));
        assert!(err.contains("CodeRunner"));
    }

    #[test]
    fn filesystem_scan_finds_nested_app_bundles() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("openwith-scan-test-{unique}"));
        let nested_app = root.join("Utilities").join("Preview.app");
        fs::create_dir_all(&nested_app).unwrap();

        let mut paths = Vec::new();
        collect_app_paths_from_dir(&root, 3, &mut paths);

        assert_eq!(paths, vec![nested_app.to_string_lossy().to_string()]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn app_supports_extension_uses_declared_content_type_direction() {
        let broad_text_app = AppInfo {
            name: "Broad Text App".to_string(),
            bundle_id: "example.broad-text".to_string(),
            extensions: vec![],
            content_types: vec!["public.text".to_string()],
        };
        let markdown_app = AppInfo {
            name: "Markdown App".to_string(),
            bundle_id: "example.markdown".to_string(),
            extensions: vec![],
            content_types: vec!["net.daringfireball.markdown".to_string()],
        };

        assert!(!super::app_supports_extension(&broad_text_app, "md"));
        assert!(super::app_supports_extension(&markdown_app, "md"));
    }
}
