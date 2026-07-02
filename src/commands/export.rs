use anyhow::Result;

use crate::core::{config, scanner};

pub fn run(output: Option<&str>) -> Result<()> {
    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    eprintln!("Querying defaults...");
    let (cfg, display_names) = config::export_associations(&apps)?;
    let toml_str = config::to_toml(&cfg, &display_names)?;

    match output {
        Some(path) => {
            std::fs::write(path, &toml_str)?;
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
