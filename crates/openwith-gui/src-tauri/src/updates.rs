//! The release check: one answer for every surface.
//!
//! The check used to live in the main window's script, so its answer stayed
//! there: the popover never heard about an update, and nothing outside
//! Settings could say one existed. It runs here now, and both windows read
//! the same remembered answer (persisted in `update.json`, so the notice
//! survives a relaunch) and hear about changes through `update-status`.
//!
//! The arithmetic is in `release.rs`; this is the network and the state.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::release;

const RELEASES_URL: &str = "https://api.github.com/repos/ColeMei/openwith/releases?per_page=15";

/// What `update.json` keeps between launches.
#[derive(Default, Serialize, Deserialize)]
struct Saved {
    /// The newer version the last check found, and the channel it was found
    /// on — a Beta rc is no news to someone who has since switched to Stable.
    remembered: Option<String>,
    channel: Option<String>,
    /// Unix seconds of the last check that got an answer.
    checked_at: Option<u64>,
}

#[derive(Default)]
struct Inner {
    saved: Saved,
    /// Unix seconds of the last request, answered or not; in memory only.
    last_attempt: Option<u64>,
    /// Why the last check failed, if it did.
    error: Option<String>,
}

pub struct Updates {
    inner: Mutex<Inner>,
    checking: AtomicBool,
    path: Option<PathBuf>,
}

impl Updates {
    pub fn load(app: &AppHandle) -> Self {
        let path = app
            .path()
            .app_data_dir()
            .ok()
            .map(|dir| dir.join("update.json"));
        let saved = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(Inner {
                saved,
                ..Inner::default()
            }),
            checking: AtomicBool::new(false),
            path,
        }
    }

    /// Best-effort: an unwritable file only costs the notice a relaunch.
    fn save(&self, saved: &Saved) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(saved) {
            let _ = std::fs::write(path, json);
        }
    }
}

#[derive(Clone, Serialize)]
pub struct UpdateStatusDto {
    pub running: String,
    /// The newer version waiting on this channel, without the `v`.
    pub pending: Option<String>,
    /// Unix seconds of the last check that got an answer.
    pub checked_at: Option<u64>,
    pub checking: bool,
    pub error: Option<String>,
    /// Set on the event a scheduled check sends when it finds an update: the
    /// once-a-day cue for the toast. A manual check's answer is already on
    /// screen, so it never announces.
    pub announce: bool,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn running_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn normalize_channel(channel: &str) -> &'static str {
    if channel == "beta" { "beta" } else { "stable" }
}

fn status_dto(app: &AppHandle, channel: &str, announce: bool) -> UpdateStatusDto {
    let updates = app.state::<Updates>();
    let inner = updates.inner.lock().expect("update state poisoned");
    let running = running_version(app);
    let remembered = inner
        .saved
        .remembered
        .as_deref()
        .filter(|_| inner.saved.channel.as_deref() == Some(channel));
    let pending = release::pending(remembered, &running);
    UpdateStatusDto {
        announce: announce && pending.is_some(),
        running,
        pending,
        checked_at: inner.saved.checked_at,
        checking: updates.checking.load(Ordering::Relaxed),
        error: inner.error.clone(),
    }
}

async fn fetch_newest_tag(app: &AppHandle, beta: bool) -> Result<Option<String>, String> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        // Fails only if another thread installed one first, which is fine.
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    let client = reqwest::Client::builder()
        // GitHub refuses API requests without a User-Agent.
        .user_agent(format!("OpenWith/{}", running_version(app)))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| "couldn't reach GitHub".to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub returned {}", response.status().as_u16()));
    }
    let releases: Vec<release::Release> = response
        .json()
        .await
        .map_err(|_| "unexpected reply from GitHub".to_string())?;
    Ok(release::newest_offered(&releases, beta).map(|r| r.tag_name.clone()))
}

/// Whether this build registered the updater, i.e. ships a real public key.
pub struct UpdaterReady(pub bool);

/// Read `plugins.updater.pubkey` from the bundled config.
pub fn updater_configured(config: &tauri::Config) -> bool {
    config
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("pubkey"))
        .and_then(|key| key.as_str())
        .is_some_and(release::is_real_pubkey)
}

/// Whether "Update to X…" can install in place, or must open the page.
#[tauri::command]
pub fn updater_available(ready: tauri::State<'_, UpdaterReady>) -> bool {
    ready.0
}

/// Open the release page for `version` in the browser: the way to get an
/// update by hand.
#[tauri::command]
pub fn open_release_page(app: AppHandle, version: String) -> Result<(), String> {
    app.opener()
        .open_url(release::release_page(&version), None::<&str>)
        .map_err(|e| e.to_string())
}

/// The current answer, without touching the network.
#[tauri::command]
pub fn get_update_status(app: AppHandle, channel: String) -> UpdateStatusDto {
    status_dto(&app, normalize_channel(&channel), false)
}

/// Ask GitHub for the newest release on `channel`. A manual check (Check
/// Now, switching channel) always asks; a scheduled one — launch, focus, a
/// popover open — asks only when `release::should_check` says it's due,
/// which `enabled = false` never is. Every answer is broadcast to both
/// windows as `update-status`.
#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    channel: String,
    manual: bool,
    enabled: bool,
) -> UpdateStatusDto {
    let channel = normalize_channel(&channel);
    let updates = app.state::<Updates>();
    {
        let inner = updates.inner.lock().expect("update state poisoned");
        let due = release::should_check(enabled, inner.saved.checked_at, inner.last_attempt, now());
        if !manual && !due {
            drop(inner);
            return status_dto(&app, channel, false);
        }
    }
    // One request at a time; a caller arriving mid-check gets the event.
    if updates.checking.swap(true, Ordering::Relaxed) {
        return status_dto(&app, channel, false);
    }
    updates
        .inner
        .lock()
        .expect("update state poisoned")
        .last_attempt = Some(now());

    let result = fetch_newest_tag(&app, channel == "beta").await;

    {
        let mut inner = updates.inner.lock().expect("update state poisoned");
        match result {
            Ok(tag) => {
                let status = match tag {
                    Some(tag) => release::status(&running_version(&app), &tag),
                    // Nothing published on this channel yet: nothing newer.
                    None => release::Status::Current,
                };
                let previously = inner
                    .saved
                    .remembered
                    .take()
                    .filter(|_| inner.saved.channel.as_deref() == Some(channel));
                inner.saved.remembered = release::remembered(&status, previously);
                inner.saved.channel = Some(channel.to_string());
                inner.saved.checked_at = Some(now());
                inner.error = None;
                updates.save(&inner.saved);
            }
            Err(error) => inner.error = Some(error),
        }
    }
    updates.checking.store(false, Ordering::Relaxed);

    let dto = status_dto(&app, channel, !manual);
    let _ = app.emit("update-status", dto.clone());
    dto
}
