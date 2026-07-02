use anyhow::Result;

use crate::core::{launchservices, scanner, uti};

pub fn run(ext: &str, app_name: &str) -> Result<()> {
    let ext = ext.trim_start_matches('.');

    // Find the app (by name or bundle ID)
    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;
    let (bundle_id, display_name) = scanner::resolve_app_or_bundle_id(&apps, app_name)?;

    // Resolve UTI
    let uti = uti::uti_for_extension(ext)?;

    // Set default
    launchservices::set_default(&bundle_id, &uti)?;

    // Verify
    match launchservices::query_default_bundle_id(ext)? {
        Some(bid) if bid.eq_ignore_ascii_case(&bundle_id) => {
            println!("Set .{} -> {}", ext, display_name);
        }
        _ => {
            println!("Set .{} -> {} (could not verify)", ext, display_name);
        }
    }

    let siblings = uti::extensions_sharing_uti(ext, &uti, &scanner::all_extensions(&apps));
    if let Some(note) = uti::shared_uti_note(ext, &uti, &siblings) {
        eprintln!("note: {}", note);
    }

    Ok(())
}
