use anyhow::{Result, bail};

use openwith_core::history::{self, HistoryEvent};
use openwith_core::{launchservices, scanner, uti};

/// Revert the most recent recorded default change (from CLI, GUI, or import).
pub fn run(force: bool) -> Result<()> {
    let events = history::recent(100)?;
    let Some(event) = events
        .iter()
        .find(|e| matches!(e.kind.as_str(), "set" | "set_scheme") && e.old.is_some())
    else {
        println!("Nothing to undo — no recorded change has a previous default.");
        return Ok(());
    };

    let old = event.old.as_deref().expect("checked above");
    let new = event.new.as_deref().unwrap_or_default();

    // If the default has drifted since the event was recorded, a blind revert
    // would clobber a change the user made elsewhere.
    let current = current_handler(event)?;
    if !force
        && current
            .as_deref()
            .is_none_or(|c| !c.eq_ignore_ascii_case(new))
    {
        bail!(
            "the default for {} has changed since that event (now {}, event set {}); \
             re-run with --force to revert to {} anyway",
            event.key,
            current.as_deref().unwrap_or("unset"),
            new,
            old
        );
    }

    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;

    apply_revert(event, old)?;

    let _ = history::record(HistoryEvent {
        kind: event.kind.clone(),
        key: event.key.clone(),
        old: event.new.clone(),
        new: Some(old.to_string()),
        detail: None,
        timestamp: history::now_secs(),
        source: "cli".into(),
    });

    println!(
        "Reverted {} → {} (was {})",
        event.key,
        scanner::resolve_name(&apps, old),
        scanner::resolve_name(&apps, new)
    );

    if event.kind == "set" {
        let ext = event.key.trim_start_matches('.');
        if let Ok(uti_str) = uti::uti_for_extension(ext) {
            let siblings =
                uti::extensions_sharing_uti(ext, &uti_str, &scanner::all_extensions(&apps));
            if let Some(note) = uti::shared_uti_note(ext, &uti_str, &siblings) {
                eprintln!("note: {}", note);
            }
        }
    }

    Ok(())
}

fn current_handler(event: &HistoryEvent) -> Result<Option<String>> {
    if event.kind == "set_scheme" {
        let scheme = event.key.trim_end_matches("://");
        Ok(launchservices::query_default_scheme_handler(scheme)?)
    } else {
        let ext = event.key.trim_start_matches('.');
        Ok(launchservices::query_default_bundle_id(ext)?)
    }
}

fn apply_revert(event: &HistoryEvent, old: &str) -> Result<()> {
    if event.kind == "set_scheme" {
        let scheme = event.key.trim_end_matches("://");
        launchservices::set_default_scheme_handler(old, scheme)?;
    } else {
        let ext = event.key.trim_start_matches('.');
        let uti_str = uti::uti_for_extension(ext)?;
        launchservices::set_default(old, &uti_str)?;
    }
    Ok(())
}
