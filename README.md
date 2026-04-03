# OpenWith

**OpenWith** is a small Rust-based macOS terminal tool for inspecting and managing default apps for file types. It lets you see which app is currently set as the default for each file extension and update those associations from one place, without repetitive clicks or guessing bundle IDs.


## Install

```bash
# From GitHub
cargo install --git https://github.com/ColeMei/openwith

# Or clone and build locally
cargo install --path .
```

## Quick Start

```bash
openwith              # Launch interactive TUI
openwith list         # List all extensions with current defaults
openwith current pdf  # Show default app for .pdf
openwith set md Typora  # Set Typora as default for .md
```

Run `openwith --help` to see all commands, including `apps`, `export`, and `import`.

### Interactive TUI

Run `openwith` with no arguments to browse all file extensions, see their current defaults, and change them interactively. Press `?` inside the TUI for keyboard shortcuts.

### Export & Import

You can export your current associations to a TOML file and import them on another machine — making your "Open With" preferences portable, like a dotfile.

```bash
openwith export -o openwith.toml  # Export
openwith import openwith.toml     # Import on a new machine
```

Import skips associations where the app isn't found, so the same config works across machines with different setups.

## How It Works

1. Scans `/Applications`, `/System/Applications`, and `~/Applications` for `.app` bundles
2. Reads each app's `Info.plist` to discover supported file extensions
3. Queries and sets defaults via native macOS Launch Services APIs

## License

MIT
