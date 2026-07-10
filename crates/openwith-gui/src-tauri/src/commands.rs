use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use openwith_core::history::{self, HistoryEvent};
use openwith_core::types::AppInfo;
use openwith_core::{config, launchservices, listing, scanner, uti};

use crate::tray;

/// History writes are best-effort — never fail the change that triggered them.
fn record_history(event: HistoryEvent) {
    let _ = history::record(event);
}

/// Scanned apps, shared between the main window and the menu-bar popover so
/// popover lookups don't pay the multi-second scan.
#[derive(Default)]
pub struct AppsCache(Mutex<Option<Arc<Vec<AppInfo>>>>);

fn cached_apps(cache: &State<'_, AppsCache>) -> Result<Arc<Vec<AppInfo>>, String> {
    let mut slot = cache.0.lock().expect("apps cache poisoned");
    if let Some(apps) = slot.as_ref() {
        return Ok(Arc::clone(apps));
    }
    let apps = Arc::new(scanner::scan_all_apps().map_err(|e| e.to_string())?);
    *slot = Some(Arc::clone(&apps));
    Ok(apps)
}

fn refresh_apps(cache: &State<'_, AppsCache>) -> Result<Arc<Vec<AppInfo>>, String> {
    let apps = Arc::new(scanner::scan_all_apps().map_err(|e| e.to_string())?);
    *cache.0.lock().expect("apps cache poisoned") = Some(Arc::clone(&apps));
    Ok(apps)
}

#[derive(Serialize)]
pub struct AppDto {
    pub name: String,
    pub bundle_id: String,
    pub extensions: Vec<String>,
    pub url_schemes: Vec<String>,
}

#[derive(Serialize)]
pub struct AssociationDto {
    pub ext: String,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    pub conflict: bool,
    pub siblings: Vec<String>,
}

