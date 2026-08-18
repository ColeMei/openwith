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
    openwith history            Show recent changes, last 7 days (--all, --json)
    openwith undo               Revert the most recent change

CONFIG
    openwith export         Export current associations to TOML
    openwith import <path>  Import associations (--dry-run to preview)
    openwith completions <shell>    Generate shell completions

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
    disable_version_flag = true
)]
pub struct Cli {
    /// Print version
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    pub version: (),

    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    pub fn parse_with_help() -> Self {
        use clap::CommandFactory;
        let mut cmd = Self::command();
        // Only the root gets the hand-written logo template; subcommands keep
        // clap's generated help, which lists their actual flags. `-h`/`--help`
        // stay enabled everywhere — disabling them at the root propagated to
        // every subcommand, leaving `openwith set --help` an "unexpected
        // argument" error.
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
    /// Show recent default changes recorded by the CLI and GUI
    History {
        /// Maximum number of events to show
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Only show events from the last N days
        #[arg(short = 'd', long, default_value_t = openwith_core::history::DEFAULT_WINDOW_DAYS)]
        days: u64,
        /// Show every retained event, ignoring --days
        #[arg(long, conflicts_with = "days")]
        all: bool,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Revert the most recent default change
    Undo {
        /// Revert even if the default has changed since the recorded event
        #[arg(long)]
        force: bool,
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
        /// Preview the changes without applying them
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Generate a man page (roff) on stdout
    #[command(hide = true)]
    Mangen,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    /// `disable_help_flag` on the root propagates to subcommands, which once
    /// left every `openwith <sub> --help` failing as an unexpected argument
    /// while the root's own help still worked — so the breakage was invisible
    /// unless a subcommand was tried directly.
    #[test]
    fn every_subcommand_accepts_help() {
        let mut cmd = Cli::command();
        cmd.build();
        for sub in cmd.get_subcommands() {
            // clap's own `openwith help <sub>` command takes no flags itself.
            if sub.get_name() == "help" {
                continue;
            }
            assert!(
                sub.get_arguments().any(|a| a.get_id() == "help"),
                "`openwith {}` has no --help flag",
                sub.get_name()
            );
        }
    }
}
