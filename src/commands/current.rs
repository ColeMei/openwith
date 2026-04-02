use anyhow::Result;

use crate::core::{duti, scanner};

pub fn run(ext: &str) -> Result<()> {
    let ext = ext.trim_start_matches('.');

    // Query current default
    match duti::query_default(ext)? {
        Some(default) => {
            println!(".{} -> {} ({})", ext, default.name, default.bundle_id);
        }
        None => {
            println!(".{} -> (no default set)", ext);
        }
    }

    // Show supporting apps
    eprintln!("\nScanning for supporting apps...");
    let apps = scanner::scan_all_apps()?;
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
