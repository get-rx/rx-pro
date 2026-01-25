//! Security audit module for vulnerability checking
//!
//! Checks packages against vulnerability databases and provides
//! automated fix recommendations.

pub mod osv;
pub mod types;

pub use osv::OsvClient;
pub use types::*;

use std::collections::{HashMap, HashSet};

use crate::lockfile::Lockfile;
use crate::resolver::Resolver;
use crate::pep::Requirement;
use crate::Result;

/// Auditor for checking package vulnerabilities
pub struct Auditor {
    /// OSV client for vulnerability queries
    osv_client: OsvClient,
    /// Set of vulnerability IDs to ignore
    ignored_ids: HashSet<String>,
}

impl Auditor {
    /// Create a new auditor
    pub fn new() -> Self {
        Self {
            osv_client: OsvClient::new(),
            ignored_ids: HashSet::new(),
        }
    }

    /// Create an auditor with ignored vulnerabilities
    pub fn with_ignored(ignored: Vec<String>) -> Self {
        Self {
            osv_client: OsvClient::new(),
            ignored_ids: ignored.into_iter().collect(),
        }
    }

    /// Add vulnerability IDs to ignore
    pub fn ignore(&mut self, ids: impl IntoIterator<Item = String>) {
        self.ignored_ids.extend(ids);
    }

    /// Audit packages from a lockfile
    pub async fn audit_lockfile(&self, lockfile: &Lockfile) -> Result<AuditReport> {
        let packages: Vec<(&str, &str)> = lockfile
            .packages
            .iter()
            .map(|(name, pkg)| (name.as_str(), pkg.version.as_str()))
            .collect();

        self.audit_packages(&packages).await
    }

    /// Audit a list of packages
    pub async fn audit_packages(&self, packages: &[(&str, &str)]) -> Result<AuditReport> {
        tracing::info!("Checking {} packages for vulnerabilities...", packages.len());

        // Query OSV for all packages
        let vuln_map = self.osv_client.query_batch(packages).await?;

        let mut report = AuditReport::new();

        for (name, version) in packages {
            let vulnerabilities: Vec<Vulnerability> = vuln_map
                .get(*name)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|v| !self.ignored_ids.contains(&v.id))
                .filter(|v| !v.aliases.iter().any(|a| self.ignored_ids.contains(a)))
                .collect();

            let ignored_count = vuln_map
                .get(*name)
                .map(|vs| vs.len())
                .unwrap_or(0)
                - vulnerabilities.len();

            if ignored_count > 0 {
                report.ignored.push(format!(
                    "{} ({} ignored)",
                    name, ignored_count
                ));
            }

            report.packages.push(PackageAuditResult {
                name: name.to_string(),
                version: version.to_string(),
                vulnerabilities,
            });
        }

