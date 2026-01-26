//! PEP 440 - Version Identification and Dependency Specification

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::Error;

/// A PEP 440 compliant version
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Version {
    /// Epoch (optional, defaults to 0)
    pub epoch: u32,
    /// Release segment (major.minor.patch...)
    pub release: Vec<u32>,
    /// Pre-release (alpha, beta, rc)
    pub pre: Option<PreRelease>,
    /// Post-release
    pub post: Option<u32>,
    /// Development release
    pub dev: Option<u32>,
    /// Local version segment
    pub local: Option<String>,
}

/// Pre-release type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PreRelease {
    Alpha(u32),
    Beta(u32),
    Rc(u32),
}

impl PreRelease {
    /// Get the numeric value for ordering
    fn order_key(&self) -> (u8, u32) {
        match self {
            PreRelease::Alpha(n) => (0, *n),
            PreRelease::Beta(n) => (1, *n),
            PreRelease::Rc(n) => (2, *n),
        }
    }
}

impl PartialOrd for PreRelease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PreRelease {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order_key().cmp(&other.order_key())
    }
}

impl Version {
    /// Create a new version from release numbers
    pub fn new(release: Vec<u32>) -> Self {
        Self {
            epoch: 0,
            release,
            pre: None,
            post: None,
            dev: None,
            local: None,
        }
    }

    /// Parse a version string
    pub fn parse(s: &str) -> Result<Self, Error> {
        let s = s.trim();
        if s.is_empty() {
            return Err(Error::InvalidVersion(s.to_string()));
        }

        let mut epoch = 0u32;
        let mut remaining = s;

        // Parse epoch (N!)
        if let Some(excl_pos) = remaining.find('!') {
            epoch = remaining[..excl_pos]
                .parse()
                .map_err(|_| Error::InvalidVersion(s.to_string()))?;
            remaining = &remaining[excl_pos + 1..];
        }

        // Split off local version (+local)
        let (version_part, local) = if let Some(plus_pos) = remaining.find('+') {
            let local = remaining[plus_pos + 1..].to_string();
            (&remaining[..plus_pos], Some(local))
        } else {
            (remaining, None)
        };

        // Parse the version part
        let (release, pre, post, dev) = Self::parse_version_part(version_part, s)?;

        Ok(Self {
            epoch,
            release,
            pre,
            post,
            dev,
            local,
        })
    }

    #[allow(clippy::type_complexity)]
    fn parse_version_part(
        s: &str,
        original: &str,
    ) -> Result<(Vec<u32>, Option<PreRelease>, Option<u32>, Option<u32>), Error> {
        let mut pre = None;
        let mut post = None;
        let mut dev = None;

        // Find where release ends and pre/post/dev begins
        // The release is a sequence of numbers separated by dots
        let mut release_end = 0;
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            if c.is_numeric() {
                // Scan the number
                while i < chars.len() && chars[i].is_numeric() {
                    i += 1;
                }
                release_end = i;
                // Check if followed by a dot and more digits
                if i < chars.len() && chars[i] == '.' {
                    // Look ahead to see if there's a number after the dot
                    if i + 1 < chars.len() && chars[i + 1].is_numeric() {
                        i += 1; // Skip the dot
                        continue;
                    }
                }
                break;
            } else if c == '.' {
                i += 1;
            } else {
                break;
            }
        }

        let release_str = &s[..release_end];
        let remaining = s[release_end..].trim_start_matches('.');

