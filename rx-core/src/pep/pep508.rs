//! PEP 508 - Dependency specification for Python packages

use serde::{Deserialize, Serialize};

use crate::Error;

/// A PEP 508 dependency requirement
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirement {
    /// Package name
    pub name: String,
    /// Version specifiers (e.g., ">=1.0,<2.0")
    pub specifier: Option<String>,
    /// Extras (e.g., [dev, test])
    pub extras: Vec<String>,
    /// Environment markers (e.g., python_version >= "3.8")
    pub marker: Option<String>,
    /// URL for direct references
    pub url: Option<String>,
}

impl Requirement {
    /// Create a simple requirement with just a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            specifier: None,
            extras: vec![],
            marker: None,
            url: None,
        }
    }

    /// Parse a PEP 508 requirement string
    pub fn parse(s: &str) -> Result<Self, Error> {
        // TODO: Implement full PEP 508 parsing
        // For now, handle simple cases like "package>=1.0"

        let s = s.trim();

        // Find where the name ends
        let name_end = s
            .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.')
            .unwrap_or(s.len());

        let name = s[..name_end].to_string();
        if name.is_empty() {
            return Err(Error::InvalidDependency(s.to_string()));
        }

        let specifier = if name_end < s.len() {
            Some(s[name_end..].to_string())
        } else {
            None
        };

        Ok(Self {
            name,
            specifier,
            extras: vec![],
            marker: None,
            url: None,
        })
    }

    /// Add an extra
    pub fn with_extra(mut self, extra: impl Into<String>) -> Self {
        self.extras.push(extra.into());
        self
    }

    /// Set the version specifier
    pub fn with_specifier(mut self, specifier: impl Into<String>) -> Self {
        self.specifier = Some(specifier.into());
        self
    }

    /// Set the environment marker
    pub fn with_marker(mut self, marker: impl Into<String>) -> Self {
        self.marker = Some(marker.into());
        self
    }
}

impl std::fmt::Display for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;

        if !self.extras.is_empty() {
            write!(f, "[{}]", self.extras.join(","))?;
        }

        if let Some(ref spec) = self.specifier {
            write!(f, "{}", spec)?;
        }

        if let Some(ref marker) = self.marker {
            write!(f, " ; {}", marker)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let req = Requirement::parse("requests").unwrap();
        assert_eq!(req.name, "requests");
        assert!(req.specifier.is_none());
    }

    #[test]
    fn test_parse_with_version() {
        let req = Requirement::parse("requests>=2.0").unwrap();
        assert_eq!(req.name, "requests");
        assert_eq!(req.specifier, Some(">=2.0".to_string()));
    }

    #[test]
    fn test_display() {
        let req = Requirement::new("requests").with_specifier(">=2.0");
        assert_eq!(req.to_string(), "requests>=2.0");
    }
}
