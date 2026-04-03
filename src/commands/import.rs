use anyhow::{Context, Result};

use crate::core::{config, scanner};

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

    for (ext, app) in &result.applied {
        println!("  {} -> {}", ext, app);
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
