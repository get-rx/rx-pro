//! OSV (Open Source Vulnerabilities) API client
//!
//! Documentation: https://osv.dev/docs/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{Severity, Vulnerability};
use crate::{Error, Result};

/// OSV API base URL
const OSV_API_URL: &str = "https://api.osv.dev/v1";

/// OSV API client
pub struct OsvClient {
    client: reqwest::Client,
}

impl OsvClient {
    /// Create a new OSV client
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Pro/0.1.0")
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Query vulnerabilities for a single package
    pub async fn query(&self, package: &str, version: &str) -> Result<Vec<Vulnerability>> {
        let request = OsvQueryRequest {
            package: OsvPackage {
                name: package.to_string(),
                ecosystem: "PyPI".to_string(),
            },
            version: version.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/query", OSV_API_URL))
            .json(&request)
            .send()
            .await
            .map_err(Error::Network)?;

        if !response.status().is_success() {
            return Err(Error::Index(format!(
                "OSV API error: HTTP {}",
                response.status()
            )));
        }

        let osv_response: OsvQueryResponse = response.json().await.map_err(Error::Network)?;

        Ok(osv_response
            .vulns
            .unwrap_or_default()
            .into_iter()
            .map(|v| convert_osv_vuln(v, package))
            .collect())
    }

    /// Query vulnerabilities for multiple packages in batch
    /// Uses batch API for detection, then fetches full details for affected packages
    pub async fn query_batch(
        &self,
        packages: &[(&str, &str)], // (name, version)
    ) -> Result<HashMap<String, Vec<Vulnerability>>> {
        if packages.is_empty() {
            return Ok(HashMap::new());
        }

        // First, use batch API to detect which packages have vulnerabilities
        let queries: Vec<OsvBatchQuery> = packages
            .iter()
            .map(|(name, version)| OsvBatchQuery {
                package: OsvPackage {
                    name: name.to_string(),
                    ecosystem: "PyPI".to_string(),
                },
                version: version.to_string(),
            })
            .collect();

        let request = OsvBatchRequest { queries };

        let response = self
            .client
            .post(format!("{}/querybatch", OSV_API_URL))
            .json(&request)
            .send()
            .await
            .map_err(Error::Network)?;

        if !response.status().is_success() {
            return Err(Error::Index(format!(
                "OSV API error: HTTP {}",
                response.status()
            )));
        }

        let batch_response: OsvBatchResponse = response.json().await.map_err(Error::Network)?;

        // Identify packages with vulnerabilities
        let mut vulnerable_packages: Vec<(&str, &str)> = Vec::new();
        for (i, result) in batch_response.results.iter().enumerate() {
            if i < packages.len() && result.vulns.as_ref().is_some_and(|v| !v.is_empty()) {
                vulnerable_packages.push(packages[i]);
            }
        }

        // Fetch full details for vulnerable packages using single query API
        let mut results = HashMap::new();
        for (name, version) in vulnerable_packages {
            match self.query(name, version).await {
                Ok(vulns) => {
                    if !vulns.is_empty() {
                        results.insert(name.to_string(), vulns);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch vulnerability details for {}: {}", name, e);
                }
            }
        }

        Ok(results)
    }
}

impl Default for OsvClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert OSV vulnerability to our Vulnerability type
fn convert_osv_vuln(osv: OsvVulnerability, package: &str) -> Vulnerability {
    // Extract severity from database_specific or severity field
    let (severity, cvss_score) = extract_severity(&osv);

    // Find the fixed version for this package
    // First try to match by package name, then fallback to any fixed version
    let fixed_version = osv
        .affected
        .iter()
        .filter(|a| {
            a.package
                .as_ref()
                .map(|p| p.name.to_lowercase() == package.to_lowercase())
                .unwrap_or(true) // Include entries without package info
        })
        .flat_map(|a| &a.ranges)
        .flat_map(|r| &r.events)
        .find_map(|e| e.fixed.clone());

    // Collect affected versions
    let affected_versions: Vec<String> = osv
        .affected
        .iter()
        .filter(|a| {
            a.package
                .as_ref()
                .map(|p| p.name.to_lowercase() == package.to_lowercase())
                .unwrap_or(false)
        })
        .flat_map(|a| &a.versions)
        .cloned()
        .collect();

    Vulnerability {
        id: osv.id.clone(),
        aliases: osv.aliases.unwrap_or_default(),
        summary: osv.summary.unwrap_or_else(|| osv.id.clone()),
        details: osv.details.unwrap_or_default(),
        severity,
        cvss_score,
        package: package.to_string(),
        affected_versions,
        fixed_version,
        references: osv
            .references
            .unwrap_or_default()
            .into_iter()
            .map(|r| r.url)
            .collect(),
        published: osv.published,
        modified: osv.modified,
    }
}

/// Extract severity and CVSS score from OSV vulnerability
fn extract_severity(osv: &OsvVulnerability) -> (Severity, Option<f32>) {
    // Try to get severity from the severity field first (CVSS score)
    if let Some(severities) = &osv.severity {
        for sev in severities {
            // Try parsing as CVSS vector string (e.g., "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:L/A:N")
            if sev.severity_type == "CVSS_V3" || sev.severity_type == "CVSS_V2" {
                // Try to extract base score from CVSS vector
                if let Some(score) = parse_cvss_score(&sev.score) {
                    let severity = cvss_to_severity(score);
                    return (severity, Some(score));
                }
                // Try parsing as direct score
                if let Ok(score) = sev.score.parse::<f32>() {
                    let severity = cvss_to_severity(score);
                    return (severity, Some(score));
                }
            }
        }
    }

    // Try database_specific.severity (GHSA uses this)
    if let Some(db_specific) = &osv.database_specific {
        if let Some(severity_str) = &db_specific.severity {
            return (severity_str.parse().unwrap_or(Severity::Unknown), None);
        }
        // Try CVSS score in database_specific
        if let Some(score) = db_specific.cvss_score {
            let severity = cvss_to_severity(score);
            return (severity, Some(score));
        }
        // Try cvss object with score field
        if let Some(cvss) = &db_specific.cvss {
            if let Some(score) = cvss.get("score").and_then(|v| v.as_f64()) {
                let severity = cvss_to_severity(score as f32);
                return (severity, Some(score as f32));
            }
        }
    }

    // Try to extract from affected packages' severity
    for affected in &osv.affected {
        if let Some(sev) = &affected.database_specific {
            if let Some(severity_str) = sev.get("severity").and_then(|v| v.as_str()) {
                return (severity_str.parse().unwrap_or(Severity::Unknown), None);
            }
        }
    }

    (Severity::Unknown, None)
}

/// Try to parse CVSS score from vector string
fn parse_cvss_score(vector: &str) -> Option<f32> {
    // CVSS vectors sometimes include the score at the end or can be parsed
    // For now, just check if it's a plain score
    if let Ok(score) = vector.parse::<f32>() {
        return Some(score);
    }
    None
}

/// Convert CVSS score to severity level
fn cvss_to_severity(score: f32) -> Severity {
    match score {
        s if s >= 9.0 => Severity::Critical,
        s if s >= 7.0 => Severity::High,
        s if s >= 4.0 => Severity::Medium,
        s if s > 0.0 => Severity::Low,
        _ => Severity::Unknown,
    }
}

// OSV API request/response types

#[derive(Debug, Serialize)]
struct OsvQueryRequest {
    package: OsvPackage,
    version: String,
}

#[derive(Debug, Serialize)]
struct OsvPackage {
    name: String,
    ecosystem: String,
}

#[derive(Debug, Deserialize)]
struct OsvQueryResponse {
    vulns: Option<Vec<OsvVulnerability>>,
}

#[derive(Debug, Serialize)]
struct OsvBatchRequest {
    queries: Vec<OsvBatchQuery>,
}

#[derive(Debug, Serialize)]
struct OsvBatchQuery {
    package: OsvPackage,
    version: String,
}

#[derive(Debug, Deserialize)]
struct OsvBatchResponse {
    results: Vec<OsvBatchResult>,
}

#[derive(Debug, Deserialize)]
struct OsvBatchResult {
    vulns: Option<Vec<OsvVulnerability>>,
}

#[derive(Debug, Deserialize)]
struct OsvVulnerability {
    id: String,
    aliases: Option<Vec<String>>,
    summary: Option<String>,
    details: Option<String>,
    severity: Option<Vec<OsvSeverity>>,
    #[serde(default)]
    affected: Vec<OsvAffected>,
    references: Option<Vec<OsvReference>>,
    database_specific: Option<OsvDatabaseSpecific>,
    published: Option<String>,
    modified: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OsvSeverity {
    #[serde(rename = "type")]
    severity_type: String,
    score: String,
}

#[derive(Debug, Deserialize)]
struct OsvAffected {
    package: Option<OsvAffectedPackage>,
    #[serde(default)]
    ranges: Vec<OsvRange>,
    #[serde(default)]
    versions: Vec<String>,
    #[serde(default)]
    database_specific: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OsvAffectedPackage {
    name: String,
    #[allow(dead_code)]
    ecosystem: String,
}

#[derive(Debug, Deserialize)]
struct OsvRange {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    range_type: String,
    events: Vec<OsvEvent>,
}

#[derive(Debug, Deserialize)]
struct OsvEvent {
    #[allow(dead_code)]
    introduced: Option<String>,
    fixed: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OsvReference {
    url: String,
}

#[derive(Debug, Deserialize)]
struct OsvDatabaseSpecific {
    severity: Option<String>,
    #[serde(default)]
    cvss_score: Option<f32>,
    #[serde(default)]
    cvss: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cvss_to_severity() {
        assert_eq!(cvss_to_severity(9.5), Severity::Critical);
        assert_eq!(cvss_to_severity(9.0), Severity::Critical);
        assert_eq!(cvss_to_severity(8.0), Severity::High);
        assert_eq!(cvss_to_severity(7.0), Severity::High);
        assert_eq!(cvss_to_severity(5.0), Severity::Medium);
        assert_eq!(cvss_to_severity(4.0), Severity::Medium);
        assert_eq!(cvss_to_severity(2.0), Severity::Low);
        assert_eq!(cvss_to_severity(0.0), Severity::Unknown);
    }

    #[tokio::test]
    #[ignore] // Requires network
    async fn test_query_known_vulnerable_package() {
        let client = OsvClient::new();
        // urllib3 < 1.26.5 has known vulnerabilities
        let vulns = client.query("urllib3", "1.26.0").await.unwrap();
        // Should find at least one vulnerability
        assert!(!vulns.is_empty());
    }
}
