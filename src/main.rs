use anyhow::Result;
use clap::Parser;

mod cli;
mod commands;
mod core;

fn main() -> Result<()> {
    let args = cli::Cli::parse();

    // Ensure duti is available (auto-installs via brew if missing)
    core::duti::ensure_available()?;

    match args.command {
        Some(cli::Commands::List { filter }) => {
            commands::list::run(filter.as_deref())?;
        }
        Some(cli::Commands::Current { ext }) => {
            commands::current::run(&ext)?;
        }
        Some(cli::Commands::Set { ext, app }) => {
            commands::set::run(&ext, &app)?;
        }
        None => {
            commands::tui::run()?;
        }
    }

    Ok(())
}
