//! Comparing the running version against a published one.
//!
//! Pure arithmetic on version strings, kept apart from the commands so it can
//! be unit-tested: this is the part that can be wrong quietly. A comparison
//! that says "up to date" when it isn't is worse than no check at all, and
//! the old exact-string test offered a Stable-channel rc build the older
//! release it was ahead of.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "version", rename_all = "lowercase")]
pub enum Status {
    /// A newer release exists; carries its version without the `v`.
    Behind(String),
    /// Nothing newer. Also what a *newer* local build reports: a build ahead
    /// of the last tag is not behind, and offering it a downgrade is nonsense.
    Current,
    /// One of the two could not be read as a version at all.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Identifier {
    Numeric(u64),
    Alpha(String),
}

impl Ord for Identifier {
    // SemVer §11: numeric identifiers sort numerically and below any
    // alphanumeric one; alphanumerics compare in ASCII order.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Numeric(a), Self::Numeric(b)) => a.cmp(b),
            (Self::Numeric(_), Self::Alpha(_)) => Ordering::Less,
            (Self::Alpha(_), Self::Numeric(_)) => Ordering::Greater,
            (Self::Alpha(a), Self::Alpha(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for Identifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    numbers: Vec<u64>,
    pre: Vec<Identifier>,
}

impl Version {
    /// `v0.7.0`, `0.7.0` and `0.7` all read the same. A `-rc.1` suffix is a
    /// pre-release and `+build` metadata is ignored, as in SemVer. Anything
    /// else is refused rather than guessed at.
    ///
    /// Unlike Pane, which drops the suffix, a pre-release ranks *below* its
    /// release: someone running `0.7.0-rc.1` must still hear about `0.7.0`.
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        let body = trimmed.strip_prefix('v').unwrap_or(trimmed);
        let body = body.split_once('+').map_or(body, |(core, _)| core);
        let (core, pre) = match body.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (body, None),
        };

        let numbers = core
            .split('.')
            .map(|part| {
                if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                part.parse().ok()
            })
            .collect::<Option<Vec<u64>>>()?;

        let pre = match pre {
            None => Vec::new(),
            Some(pre) => pre
                .split('.')
                .map(|part| {
                    if part.is_empty()
                        || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        None
                    } else if part.bytes().all(|b| b.is_ascii_digit()) {
                        part.parse().ok().map(Identifier::Numeric)
                    } else {
                        Some(Identifier::Alpha(part.to_string()))
                    }
                })
                .collect::<Option<Vec<_>>>()?,
        };

        Some(Self { numbers, pre })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // Component by component, the shorter padded with zeros, so `0.7` is
        // `0.7.0` and `0.10.0` is newer than `0.9.0` (which a string
        // comparison gets backwards).
        let len = self.numbers.len().max(other.numbers.len());
        for i in 0..len {
            let a = self.numbers.get(i).copied().unwrap_or(0);
            let b = other.numbers.get(i).copied().unwrap_or(0);
            match a.cmp(&b) {
                Ordering::Equal => {}
                unequal => return unequal,
            }
        }
        match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            // Identifier by identifier; a longer list wins a shared prefix.
            (false, false) => self.pre.cmp(&other.pre),
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Where `running` stands against the published `latest`.
pub fn status(running: &str, latest: &str) -> Status {
    let (Some(mine), Some(theirs)) = (Version::parse(running), Version::parse(latest)) else {
        return Status::Unknown;
    };
    if mine < theirs {
        let latest = latest.trim();
        Status::Behind(latest.strip_prefix('v').unwrap_or(latest).to_string())
    } else {
        Status::Current
    }
}

// ---------- when to look, and what a look leaves behind ----------

/// How long between scheduled checks. OpenWith sits in the menu bar for
/// weeks, so "once per launch" would mean once per reboot on some machines;
/// releases come a few times a month, so a day finds everything in time.
pub const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;

/// How soon a scheduled check retries after one that failed. Checks follow
/// focus and popover opens, so without this an offline machine would ask
/// the network on every click.
pub const RETRY_SECS: u64 = 15 * 60;

/// Whether a scheduled check may touch the network. `enabled` is the
/// "Check automatically" setting and comes first: off means no request at
/// all, not a request whose answer is ignored. A clock that went backwards
/// (timezone change, manual set) reads as due rather than waiting it out.
pub fn should_check(
    enabled: bool,
    last_success: Option<u64>,
    last_attempt: Option<u64>,
    now: u64,
) -> bool {
    let elapsed_at_least = |since: Option<u64>, secs: u64| match since {
        None => true,
        Some(t) => now < t || now - t >= secs,
    };
    enabled
        && elapsed_at_least(last_success, CHECK_INTERVAL_SECS)
        && elapsed_at_least(last_attempt, RETRY_SECS)
}

/// What a check's answer leaves behind: the newer version it found, or none.
/// Both windows and every surface read this one value, so the Settings line,
/// the toast and the popover can't disagree.
///
/// `Unknown` (offline, a captive portal, a release that won't parse) keeps
/// what was there: failing to reach GitHub is not news that the update went
/// away.
pub fn remembered(status: &Status, previously: Option<String>) -> Option<String> {
    match status {
        Status::Behind(version) => Some(version.clone()),
        Status::Current => None,
        Status::Unknown => previously,
    }
}

/// The newer version still waiting, or none. Re-compared against the running
/// build rather than trusted as stored, so the first launch after an upgrade
/// clears every notice at once instead of at the next check. Nothing marks a
/// notice read or dismissed; it is derived, so it can't go stale.
pub fn pending(remembered: Option<&str>, running: &str) -> Option<String> {
    match status(running, remembered?) {
        Status::Behind(version) => Some(version),
        _ => None,
    }
}

/// The subset of GitHub's release JSON the check reads.
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
}

