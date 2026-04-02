use anyhow::Result;
use std::sync::Mutex;
use tabled::{Table, Tabled};

use crate::core::{duti, scanner};

#[derive(Tabled)]
struct Row {
    #[tabled(rename = "EXT")]
    ext: String,
    #[tabled(rename = "DEFAULT APP")]
    app: String,
    #[tabled(rename = "BUNDLE ID")]
    bundle_id: String,
}

pub fn run(filter: Option<&str>) -> Result<()> {
    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    // Collect all unique extensions (normalized to lowercase)
    let mut extensions: Vec<String> = apps
        .iter()
        .flat_map(|app| app.extensions.iter().map(|e| e.to_lowercase()))
        .collect();
    extensions.sort();
    extensions.dedup();

    // Query defaults in parallel
    let rows: Mutex<Vec<Row>> = Mutex::new(Vec::new());

    std::thread::scope(|s| {
        let chunk_size = 20;
        for chunk in extensions.chunks(chunk_size) {
            let rows = &rows;
            let chunk = chunk.to_vec();
            s.spawn(move || {
                for ext in chunk {
                    let default = duti::query_default(&ext).ok().flatten();
                    let (app_name, bundle_id) = match &default {
                        Some(d) => (d.name.clone(), d.bundle_id.clone()),
                        None => ("-".to_string(), "-".to_string()),
                    };
                    rows.lock().unwrap().push(Row {
                        ext,
                        app: app_name,
                        bundle_id,
                    });
                }
            });
        }
    });

    let mut rows = rows.into_inner().unwrap();
    rows.sort_by(|a, b| a.ext.cmp(&b.ext));

    // Apply filter
    if let Some(f) = filter {
        let f = f.to_lowercase();
        rows.retain(|r| r.ext.to_lowercase().contains(&f) || r.app.to_lowercase().contains(&f));
    }

    if rows.is_empty() {
        println!("No extensions found.");
        return Ok(());
    }

    let table = Table::new(&rows).to_string();
    println!("{}", table);
    println!("\n{} extensions", rows.len());

    Ok(())
}
