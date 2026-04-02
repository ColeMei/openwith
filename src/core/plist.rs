use anyhow::Result;
use std::collections::HashSet;
use std::process::Command;

/// Parse an application's Info.plist to extract supported file extensions.
pub fn parse_extensions(plist_path: &str) -> Result<Vec<String>> {
    if !std::path::Path::new(plist_path).exists() {
        return Ok(vec![]);
    }

    let output = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg("Print :CFBundleDocumentTypes")
        .arg(plist_path)
        .output();

    let mut extensions = HashSet::new();

    if let Ok(output) = output {
        let content = String::from_utf8_lossy(&output.stdout);
        let mut is_collecting = false;

        for line in content.lines() {
            let line = line.trim();
            if line == "}" && is_collecting {
                is_collecting = false;
                continue;
            }
            if is_collecting && !line.is_empty() {
                extensions.insert(line.to_string());
                continue;
            }
            if line == "CFBundleTypeExtensions = Array {" {
                is_collecting = true;
            }
        }
    }

    let mut result: Vec<String> = extensions.into_iter().collect();
    result.sort();
    Ok(result)
}
