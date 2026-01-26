//! SemVer range parsing and matching
//!
//! Supports npm/Cargo-style version ranges:
//! - Exact: `=1.2.3`, `1.2.3`
//! - Comparison: `>1.0.0`, `>=1.0.0`, `<2.0.0`, `<=2.0.0`
//! - Caret: `^1.2.3` (compatible with 1.x.x, >=1.2.3 <2.0.0)
//! - Tilde: `~1.2.3` (compatible with 1.2.x, >=1.2.3 <1.3.0)
//! - Wildcard: `1.*`, `1.2.*`, `*`
//! - Hyphen: `1.0.0 - 2.0.0` (>=1.0.0 <=2.0.0)
//! - OR: `^1.0.0 || ^2.0.0`

use std::fmt;
use std::str::FromStr;

use crate::semver::Version;
use crate::Error;

/// A version requirement (set of ranges combined with OR)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionReq {
    /// Ranges combined with OR (any match satisfies)
    pub ranges: Vec<Range>,
}

impl VersionReq {
    /// Matches any version
    pub const STAR: Self = Self { ranges: Vec::new() };

    /// Parse a version requirement string
    pub fn parse(text: &str) -> Result<Self, Error> {
        let text = text.trim();

        if text.is_empty() || text == "*" {
            return Ok(Self::STAR);
        }

        // Split by || for OR combinations
        let ranges: Result<Vec<_>, _> = text.split("||").map(|s| Range::parse(s.trim())).collect();

        Ok(Self { ranges: ranges? })
    }

    /// Check if a version satisfies this requirement
    pub fn matches(&self, version: &Version) -> bool {
        // Empty ranges means * (match all)
        if self.ranges.is_empty() {
            return true;
        }

        // Any range matching is sufficient (OR)
        self.ranges.iter().any(|r| r.matches(version))
    }

    /// Check if this matches any version
    pub fn is_any(&self) -> bool {
        self.ranges.is_empty()
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ranges.is_empty() {
            return write!(f, "*");
        }

        let mut first = true;
        for range in &self.ranges {
            if !first {
                write!(f, " || ")?;
            }
            first = false;
            write!(f, "{}", range)?;
        }
        Ok(())
    }
}

impl FromStr for VersionReq {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Default for VersionReq {
    fn default() -> Self {
        Self::STAR
    }
}

/// A range of versions (comparators combined with AND)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Range {
    /// Comparators combined with AND (all must match)
    pub comparators: Vec<Comparator>,
}

impl Range {
    /// Parse a range string (space or comma separated comparators)
    pub fn parse(text: &str) -> Result<Self, Error> {
        let text = text.trim();

        if text.is_empty() || text == "*" {
            return Ok(Self {
                comparators: vec![],
            });
        }

        // Check for hyphen range first: "1.0.0 - 2.0.0"
        if let Some(idx) = text.find(" - ") {
            let lower = text[..idx].trim();
            let upper = text[idx + 3..].trim();
            return Self::parse_hyphen_range(lower, upper);
        }

        // Split by whitespace or comma for AND combinations
        let parts: Vec<&str> = text
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .collect();

        let comparators: Result<Vec<_>, _> = parts.iter().map(|s| Comparator::parse(s)).collect();

        Ok(Self {
            comparators: comparators?,
        })
    }

    /// Parse hyphen range: "1.0.0 - 2.0.0" => >=1.0.0 <=2.0.0
    fn parse_hyphen_range(lower: &str, upper: &str) -> Result<Self, Error> {
        let lower_version = Version::parse(lower)?;
        let upper_version = Version::parse(upper)?;

        Ok(Self {
            comparators: vec![
                Comparator {
                    op: Op::GreaterEq,
                    major: lower_version.major,
                    minor: Some(lower_version.minor),
                    patch: Some(lower_version.patch),
                    pre: lower_version.pre,
                },
                Comparator {
                    op: Op::LessEq,
                    major: upper_version.major,
                    minor: Some(upper_version.minor),
                    patch: Some(upper_version.patch),
                    pre: upper_version.pre,
                },
            ],
        })
    }

    /// Check if a version satisfies this range
    pub fn matches(&self, version: &Version) -> bool {
        // Empty comparators means * (match all)
        if self.comparators.is_empty() {
            return true;
        }

        // All comparators must match (AND)
        self.comparators.iter().all(|c| c.matches(version))
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.comparators.is_empty() {
            return write!(f, "*");
        }

        let mut first = true;
        for comp in &self.comparators {
            if !first {
                write!(f, " ")?;
            }
            first = false;
            write!(f, "{}", comp)?;
        }
        Ok(())
    }
}

