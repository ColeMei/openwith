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
cargo run -p openwith-cli -- history               # recent changes, last 7 days (--days N, --all, --json)
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
    uti.rs             -- UTI resolution: whatever Launch Services reports, incl. dynamic `dyn.*` types, memoized; shared-UTI sibling detection
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
      history.rs         -- `openwith history` list recent events (relative dates, --days/--all, --json)
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
      release.rs        -- pure, unit-tested release arithmetic: version compare, daily-check gate, pending/remembered, release-page URL
      updates.rs        -- the release check: GitHub fetch, shared answer in update.json, `update-status` event, updater gating
      lib.rs            -- tauri Builder, plugins (dialog, opener, positioner, autostart, process, updater only with a real key, global-shortcut — default ⌥⌘O, user-configurable)
.github/workflows/
  ci.yml                -- PRs + main: fmt/check/clippy/test on macOS, frontend build
  release.yml           -- v* tag (web-UI publish or git push): build on macOS, sign in the `release` environment, attach assets
  latest.yml            -- release published/edited: write + attach latest.json (stable only)
  tap.yml               -- release published: bump cask + formula in ColeMei/homebrew-openwith
  release-check.yml     -- manual: the signing secret matches the shipped public key
```

### Key patterns

- `openwith-core::launchservices` uses FFI to `LSCopyDefaultRoleHandlerForContentType`, `LSSetDefaultRoleHandlerForContentType`, and the URL scheme equivalents. No external CLI dependencies.
- `openwith-core::uti` returns whatever UTI Launch Services reports and nothing else — that mapping is the one Finder consults, so writing a handler to any other UTI silently has no effect. That includes **dynamic** (`dyn.*`) types, minted for extensions no installed app claims: they are ordinary settable targets, and their identifiers encode the extension itself, so they are stable per-extension across machines. A hardcoded UTI table used to override them; it aimed `set` at types the system did not map the extension to, so writes landed where nothing read them (`.jsx`, `.tsx`, `.rar`) and `.env` fell through to the shared `public.plain-text` — removed (issue #16). What survives is `COMMON_EXTENSIONS`, a plain candidate *list* for sibling detection with no UTI opinions. Launch Services mints a dynamic UTI for any tag including the empty string, so an empty extension is the one input `uti_for_extension` rejects itself. Lookups are memoized process-wide.
- macOS maps defaults to UTIs, not extensions; `uti::extensions_sharing_uti` finds sibling extensions so commands can warn about side effects.
- `openwith-core::scanner` has `resolve_app_or_bundle_id(apps, value)` accepting app names or bundle IDs, and `resolve_name(apps, bundle_id)` to map bundle IDs back to app names.
- `openwith-core::listing` parallelizes default queries using `std::thread::scope` with chunks of 20; the TUI runs it in a background thread behind the loading screen.
- TUI uses a `Tab` enum (`Extensions`, `Apps`) and a `View` enum state machine: `ExtensionList`, `AppPicker`, `AppsBrowser`, `Help`. Terminal setup is wrapped in an RAII guard plus a panic hook so raw mode is always restored.
- `Tab` key switches between Extensions and Apps views at top level; inside `AppPicker`, `Tab` toggles supporting/all apps.
- Apps browser uses a master-detail layout: left pane is app list, right pane shows details (supported extensions, defaults).
- Loading screen enters TUI alternate screen immediately, shows ASCII logo + spinner while scanning in background.
- Export/import uses serde + toml crate with `BTreeMap<String, String>` for sorted, human-readable TOML; import validates apps exist and skips associations already set correctly.
- GUI: single `get_snapshot` command returns apps + associations (with sibling-UTI conflict data) + contested schemes in one call; the frontend is a plain render-to-innerHTML loop with `data-action` event delegation, no framework. Versions are lockstep: `tauri.conf.json` omits `version` so the app version comes from `workspace.package` in the root Cargo.toml.
- GUI startup: the main window is configured `visible: false`, so the first frame the user sees is a painted one — previously the native window appeared before WebKit's first frame, which read as a white flash (issue #20). After applying the theme and rendering the loading UI, the frontend calls `main_window_ready`, which matches the native background to the CSS `--bg` and reveals the window; Dock/popover reopen paths are gated on the same `MainWindowReady` flag so they can't put the empty webview on screen first. Because nothing but that call reveals the window, two fallbacks keep a hidden-forever window off the table: the command parses the colour best-effort (an unparseable `--bg` still reveals) and `setup` reveals unconditionally after 3s if the webview never reported ready. A slow launch (at login, after a reboot) can hit that 3s fallback, so `setup` first paints the native window with the system appearance's `--bg` (`BG_LIGHT`/`BG_DARK` in commands.rs, mirroring styles.css), so the fallback reveal is themed too, and logs to stderr when the fallback is what revealed the window (issue #22). The theme itself must be right before that reveal: `theme.ts` resolves "System" from the native window theme (`getCurrentWindow().theme()` + `onThemeChanged`), with `prefers-color-scheme` only as a stand-in until it arrives, and `bootstrap` awaits `themeReady` before the first render. WebKit's media query answered "light" for the still-hidden window on a macOS 27 beta, so the window revealed on the light palette and flipped to dark a frame later; the reporter's video in #22 shows that frame. Reproduce by forcing the media query to read light until `main_window_ready` returns. `reveal_main` is idempotent via an atomic swap, so the fallback can't re-show a window the user has since closed. `StateFlags` deliberately omits `VISIBLE`, so window-state restore doesn't fight the hidden start. Animation-frame waits happen only after showing, with a timeout for hidden/minimized windows; `get_snapshot` runs through Tauri's async command path so scanning doesn't block the native event loop while the loading screen is trying to paint.
- `openwith-core::history` is the shared change log (capped at 500 events / 90 days, best-effort writes that never fail the triggering change). CLI, GUI, and core import all record into it; the GUI Profiles panel shows export/import events, the menu-bar popover shows set events with per-entry Undo, and `openwith history`/`openwith undo` read the same file.
- History retention is a **display window, not deletion**. The 90-day/500-event prune in `record_at` is only a ledger backstop and runs on *write*, so a dormant install would otherwise show months-old rows forever — the window is therefore applied on **read**, via `history::recent_within`. Default is 7 days (`DEFAULT_WINDOW_DAYS`), settable in Settings → Behavior → "Show history for" (1 week / 1 month / All — the ledger's own 90-day cap makes a "3 months" option redundant; persisted as `historyWindowDays` in localStorage and shared by both windows) and via `openwith history --days N` / `--all`. The Profiles HISTORY panel head shows the active window plus a session-only "Show all" toggle (`state.historyShowAll`, deliberately not persisted). `undo_change` and `openwith undo` keep reading the *unwindowed* ledger (`history::recent`) so a hidden event stays revertible.
- The GUI is two windows off one Vite bundle: `main` and a hidden transparent `menubar` popover (requires `macOSPrivateApi: true`). The popover hides on blur and is toggled by the tray icon or a configurable global shortcut (default ⌥⌘O; `set_toggle_shortcut` swaps the registration at runtime, the saved accelerator is re-applied at bootstrap). A **Pin** button suspends hide-on-blur for one showing (backend `PopoverPinned` AtomicBool, reset on every toggle) so a file can be dragged in from Finder — without it the click into Finder blurs and hides the panel. Focus events are unreliable for the transparent panel, so the backend emits `popover-shown` on every open and the popover refreshes from that, not just from focus. A backend `AppsCache` (refreshed by `get_snapshot`) keeps popover lookups instant.
- App icons come from the 2026-07 logo renders in `artifacts/` (`logo-mono-glyph.png`, `logo-icon-dark.png`, `logo-icon-light.png` — AI renders with **no alpha**, so masking is scripted, not manual): light → 1024 master → `tauri icon` set; dark → `icons/icon-dark.png`; mono glyph → `icons/tray-template.png` (64×44 black+alpha template, glyph ≈30px tall → ~22 × 12pt on screen). The Dock icon follows the app's *resolved* appearance at runtime — `set_dock_icon_dark` swaps `NSApplication.applicationIconImage` between `icon.png`/`icon-dark.png` (macOS only re-renders bundle icons for the system appearance), invoked from `theme.ts` on every theme change. `icon.icns` is **hand-built**: the ≤64px slots use a legibility variant (header dots inpainted away, no shadow, larger glyph), so rerunning `tauri icon` clobbers it. All of these regenerate with `uv run scripts/gen-icons.py` (run it *after* any `tauri icon` invocation; it prints the `tauri icon` command for the PNG set).
- GUI lists have exactly **one** highlight, driven by the pointer and the arrow keys alike: hovering a row *moves* the keyboard cursor (`state.extCursor` / `state.sheetCursor`, popover `state.cursor`) rather than painting a second, competing highlight. Hover updates the `.active` class imperatively — re-rendering 369 rows per row crossed would be visibly janky and fight `scrollIntoView`. While the arrows are driving, a `kbd-nav` class on `root` mutes hover; it clears on the next mousemove **whose coordinates actually changed**, because scrolling under a stationary pointer makes WebKit synthesize a same-position mousemove, and honouring that let hover re-seat the cursor on whatever row had slid underneath (three keypresses walked eleven rows). The Apps tab is the deliberate exception — it keeps click-to-select, since hovering would swap the detail pane out from under the pointer.
- **Updates: the app's check is the news, the updater is only the installer** (modelled on Pane v0.7.3). `updates.rs` asks the GitHub releases API, at launch, on main-window focus and on popover open, at most once a day (`release::should_check`; a failed check retries after 15 min), and never with "Check automatically" off: off means no request, not an ignored answer. Check Now and channel switches always ask. A release isn't offered until its .dmg is attached, because a web-UI release is public for the ~15 min CI takes to build it. The answer (the newer version on the user's channel) is remembered in `update.json` in app data and broadcast to both windows as `update-status`, so Settings, the toast and the popover badge can't disagree. It is re-compared against the running build on every read, so an upgrade clears every notice at once; there is no dismissed flag. `Unknown` (offline) keeps the remembered answer. The toast fires only on a scheduled check's answer (`announce`), so it repeats daily while behind and never elbows out a toast already on screen.
- **Version comparison lives in Rust** (`release.rs`, with unit tests), not TS: `v`-tolerant, component by component (`0.10 > 0.9`, `0.7 == 0.7.0`), a newer local build is current, unparseable is unknown. Unlike Pane, a pre-release ranks below its release (SemVer), so someone on `0.7.0-rc.1` still hears about `0.7.0`. The old exact-string compare offered a Stable-channel rc the older release it was ahead of.
- **"Update to X…" installs exactly X or opens X's release page.** The confirm sheet (main window; the popover badge hands over via `start-update`) runs `check()` against `releases/latest/download/latest.json` and installs only if the manifest offers exactly the version the notice named. Anything else (no real key in this build, the manifest offering another version, which is how a Beta rc lands here since `latest.json` only carries stable releases, a failed check, download or signature) closes the sheet and opens the release page. Every update is verified against `plugins.updater.pubkey`, and `requireSignedVersion` makes the signature's trusted comment name the version, so a tampered manifest can't pair a new version number with an older signed build. A build whose pubkey is still `__TAURI_UPDATER_PUBKEY__` never registers the updater plugin (`updater_available` → false). Release notes render as escaped plain text: the frontend is innerHTML with IPC access. Replacing the bundle drops a custom Finder icon (`Icon\r`) on OpenWith.app, the same as a reinstall (#25); the sheet says so.
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
| `-rc.N` (pre-release) | A fix the reporter must confirm before it ships, typically one we can only reproduce synthetically | v0.6.5-rc.1 |

When releasing, update `version` in the root `Cargo.toml` (`workspace.package`) to match the tag — CLI and GUI share this single version (lockstep). Also keep `crates/openwith-gui/package.json` in sync (cosmetic only; the bundle version comes from Cargo).

### Release process

Releases are built, signed and published by CI; nothing is built on a Mac by hand. Publishing a release in the GitHub web UI pushes its tag, and the tag push is what triggers `release.yml`.

1. Ensure all changes are merged to `main`.
2. Bump the workspace version in `Cargo.toml` (and `package.json`) on `main` (`chore: bump version to vX.Y.Z`). `release.yml` refuses a stable tag that doesn't match it, because Homebrew builds the CLI from the tag's source tarball.
3. Run validation:
   ```bash
   cargo fmt --check
   cargo check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   (cd crates/openwith-gui && npm run build)
   ```
   If the release touches the GUI, the **GUI smoke-test checklist** below is mandatory before the final tag, run against a CI-built `.app` (an rc of the same commit) or a local `npm run tauri build`.
4. Publish the release in the web UI: new tag `vX.Y.Z` on `main`, title `vX.Y.Z`, notes from the template below. (Or push a tag from git: `release.yml` then creates the release as a **draft**; review the notes and publish it.)
5. Approve the `release` environment deployment when Actions asks. `release.yml`'s `build` job (macOS, no secrets) builds the .app and .dmg, checks the version, key and endpoint inside the dmg, and packs `OpenWith_X.Y.Z_aarch64.app.tar.gz`. The `sign` job (the only job holding the key) signs that archive bound to the version, verifies the signature against the pubkey in `tauri.conf.json`, and attaches the dmg, archive and `.sig`.
6. Once the assets are attached to the published release, `latest.yml` writes `latest.json` from the published notes (dropping the Install section) and attaches it, and `tap.yml` bumps `Casks/openwith-gui.rb` and `Formula/openwith.rb` in `ColeMei/homebrew-openwith`. Editing the notes later rewrites `latest.json`. Re-run either by hand (`workflow_dispatch` with the tag) if a run failed. The formula's `install` block must keep `system "cargo", "install", *std_cargo_args(path: "crates/openwith-cli")` (the repo root is a virtual workspace; a separate `--path` flag duplicates the helper's built-in `--path=.` and cargo rejects it).
7. Check `https://github.com/ColeMei/openwith/releases/latest/download/latest.json` serves the new version, and the tap commit landed.

