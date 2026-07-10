//! Append-only change log shared by the CLI and GUI, backing the GUI's
//! History panel and (in 0.5.1) `openwith history` / `openwith undo`.
//!
//! Events live in `~/Library/Application Support/openwith/history.json`.
//! Recording is best-effort: a failure to write history must never fail the
//! association change that triggered it, so callers typically ignore errors.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Keep the log bounded; older events fall off the front.
const MAX_EVENTS: usize = 500;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct HistoryEvent {
    /// "set" | "set_scheme" | "export" | "import"
    pub kind: String,
    /// What changed: ".md" / "http://" for sets, the file name for export/import.
    pub key: String,
    /// Previous handler bundle ID (sets only).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub old: Option<String>,
    /// New handler bundle ID (sets only).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub new: Option<String>,
    /// Human summary for export/import events, e.g. "6 applied · 12 skipped".
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub detail: Option<String>,
    /// Unix seconds.
    pub timestamp: u64,
    /// "cli" | "gui" | "import"
    pub source: String,
    /// This change was later reverted via Undo. The ledger keeps the row;
    /// undo-stack views (popover Recent Changes) hide it.
    #[serde(default)]
    pub undone: bool,
    /// This event is itself an Undo (the compensating revert).
    #[serde(default)]
    pub is_undo: bool,
}

impl HistoryEvent {
    /// A live, revertible change: has a previous handler and hasn't been
    /// undone, and isn't itself a revert.
    pub fn undoable(&self) -> bool {
        matches!(self.kind.as_str(), "set" | "set_scheme")
            && self.old.is_some()
            && !self.undone
            && !self.is_undo
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn history_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join("Library/Application Support/openwith")
        .join("history.json"))
}

/// Append an event to the default history file.
pub fn record(event: HistoryEvent) -> Result<()> {
    record_at(&history_path()?, event)
}

/// Newest-first slice of the default history file. Missing file → empty.
pub fn recent(limit: usize) -> Result<Vec<HistoryEvent>> {
    recent_at(&history_path()?, limit)
}

fn load(path: &Path) -> Vec<HistoryEvent> {
    // A missing or corrupt log starts fresh rather than blocking changes.
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn record_at(path: &Path, event: HistoryEvent) -> Result<()> {
    let mut events = load(path);
    events.push(event);
    if events.len() > MAX_EVENTS {
        events.drain(0..events.len() - MAX_EVENTS);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let json = serde_json::to_string_pretty(&events)?;
    std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

pub fn recent_at(path: &Path, limit: usize) -> Result<Vec<HistoryEvent>> {
    let mut events = load(path);
    events.reverse();
    events.truncate(limit);
    Ok(events)
}

/// Flag the newest matching *undoable* event as undone. Matching includes the
/// new-handler value and skips consumed events: second-resolution timestamps
/// can collide (a set and its revert within one second), and flagging the
/// wrong twin would leave the undone event eternally re-undoable.
/// No-op if the event has already fallen off the capped log.
pub fn mark_undone(kind: &str, key: &str, timestamp: u64, new: Option<&str>) -> Result<()> {
    mark_undone_at(&history_path()?, kind, key, timestamp, new)
}

pub fn mark_undone_at(
    path: &Path,
    kind: &str,
    key: &str,
    timestamp: u64,
    new: Option<&str>,
) -> Result<()> {
    let mut events = load(path);
    if let Some(event) = events.iter_mut().rev().find(|e| {
        e.kind == kind
            && e.key == key
            && e.timestamp == timestamp
            && e.new.as_deref() == new
            && e.undoable()
    }) {
        event.undone = true;
        let json = serde_json::to_string_pretty(&events)?;
        std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "openwith-history-test-{name}-{}.json",
            std::process::id()
        ))
    }

    fn event(kind: &str, key: &str, ts: u64) -> HistoryEvent {
        HistoryEvent {
            kind: kind.into(),
            key: key.into(),
            timestamp: ts,
            source: "gui".into(),
            ..Default::default()
        }
    }

    #[test]
    fn record_and_recent_round_trip() {
        let path = temp_log("roundtrip");
        let _ = std::fs::remove_file(&path);

        record_at(&path, event("set", ".md", 1)).unwrap();
        record_at(&path, event("export", "openwith.toml", 2)).unwrap();

        let events = recent_at(&path, 10).unwrap();
        assert_eq!(events.len(), 2);
        // newest first
        assert_eq!(events[0].kind, "export");
        assert_eq!(events[1].key, ".md");

        let one = recent_at(&path, 1).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].kind, "export");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn missing_and_corrupt_files_start_fresh() {
        let path = temp_log("corrupt");
        let _ = std::fs::remove_file(&path);
        assert!(recent_at(&path, 5).unwrap().is_empty());

        std::fs::write(&path, "not json").unwrap();
        assert!(recent_at(&path, 5).unwrap().is_empty());
        record_at(&path, event("import", "a.toml", 3)).unwrap();
        assert_eq!(recent_at(&path, 5).unwrap().len(), 1);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn mark_undone_flags_the_matching_event() {
        let path = temp_log("undone");
        let _ = std::fs::remove_file(&path);

        let mut set = event("set", ".md", 10);
        set.old = Some("a".into());
        set.new = Some("b".into());
        record_at(&path, set).unwrap();
        record_at(&path, event("export", "x.toml", 11)).unwrap();

        assert!(recent_at(&path, 5).unwrap()[1].undoable());
        mark_undone_at(&path, "set", ".md", 10, Some("b")).unwrap();

        let events = recent_at(&path, 5).unwrap();
        assert!(events[1].undone);
        assert!(!events[1].undoable());
        // unknown event → silent no-op
        mark_undone_at(&path, "set", ".zzz", 99, None).unwrap();

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn mark_undone_skips_consumed_twins_on_timestamp_collision() {
        let path = temp_log("collision");
        let _ = std::fs::remove_file(&path);

        // A set and another event in the same second, the newer one already
        // undone — marking must flag the still-active older twin.
        let mut a = event("set", ".md", 10);
        a.old = Some("typora".into());
        a.new = Some("textedit".into());
        let mut b = event("set", ".md", 10);
        b.old = Some("textedit".into());
        b.new = Some("typora".into());
        b.undone = true;
        record_at(&path, a).unwrap();
        record_at(&path, b).unwrap();

        mark_undone_at(&path, "set", ".md", 10, Some("textedit")).unwrap();

        let events = recent_at(&path, 5).unwrap();
        assert!(events.iter().all(|e| e.undone));
        assert!(events.iter().all(|e| !e.undoable()));

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn log_is_capped() {
        let path = temp_log("cap");
        let _ = std::fs::remove_file(&path);
        for i in 0..(MAX_EVENTS as u64 + 20) {
            record_at(&path, event("set", ".md", i)).unwrap();
        }
        let events = recent_at(&path, MAX_EVENTS + 50).unwrap();
        assert_eq!(events.len(), MAX_EVENTS);
        assert_eq!(events[0].timestamp, MAX_EVENTS as u64 + 19);

        std::fs::remove_file(&path).unwrap();
    }
}
