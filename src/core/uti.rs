use anyhow::Result;
use std::process::Command;

/// Resolve the UTI for a file extension.
/// Uses a hardcoded map for common types, falls back to mdls for unknown ones.
pub fn uti_for_extension(ext: &str) -> Result<String> {
    let ext = ext.trim_start_matches('.').to_lowercase();

    if let Some(uti) = hardcoded_uti(&ext) {
        return Ok(uti.to_string());
    }

    // Single mdls attempt for unknown extensions
    mdls_uti(&ext)
}

fn mdls_uti(ext: &str) -> Result<String> {
    let temp_file = std::env::temp_dir().join(format!("dutis_uti_probe.{}", ext));
    std::fs::write(&temp_file, "probe")?;

    // Brief pause for Spotlight to index
    std::thread::sleep(std::time::Duration::from_millis(500));

    let output = Command::new("mdls")
        .arg("-name")
        .arg("kMDItemContentType")
        .arg("-r")
        .arg(&temp_file)
        .output();

    let _ = std::fs::remove_file(&temp_file);

    match output {
        Ok(o) if o.status.success() => {
            let uti = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !uti.is_empty() && uti != "(null)" {
                return Ok(uti);
            }
            anyhow::bail!("could not determine UTI for .{}", ext);
        }
        _ => anyhow::bail!("mdls failed for .{}", ext),
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
        "mp4" | "m4v" => "public.mpeg-4",
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