impl FromStr for Range {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Comparison operator
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    /// `=` or implied - exact match
    Exact,
    /// `>` - greater than
    Greater,
    /// `>=` - greater than or equal
    GreaterEq,
    /// `<` - less than
    Less,
    /// `<=` - less than or equal
    LessEq,
    /// `^` - caret (compatible)
    Caret,
    /// `~` - tilde (patch-level compatible)
    Tilde,
    /// `*` - wildcard
    Wildcard,
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Op::Exact => Ok(()),
            Op::Greater => write!(f, ">"),
            Op::GreaterEq => write!(f, ">="),
            Op::Less => write!(f, "<"),
            Op::LessEq => write!(f, "<="),
            Op::Caret => write!(f, "^"),
            Op::Tilde => write!(f, "~"),
            Op::Wildcard => write!(f, "*"),
        }
    }
}

/// A single version comparator
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparator {
    /// Comparison operator
    pub op: Op,
    /// Major version
    pub major: u64,
    /// Minor version (None for wildcards like 1.*)
    pub minor: Option<u64>,
    /// Patch version (None for wildcards like 1.2.*)
    pub patch: Option<u64>,
    /// Prerelease
    pub pre: crate::semver::Prerelease,
}

impl Comparator {
    /// Parse a single comparator
    pub fn parse(text: &str) -> Result<Self, Error> {
        let text = text.trim();

        if text.is_empty() {
            return Err(Error::InvalidVersion("empty comparator".to_string()));
        }

        // Handle pure wildcard
        if text == "*" || text == "x" || text == "X" {
            return Ok(Self {
                op: Op::Wildcard,
                major: 0,
                minor: None,
                patch: None,
                pre: crate::semver::Prerelease::EMPTY,
            });
        }

        // Parse operator prefix
        let (op, rest) = if let Some(rest) = text.strip_prefix(">=") {
            (Op::GreaterEq, rest)
        } else if let Some(rest) = text.strip_prefix("<=") {
            (Op::LessEq, rest)
        } else if let Some(rest) = text.strip_prefix('>') {
            (Op::Greater, rest)
        } else if let Some(rest) = text.strip_prefix('<') {
            (Op::Less, rest)
        } else if let Some(rest) = text.strip_prefix('^') {
            (Op::Caret, rest)
        } else if let Some(rest) = text.strip_prefix('~') {
            (Op::Tilde, rest)
        } else if let Some(rest) = text.strip_prefix('=') {
            (Op::Exact, rest)
        } else {
            (Op::Exact, text)
        };

        let rest = rest.trim();

        // Handle wildcards in version
        if rest.contains('*') || rest.contains('x') || rest.contains('X') {
            return Self::parse_wildcard(rest);
        }

        // Parse the version part
        let version = Version::parse(rest)?;

        Ok(Self {
            op,
            major: version.major,
            minor: Some(version.minor),
            patch: Some(version.patch),
            pre: version.pre,
        })
    }

    /// Parse a wildcard version like 1.*, 1.2.*, 1.x
    fn parse_wildcard(text: &str) -> Result<Self, Error> {
        let parts: Vec<&str> = text.split('.').collect();

        let parse_part = |s: &str| -> Option<u64> {
            if s == "*" || s == "x" || s == "X" {
                None
            } else {
                s.parse().ok()
            }
        };

        let major = parts
            .first()
            .and_then(|s| parse_part(s))
            .ok_or_else(|| Error::InvalidVersion("invalid wildcard".to_string()))?;

        let minor = parts.get(1).and_then(|s| parse_part(s));
        let patch = parts.get(2).and_then(|s| parse_part(s));

        Ok(Self {
            op: Op::Wildcard,
            major,
            minor,
            patch,
            pre: crate::semver::Prerelease::EMPTY,
        })
    }

    /// Check if a version matches this comparator
    pub fn matches(&self, version: &Version) -> bool {
        match self.op {
            Op::Exact => self.matches_exact(version),
            Op::Greater => self.matches_greater(version),
            Op::GreaterEq => self.matches_greater_eq(version),
            Op::Less => self.matches_less(version),
            Op::LessEq => self.matches_less_eq(version),
            Op::Caret => self.matches_caret(version),
            Op::Tilde => self.matches_tilde(version),
            Op::Wildcard => self.matches_wildcard(version),
        }
    }

    fn matches_exact(&self, version: &Version) -> bool {
        version.major == self.major
            && self.minor.map_or(true, |m| version.minor == m)
            && self.patch.map_or(true, |p| version.patch == p)
            && (self.pre.is_empty() || version.pre == self.pre)
    }