/// The release a channel is offered: the highest version among published
/// ones (pre-releases only on Beta) that already carry their .dmg.
///
/// A release published from the web UI is public about a quarter of an hour
/// before CI has built and attached its download; announcing it in that
/// window would send people to a page with nothing to download.
pub fn newest_offered(releases: &[Release], beta: bool) -> Option<&Release> {
    releases
        .iter()
        .filter(|r| !r.draft && (beta || !r.prerelease))
        .filter(|r| r.assets.iter().any(|a| a.name.ends_with(".dmg")))
        .filter_map(|r| Version::parse(&r.tag_name).map(|v| (v, r)))
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, r)| r)
}

// ---------- where to send somebody ----------

const RELEASES_PAGE: &str = "https://github.com/ColeMei/openwith/releases";

/// The page for one release: its notes and its download. The version comes
/// over the network, so it only goes into the URL when it parses as a
/// version and holds nothing but the characters one is made of; anything
/// else gets the list of releases, one click further away.
pub fn release_page(version: &str) -> String {
    let version = version.trim();
    let version = version.strip_prefix('v').unwrap_or(version);
    let plain = Version::parse(version).is_some()
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    if plain {
        format!("{RELEASES_PAGE}/tag/v{version}")
    } else {
        RELEASES_PAGE.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn behind(v: &str) -> Status {
        Status::Behind(v.to_string())
    }

    #[test]
    fn v_prefix_is_ignored() {
        assert_eq!(status("0.6.5", "v0.6.5"), Status::Current);
        assert_eq!(status("v0.6.5", "0.6.5"), Status::Current);
        assert_eq!(status("0.6.5", "v0.7.0"), behind("0.7.0"));
    }

    #[test]
    fn components_compare_numerically() {
        assert_eq!(status("0.9.0", "0.10.0"), behind("0.10.0"));
        assert_eq!(status("0.10.0", "0.9.9"), Status::Current);
        assert_eq!(status("1.0.0", "0.99.99"), Status::Current);
    }

    #[test]
    fn missing_components_are_zero() {
        assert_eq!(status("0.7", "0.7.0"), Status::Current);
        assert_eq!(status("0.7.0", "0.7"), Status::Current);
        assert_eq!(status("0.7", "0.7.1"), behind("0.7.1"));
    }

    #[test]
    fn newer_local_build_is_current() {
        assert_eq!(status("0.7.1", "0.7.0"), Status::Current);
        // An rc on the Stable channel was offered the older release before.
        assert_eq!(status("0.6.6-rc.1", "0.6.5"), Status::Current);
    }

    #[test]
    fn prerelease_ranks_below_its_release() {
        assert_eq!(status("0.7.0-rc.1", "0.7.0"), behind("0.7.0"));
        assert_eq!(status("0.7.0", "0.7.0-rc.2"), Status::Current);
        assert_eq!(status("0.6.5", "0.7.0-rc.1"), behind("0.7.0-rc.1"));
    }

    #[test]
    fn prerelease_identifiers_follow_semver() {
        assert_eq!(status("0.7.0-rc.1", "0.7.0-rc.2"), behind("0.7.0-rc.2"));
        assert_eq!(status("0.7.0-rc.9", "0.7.0-rc.10"), behind("0.7.0-rc.10"));
        assert_eq!(status("0.7.0-beta.2", "0.7.0-rc.1"), behind("0.7.0-rc.1"));
        assert_eq!(status("0.7.0-rc", "0.7.0-rc.1"), behind("0.7.0-rc.1"));
        assert_eq!(status("0.7.0-1", "0.7.0-rc"), behind("0.7.0-rc"));
    }

    #[test]
    fn build_metadata_is_ignored() {
        assert_eq!(status("0.7.0+abc", "0.7.0"), Status::Current);
        assert_eq!(status("0.7.0", "0.7.0+def"), Status::Current);
    }

    #[test]
    fn unparseable_is_unknown() {
        for bad in [
            "",
            "v",
            "latest",
            "0..1",
            "0.7.x",
            "0.7.0-",
            "0.7.0-rc..1",
            "1.0 beta",
        ] {
            assert_eq!(status("0.7.0", bad), Status::Unknown, "{bad:?}");
            assert_eq!(status(bad, "0.7.0"), Status::Unknown, "{bad:?}");
        }
    }

    #[test]
    fn whitespace_is_trimmed() {
        assert_eq!(status(" 0.7.0\n", "v0.7.1 "), behind("0.7.1"));
    }

    const DAY: u64 = CHECK_INTERVAL_SECS;

    #[test]
    fn switched_off_never_checks() {
        assert!(!should_check(false, None, None, 10 * DAY));
    }

    #[test]
    fn checks_once_a_day() {
        assert!(should_check(true, None, None, 10 * DAY));
        assert!(!should_check(
            true,
            Some(10 * DAY),
            Some(10 * DAY),
            10 * DAY + DAY - 1
        ));
        assert!(should_check(true, Some(10 * DAY), Some(10 * DAY), 11 * DAY));
    }

    #[test]
    fn a_failed_check_retries_after_a_while() {
        let now = 10 * DAY;
        assert!(!should_check(true, None, Some(now - 60), now));
        assert!(should_check(true, None, Some(now - RETRY_SECS), now));
    }

    #[test]
    fn clock_going_backwards_counts_as_due() {
        assert!(should_check(true, Some(10 * DAY), Some(10 * DAY), 9 * DAY));
    }

    #[test]
    fn unknown_keeps_what_was_remembered() {
        let before = Some("0.7.1".to_string());
        assert_eq!(remembered(&Status::Unknown, before.clone()), before);
        assert_eq!(remembered(&Status::Current, before.clone()), None);
        assert_eq!(
            remembered(&behind("0.7.2"), before),
            Some("0.7.2".to_string())
        );
    }

    #[test]
    fn pending_clears_once_the_running_build_catches_up() {
        assert_eq!(pending(Some("0.7.1"), "0.7.0"), Some("0.7.1".to_string()));
        assert_eq!(pending(Some("0.7.1"), "0.7.1"), None);
        assert_eq!(pending(Some("0.7.1"), "0.7.2"), None);
        assert_eq!(pending(None, "0.7.0"), None);
        assert_eq!(pending(Some("garbage"), "0.7.0"), None);
    }

    fn rel(tag: &str, prerelease: bool, draft: bool, dmg: bool) -> Release {
        let assets = if dmg {
            vec![Asset {
                name: format!("OpenWith_{}_aarch64.dmg", tag.trim_start_matches('v')),
            }]
        } else {
            vec![]
        };
        Release {
            tag_name: tag.to_string(),
            draft,
            prerelease,
            assets,
        }
    }

    fn offered(releases: &[Release], beta: bool) -> Option<&str> {
        newest_offered(releases, beta).map(|r| r.tag_name.as_str())
    }

    #[test]
    fn stable_skips_prereleases_and_drafts() {
        let releases = [
            rel("v0.8.0", false, true, true),
            rel("v0.7.1-rc.1", true, false, true),
            rel("v0.7.0", false, false, true),
        ];
        assert_eq!(offered(&releases, false), Some("v0.7.0"));
        assert_eq!(offered(&releases, true), Some("v0.7.1-rc.1"));
    }

    #[test]
    fn a_release_without_its_dmg_is_not_offered_yet() {
        let releases = [
            rel("v0.7.1", false, false, false),
            rel("v0.7.0", false, false, true),
        ];
        assert_eq!(offered(&releases, false), Some("v0.7.0"));
    }

    #[test]
    fn highest_version_wins_over_newest_published() {
        // A backport published after a newer line must not read as "latest".
        let releases = [
            rel("v0.6.9", false, false, true),
            rel("v0.7.0", false, false, true),
        ];
        assert_eq!(offered(&releases, false), Some("v0.7.0"));
    }

    #[test]
    fn beta_offers_the_final_over_its_rcs() {
        let releases = [
            rel("v0.7.0", false, false, true),
            rel("v0.7.0-rc.2", true, false, true),
        ];
        assert_eq!(offered(&releases, true), Some("v0.7.0"));
    }

    #[test]
    fn unparseable_tags_are_skipped() {
        let releases = [
            rel("nightly", false, false, true),
            rel("v0.7.0", false, false, true),
        ];
        assert_eq!(offered(&releases, false), Some("v0.7.0"));
        assert_eq!(offered(&[], false), None);
    }

    #[test]
    fn release_page_names_the_version() {
        let tag = "https://github.com/ColeMei/openwith/releases/tag/";
        assert_eq!(release_page("0.7.1"), format!("{tag}v0.7.1"));
        assert_eq!(release_page("v0.7.1"), format!("{tag}v0.7.1"));
        assert_eq!(release_page("0.7.0-rc.2"), format!("{tag}v0.7.0-rc.2"));
    }

    #[test]
    fn release_page_falls_back_to_the_list() {
        for odd in [
            "",
            "latest",
            "0.7.1/../../x",
            "0.7.1?x=1",
            "0.7.1#x",
            "0.7.1+abc",
        ] {
            assert_eq!(release_page(odd), RELEASES_PAGE, "{odd:?}");
        }
    }
}
