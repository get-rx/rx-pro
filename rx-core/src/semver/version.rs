//! SemVer version parsing and manipulation

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::Error;

/// A semantic version number (MAJOR.MINOR.PATCH[-prerelease][+build])
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Version {
    /// Major version (breaking changes)
    pub major: u64,
    /// Minor version (new features, backwards compatible)
    pub minor: u64,
    /// Patch version (bug fixes, backwards compatible)
    pub patch: u64,
    /// Pre-release identifier (e.g., alpha, beta, rc.1)
    pub pre: Prerelease,
    /// Build metadata (ignored in version precedence)
    pub build: BuildMetadata,
}

impl Version {
    /// Create a new version
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: Prerelease::EMPTY,
            build: BuildMetadata::EMPTY,
        }
    }

    /// Parse a version string
    pub fn parse(text: &str) -> Result<Self, Error> {
        let text = text.trim();

        // Strip leading 'v' or 'V' if present
        let text = text.strip_prefix('v').or_else(|| text.strip_prefix('V')).unwrap_or(text);

        if text.is_empty() {
            return Err(Error::InvalidVersion("empty version string".to_string()));
        }

        // Split off build metadata first (+...)
        let (version_pre, build) = match text.find('+') {
            Some(pos) => {
                let build = BuildMetadata::new(&text[pos + 1..])?;
                (&text[..pos], build)
            }
            None => (text, BuildMetadata::EMPTY),
        };

        // Split off prerelease (-...)
        let (version, pre) = match version_pre.find('-') {
            Some(pos) => {
                let pre = Prerelease::new(&version_pre[pos + 1..])?;
                (&version_pre[..pos], pre)
            }
            None => (version_pre, Prerelease::EMPTY),
        };

        // Parse major.minor.patch
        let mut parts = version.split('.');

        let major = parts
            .next()
            .ok_or_else(|| Error::InvalidVersion("missing major version".to_string()))?
            .parse::<u64>()
            .map_err(|_| Error::InvalidVersion("invalid major version".to_string()))?;

        let minor = parts
            .next()
            .map(|s| s.parse::<u64>())
            .transpose()
            .map_err(|_| Error::InvalidVersion("invalid minor version".to_string()))?
            .unwrap_or(0);

        let patch = parts
            .next()
            .map(|s| s.parse::<u64>())
            .transpose()
            .map_err(|_| Error::InvalidVersion("invalid patch version".to_string()))?
            .unwrap_or(0);

        // Reject extra parts
        if parts.next().is_some() {
            return Err(Error::InvalidVersion("too many version parts".to_string()));
        }

        Ok(Self {
            major,
            minor,
            patch,
            pre,
            build,
        })
    }

    /// Check if this is a prerelease version
    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }

    /// Bump the major version (resets minor, patch, prerelease)
    pub fn bump_major(&self) -> Self {
        Self::new(self.major + 1, 0, 0)
    }

    /// Bump the minor version (resets patch, prerelease)
    pub fn bump_minor(&self) -> Self {
        Self::new(self.major, self.minor + 1, 0)
    }

    /// Bump the patch version (resets prerelease)
    pub fn bump_patch(&self) -> Self {
        Self::new(self.major, self.minor, self.patch + 1)
    }

    /// Set prerelease identifier
    pub fn with_prerelease(mut self, pre: Prerelease) -> Self {
        self.pre = pre;
        self
    }

    /// Set build metadata
    pub fn with_build(mut self, build: BuildMetadata) -> Self {
        self.build = build;
        self
    }

    /// Get the base version without prerelease or build metadata
    pub fn base(&self) -> Self {
        Self::new(self.major, self.minor, self.patch)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre)?;
        }
        if !self.build.is_empty() {
            write!(f, "+{}", self.build)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Version({})", self)
    }
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare major.minor.patch first
        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // Prerelease comparison (per SemVer spec):
        // - A version without prerelease > a version with prerelease
        // - Compare prerelease identifiers left to right
        self.pre.cmp(&other.pre)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Default for Version {
    fn default() -> Self {
        Self::new(0, 1, 0)
    }
}

/// Pre-release identifier (e.g., "alpha", "beta.1", "rc.2")
#[derive(Clone, Eq, PartialEq, Hash, Default, Serialize, Deserialize)]
pub struct Prerelease {
    identifier: String,
}

