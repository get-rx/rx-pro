//! Package representation for the resolver

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// A Python package identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Package {
    /// Normalized package name
    pub name: String,
}

impl PartialOrd for Package {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Package {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl Package {
    /// Create a new package with normalized name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Self::normalize(name.into()),
        }
    }

    /// Normalize a package name according to PEP 503
    /// - Lowercase
    /// - Replace runs of [-_.] with single hyphen
    fn normalize(name: String) -> String {
        let mut result = String::with_capacity(name.len());
        let mut prev_was_separator = false;

        for c in name.chars() {
            match c {
                '-' | '_' | '.' => {
                    if !prev_was_separator && !result.is_empty() {
                        result.push('-');
                        prev_was_separator = true;
                    }
                }
                c => {
                    result.push(c.to_ascii_lowercase());
                    prev_was_separator = false;
                }
            }
        }

        // Remove trailing separator
        if result.ends_with('-') {
            result.pop();
        }

        result
    }
}

impl std::fmt::Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_simple() {
        assert_eq!(Package::new("requests").name, "requests");
    }

    #[test]
    fn test_normalize_uppercase() {
        assert_eq!(Package::new("Requests").name, "requests");
    }

    #[test]
    fn test_normalize_underscore() {
        assert_eq!(Package::new("my_package").name, "my-package");
    }

    #[test]
    fn test_normalize_dots() {
        assert_eq!(Package::new("zope.interface").name, "zope-interface");
    }

    #[test]
    fn test_normalize_mixed() {
        assert_eq!(Package::new("My__Package..Name").name, "my-package-name");
    }
}