#[derive(Serialize)]
pub struct SchemeDto {
    pub scheme: String,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

#[derive(Serialize)]
pub struct SnapshotDto {
    pub apps: Vec<AppDto>,
    pub associations: Vec<AssociationDto>,
    pub schemes: Vec<SchemeDto>,
}

#[derive(Serialize)]
pub struct SetResultDto {
    pub key: String,
    pub kind: String,
    pub app_name: String,
    pub bundle_id: String,
    pub previous_app_name: Option<String>,
    pub unchanged: bool,
    pub siblings: Vec<String>,
    /// Timestamp of the recorded history event; lets the frontend undo this
    /// exact change (0 when nothing was recorded).
    pub timestamp: u64,
}

#[derive(Serialize)]
pub struct ExportResultDto {
    pub toml: String,
    pub association_count: usize,
    pub scheme_count: usize,
}

#[derive(Serialize)]
pub struct ImportPreviewDto {
    pub applied: Vec<ImportAppliedDto>,
    pub unchanged: usize,
    pub skipped: Vec<ImportSkippedDto>,
}

#[derive(Serialize)]
pub struct ImportAppliedDto {
    pub key: String,
    pub app_name: String,
    pub previous_app_name: Option<String>,
}

#[derive(Serialize)]
pub struct ImportSkippedDto {
    pub key: String,
    pub app_name: String,
    pub reason: String,
}

#[derive(Serialize)]
pub struct ExtMatchDto {
    pub ext: String,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

#[derive(Serialize)]
pub struct PickerAppDto {
    pub name: String,
    pub bundle_id: String,
    pub current: bool,
}

#[derive(Serialize)]
pub struct RecentChangeDto {
    pub kind: String,
    pub key: String,
    pub app_name: String,
    pub old_bundle_id: Option<String>,
    pub timestamp: u64,
}

/// Prefix-match known extensions for the menu-bar popover, newest defaults
/// resolved live (cheap: one Launch Services query per shown row).
#[tauri::command]
pub fn search_extensions(
    query: String,
    cache: State<'_, AppsCache>,
) -> Result<Vec<ExtMatchDto>, String> {
    let q = query.trim().trim_start_matches('.').to_lowercase();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let apps = cached_apps(&cache)?;
    let mut exts = scanner::all_extensions(&apps);
    exts.retain(|e| e.starts_with(&q));
    exts.sort();
    exts.truncate(3);

    Ok(exts
        .into_iter()
        .map(|ext| {
            let bundle_id = launchservices::query_default_bundle_id(&ext).ok().flatten();
            let app_name = bundle_id.as_ref().map(|b| scanner::resolve_name(&apps, b));
            ExtMatchDto {
                ext,
                app_name,
                bundle_id,
            }
        })
        .collect())
}

/// Apps offered in the popover's picker for one extension: declared
/// supporters, or every app when nothing declares it.
#[tauri::command]
pub fn get_ext_picker(
    ext: String,
    cache: State<'_, AppsCache>,
) -> Result<Vec<PickerAppDto>, String> {
    let ext = ext.trim_start_matches('.').to_lowercase();
    let apps = cached_apps(&cache)?;
    let current = launchservices::query_default_bundle_id(&ext).ok().flatten();

    let mut source: Vec<&AppInfo> = apps
        .iter()
        .filter(|a| a.extensions.contains(&ext))
        .collect();
    if source.is_empty() {
        source = apps.iter().collect();
    }
    source.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(source
        .into_iter()
        .map(|a| PickerAppDto {
            name: a.name.clone(),
            bundle_id: a.bundle_id.clone(),
            current: current
                .as_deref()
                .is_some_and(|c| c.eq_ignore_ascii_case(&a.bundle_id)),
        })
        .collect())
}

/// Recent set events for the popover's Recent Changes list, names resolved.
/// Undo-stack view: undone changes and the reverts themselves are hidden.
#[tauri::command]
pub fn get_recent_changes(
    limit: usize,
    cache: State<'_, AppsCache>,
) -> Result<Vec<RecentChangeDto>, String> {
    let apps = cached_apps(&cache)?;
    let events = history::recent(100).map_err(|e| e.to_string())?;
    Ok(events
        .into_iter()
        .filter(|e| matches!(e.kind.as_str(), "set" | "set_scheme") && !e.undone && !e.is_undo)
        .take(limit)
        .map(|e| RecentChangeDto {
            kind: e.kind,
            key: e.key,
            app_name: e
                .new
                .as_ref()
                .map(|b| scanner::resolve_name(&apps, b))
                .unwrap_or_else(|| "?".into()),
            old_bundle_id: e.old,
            timestamp: e.timestamp,
        })
        .collect())
}

/// Undo one recorded change: restore the previous handler, mark the original
/// event consumed, and record the revert as an is_undo event.
#[tauri::command]
pub fn undo_change(
    kind: String,
    key: String,
    timestamp: u64,
    cache: State<'_, AppsCache>,
) -> Result<SetResultDto, String> {
    let apps = cached_apps(&cache)?;
    let events = history::recent(500).map_err(|e| e.to_string())?;
    let event = events
        .iter()
        .find(|e| e.kind == kind && e.key == key && e.timestamp == timestamp && e.undoable())
        .ok_or("that change is no longer undoable")?;
    let old = event.old.clone().expect("undoable implies old");
    let new = event.new.clone().unwrap_or_default();

    let siblings = if kind == "set_scheme" {
        let scheme = key.trim_end_matches("://");
        launchservices::set_default_scheme_handler(&old, scheme).map_err(|e| e.to_string())?;
        Vec::new()
    } else {
        let ext = key.trim_start_matches('.');
        let uti_str = uti::uti_for_extension(ext).map_err(|e| e.to_string())?;
        launchservices::set_default(&old, &uti_str).map_err(|e| e.to_string())?;
        uti::extensions_sharing_uti(ext, &uti_str, &scanner::all_extensions(&apps))
    };

    let _ = history::mark_undone(&kind, &key, timestamp, event.new.as_deref());
    let now = history::now_secs();
    record_history(HistoryEvent {
        kind: kind.clone(),
        key: key.clone(),
        old: event.new.clone(),
        new: Some(old.clone()),
        timestamp: now,
        source: "gui".into(),
        is_undo: true,
        ..Default::default()
    });

    Ok(SetResultDto {
        key,
        kind,
        app_name: scanner::resolve_name(&apps, &old),
        bundle_id: old,
        previous_app_name: Some(scanner::resolve_name(&apps, &new)),
        unchanged: false,
        siblings,
        timestamp: now,
    })
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Some(popover) = app.get_webview_window("menubar") {
        let _ = popover.hide();
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn set_tray_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    tray::set_enabled(&app, enabled).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct HistoryEventDto {
    pub kind: String,
    pub key: String,
    pub old_name: Option<String>,
    pub new_name: Option<String>,
    pub detail: Option<String>,
    pub timestamp: u64,
    pub source: String,
    pub undone: bool,
    pub is_undo: bool,
}

/// Full ledger for the Profiles HISTORY panel, bundle IDs resolved to names.
#[tauri::command]
pub fn get_history(
    limit: usize,
    cache: State<'_, AppsCache>,
) -> Result<Vec<HistoryEventDto>, String> {
    let apps = cached_apps(&cache)?;
    let events = history::recent(limit).map_err(|e| e.to_string())?;
    Ok(events
        .into_iter()
        .map(|e| HistoryEventDto {
            kind: e.kind,
            key: e.key,
            old_name: e.old.as_ref().map(|b| scanner::resolve_name(&apps, b)),
            new_name: e.new.as_ref().map(|b| scanner::resolve_name(&apps, b)),
            detail: e.detail,
            timestamp: e.timestamp,
            source: e.source,
            undone: e.undone,
            is_undo: e.is_undo,
        })
        .collect())
}

/// Version string reported by an installed `openwith` CLI, if any.
/// Apps launched from Finder don't inherit the shell PATH, so after a plain
/// PATH lookup we probe the Homebrew install locations directly.
#[tauri::command]
pub fn detect_cli() -> Option<String> {
    const CANDIDATES: [&str; 3] = [
        "openwith",
        "/opt/homebrew/bin/openwith",
        "/usr/local/bin/openwith",
    ];
    CANDIDATES.iter().find_map(|cmd| {
        let output = std::process::Command::new(cmd)
            .arg("--version")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() { None } else { Some(text) }
    })
}

/// Relaunch Finder so it drops stale icon caches after a default changes.
/// Finder restarts itself automatically after `killall`.
#[tauri::command]
pub fn relaunch_finder() -> Result<(), String> {
    std::process::Command::new("killall")
        .arg("Finder")
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_snapshot(cache: State<'_, AppsCache>) -> Result<SnapshotDto, String> {
    let apps = refresh_apps(&cache)?;

    let app_dtos = apps
        .iter()
        .map(|a| AppDto {
            name: a.name.clone(),
            bundle_id: a.bundle_id.clone(),
            extensions: a.extensions.clone(),
            url_schemes: a.url_schemes.clone(),
        })
        .collect();

    let all_extensions = scanner::all_extensions(&apps);
    let associations = listing::query_all(&apps, &|| {})
        .into_iter()
        .map(|assoc| {
            let siblings = uti::uti_for_extension(&assoc.ext)
                .map(|uti_str| uti::extensions_sharing_uti(&assoc.ext, &uti_str, &all_extensions))
                .unwrap_or_default();
            AssociationDto {
                ext: assoc.ext,
                app_name: assoc.app_name,
                bundle_id: assoc.bundle_id,
                conflict: !siblings.is_empty(),
                siblings,
            }
        })
        .collect();

    let schemes = listing::query_all_schemes(&apps)
        .into_iter()
        .map(|s| SchemeDto {
            scheme: s.scheme,
            app_name: s.app_name,
            bundle_id: s.bundle_id,
        })
        .collect();

    Ok(SnapshotDto {
        apps: app_dtos,
        associations,
        schemes,
    })
}

#[tauri::command]
pub fn set_default(
    ext: String,
    app: String,
    cache: State<'_, AppsCache>,
) -> Result<SetResultDto, String> {
    let apps = cached_apps(&cache)?;
    let (bundle_id, display_name) =
        scanner::resolve_app_or_bundle_id(&apps, &app).map_err(|e| e.to_string())?;

    let ext = ext.trim_start_matches('.').to_string();
    let uti_str = uti::uti_for_extension(&ext).map_err(|e| e.to_string())?;

    let previous = launchservices::query_default_bundle_id(&ext).map_err(|e| e.to_string())?;
    if previous
        .as_deref()
        .is_some_and(|p| p.eq_ignore_ascii_case(&bundle_id))
    {
        return Ok(SetResultDto {
            key: format!(".{ext}"),
            kind: "set".into(),
            app_name: display_name,
            bundle_id,
            previous_app_name: None,
            unchanged: true,
            siblings: Vec::new(),
            timestamp: 0,
        });
    }

    launchservices::set_default(&bundle_id, &uti_str).map_err(|e| e.to_string())?;

    let timestamp = history::now_secs();
    record_history(HistoryEvent {
        kind: "set".into(),
        key: format!(".{ext}"),
        old: previous.clone(),
        new: Some(bundle_id.clone()),
        timestamp,
        source: "gui".into(),
        ..Default::default()
    });

    let previous_app_name = previous.map(|p| scanner::resolve_name(&apps, &p));

    let all_extensions = scanner::all_extensions(&apps);
    let siblings = uti::extensions_sharing_uti(&ext, &uti_str, &all_extensions);

    Ok(SetResultDto {
        key: format!(".{ext}"),
        kind: "set".into(),
        app_name: display_name,
        bundle_id,
        previous_app_name,
        unchanged: false,
        siblings,
        timestamp,
    })
}

#[tauri::command]
pub fn set_scheme_default(
    scheme: String,
    app: String,
    cache: State<'_, AppsCache>,
) -> Result<SetResultDto, String> {
    let apps = cached_apps(&cache)?;
    let (bundle_id, display_name) =
        scanner::resolve_app_or_bundle_id(&apps, &app).map_err(|e| e.to_string())?;

    let scheme = scheme
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(':')
        .to_lowercase();

    let previous =
        launchservices::query_default_scheme_handler(&scheme).map_err(|e| e.to_string())?;
    if previous
        .as_deref()
        .is_some_and(|p| p.eq_ignore_ascii_case(&bundle_id))
    {
        return Ok(SetResultDto {
            key: format!("{scheme}://"),
            kind: "set_scheme".into(),
            app_name: display_name,
            bundle_id,
            previous_app_name: None,
            unchanged: true,
            siblings: Vec::new(),
            timestamp: 0,
        });
    }

    launchservices::set_default_scheme_handler(&bundle_id, &scheme).map_err(|e| e.to_string())?;

    let timestamp = history::now_secs();
    record_history(HistoryEvent {
        kind: "set_scheme".into(),
        key: format!("{scheme}://"),
        old: previous.clone(),
        new: Some(bundle_id.clone()),
        timestamp,
        source: "gui".into(),
        ..Default::default()
    });

    let previous_app_name = previous.map(|p| scanner::resolve_name(&apps, &p));

    Ok(SetResultDto {
        key: format!("{scheme}://"),
        kind: "set_scheme".into(),
        app_name: display_name,
        bundle_id,
        previous_app_name,
        unchanged: false,
        siblings: Vec::new(),
        timestamp,
    })
}

#[tauri::command]
pub fn export_toml(
    path: Option<String>,
    cache: State<'_, AppsCache>,
) -> Result<ExportResultDto, String> {
    let apps = cached_apps(&cache)?;
    let (cfg, display_names) = config::export_associations(&apps).map_err(|e| e.to_string())?;
    let toml_str = config::to_toml(&cfg, &display_names).map_err(|e| e.to_string())?;

    if let Some(path) = &path {
        std::fs::write(path, &toml_str).map_err(|e| e.to_string())?;
        let file_name = std::path::Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        record_history(HistoryEvent {
            kind: "export".into(),
            key: file_name,
            old: None,
            new: None,
            detail: Some(format!(
                "{} extensions · {} schemes",
                cfg.associations.len(),
                cfg.schemes.len()
            )),
            timestamp: history::now_secs(),
            source: "gui".into(),
            ..Default::default()
        });
    }

    Ok(ExportResultDto {
        toml: toml_str,
        association_count: cfg.associations.len(),
        scheme_count: cfg.schemes.len(),
    })
}

#[tauri::command]
pub fn import_toml(
    path: String,
    dry_run: bool,
    cache: State<'_, AppsCache>,
) -> Result<ImportPreviewDto, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let cfg = config::from_toml(&content).map_err(|e| e.to_string())?;

    let apps = cached_apps(&cache)?;
    let result = config::import_associations(&cfg, &apps, dry_run);

    if !dry_run {
        let file_name = std::path::Path::new(&path)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        record_history(HistoryEvent {
            kind: "import".into(),
            key: file_name,
            old: None,
            new: None,
            detail: Some(format!(
                "{} applied · {} skipped",
                result.applied.len(),
                result.unchanged.len() + result.skipped.len()
            )),
            timestamp: history::now_secs(),
            source: "gui".into(),
            ..Default::default()
        });
    }

    let applied = result
        .applied
        .into_iter()
        .map(|(key, app_name, previous_app_name)| ImportAppliedDto {
            key,
            app_name,
            previous_app_name,
        })
        .collect();

    let skipped = result
        .skipped
        .into_iter()
        .map(|(key, app_name, reason)| ImportSkippedDto {
            key,
            app_name,
            reason,
        })
        .collect();

    Ok(ImportPreviewDto {
        applied,
        unchanged: result.unchanged.len(),
        skipped,
    })
}
