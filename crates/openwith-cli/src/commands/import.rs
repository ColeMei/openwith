use anyhow::{Context, Result};
use std::collections::HashSet;

use openwith_core::history::{self, HistoryEvent};
use openwith_core::{config, scanner, uti};

pub fn run(path: &str, dry_run: bool) -> Result<()> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path))?;
    let cfg = config::from_toml(&content)?;

    let total = cfg.associations.len() + cfg.schemes.len();
    if total == 0 {
        println!("No associations found in '{}'.", path);
        return Ok(());
    }

    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    if dry_run {
        eprintln!("Previewing {} associations (dry run)...", total);
    } else {
        eprintln!("Applying {} associations...", total);
    }
    let result = config::import_associations(&cfg, &apps, dry_run);

    if !dry_run {
        let file_name = std::path::Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());
        let _ = history::record(HistoryEvent {
            kind: "import".into(),
            key: file_name,
            old: None,
            new: None,
            detail: Some(format!(
                "{} applied · {} skipped",
                result.applied.len(),
                result.unchanged.len() + result.skipped.len()
            )),
            timestamp: history::now_secs(),
            source: "cli".into(),
            ..Default::default()
        });
    }

    // Warn about extensions changed as a side effect of a shared UTI,
    // unless they are themselves part of the import.
    let all_extensions = scanner::all_extensions(&apps);
    let imported: HashSet<String> = cfg
        .associations
        .keys()
        .map(|k| k.trim_start_matches('.').to_lowercase())
        .collect();

    for (ext, app, previous) in &result.applied {
        let was = previous
            .as_ref()
            .map(|p| format!(" (was: {p})"))
            .unwrap_or_default();
        println!("  {} -> {}{}", ext, app, was);

        // Scheme entries (http://) have no UTI siblings.
        if !ext.starts_with('.') {
            continue;
        }

        if let Ok(uti_str) = uti::uti_for_extension(ext) {
            let siblings: Vec<String> = uti::extensions_sharing_uti(ext, &uti_str, &all_extensions)
                .into_iter()
                .filter(|s| !imported.contains(s))
                .collect();
            if !siblings.is_empty() {
                let shown: Vec<String> = siblings.iter().take(6).map(|s| format!(".{s}")).collect();
                let extra = siblings.len().saturating_sub(6);
                let more = if extra > 0 {
                    format!(" +{extra}")
                } else {
                    String::new()
                };
                println!("    also affects {}{}", shown.join(", "), more);
            }
        }
    }

    if !result.unchanged.is_empty() {
        println!("  {} already set correctly", result.unchanged.len());
    }

    if !result.skipped.is_empty() {
        eprintln!();
        for (ext, app, reason) in &result.skipped {
            eprintln!("  skipped {} -> {}: {}", ext, app, reason);
        }
    }

    println!(
        "\n{} {}, unchanged {}, skipped {}",
        if dry_run { "Would apply" } else { "Applied" },
        result.applied.len(),
        result.unchanged.len(),
        result.skipped.len()
    );

    Ok(())
}
