use anyhow::{Result, anyhow};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    static kUTTagClassFilenameExtension: CFStringRef;

    fn UTTypeCreatePreferredIdentifierForTag(
        in_tag_class: CFStringRef,
        in_tag: CFStringRef,
        in_conforming_to_uti: CFStringRef,
    ) -> CFStringRef;

    fn UTTypeConformsTo(in_uti: CFStringRef, in_conforms_to_uti: CFStringRef) -> bool;
}

/// Resolve the UTI for a file extension.
///
/// Whatever Launch Services answers is the answer, including a dynamic
/// (`dyn.*`) type. The system mapping is what Finder actually consults, so
/// writing a handler to any other UTI would silently have no effect — and a
/// `dyn.*` type is a perfectly settable target, not a failure (issue #16).
/// Dynamic identifiers encode the extension itself, so they are stable for a
/// given extension across machines.
pub fn uti_for_extension(ext: &str) -> Result<String> {
    let ext = ext.trim_start_matches('.').to_lowercase();

    // Launch Services invents a dynamic UTI for literally any tag, including
    // the empty string, so this is the one case we have to reject ourselves.
    if ext.is_empty() {
        return Err(anyhow!("no file extension given"));
    }

    if let Some(cached) = extension_cache().lock().unwrap().get(&ext) {
        return cached.clone().ok_or_else(|| unrecognized_extension(&ext));
    }

    let resolved = system_uti(&ext);
    extension_cache()
        .lock()
        .unwrap()
        .insert(ext.clone(), resolved.clone());

    resolved.ok_or_else(|| unrecognized_extension(&ext))
}

pub fn conforms_to(uti: &str, parent_uti: &str) -> bool {
    if uti.eq_ignore_ascii_case(parent_uti) {
        return true;
    }

    let key = (uti.to_string(), parent_uti.to_string());
    if let Some(&cached) = conformance_cache().lock().unwrap().get(&key) {
        return cached;
    }

    let uti_cf = CFString::new(uti);
    let parent_cf = CFString::new(parent_uti);
    let result = unsafe {
        UTTypeConformsTo(
            uti_cf.as_concrete_TypeRef(),
            parent_cf.as_concrete_TypeRef(),
        )
    };

    conformance_cache().lock().unwrap().insert(key, result);
    result
}

/// Extensions other than `ext` that resolve to the same UTI.
///
/// Launch Services associates default handlers with UTIs, not extensions, so
/// changing the default for `ext` also changes it for every sibling returned
/// here. Candidates are the hardcoded map plus `extra_candidates` (typically
/// every extension declared by an installed app).
pub fn extensions_sharing_uti(ext: &str, uti: &str, extra_candidates: &[String]) -> Vec<String> {
    let ext = ext.trim_start_matches('.').to_lowercase();
    let mut siblings = std::collections::BTreeSet::new();

    let known = COMMON_EXTENSIONS.iter().map(|e| (*e).to_string());
    let extra = extra_candidates
        .iter()
        .map(|c| c.trim_start_matches('.').to_lowercase());

    for candidate in known.chain(extra) {
        if candidate == ext {
            continue;
        }
        if let Ok(candidate_uti) = uti_for_extension(&candidate)
            && candidate_uti.eq_ignore_ascii_case(uti)
        {
            siblings.insert(candidate);
        }
    }

    siblings.into_iter().collect()
}

/// Human-readable warning for a change that affects sibling extensions.
/// Returns `None` when no other extensions share the UTI.
pub fn shared_uti_note(ext: &str, uti: &str, siblings: &[String]) -> Option<String> {
    if siblings.is_empty() {
        return None;
    }

    const MAX_SHOWN: usize = 6;
    let mut shown: Vec<String> = siblings
        .iter()
        .take(MAX_SHOWN)
        .map(|s| format!(".{s}"))
        .collect();
    if siblings.len() > MAX_SHOWN {
        shown.push(format!("and {} more", siblings.len() - MAX_SHOWN));
    }

    Some(format!(
        ".{} shares its type ({}) with {}; this change affects them too",
        ext.trim_start_matches('.'),
        uti,
        shown.join(", ")
    ))
}

/// Cache for extension -> UTI lookups; `None` records unresolvable extensions.
fn extension_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn conformance_cache() -> &'static Mutex<HashMap<(String, String), bool>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, String), bool>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn unrecognized_extension(ext: &str) -> anyhow::Error {
    anyhow!("extension .{} is not recognized by macOS", ext)
}

