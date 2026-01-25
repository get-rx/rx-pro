//! Version specifier parsing and range conversion
//!
//! Parses PEP 440 version specifiers like ">=1.0,<2.0" and converts
//! them to pubgrub ranges for dependency resolution.

use std::str::FromStr;

use pubgrub::range::Range;
use pubgrub::version::Version as PubgrubVersion;

use crate::pep::pep440::Version;
use crate::Error;

/// Helper to create a version that is the next possible version after the given one
fn next_version(v: &Version) -> Version {
    PubgrubVersion::bump(v)
}

/// Version comparison operator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// `==` - Exact match
    Equal,
    /// `!=` - Exclusion
    NotEqual,
    /// `<` - Less than
    LessThan,
    /// `<=` - Less than or equal
    LessThanOrEqual,
    /// `>` - Greater than
    GreaterThan,
    /// `>=` - Greater than or equal
    GreaterThanOrEqual,
    /// `~=` - Compatible release
    Compatible,
    /// `===` - Arbitrary equality (string match)
    ArbitraryEqual,
}

impl Operator {
    /// Parse an operator from a string prefix
    fn parse(s: &str) -> Option<(Self, usize)> {
        if s.starts_with("===") {
            Some((Operator::ArbitraryEqual, 3))
        } else if s.starts_with("==") {
            Some((Operator::Equal, 2))
        } else if s.starts_with("!=") {
            Some((Operator::NotEqual, 2))
        } else if s.starts_with("~=") {
            Some((Operator::Compatible, 2))
        } else if s.starts_with("<=") {
            Some((Operator::LessThanOrEqual, 2))
        } else if s.starts_with(">=") {
            Some((Operator::GreaterThanOrEqual, 2))
        } else if s.starts_with('<') {
            Some((Operator::LessThan, 1))
        } else if s.starts_with('>') {
            Some((Operator::GreaterThan, 1))
        } else {
            None
        }
    }
}

/// A single version specifier (operator + version)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSpecifier {
    /// The comparison operator
    pub operator: Operator,
    /// The version to compare against
    pub version: Version,
    /// Whether this is a wildcard match (e.g., `==1.0.*`)
    pub wildcard: bool,
}

impl VersionSpecifier {
    /// Create a new version specifier
    pub fn new(operator: Operator, version: Version) -> Self {
        Self {
            operator,
            version,
            wildcard: false,
        }
    }

    /// Parse a single version specifier
    pub fn parse(s: &str) -> Result<Self, Error> {
        let s = s.trim();

        let (operator, op_len) =
            Operator::parse(s).ok_or_else(|| Error::InvalidSpecifier(s.to_string()))?;

        let version_str = s[op_len..].trim();

        // Check for wildcard
        let (version_str, wildcard) = if version_str.ends_with(".*") {
            (&version_str[..version_str.len() - 2], true)
        } else if version_str.ends_with('*') {
            (&version_str[..version_str.len() - 1], true)
        } else {
            (version_str, false)
        };

        let version = Version::parse(version_str)?;

        Ok(Self {
            operator,
            version,
            wildcard,
        })
    }

    /// Check if a version satisfies this specifier
    pub fn contains(&self, v: &Version) -> bool {
        match self.operator {
            Operator::Equal => {
                if self.wildcard {
                    self.matches_prefix(v)
                } else {
                    v == &self.version
                }
            }
            Operator::NotEqual => {
                if self.wildcard {
                    !self.matches_prefix(v)
                } else {
                    v != &self.version
                }
            }
            Operator::LessThan => v < &self.version,
            Operator::LessThanOrEqual => v <= &self.version,
            Operator::GreaterThan => v > &self.version,
            Operator::GreaterThanOrEqual => v >= &self.version,
            Operator::Compatible => {
                // ~=X.Y is equivalent to >=X.Y,==X.*
                if self.version.release.len() < 2 {
                    v >= &self.version
                } else {
                    let mut upper = self.version.clone();
                    // Bump the second-to-last release segment
                    let len = upper.release.len();
                    if len >= 2 {
                        upper.release[len - 2] += 1;
                        upper.release.truncate(len - 1);
                        upper.pre = None;
                        upper.post = None;
                        upper.dev = None;
                        upper.local = None;
                    }
                    v >= &self.version && v < &upper
                }
            }
            Operator::ArbitraryEqual => {
                // String comparison - compare string representations
                v.to_string() == self.version.to_string()
            }
        }
    }

