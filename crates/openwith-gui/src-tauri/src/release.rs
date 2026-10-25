//! Comparing the running version against a published one.
//!
//! Pure arithmetic on version strings, kept apart from the commands so it can
//! be unit-tested: this is the part that can be wrong quietly. A comparison
//! that says "up to date" when it isn't is worse than no check at all, and
//! the old exact-string test offered a Stable-channel rc build the older
//! release it was ahead of.

use std::cmp::Ordering;

use serde::Serialize;

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
}
