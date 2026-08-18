use anyhow::Result;

use openwith_core::history::{self, HistoryEvent};
use openwith_core::scanner;

/// `window_days` bounds how far back events are shown; `None` is `--all`.
pub fn run(limit: usize, window_days: Option<u64>, json: bool) -> Result<()> {
    let events = history::recent_within(limit, window_days)?;

    if json {
        let out: Vec<serde_json::Value> = events
            .iter()
            .map(|e| {
                serde_json::json!({
                    "kind": e.kind,
                    "key": e.key,
                    "old": e.old,
                    "new": e.new,
                    "detail": e.detail,
                    "timestamp": e.timestamp,
                    "source": e.source,
                    "undone": e.undone,
                    "is_undo": e.is_undo,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if events.is_empty() {
        match window_days {
            // The ledger may still hold older events — say so rather than
            // implying nothing was ever recorded.
            Some(days) => println!(
                "No changes in the last {days} day{} — use --all to see everything retained.",
                if days == 1 { "" } else { "s" }
            ),
            None => println!("No history yet — changes made by the CLI or GUI will appear here."),
        }
        return Ok(());
    }

    // Resolve bundle IDs to app names for display.
    eprintln!("Scanning applications...");
    let apps = scanner::scan_all_apps()?;
    let name_of = |bid: &Option<String>| -> Option<String> {
        bid.as_ref().map(|b| scanner::resolve_name(&apps, b))
    };

    for event in &events {
        let when = relative_time(event.timestamp);
        let line = describe(event, name_of(&event.old), name_of(&event.new));
        println!("  {:>12}  {}  [{}]", when, line, event.source);
    }

    Ok(())
}

fn describe(event: &HistoryEvent, old_name: Option<String>, new_name: Option<String>) -> String {
    match event.kind.as_str() {
        "set" | "set_scheme" => {
            let new = new_name.unwrap_or_else(|| "?".into());
            let was = old_name.map(|o| format!(" (was {o})")).unwrap_or_default();
            let verb = if event.is_undo { "undid: set" } else { "set" };
            let reverted = if event.undone { " · reverted" } else { "" };
            format!("{} {} → {}{}{}", verb, event.key, new, was, reverted)
        }
        "export" => format!(
            "exported {}{}",
            event.key,
            event
                .detail
                .as_ref()
                .map(|d| format!(" — {d}"))
                .unwrap_or_default()
        ),
        "import" => format!(
            "imported {}{}",
            event.key,
            event
                .detail
                .as_ref()
                .map(|d| format!(" — {d}"))
                .unwrap_or_default()
        ),
        other => format!("{} {}", other, event.key),
    }
}

fn relative_time(timestamp: u64) -> String {
    let now = history::now_secs();
    let ago = now.saturating_sub(timestamp);
    match ago {
        0..=59 => "just now".into(),
        60..=3599 => format!("{}m ago", ago / 60),
        3600..=86_399 => format!("{}h ago", ago / 3600),
        86_400..=604_799 => format!("{}d ago", ago / 86_400),
        _ => month_day(timestamp),
    }
}

/// "Jun 30"-style date from unix seconds, without pulling in a date crate.
/// Days-to-civil conversion per Howard Hinnant's algorithm.
fn month_day(timestamp: u64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = (timestamp / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let this_year = {
        let zy = (history::now_secs() / 86_400) as i64 + 719_468;
        let e = zy.div_euclid(146_097);
        let d = zy - e * 146_097;
        let y = (d - d / 1460 + d / 36_524 - d / 146_096) / 365;
        let dy = d - (365 * y + y / 4 - y / 100);
        let m = (5 * dy + 2) / 153;
        let m = if m < 10 { m + 3 } else { m - 9 };
        y + e * 400 + i64::from(m <= 2)
    };
    if year == this_year {
        format!("{} {}", MONTHS[(month - 1) as usize], day)
    } else {
        format!("{} {} {}", MONTHS[(month - 1) as usize], day, year)
    }
}

#[cfg(test)]
mod tests {
    use super::month_day;

    #[test]
    fn month_day_formats_known_dates() {
        // 2026-06-30 12:00:00 UTC
        assert!(month_day(1_782_820_800).starts_with("Jun 30"));
        // 2000-01-01 00:00:00 UTC — includes year since it's not current
        assert_eq!(month_day(946_684_800), "Jan 1 2000");
    }
}
