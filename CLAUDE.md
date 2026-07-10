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
cargo run -p openwith-cli -- history               # recent changes from CLI + GUI (--json)
cargo run -p openwith-cli -- undo                  # revert the most recent change
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
    history.rs         -- append-only change log (~/Library/Application Support/openwith/history.json)
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
      history.rs         -- `openwith history` list recent events (relative dates, --json)
      undo.rs            -- `openwith undo` revert last set (drift check, --force)
      tui.rs             -- ratatui TUI: Extensions + Apps tabs, loading screen, AppPicker + Help
  openwith-gui/         -- Tauri v2 GUI ("OpenWith.app")
    src/                -- vanilla TS + Vite frontend (no framework)
      main.ts           -- entry: dispatches to app.ts (main window) or menubar.ts by window label
      app.ts            -- main window render functions + delegated event handling
      menubar.ts        -- tray popover (prototype 1d/2b): ext lookup, Recent Changes + Undo
      state.ts          -- app state, derived views, settings persistence (localStorage)
      api.ts            -- typed invoke() wrappers mirroring the Rust DTOs
      styles.css        -- design-prototype palette, light + dark via prefers-color-scheme
    src-tauri/src/
      commands.rs       -- #[tauri::command] wrappers over openwith-core + apps cache
      tray.rs           -- tray icon lifecycle + popover positioning (plugin-positioner)
      lib.rs            -- tauri Builder, plugins (dialog, opener, positioner, autostart, global-shortcut — default ⌥⌘O, user-configurable)
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
- `openwith-core::history` is the shared change log (capped at 500 events, best-effort writes that never fail the triggering change). CLI, GUI, and core import all record into it; the GUI Profiles panel shows export/import events, the menu-bar popover shows set events with per-entry Undo, and `openwith history`/`openwith undo` read the same file.
- The GUI is two windows off one Vite bundle: `main` and a hidden transparent `menubar` popover (requires `macOSPrivateApi: true`). The popover hides on blur and is toggled by the tray icon or a configurable global shortcut (default ⌥⌘O; `set_toggle_shortcut` swaps the registration at runtime, the saved accelerator is re-applied at bootstrap). A **Pin** button suspends hide-on-blur for one showing (backend `PopoverPinned` AtomicBool, reset on every toggle) so a file can be dragged in from Finder — without it the click into Finder blurs and hides the panel. Focus events are unreliable for the transparent panel, so the backend emits `popover-shown` on every open and the popover refreshes from that, not just from focus. A backend `AppsCache` (refreshed by `get_snapshot`) keeps popover lookups instant.
- GUI settings live in localStorage (`openwith.settings`). The Settings pane mirrors the design prototype's full layout; controls whose feature ships in a later 0.5.x phase (launch at login, menu bar) render disabled with an "arrives in v0.5.1" note rather than as silently-dead toggles.
- GUI visual source of truth is the claude.design prototype "OpenWith GUI Explorations" (project 14225854-984c-4e5c-8d2b-8c9ce38a1624), variants 1c (light) / 2a (dark): glyph tab icons (⌸ ⊞ ⤴ ⇅ — never emoji), 2-char initial chips (20px rows / 26px app list / 52px detail), fixed mid accent oklch(0.62 0.14 45) for tab underline + toggles, inverted toast. Check UI changes against it before shipping.

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
   If the release touches the GUI, also run the **GUI smoke-test checklist** below against a real `npm run tauri build` bundle — mandatory before tagging.
4. Create a git tag: `git tag vX.Y.Z`
5. Push the tag: `git push origin vX.Y.Z`
6. Create GitHub release with `gh release create` using the appropriate template below.
7. Build the GUI bundle and attach it to the release:
   ```bash
   (cd crates/openwith-gui && npm run tauri build)
   gh release upload vX.Y.Z target/release/bundle/dmg/OpenWith_X.Y.Z_aarch64.dmg
   ```
   The app is unsigned (no Apple Developer ID yet) — first launch needs `xattr -dr com.apple.quarantine /Applications/OpenWith.app` or right-click → Open.
