# openwith

Manage macOS "Open With" file extension associations from the terminal.

Scan your installed applications, see what opens each file type, and change defaults — all without touching Finder.

## Install

```bash
cargo install --path .
```

[duti](https://github.com/moretension/duti) is required and will be installed automatically via Homebrew on first run.

## Usage

```
openwith                    Launch interactive TUI
openwith list               List all extensions with current defaults
openwith list -f py         Filter by extension or app name
openwith current pdf        Show current default for .pdf
openwith set pdf Preview    Set Preview as default for .pdf
```

### Interactive TUI

Run `openwith` with no arguments to browse all file extensions, see their current defaults, and change them interactively.

| Key | Action |
|-----|--------|
| `j` / `k` / arrows | Navigate |
| `/` | Filter by extension or app name |
| `Enter` | Change default app |
| `Tab` | Toggle between supporting apps and all apps |
| `q` | Quit |

## How it works

1. Scans `/Applications`, `/System/Applications`, and `~/Applications` for `.app` bundles
2. Reads each app's `Info.plist` to discover supported file extensions
3. Queries current defaults via `duti -x`
4. Sets new defaults via `duti -s` using UTI (Uniform Type Identifier) mapping

## License

MIT
