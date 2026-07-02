use clap::{FromArgMatches, Parser, Subcommand};

use crate::logo::LOGO;

fn help_template() -> String {
    format!(
        "\
{LOGO}
      https://github.com/ColeMei/openwith
      Manage macOS file extension associations.

{{usage-heading}} {{usage}}

BROWSE
    openwith                Launch interactive TUI (extensions view)
    openwith list           Launch interactive TUI; --plain/--json for scripts
    openwith apps           Launch interactive TUI (apps view)

MANAGE
    openwith current <ext>      Show the default app (--json for scripts)
    openwith set <ext> <app>    Set the default app (name or bundle ID)
    openwith current -s http    Show the handler for a URL scheme
    openwith set -s http <app>  Set the handler for a URL scheme

CONFIG
    openwith export         Export current associations to TOML
    openwith import <path>  Import associations (--dry-run to preview)

FLAGS
    -h, --help              Show help
    -v, --version           Show version
"
    )
}

#[derive(Parser)]
#[command(
    name = "openwith",
    about = "Manage macOS file extension associations",
    version,
    disable_version_flag = true,
    disable_help_flag = true
)]
pub struct Cli {
    /// Print version
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    pub version: (),

    /// Print help
    #[arg(short = 'h', long = "help", action = clap::ArgAction::HelpLong)]
    pub help: (),

    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    pub fn parse_with_help() -> Self {
        use clap::CommandFactory;
        let mut cmd = Self::command();
        cmd = cmd.help_template(help_template());
        let matches = cmd.get_matches();
        Self::from_arg_matches(&matches).expect("failed to parse CLI arguments")
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Launch interactive TUI (extensions view); plain/JSON when scripted
    List {
        /// Print a plain table instead of launching the TUI
        #[arg(long, conflicts_with = "json")]
        plain: bool,
        /// Print JSON instead of launching the TUI
        #[arg(long)]
        json: bool,
    },
    /// Show the current default app for an extension or URL scheme
    Current {
        /// File extension (without dot), or URL scheme with --scheme
        ext: String,
        /// Print JSON
        #[arg(long)]
        json: bool,
        /// Treat the argument as a URL scheme (e.g. http, mailto)
        #[arg(short = 's', long)]
        scheme: bool,
    },
    /// Set the default app for a file extension or URL scheme
    Set {
        /// File extension (without dot), or URL scheme with --scheme
        ext: String,
        /// Application name or bundle ID (e.g., "Preview", "com.apple.Preview")
        app: String,
        /// Treat the argument as a URL scheme (e.g. http, mailto)
        #[arg(short = 's', long)]
        scheme: bool,
    },
    /// Launch interactive TUI (apps view)
    Apps,
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
        /// Preview the changes without applying them
        #[arg(long)]
        dry_run: bool,
    },
}
