use anyhow::Result;
use plist::Value;
use std::collections::BTreeSet;
use std::path::Path;

/// Metadata extracted from an application's Info.plist.
#[derive(Debug, Default)]
pub struct BundleInfo {
    pub bundle_id: Option<String>,
    pub extensions: Vec<String>,
    pub content_types: Vec<String>,
}

/// Parse an application's Info.plist (XML or binary).
/// Missing or unreadable plists yield an empty `BundleInfo` so a single
/// malformed app cannot abort a whole scan.
pub fn parse_bundle_info(plist_path: &Path) -> Result<BundleInfo> {
    match Value::from_file(plist_path) {
        Ok(value) => Ok(bundle_info_from_value(&value)),
        Err(_) => Ok(BundleInfo::default()),
    }
}

fn bundle_info_from_value(value: &Value) -> BundleInfo {
    let Some(dict) = value.as_dictionary() else {
        return BundleInfo::default();
    };

    let bundle_id = dict
        .get("CFBundleIdentifier")
        .and_then(Value::as_string)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut extensions = BTreeSet::new();
    let mut content_types = BTreeSet::new();
    if let Some(doc_types) = dict.get("CFBundleDocumentTypes").and_then(Value::as_array) {
        for doc_type in doc_types.iter().filter_map(Value::as_dictionary) {
            for ext in string_array(doc_type.get("CFBundleTypeExtensions")) {
                insert_normalized(&mut extensions, &ext, true);
            }
            for content_type in string_array(doc_type.get("LSItemContentTypes")) {
                insert_normalized(&mut content_types, &content_type, false);
            }
        }
    }

    BundleInfo {
        bundle_id,
        extensions: extensions.into_iter().collect(),
        content_types: content_types.into_iter().collect(),
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(Value::as_string)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn insert_normalized(values: &mut BTreeSet<String>, value: &str, is_extension: bool) {
    let value = value.trim().to_lowercase();
    let value = if is_extension {
        value.trim_start_matches('.')
    } else {
        &value
    };

    if !value.is_empty() && value != "*" {
        values.insert(value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::{BundleInfo, parse_bundle_info};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.example.Editor</string>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeExtensions</key>
            <array>
                <string>MD</string>
                <string>.markdown</string>
                <string>*</string>
            </array>
        </dict>
        <dict>
            <key>LSItemContentTypes</key>
            <array>
                <string>public.plain-text</string>
                <string>public.JSON</string>
            </array>
        </dict>
    </array>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>editor</string>
                <string>Editor-Beta</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
"#;

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("openwith-plist-test-{unique}-{name}"))
    }

    fn parse_fixture(name: &str, content: &str) -> BundleInfo {
        let path = temp_path(name);
        fs::write(&path, content).unwrap();
        let info = parse_bundle_info(&path).unwrap();
        fs::remove_file(&path).unwrap();
        info
    }

    #[test]
    fn parses_xml_plist_with_normalization() {
        let info = parse_fixture("xml.plist", FIXTURE);

        assert_eq!(info.bundle_id.as_deref(), Some("com.example.Editor"));
        assert_eq!(info.extensions, vec!["markdown", "md"]);
        assert_eq!(info.content_types, vec!["public.json", "public.plain-text"]);
    }

    #[test]
    fn parses_binary_plist() {
        let xml_path = temp_path("roundtrip-src.plist");
        fs::write(&xml_path, FIXTURE).unwrap();
        let value = plist::Value::from_file(&xml_path).unwrap();
        fs::remove_file(&xml_path).unwrap();

        let bin_path = temp_path("binary.plist");
        value.to_file_binary(&bin_path).unwrap();
        let info = parse_bundle_info(&bin_path).unwrap();
        fs::remove_file(&bin_path).unwrap();

        assert_eq!(info.bundle_id.as_deref(), Some("com.example.Editor"));
        assert_eq!(info.extensions, vec!["markdown", "md"]);
    }

    #[test]
    fn missing_or_invalid_plist_yields_empty_info() {
        let missing = parse_bundle_info(&temp_path("does-not-exist.plist")).unwrap();
        assert!(missing.bundle_id.is_none());
        assert!(missing.extensions.is_empty());

        let invalid = parse_fixture("invalid.plist", "not a plist at all");
        assert!(invalid.bundle_id.is_none());
        assert!(invalid.extensions.is_empty());
        assert!(invalid.content_types.is_empty());
    }
}
