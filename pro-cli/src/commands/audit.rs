//! Audit command - check for security vulnerabilities

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;

use pro_core::audit::{AuditConfig, IgnoredVulnerability};
use pro_core::pep::PyProject;
use pro_core::{AuditReport, Auditor, Lockfile, Severity};

#[derive(Args)]
pub struct AuditCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Automatically fix vulnerabilities by updating packages
    #[arg(long)]
    pub fix: bool,

    /// Force apply fixes even if they require breaking changes
    #[arg(long)]
    pub force: bool,

    /// Minimum severity level to report/fail on (unknown, low, medium, high, critical)
    #[arg(long, default_value = "unknown")]
    pub severity: String,

    /// Output format (text, json)
    #[arg(long, default_value = "text")]
    pub format: String,

    /// Vulnerability IDs to ignore (comma-separated)
    #[arg(long)]
    pub ignore: Option<String>,

    /// Skip checking for yanked package versions
    #[arg(long)]
    pub no_yanked: bool,
}

impl AuditCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let lockfile_path = project_dir.join("rx.lock");

        // Load lockfile
        let lockfile = Lockfile::load(&lockfile_path).with_context(|| {
            format!(
                "No rx.lock found in {:?}. Run 'rx lock' or 'rx add <package>' first.",
                project_dir
            )
        })?;

        if lockfile.is_empty() {
            println!("No packages to audit (lockfile is empty).");
            return Ok(());
        }

        // Parse severity threshold
        let severity_threshold = Severity::from_str(&self.severity);

        // Parse ignored vulnerabilities from CLI
        let cli_ignored: Vec<String> = self
            .ignore
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        // Load config from pyproject.toml
        let config_ignores = load_audit_config(&project_dir);

        // Build audit config
        let config = AuditConfig {
            ignored_ids: cli_ignored.into_iter().collect(),
            ignored_vulnerabilities: config_ignores,
            check_yanked: !self.no_yanked,
        };

        // Create auditor
        let auditor = Auditor::with_config(config);

        // Run audit
        println!(
            "Auditing {} packages for vulnerabilities...",
            lockfile.len()
        );
        let report = auditor.audit_lockfile(&lockfile).await?;

        // Display results
        if self.format == "json" {
            print_json_report(&report)?;
        } else {
            print_text_report(&report, severity_threshold);
        }

        // Handle fixes if requested
        if self.fix && report.total_vulnerabilities() > 0 {
            println!();
            println!("Attempting to fix vulnerabilities...");

            let fixes = auditor.generate_fixes(&report, &lockfile).await?;

            if fixes.is_empty() {
                println!("No automatic fixes available for the detected vulnerabilities.");
            } else {
                println!("Found {} potential fixes:", fixes.len());
                for fix in &fixes {
                    println!(
                        "  {} {} -> {} (fixes {})",
                        fix.package, fix.current_version, fix.fixed_version, fix.vulnerability_id
                    );
                }

                let fix_result = auditor.apply_fixes(&fixes, &lockfile, self.force).await?;

                if fix_result.needs_force() && !self.force {
                    println!();
                    println!("The following transitive updates are required:");
                    for change in &fix_result.requires_force {
                        println!("  {}", change);
                    }
                    println!();
                    println!("Use --force to apply these changes at your own risk.");
                    std::process::exit(1);
                }

                if fix_result.has_changes() {
                    // Save the updated lockfile
                    fix_result
                        .updated_lockfile
                        .save(&lockfile_path)
                        .context("Failed to save updated lockfile")?;

                    println!();
                    println!("Applied fixes:");
                    for fix in &fix_result.applied_fixes {
                        println!(
                            "  {} {} -> {} (fixed {})",
                            fix.package, fix.from_version, fix.to_version, fix.vulnerability_id
                        );
                    }
                    println!();
                    println!("Lockfile updated. Run 'rx sync' to install the fixed versions.");
                }

                if !fix_result.failed_fixes.is_empty() {
                    println!();
                    println!("Failed to fix:");
                    for fix in &fix_result.failed_fixes {
                        println!(
                            "  {} -> {}: {}",
                            fix.package, fix.target_version, fix.reason
                        );
                    }
                }
            }
        }

        // Exit with appropriate code
        let has_vulnerabilities_above_threshold = report.has_severity_at_least(severity_threshold);
        let has_yanked = report.has_yanked();

        if (has_vulnerabilities_above_threshold || has_yanked) && !self.fix {
            std::process::exit(1);
        }

        Ok(())
    }
}

