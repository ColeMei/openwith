use anyhow::Result;
mod cli;
mod commands;
mod core;
mod logo;

fn main() -> Result<()> {
    let args = cli::Cli::parse_with_help();

    match args.command {
        Some(cli::Commands::List { plain, json }) => {
            commands::list::run(plain, json)?;
        }
        Some(cli::Commands::Current { ext, json }) => {
            commands::current::run(&ext, json)?;
        }
        Some(cli::Commands::Set { ext, app }) => {
            commands::set::run(&ext, &app)?;
        }
        Some(cli::Commands::Apps) => {
            commands::tui::run(commands::tui::InitialView::Apps)?;
        }
        Some(cli::Commands::Export { output }) => {
            commands::export::run(output.as_deref())?;
        }
        Some(cli::Commands::Import { path, dry_run }) => {
            commands::import::run(&path, dry_run)?;
        }
        None => {
            commands::list::run(false, false)?;
        }
    }

    Ok(())
}
