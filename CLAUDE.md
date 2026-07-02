# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

openwith is a macOS-only CLI tool (Rust) that manages file extension associations ("Open With" defaults) and URL scheme handlers. It scans installed apps, queries/sets defaults via native macOS Launch Services APIs. Has both a ratatui-based TUI and non-interactive CLI subcommands. Supports exporting/importing associations as TOML for dotfile portability.

## Build & Run

```bash
cargo build --release              # production build
cargo run                          # TUI mode (extensions view)
cargo run -- apps                  # TUI mode (apps view)
cargo run -- list                  # TUI mode (extensions view)
cargo run -- list --json           # JSON output for scripts
cargo run -- current pdf           # show default for .pdf
cargo run -- set pdf Preview       # set default for .pdf
cargo run -- current -s http       # show default browser
cargo run -- set -s http Firefox   # set default browser
cargo run -- export -o out.toml    # export associations to TOML
cargo run -- import --dry-run out.toml  # preview an import
cargo run -- import out.toml       # import associations from TOML
cargo check                        # quick compile check
cargo test                         # run tests
cargo clippy                       # lint checks
```

## Architecture

```
src/
  main.rs              -- clap CLI dispatch
  cli.rs               -- clap derive structs, custom help template with ASCII logo
  logo.rs              -- shared ASCII art logo constant
  commands/
    list.rs            -- `openwith list`: TUI on a terminal, plain/JSON when scripted
    current.rs         -- `openwith current <ext>` (+ `-s` for URL schemes, `--json`)
    set.rs             -- `openwith set <ext> <app>` with name/bundle-ID resolution
    export.rs          -- `openwith export` dump associations + schemes to TOML
    import.rs          -- `openwith import` apply TOML (idempotent, `--dry-run`)
    tui.rs             -- ratatui TUI: Extensions + Apps tabs, loading screen, AppPicker + Help
  core/
    scanner.rs         -- app discovery via mdfind + fs walk, app/bundle-ID resolution
    plist.rs           -- Info.plist parsing via the plist crate (extensions, content types, URL schemes)
    launchservices.rs  -- native macOS Launch Services FFI: UTI and URL scheme handlers
    uti.rs             -- UTI resolution: system lookup first, hardcoded fallback map, memoized; shared-UTI sibling detection
    listing.rs         -- parallel default-handler queries shared by TUI, export, and list
    config.rs          -- TOML export/import logic ([associations] + [schemes])
    types.rs           -- AppInfo
```

### Key patterns

- `core/launchservices.rs` uses FFI to `LSCopyDefaultRoleHandlerForContentType`, `LSSetDefaultRoleHandlerForContentType`, and the URL scheme equivalents. No external CLI dependencies.
- `core/uti.rs` asks Launch Services for the UTI first (the mapping Finder actually uses) and falls back to a hardcoded table only for extensions the system maps to a dynamic (`dyn.*`) type. Lookups are memoized process-wide.
- macOS maps defaults to UTIs, not extensions; `uti::extensions_sharing_uti` finds sibling extensions so commands can warn about side effects.
- `core/scanner.rs` has `resolve_app_or_bundle_id(apps, value)` accepting app names or bundle IDs, and `resolve_name(apps, bundle_id)` to map bundle IDs back to app names.
- `core/listing.rs` parallelizes default queries using `std::thread::scope` with chunks of 20; the TUI runs it in a background thread behind the loading screen.
- TUI uses a `Tab` enum (`Extensions`, `Apps`) and a `View` enum state machine: `ExtensionList`, `AppPicker`, `AppsBrowser`, `Help`. Terminal setup is wrapped in an RAII guard plus a panic hook so raw mode is always restored.
- `Tab` key switches between Extensions and Apps views at top level; inside `AppPicker`, `Tab` toggles supporting/all apps.
- Apps browser uses a master-detail layout: left pane is app list, right pane shows details (supported extensions, defaults).
- Loading screen enters TUI alternate screen immediately, shows ASCII logo + spinner while scanning in background.
- Export/import uses serde + toml crate with `BTreeMap<String, String>` for sorted, human-readable TOML; import validates apps exist and skips associations already set correctly.

## Runtime dependencies

- `mdfind` -- macOS built-in used for app discovery (with a filesystem-walk fallback).
- No external tools required (duti removed in v0.2.0, PlistBuddy removed in v0.4.0).

## Development Workflow

### Git commits

- One logical change per commit — a single bug fix, a single feature, or a single refactor.
- Never bundle unrelated changes into one commit.
- Stage files selectively (`git add <file>...`), not `git add -A`.
- Commit message style: `type: concise description` where type is one of `feat`, `fix`, `refactor`, `docs`, `chore`, `test`.
- Run `cargo fmt` before every commit.
- Keep `main` linear. Use squash merge for pull requests by default:
  ```bash
  gh pr merge <number> --squash --delete-branch
  ```
- Do not use merge commits unless explicitly requested.
- If a release tag has already been pushed, do not rewrite history just to linearize it.

### Versioning (SemVer-ish)

| Bump | When | Example |
|------|------|---------|
| `0.0.x` (patch) | Bug fixes, small tweaks, docs | v0.1.1 |
| `0.x.0` (minor) | New features, notable behavior changes | v0.2.0 |
| `x.0.0` (major) | Reserved — standalone app, public distribution, breaking changes | future |

When releasing, also update `version` in `Cargo.toml` to match the tag.

### Release process

1. Ensure all changes are committed and pushed.
2. Update `Cargo.toml` version if not already done.
3. Run validation:
   ```bash
   cargo fmt --check
   cargo check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   ```
4. Create a git tag: `git tag vX.Y.Z`
5. Push the tag: `git push origin vX.Y.Z`
6. Create GitHub release with `gh release create` using the appropriate template below.
7. Bump the Homebrew formula in `ColeMei/homebrew-openwith` (url + sha256 of the new tag tarball).

### Release templates

**Minor release (0.x.0) — new features:**

```
<one-line summary of the theme of this release>

**Features**
- <new capability 1>
- <new capability 2>

**Changes**
- <notable behavior change or improvement>

**Fixes**
- <bug fix, if any>

**Install**
\```bash
brew tap ColeMei/openwith
brew install openwith
\```
```

**Patch release (0.0.x) — bug fixes:**

```
<one-line summary>

**Fixes**
- <fix 1>
- <fix 2>

**Install**
\```bash
brew tap ColeMei/openwith
brew install openwith
\```
```