    fn matches_greater(&self, version: &Version) -> bool {
        let cmp_version = self.to_version();
        version > &cmp_version
    }

    fn matches_greater_eq(&self, version: &Version) -> bool {
        let cmp_version = self.to_version();
        version >= &cmp_version
    }

    fn matches_less(&self, version: &Version) -> bool {
        let cmp_version = self.to_version();
        // For prereleases: 1.0.0-alpha < 1.0.0, but we shouldn't match
        // prereleases unless the comparator also has a prerelease
        if version.is_prerelease() && !cmp_version.is_prerelease() {
            // Only match prereleases of the same major.minor.patch
            if version.major == cmp_version.major
                && version.minor == cmp_version.minor
                && version.patch == cmp_version.patch
            {
                return true;
            }
            // Don't match prereleases of different versions
            if version.base() >= cmp_version {
                return false;
            }
        }
        version < &cmp_version
    }

    fn matches_less_eq(&self, version: &Version) -> bool {
        let cmp_version = self.to_version();
        if version.is_prerelease() && !cmp_version.is_prerelease() {
            if version.major == cmp_version.major
                && version.minor == cmp_version.minor
                && version.patch == cmp_version.patch
            {
                return true;
            }
            if version.base() > cmp_version {
                return false;
            }
        }
        version <= &cmp_version
    }

    /// Caret: ^1.2.3 means >=1.2.3 <2.0.0 (compatible with 1.x)
    /// ^0.2.3 means >=0.2.3 <0.3.0 (for 0.x, minor is breaking)
    /// ^0.0.3 means >=0.0.3 <0.0.4 (for 0.0.x, patch is breaking)
    fn matches_caret(&self, version: &Version) -> bool {
        // Must be >= the specified version
        if !self.matches_greater_eq(version) {
            return false;
        }

        // Determine the upper bound based on the first non-zero component
        if self.major != 0 {
            // ^1.2.3 => <2.0.0
            version.major == self.major
        } else if self.minor.unwrap_or(0) != 0 {
            // ^0.2.3 => <0.3.0
            version.major == 0 && version.minor == self.minor.unwrap_or(0)
        } else {
            // ^0.0.3 => <0.0.4
            version.major == 0 && version.minor == 0 && version.patch == self.patch.unwrap_or(0)
        }
    }

    /// Tilde: ~1.2.3 means >=1.2.3 <1.3.0 (patch-level changes only)
    fn matches_tilde(&self, version: &Version) -> bool {
        // Must be >= the specified version
        if !self.matches_greater_eq(version) {
            return false;
        }

        // Must have same major and minor
        version.major == self.major && version.minor == self.minor.unwrap_or(0)
    }

    /// Wildcard: 1.* matches 1.0.0, 1.2.3, etc.
    fn matches_wildcard(&self, version: &Version) -> bool {
        if version.major != self.major {
            return false;
        }
        if let Some(minor) = self.minor {
            if version.minor != minor {
                return false;
            }
        }
        if let Some(patch) = self.patch {
            if version.patch != patch {
                return false;
            }
        }
        true
    }

    /// Convert to a Version for comparison
    fn to_version(&self) -> Version {
        Version {
            major: self.major,
            minor: self.minor.unwrap_or(0),
            patch: self.patch.unwrap_or(0),
            pre: self.pre.clone(),
            build: crate::semver::BuildMetadata::EMPTY,
        }
    }
}