fn print_text_report(report: &AuditReport, threshold: Severity) {
    let vulnerable_packages = report.vulnerable_packages();
    let has_vulnerabilities = !vulnerable_packages.is_empty();
    let has_yanked = report.has_yanked();

    if !has_vulnerabilities && !has_yanked {
        println!();
        println!("No vulnerabilities found.");
        return;
    }

    // Print vulnerability summary
    if has_vulnerabilities {
        let total = report.total_vulnerabilities();
        let critical = report.count_by_severity(Severity::Critical);
        let high = report.count_by_severity(Severity::High);
        let medium = report.count_by_severity(Severity::Medium);
        let low = report.count_by_severity(Severity::Low);

        println!();
        println!(
            "Found {} vulnerabilities ({} critical, {} high, {} medium, {} low)",
            total, critical, high, medium, low
        );
        println!();

        for pkg in vulnerable_packages {
            // Filter vulnerabilities by threshold
            let vulns: Vec<_> = pkg
                .vulnerabilities
                .iter()
                .filter(|v| v.severity >= threshold)
                .collect();

            if vulns.is_empty() {
                continue;
            }

            println!("{}@{}", pkg.name, pkg.version);
            for vuln in vulns {
                let fix_info = vuln
                    .fixed_version
                    .as_ref()
                    .map(|v| format!(" (fix: {})", v))
                    .unwrap_or_default();

                let cve = vuln.cve_id().unwrap_or(&vuln.id);

                println!(
                    "  {} {} - {}{}",
                    vuln.severity.emoji(),
                    cve,
                    vuln.summary,
                    fix_info
                );

                // Show references
                if !vuln.references.is_empty() {
                    if let Some(ref_url) = vuln.references.first() {
                        println!("    {}", ref_url);
                    }
                }
            }
            println!();
        }

        // Show summary
        let fixable = report.fixable_vulnerabilities().len();
        if fixable > 0 {
            println!(
                "{} vulnerabilities have fixes available. Run 'rx audit --fix' to update.",
                fixable
            );
        }
    }

    // Print yanked packages
    if has_yanked {
        println!();
        println!("Yanked packages ({} found):", report.yanked_packages.len());
        println!();
        for yanked in &report.yanked_packages {
            let reason = yanked
                .reason
                .as_ref()
                .map(|r| format!(" - {}", r))
                .unwrap_or_default();
            println!("  {}@{}{}", yanked.name, yanked.version, reason);
        }
        println!();
        println!("Yanked packages may have security issues or critical bugs.");
        println!("Consider updating to a non-yanked version.");
    }

    // Show ignored
    if !report.ignored.is_empty() {
        println!();
        println!("Ignored: {}", report.ignored.join(", "));
    }
}

fn print_json_report(report: &AuditReport) -> Result<()> {
    #[derive(serde::Serialize)]
    struct JsonReport {
        total: usize,
        critical: usize,
        high: usize,
        medium: usize,
        low: usize,
        packages: Vec<JsonPackage>,
        yanked: Vec<JsonYanked>,
    }

    #[derive(serde::Serialize)]
    struct JsonPackage {
        name: String,
        version: String,
        vulnerabilities: Vec<JsonVuln>,
    }

    #[derive(serde::Serialize)]
    struct JsonVuln {
        id: String,
        severity: String,
        summary: String,
        fixed_version: Option<String>,
    }

    #[derive(serde::Serialize)]
    struct JsonYanked {
        name: String,
        version: String,
        reason: Option<String>,
    }

    let json_report = JsonReport {
        total: report.total_vulnerabilities(),
        critical: report.count_by_severity(Severity::Critical),
        high: report.count_by_severity(Severity::High),
        medium: report.count_by_severity(Severity::Medium),
        low: report.count_by_severity(Severity::Low),
        packages: report
            .vulnerable_packages()
            .iter()
            .map(|p| JsonPackage {
                name: p.name.clone(),
                version: p.version.clone(),
                vulnerabilities: p
                    .vulnerabilities
                    .iter()
                    .map(|v| JsonVuln {
                        id: v.id.clone(),
                        severity: v.severity.to_string(),
                        summary: v.summary.clone(),
                        fixed_version: v.fixed_version.clone(),
                    })
                    .collect(),
            })
            .collect(),
        yanked: report
            .yanked_packages
            .iter()
            .map(|y| JsonYanked {
                name: y.name.clone(),
                version: y.version.clone(),
                reason: y.reason.clone(),
            })
            .collect(),
    };

    println!("{}", serde_json::to_string_pretty(&json_report)?);
    Ok(())
}

/// Load audit configuration from pyproject.toml [tool.rx.audit]
fn load_audit_config(project_dir: &Path) -> Vec<IgnoredVulnerability> {
    let pyproject = match PyProject::load(project_dir) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    let rx_config = match pyproject.tool.get("rx") {
        Some(c) => c,
        None => return Vec::new(),
    };

    let audit_config = match rx_config.get("audit") {
        Some(c) => c,
        None => return Vec::new(),
    };

    let ignore_array = match audit_config.get("ignore") {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    let arr = match ignore_array.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    let mut ignores = Vec::new();

    for item in arr {
        // Support both string format and table format
        if let Some(id) = item.as_str() {
            // Simple string: just the ID
            ignores.push(IgnoredVulnerability {
                id: id.to_string(),
                reason: None,
                expires: None,
            });
        } else if let Some(table) = item.as_table() {
            // Table format with id, reason, expires
            let id = table
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if id.is_empty() {
                continue;
            }

            let reason = table
                .get("reason")
                .and_then(|v| v.as_str())
                .map(String::from);

            let expires = table
                .get("expires")
                .and_then(|v| v.as_str())
                .map(String::from);

            ignores.push(IgnoredVulnerability {
                id,
                reason,
                expires,
            });
        }
    }

    ignores
}