    /// Check if version matches the prefix (for wildcard matching)
    fn matches_prefix(&self, v: &Version) -> bool {
        // Epochs must match
        if v.epoch != self.version.epoch {
            return false;
        }

        // Check release prefix
        for (i, seg) in self.version.release.iter().enumerate() {
            if v.release.get(i) != Some(seg) {
                return false;
            }
        }

        true
    }

    /// Convert to a pubgrub range
    pub fn to_range(&self) -> Range<Version> {
        match self.operator {
            Operator::Equal => {
                if self.wildcard {
                    // ==1.0.* means >=1.0.0,<1.1.0
                    let mut upper = self.version.clone();
                    if let Some(last) = upper.release.last_mut() {
                        *last += 1;
                    }
                    Range::between(self.version.clone(), upper)
                } else {
                    // Exact match
                    Range::exact(self.version.clone())
                }
            }
            Operator::NotEqual => {
                // For pubgrub, != means everything except this version
                Range::exact(self.version.clone()).negate()
            }
            Operator::LessThan => Range::strictly_lower_than(self.version.clone()),
            Operator::LessThanOrEqual => {
                // <= v means < v.bump()
                Range::strictly_lower_than(next_version(&self.version))
            }
            Operator::GreaterThan => {
                // > v means >= v.bump()
                Range::higher_than(next_version(&self.version))
            }
            Operator::GreaterThanOrEqual => Range::higher_than(self.version.clone()),
            Operator::Compatible => {
                // ~=X.Y.Z is >=X.Y.Z,<X.(Y+1).0
                if self.version.release.len() < 2 {
                    Range::higher_than(self.version.clone())
                } else {
                    let mut upper = self.version.clone();
                    let len = upper.release.len();
                    upper.release[len - 2] += 1;
                    upper.release.truncate(len - 1);
                    upper.pre = None;
                    upper.post = None;
                    upper.dev = None;
                    upper.local = None;
                    Range::between(self.version.clone(), upper)
                }
            }
            Operator::ArbitraryEqual => {
                // Treat as exact match
                Range::exact(self.version.clone())
            }
        }
    }
}

impl FromStr for VersionSpecifier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// A collection of version specifiers (e.g., ">=1.0,<2.0")
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSpecifiers(pub Vec<VersionSpecifier>);

impl VersionSpecifiers {
    /// Create empty specifiers (matches all versions)
    pub fn any() -> Self {
        Self(vec![])
    }

    /// Parse a comma-separated list of version specifiers
    pub fn parse(s: &str) -> Result<Self, Error> {
        let s = s.trim();

        if s.is_empty() {
            return Ok(Self::any());
        }

        let specifiers: Result<Vec<_>, _> = s
            .split(',')
            .map(|part| VersionSpecifier::parse(part.trim()))
            .collect();

        Ok(Self(specifiers?))
    }

    /// Check if a version satisfies all specifiers
    pub fn contains(&self, v: &Version) -> bool {
        self.0.iter().all(|spec| spec.contains(v))
    }

    /// Check if this matches any version
    pub fn is_any(&self) -> bool {
        self.0.is_empty()
    }

    /// Convert to a pubgrub range by intersecting all specifier ranges
    pub fn to_pubgrub_range(&self) -> Range<Version> {
        if self.0.is_empty() {
            return Range::any();
        }

        let mut result = Range::any();

        for spec in &self.0 {
            let range = spec.to_range();
            result = result.intersection(&range);
        }

        result
    }
}