impl fmt::Display for Comparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.op)?;

        if self.op == Op::Wildcard && self.minor.is_none() {
            return write!(f, "{}.*", self.major);
        }

        write!(f, "{}", self.major)?;
        if let Some(minor) = self.minor {
            write!(f, ".{}", minor)?;
            if let Some(patch) = self.patch {
                write!(f, ".{}", patch)?;
            } else if self.op == Op::Wildcard {
                write!(f, ".*")?;
            }
        } else if self.op == Op::Wildcard {
            write!(f, ".*")?;
        }

        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exact() {
        let req = VersionReq::parse("1.2.3").unwrap();
        assert!(req.matches(&Version::new(1, 2, 3)));
        assert!(!req.matches(&Version::new(1, 2, 4)));
    }

    #[test]
    fn test_parse_comparison() {
        let req = VersionReq::parse(">=1.0.0").unwrap();
        assert!(req.matches(&Version::new(1, 0, 0)));
        assert!(req.matches(&Version::new(2, 0, 0)));
        assert!(!req.matches(&Version::new(0, 9, 0)));

        let req = VersionReq::parse("<2.0.0").unwrap();
        assert!(req.matches(&Version::new(1, 0, 0)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
    }

    #[test]
    fn test_parse_combined() {
        let req = VersionReq::parse(">=1.0.0 <2.0.0").unwrap();
        assert!(req.matches(&Version::new(1, 0, 0)));
        assert!(req.matches(&Version::new(1, 5, 0)));
        assert!(!req.matches(&Version::new(0, 9, 0)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
    }

    #[test]
    fn test_parse_caret() {
        // ^1.2.3 := >=1.2.3 <2.0.0
        let req = VersionReq::parse("^1.2.3").unwrap();
        assert!(req.matches(&Version::new(1, 2, 3)));
        assert!(req.matches(&Version::new(1, 9, 0)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
        assert!(!req.matches(&Version::new(1, 2, 2)));

        // ^0.2.3 := >=0.2.3 <0.3.0
        let req = VersionReq::parse("^0.2.3").unwrap();
        assert!(req.matches(&Version::new(0, 2, 3)));
        assert!(req.matches(&Version::new(0, 2, 9)));
        assert!(!req.matches(&Version::new(0, 3, 0)));

        // ^0.0.3 := >=0.0.3 <0.0.4
        let req = VersionReq::parse("^0.0.3").unwrap();
        assert!(req.matches(&Version::new(0, 0, 3)));
        assert!(!req.matches(&Version::new(0, 0, 4)));
    }

    #[test]
    fn test_parse_tilde() {
        // ~1.2.3 := >=1.2.3 <1.3.0
        let req = VersionReq::parse("~1.2.3").unwrap();
        assert!(req.matches(&Version::new(1, 2, 3)));
        assert!(req.matches(&Version::new(1, 2, 9)));
        assert!(!req.matches(&Version::new(1, 3, 0)));
        assert!(!req.matches(&Version::new(1, 2, 2)));
    }

    #[test]
    fn test_parse_wildcard() {
        let req = VersionReq::parse("1.*").unwrap();
        assert!(req.matches(&Version::new(1, 0, 0)));
        assert!(req.matches(&Version::new(1, 9, 9)));
        assert!(!req.matches(&Version::new(2, 0, 0)));

        let req = VersionReq::parse("1.2.*").unwrap();
        assert!(req.matches(&Version::new(1, 2, 0)));
        assert!(req.matches(&Version::new(1, 2, 9)));
        assert!(!req.matches(&Version::new(1, 3, 0)));
    }

    #[test]
    fn test_parse_hyphen() {
        let req = VersionReq::parse("1.0.0 - 2.0.0").unwrap();
        assert!(req.matches(&Version::new(1, 0, 0)));
        assert!(req.matches(&Version::new(1, 5, 0)));
        assert!(req.matches(&Version::new(2, 0, 0)));
        assert!(!req.matches(&Version::new(0, 9, 0)));
        assert!(!req.matches(&Version::new(2, 0, 1)));
    }

    #[test]
    fn test_parse_or() {
        let req = VersionReq::parse("^1.0.0 || ^2.0.0").unwrap();
        assert!(req.matches(&Version::new(1, 5, 0)));
        assert!(req.matches(&Version::new(2, 5, 0)));
        assert!(!req.matches(&Version::new(3, 0, 0)));
    }

    #[test]
    fn test_star() {
        let req = VersionReq::parse("*").unwrap();
        assert!(req.matches(&Version::new(0, 0, 0)));
        assert!(req.matches(&Version::new(999, 999, 999)));

        let req = VersionReq::parse("").unwrap();
        assert!(req.matches(&Version::new(1, 0, 0)));
    }

    #[test]
    fn test_prerelease_matching() {
        // Prereleases only match if explicitly specified or same base version
        let req = VersionReq::parse(">=1.0.0-alpha").unwrap();
        assert!(req.matches(&Version::parse("1.0.0-alpha").unwrap()));
        assert!(req.matches(&Version::parse("1.0.0-beta").unwrap()));
        assert!(req.matches(&Version::new(1, 0, 0)));
    }

    #[test]
    fn test_display() {
        assert_eq!(VersionReq::parse("^1.2.3").unwrap().to_string(), "^1.2.3");
        assert_eq!(
            VersionReq::parse(">=1.0.0 <2.0.0").unwrap().to_string(),
            ">=1.0.0 <2.0.0"
        );
        assert_eq!(VersionReq::parse("*").unwrap().to_string(), "*");
    }
}
