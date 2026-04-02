use anyhow::Result;

use crate::core::{duti, scanner, uti};

pub fn run(ext: &str, app_name: &str) -> Result<()> {
    let ext = ext.trim_start_matches('.');

    // Find the app
    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;
    let app = scanner::resolve_app(&apps, app_name)?;

    if app.bundle_id.is_empty() {
        anyhow::bail!("could not determine bundle ID for '{}'", app.name);
    }

    // Resolve UTI
    let uti = uti::uti_for_extension(ext)?;

    // Set default
    duti::set_default(&app.bundle_id, &uti)?;

    // Verify
    match duti::query_default(ext)? {
        Some(d) if d.bundle_id == app.bundle_id => {
            println!("Set .{} -> {}", ext, app.name);
        }
        _ => {
            println!("Set .{} -> {} (could not verify)", ext, app.name);
        }
    }

    Ok(())
}