8. Bump the Homebrew formula in `ColeMei/homebrew-openwith` (url + sha256 of the new tag tarball). Since the workspace conversion, the formula's `install` block must use `system "cargo", "install", *std_cargo_args(path: "crates/openwith-cli")` (the repo root is a virtual workspace with no installable package at `.`; the path must go through the helper's `path:` keyword — appending a separate `--path` flag duplicates the helper's built-in `--path=.` and cargo rejects it).
9. Update the `openwith-gui` cask in `ColeMei/homebrew-openwith` (`Casks/openwith-gui.rb`, url + sha256 of the .dmg release asset) with the quarantine caveat, so `brew install --cask ColeMei/openwith/openwith-gui` works for the GUI. (The cask was named `openwith` before v0.5.2.)

### GUI smoke-test checklist

Run against the built .app (not just `tauri dev`) before tagging any release with GUI changes. Naive "it compiles + the window opens" testing has shipped real bugs; every control must be exercised for a *real observable effect* (confirm sets/undos with `openwith current <ext>`).

- [ ] Close the main window, reopen via Dock click AND via popover "Open main window" — repeat ×3
- [ ] Toggle "Show in menu bar" off/on ×3 — exactly one tray icon at every step
- [ ] Hide Dock icon on/off; then turn the tray off while the Dock is hidden — Dock icon must come back
- [ ] Appearance: flip System/Light/Dark with the popover open — both windows restyle
- [ ] Every Settings toggle: launch at login, confirm before applying, warn on UTI conflicts, show bundle IDs, relaunch Finder, check automatically, channel, open-on-tab
- [ ] Set a default from the Extensions sheet; verify with `openwith current <ext>`; Undo from the toast; verify again
- [ ] Toasts dismiss themselves (~5s, ~8s with an Undo button) without being replaced
- [ ] Popover: extension lookup, change, Recent Changes + per-entry Undo
- [ ] Make a change in the main window, then open the popover — it appears under Recent Changes
- [ ] Popover Pin: pin, click into Finder (panel must stay up), drag a file onto it — extension lookup runs; unpinned panel still hides on blur; Esc and tray-toggle reset the pin
- [ ] Rebind the popover shortcut in Settings; old combo dead, new combo toggles; survives an app relaunch; ⌥⌘O labels in Settings + popover follow
- [ ] Toggle "Warn on UTI conflicts" off — UTI ⚠ badges disappear from the Extensions table; sheet + toast warnings stay off too
- [ ] Toggle "Show bundle IDs" off — bundle IDs vanish from Extensions table, Apps detail header, and popover rows
- [ ] With the CLI upgraded via brew while the app runs: close and reopen Settings — the Command Line panel shows the new version without an app relaunch
- [ ] Profiles: export; import via choose AND drag-drop; dry-run preview; apply; dismiss
- [ ] History panel scrolls at 50 entries and updates after changes
- [ ] Check Now (updates) reports a sensible result on both channels
- [ ] README screenshots (from the design prototype, `artifacts/gui-*.png`) still match the shipped UI — recapture if the UI changed

### Release templates

**Minor release (0.x.0) — new features:**

```
<one-line summary of the theme of this release>

**Features**
- CLI/TUI: <new capability 1>
- GUI: <new capability 2>

**Changes**
- <notable behavior change or improvement>

**Fixes**
- <bug fix, if any>

**Install**
\```bash
brew install ColeMei/openwith/openwith            # CLI + TUI
brew install --cask ColeMei/openwith/openwith-gui # GUI app
\```
```

**Patch release (0.0.x) — bug fixes:**

```
<one-line summary>

**Fixes**
- CLI/TUI: <fix 1>
- GUI: <fix 2>

**Install**
\```bash
brew install ColeMei/openwith/openwith            # CLI + TUI
brew install --cask ColeMei/openwith/openwith-gui # GUI app
\```
```

Group bullets under CLI/TUI and GUI prefixes when a release touches both; drop the prefix when a release is single-surface.