Between publishing and step 6 (~15 min plus the approval) the release has no assets. The app doesn't announce a release until its .dmg is attached, and an "Update to X…" pressed then falls back to the release page.

**Update signing key.** Generated once by the maintainer (`npx @tauri-apps/cli signer generate`), private half and password kept offline and as the `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets of the `release` environment (required reviewer: the maintainer, self-review allowed; deployment refs: tags `v*` and branch `main`, since the key check and test builds are dispatched from `main`). Never generate, print or handle it in a session. The public half is `plugins.updater.pubkey` in `tauri.conf.json`. Run **Release setup check** (`release-check.yml`) after setting or rotating it. Losing the key means no installed copy can update in place again (manual reinstall); rotating it means every copy before the rotation must reinstall by hand.

### Pre-releases

Cut a pre-release when a fix needs the reporter's confirmation before it ships, typically when the bug only occurs on their setup (e.g. a macOS beta) and we can only reproduce it synthetically. v0.6.4 shipped as "fixed" for a bug that came back on the reporter's machine; a pre-release avoids another round of that.

1. Keep the fix in its PR; don't merge it yet. Don't bump any version: CI stamps an rc's version from its tag, so the build reports `X.Y.Z-rc.N` (the update check needs that, or an rc reporting the last stable version would be offered that release).
2. Run the same validation as a release.
3. Publish a release in the web UI: new tag `vX.Y.Z-rc.N` targeting **the PR branch**, title `vX.Y.Z-rc.N`, notes from the template below, **"Set as a pre-release"** ticked. Approve the `release` deployment. CI attaches the signed dmg and archive; `latest.yml` and `tap.yml` skip pre-releases, so `releases/latest` and Homebrew stay on stable. Check `gh api repos/ColeMei/openwith/releases/latest` still returns the last stable tag.
4. For the smoke test, the checklist items that exercise the change are enough for an rc (run them against the CI-built .app); the **full** checklist is still mandatory before the final tag.
5. Point the reporter at the release page in the issue.
6. Once confirmed, merge the PR and cut `vX.Y.Z` from `main` with the normal release process. Leave the pre-release up.

Users on the GUI's **Beta** update channel see pre-releases (Settings → Updates); Stable users don't. "Update to X…" on an rc opens its release page: the in-place installer only offers what `latest.json` carries, which is stable releases.

**End-to-end updater test** (two signed builds; needs a Mac). Any change to the updater, the signing or the manifest is verified this way before the stable tag:

1. Publish `vX.Y.Z-rc.1` (as above). Then run `release.yml` by hand with `tag = vX.Y.Z-rc.1` and `updater_endpoint = https://github.com/ColeMei/openwith/releases/download/vX.Y.Z-rc.2/latest.json` (the URL is predictable before rc.2 exists). That attaches `OpenWith_X.Y.Z-rc.1_aarch64_update-test.dmg`, a build that reads rc.2's manifest instead of `releases/latest`.
2. Publish `vX.Y.Z-rc.2`, then run `latest.yml` by hand with `tag = vX.Y.Z-rc.2` and **include_prerelease** ticked.
3. On the Mac: install the rc.1 `_update-test` dmg, set the channel to Beta, Check Now. The notice names rc.2; Update to rc.2… shows the sheet and its notes; Install downloads, replaces and relaunches; Settings then reports rc.2 and the notice is gone. Check the app still opens without a quarantine prompt.
4. Negative paths, each starting again from the rc.1 test build: delete rc.2's `latest.json` asset → the release page opens. Re-upload it with one character of `signature` changed → the install is refused and the release page opens. A normal (non-test) rc build → the release page opens. Stable channel → no notice for rc.2. Rewrite `latest.json` back to the real one afterwards (`latest.yml`, include_prerelease).