        // Parse release segment
        let release: Result<Vec<u32>, _> = release_str
            .split('.')
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<u32>())
            .collect();

        let release = release.map_err(|_| Error::InvalidVersion(original.to_string()))?;

        if release.is_empty() {
            return Err(Error::InvalidVersion(original.to_string()));
        }

        // Parse pre-release, post-release, and dev
        let remaining_lower = remaining.to_lowercase();
        let mut pos = 0;

        while pos < remaining_lower.len() {
            let rest = &remaining_lower[pos..];

            if rest.starts_with("dev") {
                let num_start = 3;
                let num_end = rest[num_start..]
                    .find(|c: char| !c.is_numeric())
                    .map(|i| num_start + i)
                    .unwrap_or(rest.len());

                let num: u32 = if num_end > num_start {
                    rest[num_start..num_end].parse().unwrap_or(0)
                } else {
                    0
                };
                dev = Some(num);
                pos += num_end;
            } else if rest.starts_with("post") || rest.starts_with("-") || rest.starts_with("r") {
                let prefix_len = if rest.starts_with("post") { 4 } else { 1 };
                let num_end = rest[prefix_len..]
                    .find(|c: char| !c.is_numeric())
                    .map(|i| prefix_len + i)
                    .unwrap_or(rest.len());

                let num: u32 = if num_end > prefix_len {
                    rest[prefix_len..num_end].parse().unwrap_or(0)
                } else {
                    0
                };
                post = Some(num);
                pos += num_end;
            } else if rest.starts_with("alpha") || rest.starts_with('a') {
                let prefix_len = if rest.starts_with("alpha") { 5 } else { 1 };
                let num_end = rest[prefix_len..]
                    .find(|c: char| !c.is_numeric())
                    .map(|i| prefix_len + i)
                    .unwrap_or(rest.len());

                let num: u32 = if num_end > prefix_len {
                    rest[prefix_len..num_end].parse().unwrap_or(0)
                } else {
                    0
                };
                pre = Some(PreRelease::Alpha(num));
                pos += num_end;
            } else if rest.starts_with("beta") || rest.starts_with('b') {
                let prefix_len = if rest.starts_with("beta") { 4 } else { 1 };
                let num_end = rest[prefix_len..]
                    .find(|c: char| !c.is_numeric())
                    .map(|i| prefix_len + i)
                    .unwrap_or(rest.len());

                let num: u32 = if num_end > prefix_len {
                    rest[prefix_len..num_end].parse().unwrap_or(0)
                } else {
                    0
                };
                pre = Some(PreRelease::Beta(num));
                pos += num_end;
            } else if rest.starts_with("rc") || rest.starts_with('c') || rest.starts_with("preview")
            {
                let prefix_len = if rest.starts_with("preview") {
                    7
                } else if rest.starts_with("rc") {
                    2
                } else {
                    1
                };
                let num_end = rest[prefix_len..]
                    .find(|c: char| !c.is_numeric())
                    .map(|i| prefix_len + i)
                    .unwrap_or(rest.len());

                let num: u32 = if num_end > prefix_len {
                    rest[prefix_len..num_end].parse().unwrap_or(0)
                } else {
                    0
                };
                pre = Some(PreRelease::Rc(num));
                pos += num_end;
            } else {
                // Skip separator characters or unknown characters
                pos += 1;
            }
        }

        Ok((release, pre, post, dev))
    }
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.epoch > 0 {
            write!(f, "{}!", self.epoch)?;
        }

        let release: Vec<String> = self.release.iter().map(|n| n.to_string()).collect();
        write!(f, "{}", release.join("."))?;

        if let Some(ref pre) = self.pre {
            match pre {
                PreRelease::Alpha(n) => write!(f, "a{}", n)?,
                PreRelease::Beta(n) => write!(f, "b{}", n)?,
                PreRelease::Rc(n) => write!(f, "rc{}", n)?,
            }
        }

        if let Some(post) = self.post {
            write!(f, ".post{}", post)?;
        }

        if let Some(dev) = self.dev {
            write!(f, ".dev{}", dev)?;
        }

        if let Some(ref local) = self.local {
            write!(f, "+{}", local)?;
        }

        Ok(())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare epoch first
        match self.epoch.cmp(&other.epoch) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // Compare release segments
        let max_len = self.release.len().max(other.release.len());
        for i in 0..max_len {
            let a = self.release.get(i).unwrap_or(&0);
            let b = other.release.get(i).unwrap_or(&0);
            match a.cmp(b) {
                Ordering::Equal => {}
                ord => return ord,
            }
        }

        // Compare pre-release
        // No pre < pre (pre-releases come before release)
        // But dev releases without pre are even earlier
        match (&self.pre, &other.pre) {
            (None, Some(_)) => {
                // self has no pre, other has pre
                // A release without pre is GREATER than one with pre
                // UNLESS self has dev (then it's less)
                if self.dev.is_some() && other.dev.is_none() {
                    return Ordering::Less;
                }
                return Ordering::Greater;
            }
            (Some(_), None) => {
                // self has pre, other has no pre
                if other.dev.is_some() && self.dev.is_none() {
                    return Ordering::Greater;
                }
                return Ordering::Less;
            }
            (Some(a), Some(b)) => match a.cmp(b) {
                Ordering::Equal => {}
                ord => return ord,
            },
            (None, None) => {}
        }

        // Compare dev (dev releases come before non-dev)
        match (&self.dev, &other.dev) {
            (None, Some(_)) => return Ordering::Greater,
            (Some(_), None) => return Ordering::Less,
            (Some(a), Some(b)) => match a.cmp(b) {
                Ordering::Equal => {}
                ord => return ord,
            },
            (None, None) => {}
        }

        // Compare post (post releases come after non-post)
        match (&self.post, &other.post) {
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) => match a.cmp(b) {
                Ordering::Equal => {}
                ord => return ord,
            },
            (None, None) => {}
        }

        // Local versions: presence of local makes it greater, then compare lexically
        match (&self.local, &other.local) {
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(b),
            (None, None) => Ordering::Equal,
        }
    }
}

// Implement pubgrub's Version trait
impl pubgrub::version::Version for Version {
    /// Returns the lowest possible version
    fn lowest() -> Self {
        Self {
            epoch: 0,
            release: vec![0],
            pre: Some(PreRelease::Alpha(0)),
            dev: Some(0),
            post: None,
            local: None,
        }
    }

