use anyhow::Result;
use std::path::Path;
use std::process::Command;

use super::plist;
use super::types::AppInfo;

/// Scan the system for installed applications, returning their paths.
pub fn scan_app_paths() -> Result<Vec<String>> {
    let mut app_paths = Vec::new();

    let output = Command::new("mdfind")
        .arg("kMDItemContentType == 'com.apple.application-bundle'")
        .arg("-onlyin")
        .arg("/System/Applications")
        .arg("-onlyin")
        .arg("/Applications")
        .output()?;

    let content = String::from_utf8_lossy(&output.stdout);
    for line in content.lines() {
        let line = line.trim();
        if !line.is_empty() && line.ends_with(".app") {
            app_paths.push(line.to_string());
        }
    }

    // Add user ~/Applications
    if let Ok(home) = std::env::var("HOME") {
        let user_apps = format!("{}/Applications", home);
        if let Ok(entries) = std::fs::read_dir(&user_apps) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension() == Some(std::ffi::OsStr::new("app")) {
                    if let Some(path_str) = path.to_str() {
                        app_paths.push(path_str.to_string());
                    }
                }
            }
        }
    }

    app_paths.sort();
    app_paths.dedup();
    Ok(app_paths)
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
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
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
        let extensions = plist::parse_extensions(&info_plist).unwrap_or_default();
        let bundle_id = read_bundle_id_from_plist(&info_plist).unwrap_or_default();

        apps.push(AppInfo {
            name,
            bundle_id,
            extensions,
        });
    }

    Ok(apps)
}
