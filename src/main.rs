use anyhow::Result;
mod cli;
mod commands;
mod core;
mod logo;

fn main() -> Result<()> {
    let args = cli::Cli::parse_with_help();

    match args.command {
        Some(cli::Commands::List { .. }) => {
            commands::tui::run(commands::tui::InitialView::Extensions)?;
        }
        Some(cli::Commands::Current { ext }) => {
            commands::current::run(&ext)?;
        }
        Some(cli::Commands::Set { ext, app }) => {
            commands::set::run(&ext, &app)?;
        }
        Some(cli::Commands::Apps { .. }) => {
            commands::tui::run(commands::tui::InitialView::Apps)?;
        }
        Some(cli::Commands::Export { output }) => {
            commands::export::run(output.as_deref())?;
        }
        Some(cli::Commands::Import { path }) => {
            commands::import::run(&path)?;
        }
        None => {
            commands::tui::run(commands::tui::InitialView::Extensions)?;
        }
    }

    Ok(())
}
