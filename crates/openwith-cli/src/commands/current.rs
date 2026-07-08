use anyhow::Result;

use openwith_core::{launchservices, scanner};

pub fn run(ext: &str, json: bool, scheme: bool) -> Result<()> {
    if scheme {
        return run_scheme(ext, json);
    }

    let ext = ext.trim_start_matches('.');

    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    let bundle_id = launchservices::query_default_bundle_id(ext)?;
    let name = bundle_id
        .as_ref()
        .map(|bid| scanner::resolve_name(&apps, bid));

    let supporting: Vec<&str> = apps
        .iter()
        .filter(|app| scanner::app_supports_extension(app, ext))
        .map(|app| app.name.as_str())
        .collect();

    if json {
        let out = serde_json::json!({
            "ext": ext,
            "app": name,
            "bundle_id": bundle_id,
            "supporting_apps": supporting,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    match (&name, &bundle_id) {
        (Some(name), Some(bundle_id)) => println!(".{} -> {} ({})", ext, name, bundle_id),
        _ => println!(".{} -> (no default set)", ext),
    }

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

fn run_scheme(scheme: &str, json: bool) -> Result<()> {
    let scheme = normalize_scheme(scheme);

    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    let bundle_id = launchservices::query_default_scheme_handler(&scheme)?;
    let name = bundle_id
        .as_ref()
        .map(|bid| scanner::resolve_name(&apps, bid));

    let supporting: Vec<&str> = apps
        .iter()
        .filter(|app| {
            app.url_schemes
                .iter()
                .any(|s| s.eq_ignore_ascii_case(&scheme))
        })
        .map(|app| app.name.as_str())
        .collect();

    if json {
        let out = serde_json::json!({
            "scheme": scheme,
            "app": name,
            "bundle_id": bundle_id,
            "supporting_apps": supporting,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    match (&name, &bundle_id) {
        (Some(name), Some(bundle_id)) => {
            println!("{}:// -> {} ({})", scheme, name, bundle_id)
        }
        _ => println!("{}:// -> (no default set)", scheme),
    }

    if supporting.is_empty() {
        println!("No apps found that declare support for {}://", scheme);
    } else {
        println!("\nApps supporting {}://:", scheme);
        for app in &supporting {
            println!("  {}", app);
        }
    }

    Ok(())
}

/// Accept "http", "http:", or "http://" and normalize to the bare scheme.
pub fn normalize_scheme(scheme: &str) -> String {
    scheme
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(':')
        .to_lowercase()
}
