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
/// Asks Launch Services first: apps can register their own UTIs, and the
/// system mapping is what Finder actually consults, so writing a handler to
/// any other UTI would silently have no effect. The hardcoded map is only a
/// fallback for extensions the system maps to a dynamic (`dyn.*`) type.
pub fn uti_for_extension(ext: &str) -> Result<String> {
    let ext = ext.trim_start_matches('.').to_lowercase();

    if let Some(cached) = extension_cache().lock().unwrap().get(&ext) {
        return cached.clone().ok_or_else(|| unrecognized_extension(&ext));
    }

    let resolved = system_uti(&ext).or_else(|| hardcoded_uti(&ext).map(str::to_string));
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
    if uti.is_empty() || uti.starts_with("dyn.") {
        None
    } else {
        Some(uti)
    }
}

fn hardcoded_uti(ext: &str) -> Option<&'static str> {
    let uti = match ext {
        // Text / markup
        "txt" => "public.plain-text",
        "rtf" => "public.rtf",
        "md" | "markdown" => "net.daringfireball.markdown",
        "log" => "public.log",
        "csv" => "public.comma-separated-values-text",
        "tsv" => "public.tab-separated-values-text",

        // Web
        "html" | "htm" => "public.html",
        "css" => "public.css",
        "js" => "com.netscape.javascript-source",
        "json" => "public.json",
        "xml" => "public.xml",
        "svg" => "public.svg-image",

        // Programming languages
        "rs" => "org.rust-lang.rust-source",
        "py" => "public.python-script",
        "rb" => "public.ruby-script",
        "go" => "org.golang.go-source",
        "java" => "com.sun.java-source",
        "c" => "public.c-source",
        "cpp" | "cc" | "cxx" => "public.c-plus-plus-source",
        "h" => "public.c-header",
        "hpp" => "public.c-plus-plus-header",
        "swift" => "public.swift-source",
        "m" => "public.objective-c-source",
        "ts" => "org.typescriptlang.typescript",
        "tsx" => "org.typescriptlang.typescriptx",
        "jsx" => "org.reactjs.jsx",
        "sh" | "bash" | "zsh" => "public.shell-script",
        "pl" => "public.perl-script",
        "php" => "public.php-script",
        "lua" => "org.lua.lua-source",
        "r" => "org.r-project.r-source",
        "sql" => "public.sql",

        // Config / data
        "yaml" | "yml" => "public.yaml",
        "toml" => "public.toml",
        "ini" | "cfg" => "public.ini",
        "plist" => "com.apple.property-list",
        "env" => "public.plain-text",

        // Documents
        "pdf" => "com.adobe.pdf",
        "doc" => "com.microsoft.word.doc",
        "docx" => "org.openxmlformats.wordprocessingml.document",
        "xls" => "com.microsoft.excel.xls",
        "xlsx" => "org.openxmlformats.spreadsheetml.sheet",
        "ppt" => "com.microsoft.powerpoint.ppt",
        "pptx" => "org.openxmlformats.presentationml.presentation",
        "pages" => "com.apple.iwork.pages.sffpages",
        "numbers" => "com.apple.iwork.numbers.sffnumbers",
        "keynote" => "com.apple.iwork.keynote.sffkey",

        // Images
        "jpg" | "jpeg" => "public.jpeg",
        "png" => "public.png",
        "gif" => "com.compuserve.gif",
        "bmp" => "com.microsoft.bmp",
        "tiff" | "tif" => "public.tiff",
        "webp" => "public.webp",
        "ico" => "com.microsoft.ico",
        "heic" | "heif" => "public.heic",
        "raw" => "public.camera-raw-image",
        "psd" => "com.adobe.photoshop-image",

        // Audio
        "mp3" => "public.mp3",
        "wav" => "com.microsoft.waveform-audio",
        "aac" => "public.aac-audio",
        "flac" => "org.xiph.flac",
        "ogg" => "org.xiph.ogg-vorbis",
        "m4a" => "com.apple.m4a-audio",
        "aiff" | "aif" => "public.aiff-audio",
        "wma" => "com.microsoft.windows-media-wma",

        // Video
        "mp4" => "public.mpeg-4",
        "m4v" => "com.apple.m4v-video",
        "mov" => "com.apple.quicktime-movie",
        "avi" => "public.avi",
        "mkv" => "org.matroska.mkv",
        "webm" => "org.webmproject.webm",
        "wmv" => "com.microsoft.windows-media-wmv",
        "flv" => "com.adobe.flash-video",

        // Archives
        "zip" => "public.zip-archive",
        "tar" => "public.tar-archive",
        "gz" | "gzip" => "org.gnu.gnu-zip-archive",
        "bz2" => "public.bzip2-archive",
        "xz" => "org.tukaani.xz-archive",
        "7z" => "org.7-zip.7-zip-archive",
        "rar" => "com.rarlab.rar-archive",
        "dmg" => "com.apple.disk-image-udif",
        "iso" => "public.iso-image",

        // Fonts
        "ttf" => "public.truetype-ttf-font",
        "otf" => "public.opentype-font",
        "woff" => "org.w3c.woff",
        "woff2" => "org.w3c.woff2",

        _ => return None,
    };
    Some(uti)
}

#[cfg(test)]
mod tests {
    use super::uti_for_extension;

    #[test]
    fn resolves_common_types() {
        assert_eq!(uti_for_extension("txt").unwrap(), "public.plain-text");
        assert_eq!(uti_for_extension(".PDF").unwrap(), "com.adobe.pdf");
    }

    #[test]
    fn rejects_unknown_extensions_instead_of_returning_dynamic_utis() {
        let err = uti_for_extension("openwithtotallyunknownext")
            .unwrap_err()
            .to_string();

        assert!(err.contains("not recognized by macOS"));
    }
}