impl Prerelease {
    /// Empty prerelease
    pub const EMPTY: Self = Self {
        identifier: String::new(),
    };

    /// Create a new prerelease identifier
    pub fn new(s: &str) -> Result<Self, Error> {
        if s.is_empty() {
            return Ok(Self::EMPTY);
        }

        // Validate: alphanumeric and hyphens, dot-separated
        for part in s.split('.') {
            if part.is_empty() {
                return Err(Error::InvalidVersion("empty prerelease identifier".to_string()));
            }
            if !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(Error::InvalidVersion(format!(
                    "invalid prerelease identifier: {}",
                    part
                )));
            }
        }

        Ok(Self {
            identifier: s.to_string(),
        })
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.identifier.is_empty()
    }

    /// Get the identifier string
    pub fn as_str(&self) -> &str {
        &self.identifier
    }

    /// Parse identifier parts for comparison
    fn parts(&self) -> impl Iterator<Item = PrereleasePart<'_>> {
        self.identifier.split('.').map(|s| {
            if let Ok(n) = s.parse::<u64>() {
                PrereleasePart::Numeric(n)
            } else {
                PrereleasePart::Alphanumeric(s)
            }
        })
    }
}

#[derive(Eq, PartialEq)]
enum PrereleasePart<'a> {
    Numeric(u64),
    Alphanumeric(&'a str),
}

impl Ord for PrereleasePart<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // Numeric < Alphanumeric
            (PrereleasePart::Numeric(_), PrereleasePart::Alphanumeric(_)) => Ordering::Less,
            (PrereleasePart::Alphanumeric(_), PrereleasePart::Numeric(_)) => Ordering::Greater,
            // Both numeric: compare as numbers
            (PrereleasePart::Numeric(a), PrereleasePart::Numeric(b)) => a.cmp(b),
            // Both alphanumeric: compare lexically
            (PrereleasePart::Alphanumeric(a), PrereleasePart::Alphanumeric(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for PrereleasePart<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Prerelease {
    fn cmp(&self, other: &Self) -> Ordering {
        // Empty prerelease (release) > non-empty prerelease
        match (self.is_empty(), other.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                // Compare parts
                let mut self_parts = self.parts();
                let mut other_parts = other.parts();

                loop {
                    match (self_parts.next(), other_parts.next()) {
                        (None, None) => return Ordering::Equal,
                        (None, Some(_)) => return Ordering::Less,
                        (Some(_), None) => return Ordering::Greater,
                        (Some(a), Some(b)) => match a.cmp(&b) {
                            Ordering::Equal => continue,
                            ord => return ord,
                        },
                    }
                }
            }
        }
    }
}

impl PartialOrd for Prerelease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Prerelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.identifier)
    }
}

impl fmt::Debug for Prerelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "Prerelease::EMPTY")
        } else {
            write!(f, "Prerelease({})", self.identifier)
        }
    }
}

/// Build metadata (e.g., "build.123", "20230101")
#[derive(Clone, Eq, PartialEq, Hash, Default, Serialize, Deserialize)]
pub struct BuildMetadata {
    identifier: String,
}

impl BuildMetadata {
    /// Empty build metadata
    pub const EMPTY: Self = Self {
        identifier: String::new(),
    };

    /// Create new build metadata
    pub fn new(s: &str) -> Result<Self, Error> {
        if s.is_empty() {
            return Ok(Self::EMPTY);
        }

        // Validate: alphanumeric and hyphens, dot-separated
        for part in s.split('.') {
            if part.is_empty() {
                return Err(Error::InvalidVersion("empty build metadata".to_string()));
            }
            if !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(Error::InvalidVersion(format!(
                    "invalid build metadata: {}",
                    part
                )));
            }
        }

        Ok(Self {
            identifier: s.to_string(),
        })
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.identifier.is_empty()
    }

    /// Get the identifier string
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

impl fmt::Display for BuildMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.identifier)
    }
}

