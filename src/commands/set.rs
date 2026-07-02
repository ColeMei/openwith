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

    // Record the previous default so the change is easy to revert by hand,
    // and skip the write entirely when nothing would change.
    let previous = launchservices::query_default_bundle_id(ext)?;
    if previous
        .as_deref()
        .is_some_and(|p| p.eq_ignore_ascii_case(&bundle_id))
    {
        println!(".{} is already {} — nothing to do", ext, display_name);
        return Ok(());
    }

    // Set default
    launchservices::set_default(&bundle_id, &uti)?;

    let was = previous
        .map(|p| format!(" (was: {})", scanner::resolve_name(&apps, &p)))
        .unwrap_or_default();

    // Verify
    match launchservices::query_default_bundle_id(ext)? {
        Some(bid) if bid.eq_ignore_ascii_case(&bundle_id) => {
            println!("Set .{} -> {}{}", ext, display_name, was);
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
