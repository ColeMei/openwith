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
    openwith list           Launch interactive TUI (extensions view)
    openwith apps           Launch interactive TUI (apps view)

MANAGE
    openwith current <ext>      Show the default app for an extension
    openwith set <ext> <app>    Set the default app for an extension

CONFIG
    openwith export         Export current associations to TOML
    openwith import <path>  Import associations from a TOML file

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
    disable_help_flag = true,
    subcommand_negates_reqs = true
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
    /// Launch interactive TUI (extensions view)
    List {},
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
    /// Launch interactive TUI (apps view)
    Apps {},
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
