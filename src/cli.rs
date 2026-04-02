use clap::{Parser, Subcommand};

const AFTER_HELP: &str = "\
Examples:
  openwith
  openwith list -f py
  openwith current pdf
  openwith set pdf Preview
  openwith set html \"Google Chrome\"";

#[derive(Parser)]
#[command(
    name = "openwith",
    about = "Manage macOS file extension associations",
    version,
    after_help = AFTER_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all extensions with their current default apps
    List {
        /// Filter by extension or app name
        #[arg(short, long)]
        filter: Option<String>,
    },
    /// Show the current default app for a specific extension
    Current {
        /// File extension (without dot)
        ext: String,
    },
    /// Set the default app for a file extension
    Set {
        /// File extension (without dot)
        ext: String,
        /// Application name (e.g., "Preview", "Visual Studio Code")
        app: String,
    },
}