fn system_uti(ext: &str) -> Option<String> {
    let extension = CFString::new(ext);
    let uti_ref = unsafe {
        UTTypeCreatePreferredIdentifierForTag(
            kUTTagClassFilenameExtension,
            extension.as_concrete_TypeRef(),
            std::ptr::null(),
        )
    };

    if uti_ref.is_null() {
        return None;
    }

    let uti = unsafe { CFString::wrap_under_create_rule(uti_ref) }.to_string();
    if uti.is_empty() { None } else { Some(uti) }
}

/// Candidate pool for sibling detection: extensions common enough to be worth
/// probing even when no installed app declares them. These are only candidates
/// — every UTI is still resolved through Launch Services.
const COMMON_EXTENSIONS: &[&str] = &[
    // Text / markup
    "txt", "rtf", "md", "markdown", "log", "csv", "tsv", // Web
    "html", "htm", "css", "js", "json", "xml", "svg", // Programming languages
    "rs", "py", "rb", "go", "java", "c", "cpp", "cc", "cxx", "h", "hpp", "swift", "m", "ts", "tsx",
    "jsx", "sh", "bash", "zsh", "pl", "php", "lua", "r", "sql", // Config / data
    "yaml", "yml", "toml", "ini", "cfg", "plist", "env", // Documents
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "pages", "numbers", "keynote",
    // Images
    "jpg", "jpeg", "png", "gif", "bmp", "tiff", "tif", "webp", "ico", "heic", "heif", "raw", "psd",
    // Audio
    "mp3", "wav", "aac", "flac", "ogg", "m4a", "aiff", "aif", "wma", // Video
    "mp4", "m4v", "mov", "avi", "mkv", "webm", "wmv", "flv", // Archives
    "zip", "tar", "gz", "gzip", "bz2", "xz", "7z", "rar", "dmg", "iso", // Fonts
    "ttf", "otf", "woff", "woff2",
];

#[cfg(test)]
mod tests {
    use super::{extensions_sharing_uti, shared_uti_note, uti_for_extension};

    #[test]
    fn resolves_common_types() {
        assert_eq!(uti_for_extension("txt").unwrap(), "public.plain-text");
        assert_eq!(uti_for_extension(".PDF").unwrap(), "com.adobe.pdf");
    }

    /// Issue #16: an extension no installed app claims resolves to a dynamic
    /// UTI, which Launch Services accepts as a handler target. Refusing it
    /// blocked `set` on extensions Finder changes without complaint.
    #[test]
    fn resolves_unclaimed_extensions_to_dynamic_utis() {
        let uti = uti_for_extension("openwithtotallyunknownext").unwrap();

        assert!(uti.starts_with("dyn."), "expected a dynamic UTI, got {uti}");
    }

    /// Launch Services mints a dynamic UTI for any tag at all, including the
    /// empty one, so an empty extension is the single case we reject.
    #[test]
    fn rejects_an_empty_extension() {
        assert!(uti_for_extension("").is_err());
        assert!(uti_for_extension(".").is_err());
    }

    /// The old hardcoded table aimed `set` at a UTI the system did not map the
    /// extension to, so the write landed on a type nothing consulted. Every
    /// answer must now come from Launch Services itself.
    #[test]
    fn resolution_matches_what_launch_services_reports() {
        for ext in ["jsx", "tsx", "rar", "env", "keynote", "erb"] {
            let resolved = uti_for_extension(ext).unwrap();
            let system = super::system_uti(ext).unwrap();

            assert_eq!(resolved, system, "{ext} resolved off-system");
        }
    }

    #[test]
    fn finds_extensions_sharing_a_uti() {
        let siblings = extensions_sharing_uti("env", "public.plain-text", &[]);

        assert!(siblings.contains(&"txt".to_string()));
        assert!(!siblings.contains(&"env".to_string()));
    }

    #[test]
    fn shared_uti_note_lists_siblings_and_truncates() {
        assert_eq!(shared_uti_note("env", "public.plain-text", &[]), None);

        let note = shared_uti_note("env", "public.plain-text", &["txt".to_string()]).unwrap();
        assert!(note.contains(".env"));
        assert!(note.contains(".txt"));
        assert!(note.contains("public.plain-text"));

        let many: Vec<String> = (0..9).map(|i| format!("ext{i}")).collect();
        let note = shared_uti_note("env", "public.plain-text", &many).unwrap();
        assert!(note.contains("and 3 more"));
    }
}
