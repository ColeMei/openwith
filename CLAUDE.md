# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

openwith is a macOS-only tool (Rust) that manages file extension associations ("Open With" defaults) and URL scheme handlers. It scans installed apps, queries/sets defaults via native macOS Launch Services APIs. Ships as a CLI (with a ratatui TUI) plus a native Tauri GUI, all sharing `openwith-core`. Supports exporting/importing associations as TOML for dotfile portability.

## Build & Run

```bash
cargo build --release              # production build (all workspace members)
cargo run -p openwith-cli          # TUI mode (extensions view)
cargo run -p openwith-cli -- apps                  # TUI mode (apps view)
cargo run -p openwith-cli -- list                  # TUI mode (extensions view)
cargo run -p openwith-cli -- list --json           # JSON output for scripts
cargo run -p openwith-cli -- current pdf           # show default for .pdf
cargo run -p openwith-cli -- set pdf Preview       # set default for .pdf
cargo run -p openwith-cli -- current -s http       # show default browser
cargo run -p openwith-cli -- set -s http Firefox   # set default browser
cargo run -p openwith-cli -- export -o out.toml    # export associations to TOML
cargo run -p openwith-cli -- import --dry-run out.toml  # preview an import
cargo run -p openwith-cli -- import out.toml       # import associations from TOML
cargo check                        # quick compile check
cargo test                         # run tests
cargo clippy                       # lint checks
```

GUI (Tauri v2; run from `crates/openwith-gui/`):

```bash
npm install                        # once, installs frontend deps + tauri CLI
npm run tauri dev                  # hot-reloading dev app
npm run tauri build                # unsigned .app/.dmg under <repo>/target/release/bundle/
npm run build                      # tsc + vite build only (frontend typecheck)
```

## Architecture

Cargo workspace with three members: `openwith-core` (library), `openwith-cli` (binary crate `openwith`), and `openwith-gui/src-tauri` (Tauri v2 app). GUI phasing follows `~/.claude/plans/use-the-claude-design-mcp-gentle-kay.md`.

```
crates/
  openwith-core/src/    -- library crate `openwith_core`
    scanner.rs         -- app discovery via mdfind + fs walk, app/bundle-ID resolution
    plist.rs           -- Info.plist parsing via the plist crate (extensions, content types, URL schemes)
    launchservices.rs  -- native macOS Launch Services FFI: UTI and URL scheme handlers
    uti.rs             -- UTI resolution: system lookup first, hardcoded fallback map, memoized; shared-UTI sibling detection
    listing.rs         -- parallel default-handler queries shared by TUI, export, and list
    config.rs          -- TOML export/import logic ([associations] + [schemes])
    types.rs           -- AppInfo
  openwith-cli/src/     -- bin crate, produces the `openwith` executable
    main.rs             -- clap CLI dispatch
    cli.rs              -- clap derive structs, custom help template with ASCII logo
    logo.rs             -- shared ASCII art logo constant
    commands/
      list.rs            -- `openwith list`: TUI on a terminal, plain/JSON when scripted
      current.rs         -- `openwith current <ext>` (+ `-s` for URL schemes, `--json`)
      set.rs             -- `openwith set <ext> <app>` with name/bundle-ID resolution
      export.rs          -- `openwith export` dump associations + schemes to TOML
      import.rs          -- `openwith import` apply TOML (idempotent, `--dry-run`)
      tui.rs             -- ratatui TUI: Extensions + Apps tabs, loading screen, AppPicker + Help
  openwith-gui/         -- Tauri v2 GUI ("OpenWith.app")
    src/                -- vanilla TS + Vite frontend (no framework)
      main.ts           -- render functions + delegated event handling
      state.ts          -- app state, derived views, settings persistence (localStorage)
      api.ts            -- typed invoke() wrappers mirroring the Rust DTOs
      styles.css        -- design-prototype palette, light + dark via prefers-color-scheme
    src-tauri/src/
      commands.rs       -- #[tauri::command] wrappers over openwith-core (snapshot, set, export/import, detect_cli)
      lib.rs            -- tauri Builder + plugin registration (dialog, opener)
```

