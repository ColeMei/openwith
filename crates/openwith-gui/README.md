# openwith-gui

Native macOS GUI for [openwith](https://github.com/ColeMei/openwith), built with Tauri v2
(Rust backend reusing `openwith-core`) and a vanilla TypeScript + Vite frontend.

## Develop

```bash
npm install        # once
npm run tauri dev  # hot-reloading dev app
```

## Build

```bash
npm run tauri build   # unsigned .app + .dmg under <repo>/target/release/bundle/
```

The app version comes from the workspace `Cargo.toml` (`tauri.conf.json` deliberately
omits `version` so everything stays lockstep with the CLI).

## Layout

- `src/` — frontend: `main.ts` (render + events), `state.ts` (state, settings persistence), `api.ts` (typed invoke wrappers), `styles.css` (palette from the approved design prototype, light + dark)
- `src-tauri/src/commands.rs` — thin `#[tauri::command]` wrappers over `openwith-core`
