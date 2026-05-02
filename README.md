<p align="center">
  <img src="artifacts/openwith-logo.png" alt="OpenWith logo" width="120">
</p>

<h1 align="center">OpenWith</h1>

<p align="center">
  Manage macOS "Open With" defaults from the terminal.
</p>

<p align="center">
  <a href="https://github.com/ColeMei/openwith/releases">Releases</a>
  ·
  <a href="LICENSE">License</a>
</p>

<p align="center">
  <a href="https://github.com/ColeMei/openwith/releases"><img src="https://img.shields.io/github/v/release/ColeMei/openwith?sort=semver&style=flat-square" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/ColeMei/openwith?style=flat-square" alt="License"></a>
  <img src="https://img.shields.io/badge/built_with-Rust-orange?logo=rust&style=flat-square" alt="Built with Rust">
  <img src="https://img.shields.io/badge/platform-macOS-lightgrey?style=flat-square" alt="Platform: macOS">
</p>

**OpenWith** is a small Rust-based macOS terminal tool for inspecting and managing default apps for file types. It lets you see which app is currently set as the default for each file extension and update those associations from one place, without repetitive clicks or guessing bundle IDs.


## Install

Homebrew is the recommended install path:

```bash
brew tap ColeMei/openwith
brew install openwith
```

If you prefer installing from source with Cargo, install Rust via [rustup](https://rustup.rs), then run:

```bash
cargo install --git https://github.com/ColeMei/openwith
```

For local development builds from this repository:

```bash
cargo install --path .
```

## Quick Start

```bash
openwith                # Launch interactive TUI (extensions view)
openwith list           # Same as above
openwith apps           # Launch interactive TUI (apps view)
openwith current pdf    # Show default app for .pdf
openwith set md Typora  # Set Typora as default for .md
```

Run `openwith --help` to see all commands.

### Interactive TUI

Run `openwith` with no arguments to browse all file extensions, see their current defaults, and change them interactively. The TUI has two tabs you can switch between with `Tab`:

- **Extensions** — browse all file extensions, see their current default app, and change defaults via an app picker
- **Apps** — browse all installed apps in a master-detail view, see which extensions each app supports and which it's the default for

Press `?` inside the TUI for keyboard shortcuts.

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

## Acknowledgement

Special thanks to [linux.do](https://linux.do)