### Key patterns

- `openwith-core::launchservices` uses FFI to `LSCopyDefaultRoleHandlerForContentType`, `LSSetDefaultRoleHandlerForContentType`, and the URL scheme equivalents. No external CLI dependencies.
- `openwith-core::uti` asks Launch Services for the UTI first (the mapping Finder actually uses) and falls back to a hardcoded table only for extensions the system maps to a dynamic (`dyn.*`) type. Lookups are memoized process-wide.
- macOS maps defaults to UTIs, not extensions; `uti::extensions_sharing_uti` finds sibling extensions so commands can warn about side effects.
- `openwith-core::scanner` has `resolve_app_or_bundle_id(apps, value)` accepting app names or bundle IDs, and `resolve_name(apps, bundle_id)` to map bundle IDs back to app names.
- `openwith-core::listing` parallelizes default queries using `std::thread::scope` with chunks of 20; the TUI runs it in a background thread behind the loading screen.
- TUI uses a `Tab` enum (`Extensions`, `Apps`) and a `View` enum state machine: `ExtensionList`, `AppPicker`, `AppsBrowser`, `Help`. Terminal setup is wrapped in an RAII guard plus a panic hook so raw mode is always restored.
- `Tab` key switches between Extensions and Apps views at top level; inside `AppPicker`, `Tab` toggles supporting/all apps.
- Apps browser uses a master-detail layout: left pane is app list, right pane shows details (supported extensions, defaults).
- Loading screen enters TUI alternate screen immediately, shows ASCII logo + spinner while scanning in background.
- Export/import uses serde + toml crate with `BTreeMap<String, String>` for sorted, human-readable TOML; import validates apps exist and skips associations already set correctly.
- GUI: single `get_snapshot` command returns apps + associations (with sibling-UTI conflict data) + contested schemes in one call; the frontend is a plain render-to-innerHTML loop with `data-action` event delegation, no framework. Versions are lockstep: `tauri.conf.json` omits `version` so the app version comes from `workspace.package` in the root Cargo.toml.
- GUI settings live in localStorage (`openwith.settings`); only controls whose behavior actually works are rendered — no stub toggles for unshipped phases.

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

When releasing, update `version` in the root `Cargo.toml` (`workspace.package`) to match the tag — CLI and GUI share this single version (lockstep). Also keep `crates/openwith-gui/package.json` in sync (cosmetic only; the bundle version comes from Cargo).

### Release process

1. Ensure all changes are committed and pushed.
2. Update the workspace version in `Cargo.toml` if not already done.
3. Run validation:
   ```bash
   cargo fmt --check
   cargo check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   (cd crates/openwith-gui && npm run build)
   ```
4. Create a git tag: `git tag vX.Y.Z`
5. Push the tag: `git push origin vX.Y.Z`
6. Create GitHub release with `gh release create` using the appropriate template below.
7. Build the GUI bundle and attach it to the release:
   ```bash
   (cd crates/openwith-gui && npm run tauri build)
   gh release upload vX.Y.Z target/release/bundle/dmg/OpenWith_X.Y.Z_aarch64.dmg
   ```
   The app is unsigned (no Apple Developer ID yet) — first launch needs `xattr -dr com.apple.quarantine /Applications/OpenWith.app` or right-click → Open.
8. Bump the Homebrew formula in `ColeMei/homebrew-openwith` (url + sha256 of the new tag tarball). Since the workspace conversion, the formula's `install` block must use `system "cargo", "install", *std_cargo_args, "--path", "crates/openwith-cli"` (the repo root is now a virtual workspace with no installable package at `.`).
9. Update the `openwith` cask in `ColeMei/homebrew-openwith` (url + sha256 of the .dmg release asset) with the quarantine caveat, so `brew install --cask` works for the GUI.

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
