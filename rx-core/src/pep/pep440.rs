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
        // TODO: Implement full PEP 440 parsing
        // For now, just handle simple x.y.z versions

        let parts: Result<Vec<u32>, _> = s
            .trim()
            .split('.')
            .map(|p| p.parse::<u32>())
            .collect();

        match parts {
            Ok(release) if !release.is_empty() => Ok(Self::new(release)),
            _ => Err(Error::InvalidVersion(s.to_string())),
        }
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

        // TODO: Compare pre, post, dev, local segments

        Ordering::Equal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.release, vec![1, 2, 3]);
    }

    #[test]
    fn test_display() {
        let v = Version::new(vec![1, 2, 3]);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_ordering() {
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("1.0.1").unwrap();
        let v3 = Version::parse("1.1.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }
}