**Pre-release template:**

```
Test build for <issue>. Not on Homebrew.

**Fixes**
- GUI: <fix>

**Install**
Download `OpenWith_X.Y.Z-rc.N_aarch64.dmg` below and drag OpenWith into Applications. The app is unsigned, so run `xattr -dr com.apple.quarantine /Applications/OpenWith.app` before first launch.
```

### GUI smoke-test checklist

Run against the built .app (not just `tauri dev`) before tagging any release with GUI changes. Naive "it compiles + the window opens" testing has shipped real bugs; every control must be exercised for a *real observable effect* (confirm sets/undos with `openwith current <ext>`).

- [ ] Launch cold: the window's first visible frame is the spinner + "Scanning applications…" on the themed background, never a white or empty pane — check in both light and dark
- [ ] Close the main window, reopen via Dock click AND via popover "Open main window" — repeat ×3
- [ ] Move and resize the main window, quit, relaunch — it returns to the same size and position; repeat with the window closed before quitting; the popover still anchors to the tray rather than a remembered spot
- [ ] Toggle "Show in menu bar" off/on ×3 — exactly one tray icon at every step
- [ ] Hide Dock icon on/off; then turn the tray off while the Dock is hidden — Dock icon must come back
- [ ] Appearance: flip System/Light/Dark with the popover open — both windows restyle AND the Dock icon flips light/dark (System follows macOS)
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
- [ ] Keyboard: ⌘F → type → ↑↓ moves the cursor → ↵ opens the sheet; ↑↓/↵ inside the sheet applies (verify with `openwith current <ext>`); Esc clears the search before releasing focus; with the pointer parked on the list, three ↓ move exactly three rows and the hovered row stays dark until the mouse really moves; the popover does the same for matches and its picker
- [ ] Menu-bar icon sits at roughly the same visual weight as neighbouring status items (it renders ~22 x 12pt; `tray-icon` fixes the canvas at 18pt tall and derives width from the canvas aspect)
- [ ] History panel scrolls at 50 entries and updates after changes
- [ ] History window: default shows only the last 7 days in both the Profiles panel and the popover; "Show all" reveals older rows and toggles back; switching the Settings segment (1 week/1 month/All) refetches both surfaces; the popover follows a change made in the main window without a relaunch
- [ ] Check Now (updates) reports a sensible result on both channels; a newer local build (an rc on Stable) reads "Up to date", not an older "update"
- [ ] "Check automatically" off: no request to api.github.com at launch, on focus or on popover open (watch with Little Snitch or `nettop`); Check Now still works
- [ ] Behind a release (run the previous version): the toast with "Update…", the popover's "Update to X…" badge and Settings' "Update to X…" all appear; the toast comes back the next day, not on every launch
- [ ] Update to X… → sheet with version, date and plain-text notes → Install → progress → relaunch on X; Settings reports X and every notice is gone. A custom Finder icon on OpenWith.app is reset (expected, #25)
- [ ] Update to X… from the popover badge opens the main window's sheet
- [ ] Fallback: with `latest.json` unreachable (offline after the notice, or a build with the placeholder key), Update to X… opens X's release page with a toast instead of failing silently
- [ ] Beta channel on an rc: the notice names the newer rc, and Update to it… opens its release page
- [ ] README screenshots still match the shipped UI — recapture if it changed. GUI shots (`artifacts/gui-*.png`) come from the design prototype; TUI shots (`artifacts/tui-*.png`) are regenerated with `./scripts/capture-tui.sh`

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
