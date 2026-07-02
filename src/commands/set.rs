use anyhow::Result;

use crate::core::{launchservices, scanner, uti};

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
    launchservices::set_default(&app.bundle_id, &uti)?;

    // Verify
    match launchservices::query_default_bundle_id(ext)? {
        Some(bid) if bid.eq_ignore_ascii_case(&app.bundle_id) => {
            println!("Set .{} -> {}", ext, app.name);
        }
        _ => {
            println!("Set .{} -> {} (could not verify)", ext, app.name);
        }
    }

    let siblings = uti::extensions_sharing_uti(ext, &uti, &scanner::all_extensions(&apps));
    if let Some(note) = uti::shared_uti_note(ext, &uti, &siblings) {
        eprintln!("note: {}", note);
    }

    Ok(())
}
