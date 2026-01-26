//! Python version types and parsing
//!
//! Handles Python version specifications (3.12, 3.11.8, etc.) and
//! provides information about available versions from python-build-standalone.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use crate::{Error, Result};

/// A Python version with major, minor, and optional patch
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PythonVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: Option<u32>,
}

impl PythonVersion {
    /// Create a new Python version
    pub fn new(major: u32, minor: u32, patch: Option<u32>) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse a version string like "3.12" or "3.12.1"
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.trim().split('.').collect();

        match parts.len() {
            2 => {
                let major: u32 = parts[0]
                    .parse()
                    .map_err(|_| Error::InvalidVersion(format!("invalid major version: {}", s)))?;
                let minor: u32 = parts[1]
                    .parse()
                    .map_err(|_| Error::InvalidVersion(format!("invalid minor version: {}", s)))?;
                Ok(Self::new(major, minor, None))
            }
            3 => {
                let major: u32 = parts[0]
                    .parse()
                    .map_err(|_| Error::InvalidVersion(format!("invalid major version: {}", s)))?;
                let minor: u32 = parts[1]
                    .parse()
                    .map_err(|_| Error::InvalidVersion(format!("invalid minor version: {}", s)))?;
                let patch: u32 = parts[2]
                    .parse()
                    .map_err(|_| Error::InvalidVersion(format!("invalid patch version: {}", s)))?;
                Ok(Self::new(major, minor, Some(patch)))
            }
            _ => Err(Error::InvalidVersion(format!(
                "invalid version format: {} (expected X.Y or X.Y.Z)",
                s
            ))),
        }
    }

    /// Check if this version matches a specification (e.g., "3.12" matches "3.12.1")
    pub fn matches(&self, spec: &PythonVersion) -> bool {
        if self.major != spec.major || self.minor != spec.minor {
            return false;
        }

        // If spec has no patch, any patch matches
        // If spec has a patch, it must match exactly
        match spec.patch {
            None => true,
            Some(p) => self.patch == Some(p),
        }
    }

    /// Get the version string (e.g., "3.12.1" or "3.12")
    pub fn to_string_full(&self) -> String {
        match self.patch {
            Some(p) => format!("{}.{}.{}", self.major, self.minor, p),
            None => format!("{}.{}", self.major, self.minor),
        }
    }

    /// Get the major.minor string (e.g., "3.12")
    pub fn to_string_short(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }
}

impl fmt::Display for PythonVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.patch {
            Some(p) => write!(f, "{}.{}.{}", self.major, self.minor, p),
            None => write!(f, "{}.{}", self.major, self.minor),
        }
    }
}

impl FromStr for PythonVersion {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

impl Ord for PythonVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => {
                    let self_patch = self.patch.unwrap_or(0);
                    let other_patch = other.patch.unwrap_or(0);
                    self_patch.cmp(&other_patch)
                }
                ord => ord,
            },
            ord => ord,
        }
    }
}

impl PartialOrd for PythonVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Information about an available Python version
#[derive(Debug, Clone)]
pub struct AvailableVersion {
    pub version: PythonVersion,
    /// The release tag from python-build-standalone
    pub release_tag: String,
}

impl AvailableVersion {
    pub fn new(version: PythonVersion, release_tag: impl Into<String>) -> Self {
        Self {
            version,
            release_tag: release_tag.into(),
        }
    }
}

/// Get a list of known available Python versions from python-build-standalone
///
/// This is a static list that should be updated periodically.
/// For a more dynamic solution, we could fetch from GitHub releases API.
pub fn available_versions() -> Vec<AvailableVersion> {
    // These are the latest versions available from python-build-standalone
    // as of January 2025
    vec![
        // Python 3.13
        AvailableVersion::new(
            PythonVersion::new(3, 13, Some(1)),
            "20250115",
        ),
        AvailableVersion::new(
            PythonVersion::new(3, 13, Some(0)),
            "20241206",
        ),
        // Python 3.12
        AvailableVersion::new(
            PythonVersion::new(3, 12, Some(8)),
            "20250115",
        ),
        AvailableVersion::new(
            PythonVersion::new(3, 12, Some(7)),
            "20241206",
        ),
        AvailableVersion::new(
            PythonVersion::new(3, 12, Some(6)),
            "20240909",
        ),
        AvailableVersion::new(
            PythonVersion::new(3, 12, Some(5)),
            "20240814",
        ),
        // Python 3.11
        AvailableVersion::new(
            PythonVersion::new(3, 11, Some(11)),
            "20250115",
        ),
        AvailableVersion::new(
            PythonVersion::new(3, 11, Some(10)),
            "20241016",
        ),
        AvailableVersion::new(
            PythonVersion::new(3, 11, Some(9)),
            "20240814",
        ),
        // Python 3.10
        AvailableVersion::new(
            PythonVersion::new(3, 10, Some(16)),
            "20250115",
        ),
        AvailableVersion::new(
            PythonVersion::new(3, 10, Some(15)),
            "20241016",
        ),
        AvailableVersion::new(
            PythonVersion::new(3, 10, Some(14)),
            "20240814",
        ),
        // Python 3.9
        AvailableVersion::new(
            PythonVersion::new(3, 9, Some(21)),
            "20250115",
        ),
        AvailableVersion::new(
            PythonVersion::new(3, 9, Some(20)),
            "20241016",
        ),
    ]
}

/// Find the latest version matching a specification
pub fn find_matching_version(spec: &PythonVersion) -> Option<AvailableVersion> {
    available_versions()
        .into_iter()
        .filter(|v| v.version.matches(spec))
        .max_by(|a, b| a.version.cmp(&b.version))
}

/// Get all versions for a specific major.minor
pub fn get_versions_for_minor(major: u32, minor: u32) -> Vec<AvailableVersion> {
    let spec = PythonVersion::new(major, minor, None);
    available_versions()
        .into_iter()
        .filter(|v| v.version.matches(&spec))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_short_version() {
        let v = PythonVersion::parse("3.12").unwrap();
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 12);
        assert_eq!(v.patch, None);
    }

    #[test]
    fn test_parse_full_version() {
        let v = PythonVersion::parse("3.12.1").unwrap();
        assert_eq!(v.major, 3);
        assert_eq!(v.minor, 12);
        assert_eq!(v.patch, Some(1));
    }

    #[test]
    fn test_parse_invalid() {
        assert!(PythonVersion::parse("3").is_err());
        assert!(PythonVersion::parse("3.12.1.2").is_err());
        assert!(PythonVersion::parse("abc").is_err());
    }

    #[test]
    fn test_matches() {
        let spec = PythonVersion::new(3, 12, None);
        let full = PythonVersion::new(3, 12, Some(1));

        assert!(full.matches(&spec));
        assert!(spec.matches(&spec));

        let different = PythonVersion::new(3, 11, Some(1));
        assert!(!different.matches(&spec));
    }

    #[test]
    fn test_version_ordering() {
        let v1 = PythonVersion::new(3, 11, Some(1));
        let v2 = PythonVersion::new(3, 12, Some(0));
        let v3 = PythonVersion::new(3, 12, Some(1));

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn test_find_matching() {
        let spec = PythonVersion::new(3, 12, None);
        let found = find_matching_version(&spec);
        assert!(found.is_some());
        assert_eq!(found.unwrap().version.minor, 12);
    }

    #[test]
    fn test_display() {
        let v = PythonVersion::new(3, 12, Some(1));
        assert_eq!(format!("{}", v), "3.12.1");

        let v = PythonVersion::new(3, 12, None);
        assert_eq!(format!("{}", v), "3.12");
    }
}
