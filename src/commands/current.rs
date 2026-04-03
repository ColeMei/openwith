use anyhow::Result;

use crate::core::{launchservices, scanner};

pub fn run(ext: &str) -> Result<()> {
    let ext = ext.trim_start_matches('.');

    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    // Query current default
    match launchservices::query_default_bundle_id(ext)? {
        Some(bundle_id) => {
            let name = scanner::resolve_name(&apps, &bundle_id);
            println!(".{} -> {} ({})", ext, name, bundle_id);
        }
        None => {
            println!(".{} -> (no default set)", ext);
        }
    }
    let supporting: Vec<&str> = apps
        .iter()
        .filter(|app| app.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
        .map(|app| app.name.as_str())
        .collect();

    if supporting.is_empty() {
        println!("No apps found that declare support for .{}", ext);
    } else {
        println!("\nApps supporting .{}:", ext);
        for app in &supporting {
            println!("  {}", app);
        }
    }

    Ok(())
}