impl fmt::Debug for BuildMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "BuildMetadata::EMPTY")
        } else {
            write!(f, "BuildMetadata({})", self.identifier)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.pre.is_empty());
        assert!(v.build.is_empty());
    }

    #[test]
    fn test_parse_with_v_prefix() {
        let v = Version::parse("v1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_parse_partial() {
        let v = Version::parse("1").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);

        let v = Version::parse("1.2").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_parse_prerelease() {
        let v = Version::parse("1.0.0-alpha").unwrap();
        assert_eq!(v.pre.as_str(), "alpha");

        let v = Version::parse("1.0.0-alpha.1").unwrap();
        assert_eq!(v.pre.as_str(), "alpha.1");

        let v = Version::parse("1.0.0-0.3.7").unwrap();
        assert_eq!(v.pre.as_str(), "0.3.7");

        let v = Version::parse("1.0.0-x.7.z.92").unwrap();
        assert_eq!(v.pre.as_str(), "x.7.z.92");
    }

    #[test]
    fn test_parse_build_metadata() {
        let v = Version::parse("1.0.0+build.123").unwrap();
        assert_eq!(v.build.as_str(), "build.123");

        let v = Version::parse("1.0.0-alpha+001").unwrap();
        assert_eq!(v.pre.as_str(), "alpha");
        assert_eq!(v.build.as_str(), "001");
    }

    #[test]
    fn test_display() {
        assert_eq!(Version::new(1, 2, 3).to_string(), "1.2.3");
        assert_eq!(
            Version::parse("1.0.0-alpha").unwrap().to_string(),
            "1.0.0-alpha"
        );
        assert_eq!(
            Version::parse("1.0.0+build").unwrap().to_string(),
            "1.0.0+build"
        );
        assert_eq!(
            Version::parse("1.0.0-alpha+build").unwrap().to_string(),
            "1.0.0-alpha+build"
        );
    }

    #[test]
    fn test_ordering_basic() {
        assert!(Version::new(2, 0, 0) > Version::new(1, 0, 0));
        assert!(Version::new(1, 1, 0) > Version::new(1, 0, 0));
        assert!(Version::new(1, 0, 1) > Version::new(1, 0, 0));
    }

    #[test]
    fn test_ordering_prerelease() {
        // Release > prerelease
        assert!(Version::new(1, 0, 0) > Version::parse("1.0.0-alpha").unwrap());

        // alpha < beta < rc
        let alpha = Version::parse("1.0.0-alpha").unwrap();
        let beta = Version::parse("1.0.0-beta").unwrap();
        let rc = Version::parse("1.0.0-rc").unwrap();
        assert!(alpha < beta);
        assert!(beta < rc);

        // Numeric comparison in prerelease
        let alpha1 = Version::parse("1.0.0-alpha.1").unwrap();
        let alpha2 = Version::parse("1.0.0-alpha.2").unwrap();
        let alpha10 = Version::parse("1.0.0-alpha.10").unwrap();
        assert!(alpha1 < alpha2);
        assert!(alpha2 < alpha10);

        // Per SemVer: numeric < alphanumeric
        let pre_1 = Version::parse("1.0.0-1").unwrap();
        let pre_alpha = Version::parse("1.0.0-alpha").unwrap();
        assert!(pre_1 < pre_alpha);
    }

    #[test]
    fn test_ordering_build_metadata_ignored() {
        let v1 = Version::parse("1.0.0+build1").unwrap();
        let v2 = Version::parse("1.0.0+build2").unwrap();
        assert_eq!(v1.cmp(&v2), Ordering::Equal);
    }

    #[test]
    fn test_bump() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.bump_major(), Version::new(2, 0, 0));
        assert_eq!(v.bump_minor(), Version::new(1, 3, 0));
        assert_eq!(v.bump_patch(), Version::new(1, 2, 4));
    }

    #[test]
    fn test_bump_clears_prerelease() {
        let v = Version::parse("1.0.0-alpha").unwrap();
        assert!(v.bump_patch().pre.is_empty());
    }

    #[test]
    fn test_is_prerelease() {
        assert!(!Version::new(1, 0, 0).is_prerelease());
        assert!(Version::parse("1.0.0-alpha").unwrap().is_prerelease());
    }

    #[test]
    fn test_base() {
        let v = Version::parse("1.2.3-alpha+build").unwrap();
        let base = v.base();
        assert_eq!(base, Version::new(1, 2, 3));
        assert!(base.pre.is_empty());
        assert!(base.build.is_empty());
    }
}
