<table align="center">
  <tr>
    <td width="160" align="center">
      <img src="artifacts/openwith-logo.png" alt="OpenWith logo" width="140">
    </td>
    <td>
      <h1>OpenWith</h1>
      <p>Manage macOS "Open With" defaults from the terminal.</p>
      <p>
        <img src="https://img.shields.io/badge/platform-macOS-lightgrey" alt="Platform: macOS">
        <img src="https://img.shields.io/badge/built_with-Rust-orange?logo=rust" alt="Built with Rust">
        <img src="https://img.shields.io/github/v/release/ColeMei/openwith?sort=semver" alt="Latest release">
        <img src="https://img.shields.io/github/license/ColeMei/openwith" alt="License: MIT">
      </p>
    </td>
  </tr>
</table>

**OpenWith** is a small Rust-based macOS terminal tool for inspecting and managing default apps for file types. It lets you see which app is currently set as the default for each file extension and update those associations from one place, without repetitive clicks or guessing bundle IDs.


## Prerequisites

OpenWith requires Rust. Install it via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Install

```bash
# From GitHub
cargo install --git https://github.com/ColeMei/openwith

# Or clone and build locally
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
