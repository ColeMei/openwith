use anyhow::Result;
use clap::Parser;

mod cli;
mod commands;
mod core;

fn main() -> Result<()> {
    let args = cli::Cli::parse();

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
        Some(cli::Commands::Apps { filter }) => {
            commands::apps::run(filter.as_deref())?;
        }
        Some(cli::Commands::Export { output }) => {
            commands::export::run(output.as_deref())?;
        }
        Some(cli::Commands::Import { path }) => {
            commands::import::run(&path)?;
        }
        None => {
            commands::tui::run()?;
        }
    }

    Ok(())
}