impl FromStr for VersionSpecifiers {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl std::fmt::Display for VersionSpecifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = self
            .0
            .iter()
            .map(|spec| {
                let op = match spec.operator {
                    Operator::Equal => "==",
                    Operator::NotEqual => "!=",
                    Operator::LessThan => "<",
                    Operator::LessThanOrEqual => "<=",
                    Operator::GreaterThan => ">",
                    Operator::GreaterThanOrEqual => ">=",
                    Operator::Compatible => "~=",
                    Operator::ArbitraryEqual => "===",
                };
                let wildcard = if spec.wildcard { ".*" } else { "" };
                format!("{}{}{}", op, spec.version, wildcard)
            })
            .collect();
        write!(f, "{}", parts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_specifier() {
        let spec = VersionSpecifier::parse(">=1.0").unwrap();
        assert_eq!(spec.operator, Operator::GreaterThanOrEqual);
        assert_eq!(spec.version.release, vec![1, 0]);
    }

    #[test]
    fn test_parse_multiple_specifiers() {
        let specs = VersionSpecifiers::parse(">=1.0,<2.0").unwrap();
        assert_eq!(specs.0.len(), 2);
        assert_eq!(specs.0[0].operator, Operator::GreaterThanOrEqual);
        assert_eq!(specs.0[1].operator, Operator::LessThan);
    }

    #[test]
    fn test_parse_wildcard() {
        let spec = VersionSpecifier::parse("==1.0.*").unwrap();
        assert_eq!(spec.operator, Operator::Equal);
        assert!(spec.wildcard);
        assert_eq!(spec.version.release, vec![1, 0]);
    }

    #[test]
    fn test_contains_ge() {
        let spec = VersionSpecifier::parse(">=1.0").unwrap();
        assert!(spec.contains(&Version::parse("1.0").unwrap()));
        assert!(spec.contains(&Version::parse("1.5").unwrap()));
        assert!(spec.contains(&Version::parse("2.0").unwrap()));
        assert!(!spec.contains(&Version::parse("0.9").unwrap()));
    }

    #[test]
    fn test_contains_lt() {
        let spec = VersionSpecifier::parse("<2.0").unwrap();
        assert!(spec.contains(&Version::parse("1.0").unwrap()));
        assert!(spec.contains(&Version::parse("1.9.9").unwrap()));
        assert!(!spec.contains(&Version::parse("2.0").unwrap()));
        assert!(!spec.contains(&Version::parse("2.1").unwrap()));
    }

    #[test]
    fn test_contains_range() {
        let specs = VersionSpecifiers::parse(">=1.0,<2.0").unwrap();
        assert!(specs.contains(&Version::parse("1.0").unwrap()));
        assert!(specs.contains(&Version::parse("1.5").unwrap()));
        assert!(!specs.contains(&Version::parse("0.9").unwrap()));
        assert!(!specs.contains(&Version::parse("2.0").unwrap()));
    }

    #[test]
    fn test_contains_compatible() {
        let spec = VersionSpecifier::parse("~=1.4.2").unwrap();
        assert!(spec.contains(&Version::parse("1.4.2").unwrap()));
        assert!(spec.contains(&Version::parse("1.4.5").unwrap()));
        assert!(!spec.contains(&Version::parse("1.5.0").unwrap()));
        assert!(!spec.contains(&Version::parse("1.4.1").unwrap()));
    }

    #[test]
    fn test_contains_wildcard() {
        let spec = VersionSpecifier::parse("==1.0.*").unwrap();
        assert!(spec.contains(&Version::parse("1.0.0").unwrap()));
        assert!(spec.contains(&Version::parse("1.0.5").unwrap()));
        assert!(!spec.contains(&Version::parse("1.1.0").unwrap()));
    }

    #[test]
    fn test_to_pubgrub_range_ge() {
        let spec = VersionSpecifier::parse(">=1.0").unwrap();
        let range = spec.to_range();
        assert!(range.contains(&Version::parse("1.0").unwrap()));
        assert!(range.contains(&Version::parse("2.0").unwrap()));
        assert!(!range.contains(&Version::parse("0.9").unwrap()));
    }

    #[test]
    fn test_to_pubgrub_range_lt() {
        let spec = VersionSpecifier::parse("<2.0").unwrap();
        let range = spec.to_range();
        assert!(range.contains(&Version::parse("1.0").unwrap()));
        assert!(!range.contains(&Version::parse("2.0").unwrap()));
    }

    #[test]
    fn test_to_pubgrub_range_combined() {
        let specs = VersionSpecifiers::parse(">=1.0,<2.0").unwrap();
        let range = specs.to_pubgrub_range();
        assert!(range.contains(&Version::parse("1.0").unwrap()));
        assert!(range.contains(&Version::parse("1.5").unwrap()));
        assert!(!range.contains(&Version::parse("0.9").unwrap()));
        assert!(!range.contains(&Version::parse("2.0").unwrap()));
    }

    #[test]
    fn test_any_specifiers() {
        let specs = VersionSpecifiers::any();
        assert!(specs.is_any());
        assert!(specs.contains(&Version::parse("1.0.0").unwrap()));
        assert!(specs.contains(&Version::parse("999.999.999").unwrap()));
    }

    #[test]
    fn test_display() {
        let specs = VersionSpecifiers::parse(">=1.0,<2.0").unwrap();
        assert_eq!(specs.to_string(), ">=1.0,<2.0");
    }
}
