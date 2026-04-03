use anyhow::Result;
use std::sync::Mutex;
use tabled::{Table, Tabled};

use crate::core::{launchservices, scanner};

#[derive(Tabled)]
struct Row {
    #[tabled(rename = "APP")]
    app: String,
    #[tabled(rename = "BUNDLE ID")]
    bundle_id: String,
    #[tabled(rename = "SUPPORTED EXTENSIONS")]
    supported: String,
    #[tabled(rename = "CURRENT DEFAULTS")]
    defaults: String,
}

pub fn run(filter: Option<&str>) -> Result<()> {
    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    // Filter to apps that have at least one extension
    let mut candidates: Vec<_> = apps
        .iter()
        .filter(|a| !a.extensions.is_empty() && !a.bundle_id.is_empty())
        .collect();

    if let Some(f) = filter {
        let f = f.to_lowercase();
        candidates.retain(|a| a.name.to_lowercase().contains(&f));
    }

    if candidates.is_empty() {
        println!("No apps found.");
        return Ok(());
    }

    candidates.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    eprintln!("Querying defaults...");

    // Collect all unique extensions we need to query
    let mut all_exts: Vec<String> = candidates
        .iter()
        .flat_map(|a| a.extensions.iter().map(|e| e.to_lowercase()))
        .collect();
    all_exts.sort();
    all_exts.dedup();

    // Query all defaults in parallel, build a map of ext -> bundle_id
    let defaults: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for chunk in all_exts.chunks(20) {
            let defaults = &defaults;
            let chunk = chunk.to_vec();
            s.spawn(move || {
                for ext in chunk {
                    if let Some(bid) = launchservices::query_default_bundle_id(&ext).ok().flatten()
                    {
                        defaults.lock().unwrap().push((ext, bid));
                    }
                }
            });
        }
    });

    let defaults = defaults.into_inner().unwrap();
    let defaults_map: std::collections::HashMap<&str, &str> = defaults
        .iter()
        .map(|(ext, bid)| (ext.as_str(), bid.as_str()))
        .collect();

    // Build rows
    let rows: Vec<Row> = candidates
        .iter()
        .map(|app| {
            let mut supported: Vec<String> =
                app.extensions.iter().map(|e| e.to_lowercase()).collect();
            supported.sort();
            supported.dedup();

            let mut current_defaults: Vec<&str> = supported
                .iter()
                .filter(|ext| {
                    defaults_map
                        .get(ext.as_str())
                        .map(|bid| bid.eq_ignore_ascii_case(&app.bundle_id))
                        .unwrap_or(false)
                })
                .map(|e| e.as_str())
                .collect();
            current_defaults.sort();

            let supported_str = truncate_list(&supported, 40);
            let defaults_str = if current_defaults.is_empty() {
                "-".to_string()
            } else {
                truncate_list(&current_defaults, 40)
            };

            Row {
                app: app.name.clone(),
                bundle_id: app.bundle_id.clone(),
                supported: supported_str,
                defaults: defaults_str,
            }
        })
        .collect();

    let table = Table::new(&rows).to_string();
    println!("{}", table);
    println!("\n{} apps", rows.len());

    Ok(())
}

fn truncate_list<S: AsRef<str>>(items: &[S], max_len: usize) -> String {
    let mut result = String::new();
    let mut remaining = items.len();

    for item in items {
        let item = item.as_ref();
        if result.is_empty() {
            result = item.to_string();
        } else {
            let next = format!("{}, {}", result, item);
            if next.len() > max_len {
                result = format!("{}, ... +{} more", result, remaining);
                break;
            }
            result = next;
        }
        remaining -= 1;
    }

    result
}
