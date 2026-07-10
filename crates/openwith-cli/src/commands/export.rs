use anyhow::Result;

use openwith_core::history::{self, HistoryEvent};
use openwith_core::{config, scanner};

pub fn run(output: Option<&str>) -> Result<()> {
    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    eprintln!("Querying defaults...");
    let (cfg, display_names) = config::export_associations(&apps)?;
    let toml_str = config::to_toml(&cfg, &display_names)?;

    match output {
        Some(path) => {
            std::fs::write(path, &toml_str)?;
            let file_name = std::path::Path::new(path)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string());
            let _ = history::record(HistoryEvent {
                kind: "export".into(),
                key: file_name,
                old: None,
                new: None,
                detail: Some(format!(
                    "{} extensions · {} schemes",
                    cfg.associations.len(),
                    cfg.schemes.len()
                )),
                timestamp: history::now_secs(),
                source: "cli".into(),
            });
            println!(
                "Exported {} associations and {} scheme handlers to {}",
                cfg.associations.len(),
                cfg.schemes.len(),
                path
            );
        }
        None => {
            print!("{}", toml_str);
        }
    }

    Ok(())
}