        Ok(report)
    }

    /// Generate fix recommendations for vulnerable packages
    pub async fn generate_fixes(
        &self,
        report: &AuditReport,
        _lockfile: &Lockfile,
    ) -> Result<Vec<FixRecommendation>> {
        let mut fixes = Vec::new();

        for pkg_result in report.vulnerable_packages() {
            for vuln in &pkg_result.vulnerabilities {
                if let Some(fixed_version) = &vuln.fixed_version {
                    fixes.push(FixRecommendation {
                        package: pkg_result.name.clone(),
                        current_version: pkg_result.version.clone(),
                        fixed_version: fixed_version.clone(),
                        vulnerability_id: vuln.id.clone(),
                        severity: vuln.severity,
                        requires_parent_update: false, // Will be determined during resolution
                    });
                }
            }
        }

        // Deduplicate fixes per package (take highest fixed version)
        let mut package_fixes: HashMap<String, FixRecommendation> = HashMap::new();
        for fix in fixes {
            package_fixes
                .entry(fix.package.clone())
                .and_modify(|existing| {
                    // Keep the fix with higher version or severity
                    if fix.severity > existing.severity {
                        *existing = fix.clone();
                    }
                })
                .or_insert(fix);
        }

        Ok(package_fixes.into_values().collect())
    }

    /// Apply fixes by re-resolving dependencies
    pub async fn apply_fixes(
        &self,
        fixes: &[FixRecommendation],
        lockfile: &Lockfile,
        force: bool,
    ) -> Result<FixResult> {
        if fixes.is_empty() {
            return Ok(FixResult {
                updated_lockfile: lockfile.clone(),
                applied_fixes: vec![],
                failed_fixes: vec![],
                requires_force: vec![],
            });
        }

        // Build new requirements with minimum versions from fixes
        let mut requirements: Vec<Requirement> = Vec::new();
        let mut min_versions: HashMap<String, String> = HashMap::new();

        for fix in fixes {
            min_versions.insert(fix.package.clone(), fix.fixed_version.clone());
        }

        // Create requirements for all packages, with minimum versions for vulnerable ones
        for (name, pkg) in &lockfile.packages {
            let version_spec = if let Some(min_ver) = min_versions.get(name) {
                format!(">={}", min_ver)
            } else {
                format!(">={}", pkg.version)
            };

            let req_str = format!("{}{}", name, version_spec);
            match Requirement::parse(&req_str) {
                Ok(req) => requirements.push(req),
                Err(e) => {
                    tracing::warn!("Failed to parse requirement {}: {}", req_str, e);
                }
            }
        }

        // Try to resolve with new constraints
        let resolver = Resolver::new();
        match resolver.resolve(&requirements).await {
            Ok(resolution) => {
                let new_lockfile = Lockfile::from_resolution(&resolution);

                // Determine which fixes were applied
                let mut applied_fixes = Vec::new();
                let mut requires_force = Vec::new();

                for fix in fixes {
                    if let Some(new_pkg) = new_lockfile.packages.get(&fix.package) {
                        if new_pkg.version != fix.current_version {
                            applied_fixes.push(AppliedFix {
                                package: fix.package.clone(),
                                from_version: fix.current_version.clone(),
                                to_version: new_pkg.version.clone(),
                                vulnerability_id: fix.vulnerability_id.clone(),
                            });
                        }
                    }
                }

                // Check for packages that changed unexpectedly (might need force)
                for (name, new_pkg) in &new_lockfile.packages {
                    if let Some(old_pkg) = lockfile.packages.get(name) {
                        if new_pkg.version != old_pkg.version && !min_versions.contains_key(name) {
                            requires_force.push(format!(
                                "{}: {} -> {} (transitive update)",
                                name, old_pkg.version, new_pkg.version
                            ));
                        }
                    }
                }

                // If there are transitive updates and force is not set, return early
                if !requires_force.is_empty() && !force {
                    return Ok(FixResult {
                        updated_lockfile: lockfile.clone(),
                        applied_fixes: vec![],
                        failed_fixes: vec![],
                        requires_force,
                    });
                }

                Ok(FixResult {
                    updated_lockfile: new_lockfile,
                    applied_fixes,
                    failed_fixes: vec![],
                    requires_force: vec![],
                })
            }
            Err(e) => {
                // Resolution failed - all fixes failed
                let failed_fixes: Vec<FailedFix> = fixes
                    .iter()
                    .map(|f| FailedFix {
                        package: f.package.clone(),
                        target_version: f.fixed_version.clone(),
                        reason: format!("Resolution failed: {}", e),
                    })
                    .collect();

                Ok(FixResult {
                    updated_lockfile: lockfile.clone(),
                    applied_fixes: vec![],
                    failed_fixes,
                    requires_force: vec![],
                })
            }
        }
    }
}

impl Default for Auditor {
    fn default() -> Self {
        Self::new()
    }
}

/// Recommendation to fix a vulnerability
#[derive(Debug, Clone)]
pub struct FixRecommendation {
    /// Package name
    pub package: String,
    /// Current installed version
    pub current_version: String,
    /// Version that fixes the vulnerability
    pub fixed_version: String,
    /// Vulnerability ID being fixed
    pub vulnerability_id: String,
    /// Severity of the vulnerability
    pub severity: Severity,
    /// Whether parent packages need updating too
    pub requires_parent_update: bool,
}

/// Result of applying fixes
#[derive(Debug, Clone)]
pub struct FixResult {
    /// Updated lockfile with fixes applied
    pub updated_lockfile: Lockfile,
    /// Successfully applied fixes
    pub applied_fixes: Vec<AppliedFix>,
    /// Fixes that failed to apply
    pub failed_fixes: Vec<FailedFix>,
    /// Changes that require --force flag
    pub requires_force: Vec<String>,
}

impl FixResult {
    /// Check if any fixes were applied
    pub fn has_changes(&self) -> bool {
        !self.applied_fixes.is_empty()
    }

    /// Check if force is required
    pub fn needs_force(&self) -> bool {
        !self.requires_force.is_empty()
    }
}

/// A successfully applied fix
#[derive(Debug, Clone)]
pub struct AppliedFix {
    /// Package name
    pub package: String,
    /// Previous version
    pub from_version: String,
    /// New version
    pub to_version: String,
    /// Vulnerability ID that was fixed
    pub vulnerability_id: String,
}

/// A fix that failed to apply
#[derive(Debug, Clone)]
pub struct FailedFix {
    /// Package name
    pub package: String,
    /// Target version that couldn't be installed
    pub target_version: String,
    /// Reason for failure
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auditor_ignore() {
        let mut auditor = Auditor::new();
        auditor.ignore(["CVE-2023-1234".to_string()]);
        assert!(auditor.ignored_ids.contains("CVE-2023-1234"));
    }

    #[test]
    fn test_fix_result_has_changes() {
        let result = FixResult {
            updated_lockfile: Lockfile::new(),
            applied_fixes: vec![AppliedFix {
                package: "test".to_string(),
                from_version: "1.0.0".to_string(),
                to_version: "1.0.1".to_string(),
                vulnerability_id: "CVE-2023-1234".to_string(),
            }],
            failed_fixes: vec![],
            requires_force: vec![],
        };
        assert!(result.has_changes());
        assert!(!result.needs_force());
    }
}
