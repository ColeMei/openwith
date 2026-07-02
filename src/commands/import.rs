use anyhow::{Context, Result};
use std::collections::HashSet;

use crate::core::{config, scanner, uti};

pub fn run(path: &str) -> Result<()> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path))?;
    let cfg = config::from_toml(&content)?;

    if cfg.associations.is_empty() {
        println!("No associations found in '{}'.", path);
        return Ok(());
    }

    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    eprintln!("Applying {} associations...", cfg.associations.len());
    let result = config::import_associations(&cfg, &apps);

    // Warn about extensions changed as a side effect of a shared UTI,
    // unless they are themselves part of the import.
    let all_extensions = scanner::all_extensions(&apps);
    let imported: HashSet<String> = cfg
        .associations
        .keys()
        .map(|k| k.trim_start_matches('.').to_lowercase())
        .collect();

    for (ext, app) in &result.applied {
        println!("  {} -> {}", ext, app);

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

    if !result.skipped.is_empty() {
        eprintln!();
        for (ext, app, reason) in &result.skipped {
            eprintln!("  skipped {} -> {}: {}", ext, app, reason);
        }
    }

    println!(
        "\nApplied {}, skipped {}",
        result.applied.len(),
        result.skipped.len()
    );

    Ok(())
}
