//! Vulnerability types and data structures

use serde::{Deserialize, Serialize};
use std::fmt;

/// Severity level of a vulnerability
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    /// Unknown severity
    Unknown,
    /// Low severity
    Low,
    /// Medium severity
    Medium,
    /// High severity
    High,
    /// Critical severity
    Critical,
}

impl Severity {
    /// Parse severity from string (case-insensitive)
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "CRITICAL" => Severity::Critical,
            "HIGH" => Severity::High,
            "MEDIUM" | "MODERATE" => Severity::Medium,
            "LOW" => Severity::Low,
            _ => Severity::Unknown,
        }
    }

    /// Get emoji for severity
    pub fn emoji(&self) -> &'static str {
        match self {
            Severity::Critical => "\u{1F6A8}", // Police car light
            Severity::High => "\u{1F534}",     // Red circle
            Severity::Medium => "\u{1F7E0}",   // Orange circle
            Severity::Low => "\u{1F7E1}",      // Yellow circle
            Severity::Unknown => "\u{26AA}",   // White circle
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Critical => write!(f, "CRITICAL"),
            Severity::High => write!(f, "HIGH"),
            Severity::Medium => write!(f, "MEDIUM"),
            Severity::Low => write!(f, "LOW"),
            Severity::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

impl Default for Severity {
    fn default() -> Self {
        Severity::Unknown
    }
}

/// A vulnerability affecting a package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    /// Vulnerability ID (e.g., CVE-2023-1234, GHSA-xxxx-xxxx-xxxx, PYSEC-2023-123)
    pub id: String,

    /// Alternative IDs (aliases)
    #[serde(default)]
    pub aliases: Vec<String>,

    /// Summary/title of the vulnerability
    pub summary: String,

    /// Detailed description
    #[serde(default)]
    pub details: String,

    /// Severity level
    pub severity: Severity,

    /// CVSS score (0.0 - 10.0)
    pub cvss_score: Option<f32>,

    /// Affected package name
    pub package: String,

    /// Affected version ranges
    pub affected_versions: Vec<String>,

    /// Fixed version (if available)
    pub fixed_version: Option<String>,

    /// Reference URLs
    #[serde(default)]
    pub references: Vec<String>,

    /// When the vulnerability was published
    pub published: Option<String>,

    /// When the vulnerability was last modified
    pub modified: Option<String>,
}

impl Vulnerability {
    /// Get the CVE ID if available
    pub fn cve_id(&self) -> Option<&str> {
        if self.id.starts_with("CVE-") {
            return Some(&self.id);
        }
        self.aliases.iter().find(|a| a.starts_with("CVE-")).map(|s| s.as_str())
    }

    /// Check if this vulnerability has a known fix
    pub fn has_fix(&self) -> bool {
        self.fixed_version.is_some()
    }
}

/// Result of auditing a single package
#[derive(Debug, Clone)]
pub struct PackageAuditResult {
    /// Package name
    pub name: String,
    /// Installed version
    pub version: String,
    /// Vulnerabilities found
    pub vulnerabilities: Vec<Vulnerability>,
}

impl PackageAuditResult {
    /// Check if the package has any vulnerabilities
    pub fn is_vulnerable(&self) -> bool {
        !self.vulnerabilities.is_empty()
    }

    /// Get the highest severity vulnerability
    pub fn highest_severity(&self) -> Option<Severity> {
        self.vulnerabilities.iter().map(|v| v.severity).max()
    }

    /// Get count of fixable vulnerabilities
    pub fn fixable_count(&self) -> usize {
        self.vulnerabilities.iter().filter(|v| v.has_fix()).count()
    }
}

/// Result of auditing all packages
#[derive(Debug, Clone)]
pub struct AuditReport {
    /// Results for each package
    pub packages: Vec<PackageAuditResult>,
    /// Packages that were ignored
    pub ignored: Vec<String>,
    /// Packages using yanked versions
    pub yanked_packages: Vec<crate::audit::pypi::YankedPackage>,
}

impl AuditReport {
    /// Create a new empty report
    pub fn new() -> Self {
        Self {
            packages: Vec::new(),
            ignored: Vec::new(),
            yanked_packages: Vec::new(),
        }
    }

    /// Check if any packages are using yanked versions
    pub fn has_yanked(&self) -> bool {
        !self.yanked_packages.is_empty()
    }

    /// Get total number of vulnerabilities
    pub fn total_vulnerabilities(&self) -> usize {
        self.packages.iter().map(|p| p.vulnerabilities.len()).sum()
    }

    /// Get count by severity
    pub fn count_by_severity(&self, severity: Severity) -> usize {
        self.packages
            .iter()
            .flat_map(|p| &p.vulnerabilities)
            .filter(|v| v.severity == severity)
            .count()
    }

    /// Get all vulnerable packages
    pub fn vulnerable_packages(&self) -> Vec<&PackageAuditResult> {
        self.packages.iter().filter(|p| p.is_vulnerable()).collect()
    }

    /// Get highest severity across all vulnerabilities
    pub fn highest_severity(&self) -> Option<Severity> {
        self.packages
            .iter()
            .filter_map(|p| p.highest_severity())
            .max()
    }

    /// Check if any vulnerabilities meet or exceed threshold
    pub fn has_severity_at_least(&self, threshold: Severity) -> bool {
        self.packages
            .iter()
            .flat_map(|p| &p.vulnerabilities)
            .any(|v| v.severity >= threshold)
    }

    /// Get fixable vulnerabilities
    pub fn fixable_vulnerabilities(&self) -> Vec<(&str, &Vulnerability)> {
        self.packages
            .iter()
            .flat_map(|p| p.vulnerabilities.iter().filter(|v| v.has_fix()).map(|v| (p.name.as_str(), v)))
            .collect()
    }
}

impl Default for AuditReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for ignoring specific vulnerabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditIgnoreConfig {
    /// List of vulnerability IDs to ignore
    #[serde(default)]
    pub ignore: Vec<IgnoredVulnerability>,
}

/// An ignored vulnerability with optional reason
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoredVulnerability {
    /// Vulnerability ID to ignore
    pub id: String,
    /// Reason for ignoring (for documentation)
    #[serde(default)]
    pub reason: Option<String>,
    /// Expiration date (ISO 8601 format)
    #[serde(default)]
    pub expires: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
        assert!(Severity::Low > Severity::Unknown);
    }

    #[test]
    fn test_severity_from_str() {
        assert_eq!(Severity::from_str("CRITICAL"), Severity::Critical);
        assert_eq!(Severity::from_str("high"), Severity::High);
        assert_eq!(Severity::from_str("Medium"), Severity::Medium);
        assert_eq!(Severity::from_str("MODERATE"), Severity::Medium);
        assert_eq!(Severity::from_str("low"), Severity::Low);
        assert_eq!(Severity::from_str("unknown"), Severity::Unknown);
    }

    #[test]
    fn test_vulnerability_cve_id() {
        let vuln = Vulnerability {
            id: "GHSA-xxxx-xxxx-xxxx".to_string(),
            aliases: vec!["CVE-2023-1234".to_string()],
            summary: "Test".to_string(),
            details: String::new(),
            severity: Severity::High,
            cvss_score: None,
            package: "test".to_string(),
            affected_versions: vec![],
            fixed_version: Some("1.0.1".to_string()),
            references: vec![],
            published: None,
            modified: None,
        };
        assert_eq!(vuln.cve_id(), Some("CVE-2023-1234"));
    }
}
