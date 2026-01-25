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
    /// Format: name[extras] specifier ; markers
    /// Examples:
    ///   - "requests"
    ///   - "requests>=2.0"
    ///   - "requests[security]>=2.0"
    ///   - "requests>=2.0 ; python_version >= '3.8'"
    ///   - "PySocks!=1.5.7,>=1.5.6; extra == 'socks'"
    pub fn parse(s: &str) -> Result<Self, Error> {
        let s = s.trim();
        if s.is_empty() {
            return Err(Error::InvalidDependency(s.to_string()));
        }

        // Split by ';' to separate marker
        let (main_part, marker) = if let Some(semi_pos) = s.find(';') {
            let marker = s[semi_pos + 1..].trim().to_string();
            let main = s[..semi_pos].trim();
            (
                main,
                if marker.is_empty() {
                    None
                } else {
                    Some(marker)
                },
            )
        } else {
            (s, None)
        };

        // Parse the main part: name[extras]specifier
        let mut name = String::new();
        let mut extras = Vec::new();
        let mut specifier = None;
        let mut chars = main_part.chars().peekable();

        // Parse name (alphanumeric, -, _, .)
        while let Some(&c) = chars.peek() {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                name.push(c);
                chars.next();
            } else {
                break;
            }
        }

        if name.is_empty() {
            return Err(Error::InvalidDependency(s.to_string()));
        }

        // Parse extras [extra1,extra2]
        if chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            let mut extra = String::new();
            while let Some(&c) = chars.peek() {
                if c == ']' {
                    chars.next();
                    if !extra.is_empty() {
                        extras.push(extra.trim().to_string());
                    }
                    break;
                } else if c == ',' {
                    chars.next();
                    if !extra.is_empty() {
                        extras.push(extra.trim().to_string());
                        extra = String::new();
                    }
                } else {
                    extra.push(c);
                    chars.next();
                }
            }
        }

        // Skip whitespace
        while chars.peek() == Some(&' ') {
            chars.next();
        }

        // Rest is specifier
        let remaining: String = chars.collect();
        if !remaining.is_empty() {
            specifier = Some(remaining.trim().to_string());
        }

        Ok(Self {
            name,
            specifier,
            extras,
            marker,
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
        assert!(req.marker.is_none());
    }

    #[test]
    fn test_parse_with_version() {
        let req = Requirement::parse("requests>=2.0").unwrap();
        assert_eq!(req.name, "requests");
        assert_eq!(req.specifier, Some(">=2.0".to_string()));
    }

    #[test]
    fn test_parse_with_extras() {
        let req = Requirement::parse("requests[security,socks]>=2.0").unwrap();
        assert_eq!(req.name, "requests");
        assert_eq!(req.extras, vec!["security", "socks"]);
        assert_eq!(req.specifier, Some(">=2.0".to_string()));
    }

    #[test]
    fn test_parse_with_marker() {
        let req = Requirement::parse("requests>=2.0 ; python_version >= '3.8'").unwrap();
        assert_eq!(req.name, "requests");
        assert_eq!(req.specifier, Some(">=2.0".to_string()));
        assert_eq!(req.marker, Some("python_version >= '3.8'".to_string()));
    }

    #[test]
    fn test_parse_pysocks() {
        let req = Requirement::parse("PySocks!=1.5.7,>=1.5.6; extra == 'socks'").unwrap();
        assert_eq!(req.name, "PySocks");
        assert_eq!(req.specifier, Some("!=1.5.7,>=1.5.6".to_string()));
        assert_eq!(req.marker, Some("extra == 'socks'".to_string()));
    }

    #[test]
    fn test_display() {
        let req = Requirement::new("requests").with_specifier(">=2.0");
        assert_eq!(req.to_string(), "requests>=2.0");
    }

    #[test]
    fn test_display_with_marker() {
        let req = Requirement::new("requests")
            .with_specifier(">=2.0")
            .with_marker("python_version >= '3.8'");
        assert_eq!(req.to_string(), "requests>=2.0 ; python_version >= '3.8'");
    }
}
