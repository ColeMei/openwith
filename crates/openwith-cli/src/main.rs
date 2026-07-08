use anyhow::Result;
mod cli;
mod commands;
mod logo;

fn main() -> Result<()> {
    let args = cli::Cli::parse_with_help();

    match args.command {
        Some(cli::Commands::List { plain, json }) => {
            commands::list::run(plain, json)?;
        }
        Some(cli::Commands::Current { ext, json, scheme }) => {
            commands::current::run(&ext, json, scheme)?;
        }
        Some(cli::Commands::Set { ext, app, scheme }) => {
            commands::set::run(&ext, &app, scheme)?;
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
        Some(cli::Commands::Completions { shell }) => {
            use clap::CommandFactory;
            let mut cmd = cli::Cli::command();
            clap_complete::generate(shell, &mut cmd, "openwith", &mut std::io::stdout());
        }
        Some(cli::Commands::Mangen) => {
            use clap::CommandFactory;
            clap_mangen::Man::new(cli::Cli::command()).render(&mut std::io::stdout())?;
        }
        None => {
            commands::list::run(false, false)?;
        }
    }

    Ok(())
}
