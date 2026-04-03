use clap::{Parser, Subcommand};

const AFTER_HELP: &str = "\
Examples:
  openwith                          Launch interactive TUI
  openwith list -f py               List extensions, filter by 'py'
  openwith current pdf              Show default app for .pdf
  openwith set pdf Preview          Set default for .pdf to Preview
  openwith apps -f safari           List apps, filter by 'safari'
  openwith export -o openwith.toml  Export associations to TOML
  openwith import openwith.toml     Import associations from TOML";

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
    /// List apps and their supported file extensions
    Apps {
        /// Filter by app name
        #[arg(short, long)]
        filter: Option<String>,
    },
    /// Export current file associations to TOML
    Export {
        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import file associations from a TOML config
    Import {
        /// Path to TOML config file
        path: String,
    },
}