    /// Returns the next version after self
    fn bump(&self) -> Self {
        let mut bumped = self.clone();

        // Clear local version (it doesn't affect ordering in pubgrub context)
        bumped.local = None;

        // If we have dev, bump it
        if let Some(dev) = bumped.dev {
            bumped.dev = Some(dev + 1);
            return bumped;
        }

        // If we have post, bump it
        if let Some(post) = bumped.post {
            bumped.post = Some(post + 1);
            return bumped;
        }

        // If we have pre, either bump or remove
        if let Some(ref pre) = bumped.pre {
            match pre {
                PreRelease::Alpha(n) => bumped.pre = Some(PreRelease::Alpha(n + 1)),
                PreRelease::Beta(n) => bumped.pre = Some(PreRelease::Beta(n + 1)),
                PreRelease::Rc(n) => bumped.pre = Some(PreRelease::Rc(n + 1)),
            }
            return bumped;
        }

        // Bump the last release segment
        if let Some(last) = bumped.release.last_mut() {
            *last += 1;
        } else {
            bumped.release.push(1);
        }

        bumped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubgrub::version::Version as PubgrubVersion;

    #[test]
    fn test_parse_simple() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.release, vec![1, 2, 3]);
    }

    #[test]
    fn test_parse_with_epoch() {
        let v = Version::parse("1!2.3.4").unwrap();
        assert_eq!(v.epoch, 1);
        assert_eq!(v.release, vec![2, 3, 4]);
    }

    #[test]
    fn test_parse_with_pre() {
        let v = Version::parse("1.0a1").unwrap();
        assert_eq!(v.pre, Some(PreRelease::Alpha(1)));

        let v = Version::parse("1.0b2").unwrap();
        assert_eq!(v.pre, Some(PreRelease::Beta(2)));

        let v = Version::parse("1.0rc3").unwrap();
        assert_eq!(v.pre, Some(PreRelease::Rc(3)));
    }

    #[test]
    fn test_parse_with_post() {
        let v = Version::parse("1.0.post1").unwrap();
        assert_eq!(v.post, Some(1));
    }

    #[test]
    fn test_parse_with_dev() {
        let v = Version::parse("1.0.dev1").unwrap();
        assert_eq!(v.dev, Some(1));
    }

    #[test]
    fn test_parse_with_local() {
        let v = Version::parse("1.0+local.1").unwrap();
        assert_eq!(v.local, Some("local.1".to_string()));
    }

    #[test]
    fn test_parse_complex() {
        let v = Version::parse("1!2.3.4a1.post2.dev3+local").unwrap();
        assert_eq!(v.epoch, 1);
        assert_eq!(v.release, vec![2, 3, 4]);
        assert_eq!(v.pre, Some(PreRelease::Alpha(1)));
        assert_eq!(v.post, Some(2));
        assert_eq!(v.dev, Some(3));
        assert_eq!(v.local, Some("local".to_string()));
    }

    #[test]
    fn test_display() {
        let v = Version::new(vec![1, 2, 3]);
        assert_eq!(v.to_string(), "1.2.3");

        let mut v = Version::new(vec![1, 0]);
        v.pre = Some(PreRelease::Alpha(1));
        assert_eq!(v.to_string(), "1.0a1");
    }

    #[test]
    fn test_ordering_release() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("1.0.1").unwrap();
        let v3 = Version::parse("1.1.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn test_ordering_pre() {
        let v1 = Version::parse("1.0a1").unwrap();
        let v2 = Version::parse("1.0a2").unwrap();
        let v3 = Version::parse("1.0b1").unwrap();
        let v4 = Version::parse("1.0rc1").unwrap();
        let v5 = Version::parse("1.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
        assert!(v4 < v5); // Pre-release < release
    }

    #[test]
    fn test_ordering_post() {
        let v1 = Version::parse("1.0").unwrap();
        let v2 = Version::parse("1.0.post1").unwrap();
        let v3 = Version::parse("1.0.post2").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
    }

    #[test]
    fn test_ordering_dev() {
        let v1 = Version::parse("1.0.dev1").unwrap();
        let v2 = Version::parse("1.0.dev2").unwrap();
        let v3 = Version::parse("1.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3); // Dev < release
    }

    #[test]
    fn test_ordering_epoch() {
        let v1 = Version::parse("1.0").unwrap();
        let v2 = Version::parse("1!0.1").unwrap();

        assert!(v1 < v2); // Epoch 0 < Epoch 1
    }

    #[test]
    fn test_bump() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = v1.bump();
        assert!(v1 < v2);

        let v3 = Version::parse("1.0.dev1").unwrap();
        let v4 = v3.bump();
        assert!(v3 < v4);
    }

    #[test]
    fn test_lowest() {
        let lowest = Version::lowest();
        let v1 = Version::parse("0.0.1").unwrap();
        assert!(lowest < v1);
    }
}
