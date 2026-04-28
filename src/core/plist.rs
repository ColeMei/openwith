use anyhow::Result;
use std::collections::HashSet;
use std::process::Command;

#[derive(Debug, Default)]
pub struct DocumentTypes {
    pub extensions: Vec<String>,
    pub content_types: Vec<String>,
}

/// Parse an application's Info.plist document declarations.
pub fn parse_document_types(plist_path: &str) -> Result<DocumentTypes> {
    if !std::path::Path::new(plist_path).exists() {
        return Ok(DocumentTypes::default());
    }

    let output = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg("Print :CFBundleDocumentTypes")
        .arg(plist_path)
        .output();

    if let Ok(output) = output {
        let content = String::from_utf8_lossy(&output.stdout);
        return Ok(parse_document_types_output(&content));
    }

    Ok(DocumentTypes::default())
}

fn parse_document_types_output(content: &str) -> DocumentTypes {
    let mut extensions = HashSet::new();
    let mut content_types = HashSet::new();
    let mut collecting = None;

    for line in content.lines() {
        let line = line.trim();
        if line == "}" && collecting.is_some() {
            collecting = None;
            continue;
        }

        match collecting {
            Some(DocumentTypeArray::Extensions) => {
                insert_normalized(&mut extensions, line, true);
                continue;
            }
            Some(DocumentTypeArray::ContentTypes) => {
                insert_normalized(&mut content_types, line, false);
                continue;
            }
            None => {}
        }

        collecting = match line {
            "CFBundleTypeExtensions = Array {" => Some(DocumentTypeArray::Extensions),
            "LSItemContentTypes = Array {" => Some(DocumentTypeArray::ContentTypes),
            _ => None,
        };
    }

    DocumentTypes {
        extensions: sorted(extensions),
        content_types: sorted(content_types),
    }
}

#[derive(Clone, Copy)]
enum DocumentTypeArray {
    Extensions,
    ContentTypes,
}

fn insert_normalized(values: &mut HashSet<String>, value: &str, is_extension: bool) {
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

fn sorted(values: HashSet<String>) -> Vec<String> {
    let mut result: Vec<String> = values.into_iter().collect();
    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::parse_document_types_output;

    #[test]
    fn parses_legacy_bundle_type_extensions() {
        let content = r#"
Array {
    Dict {
        CFBundleTypeExtensions = Array {
            md
            markdown
        }
    }
}
"#;

        let document_types = parse_document_types_output(content);

        assert_eq!(document_types.extensions, vec!["markdown", "md"]);
        assert!(document_types.content_types.is_empty());
    }

    #[test]
    fn parses_ls_item_content_types_without_mapping_them() {
        let content = r#"
Array {
    Dict {
        LSItemContentTypes = Array {
            public.jpeg
            public.plain-text
        }
    }
}
"#;

        let document_types = parse_document_types_output(content);

        assert!(document_types.extensions.is_empty());
        assert_eq!(
            document_types.content_types,
            vec!["public.jpeg", "public.plain-text"]
        );
    }
}
