use serde::Serialize;

use openwith_core::history::{self, HistoryEvent};
use openwith_core::{config, launchservices, listing, scanner, uti};

/// History writes are best-effort — never fail the change that triggered them.
fn record_history(event: HistoryEvent) {
    let _ = history::record(event);
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
    pub app_name: String,
    pub bundle_id: String,
    pub previous_app_name: Option<String>,
    pub unchanged: bool,
    pub siblings: Vec<String>,
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
pub struct HistoryEventDto {
    pub kind: String,
    pub key: String,
    pub old: Option<String>,
    pub new: Option<String>,
    pub detail: Option<String>,
    pub timestamp: u64,
    pub source: String,
}

#[tauri::command]
pub fn get_history(limit: usize) -> Result<Vec<HistoryEventDto>, String> {
    let events = history::recent(limit).map_err(|e| e.to_string())?;
    Ok(events
        .into_iter()
        .map(|e| HistoryEventDto {
            kind: e.kind,
            key: e.key,
            old: e.old,
            new: e.new,
            detail: e.detail,
            timestamp: e.timestamp,
            source: e.source,
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
pub fn get_snapshot() -> Result<SnapshotDto, String> {
    let apps = scanner::scan_all_apps().map_err(|e| e.to_string())?;

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
pub fn set_default(ext: String, app: String) -> Result<SetResultDto, String> {
    let apps = scanner::scan_all_apps().map_err(|e| e.to_string())?;
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
            app_name: display_name,
            bundle_id,
            previous_app_name: None,
            unchanged: true,
            siblings: Vec::new(),
        });
    }

    launchservices::set_default(&bundle_id, &uti_str).map_err(|e| e.to_string())?;

    record_history(HistoryEvent {
        kind: "set".into(),
        key: format!(".{ext}"),
        old: previous.clone(),
        new: Some(bundle_id.clone()),
        detail: None,
        timestamp: history::now_secs(),
        source: "gui".into(),
    });

    let previous_app_name = previous.map(|p| scanner::resolve_name(&apps, &p));

    let all_extensions = scanner::all_extensions(&apps);
    let siblings = uti::extensions_sharing_uti(&ext, &uti_str, &all_extensions);

    Ok(SetResultDto {
        key: format!(".{ext}"),
        app_name: display_name,
        bundle_id,
        previous_app_name,
        unchanged: false,
        siblings,
    })
}

#[tauri::command]
pub fn set_scheme_default(scheme: String, app: String) -> Result<SetResultDto, String> {
    let apps = scanner::scan_all_apps().map_err(|e| e.to_string())?;
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
            app_name: display_name,
            bundle_id,
            previous_app_name: None,
            unchanged: true,
            siblings: Vec::new(),
        });
    }

    launchservices::set_default_scheme_handler(&bundle_id, &scheme).map_err(|e| e.to_string())?;

    record_history(HistoryEvent {
        kind: "set_scheme".into(),
        key: format!("{scheme}://"),
        old: previous.clone(),
        new: Some(bundle_id.clone()),
        detail: None,
        timestamp: history::now_secs(),
        source: "gui".into(),
    });

    let previous_app_name = previous.map(|p| scanner::resolve_name(&apps, &p));

    Ok(SetResultDto {
        key: format!("{scheme}://"),
        app_name: display_name,
        bundle_id,
        previous_app_name,
        unchanged: false,
        siblings: Vec::new(),
    })
}

#[tauri::command]
pub fn export_toml(path: Option<String>) -> Result<ExportResultDto, String> {
    let apps = scanner::scan_all_apps().map_err(|e| e.to_string())?;
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
        });
    }

    Ok(ExportResultDto {
        toml: toml_str,
        association_count: cfg.associations.len(),
        scheme_count: cfg.schemes.len(),
    })
}

#[tauri::command]
pub fn import_toml(path: String, dry_run: bool) -> Result<ImportPreviewDto, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let cfg = config::from_toml(&content).map_err(|e| e.to_string())?;

    let apps = scanner::scan_all_apps().map_err(|e| e.to_string())?;
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
