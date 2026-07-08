use anyhow::Result;
use std::io::IsTerminal;

use openwith_core::{listing, scanner};

use super::tui;

/// `openwith list`: interactive TUI on a terminal, plain or JSON output for
/// scripts. `--plain` / `--json` force the non-interactive formats; a piped
/// stdout falls back to the plain table automatically.
pub fn run(plain: bool, json: bool) -> Result<()> {
    if json {
        print_listing(true)
    } else if plain || !std::io::stdout().is_terminal() {
        print_listing(false)
    } else {
        tui::run(tui::InitialView::Extensions)
    }
}

fn print_listing(json: bool) -> Result<()> {
    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    eprintln!("Querying defaults...");
    let rows = listing::query_all(&apps, &|| {});

    if json {
        let out: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "ext": r.ext,
                    "app": r.app_name,
                    "bundle_id": r.bundle_id,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    let ext_width = rows.iter().map(|r| r.ext.len()).max().unwrap_or(3);
    let app_width = rows
        .iter()
        .map(|r| r.app_name.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(1);
    for r in &rows {
        println!(
            "{:<ext_width$}  {:<app_width$}  {}",
            r.ext,
            r.app_name.as_deref().unwrap_or("-"),
            r.bundle_id.as_deref().unwrap_or("-"),
        );
    }
    Ok(())
}
