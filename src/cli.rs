use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "openwith",
    about = "Manage macOS file extension associations",
    disable_help_flag = true,
    disable_version_flag = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Show help
    #[arg(short, long)]
    pub help: bool,

    /// Show version
    #[arg(short, long)]
    pub version: bool,
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

const BANNER: &str = r#"
                                _ _   _
  ___  _ __   ___ _ ____      _(_) |_| |__
 / _ \| '_ \ / _ \ '_ \ \ /\ / / | __| '_ \
| (_) | |_) |  __/ | | \ V  V /| | |_| | | |
 \___/| .__/ \___|_| |_|\_/\_/ |_|\__|_| |_|
      |_|
"#;

pub fn print_help() {
    let d = "\x1b[2m";
    let c = "\x1b[36m";
    let r = "\x1b[0m";
    let b = "\x1b[1m";

    print!("{c}{BANNER}{r}");
    println!("  Manage macOS file extension associations.");
    println!("  {d}https://github.com/ColeMei/openwith{r}");
    println!();
    println!("{b}COMMANDS{r}");
    println!("  openwith                    {d}Interactive TUI{r}");
    println!("  openwith list               {d}List all extensions with defaults{r}");
    println!("  openwith current {d}<ext>{r}       {d}Show current default for extension{r}");
    println!("  openwith set {d}<ext> <app>{r}     {d}Set default app for extension{r}");
    println!("  openwith --help             {d}Show this help{r}");
    println!("  openwith --version          {d}Show version{r}");
    println!();
    println!("{b}EXAMPLES{r}");
    println!("  openwith list -f py         {d}Filter by extension or app name{r}");
    println!("  openwith current pdf        {d}Show what opens .pdf files{r}");
    println!("  openwith set pdf Preview    {d}Set Preview as default for .pdf{r}");
    println!("  openwith set html \"Chrome\"  {d}Use quotes for multi-word names{r}");
}

pub fn print_version() {
    println!("openwith {}", env!("CARGO_PKG_VERSION"));
}
