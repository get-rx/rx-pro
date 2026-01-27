//! Update command - update dependencies to latest versions within constraints
//!
//! Supports updating to specific versions:
//! - `rx update` - update all packages to latest
//! - `rx update requests` - update specific package to latest
//! - `rx update requests@2.31.0` - update to specific version
//! - `rx update requests==2.31.0` - update to specific version (pip syntax)

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use tracing::info;

use pro_core::pep::{PyProject, Requirement};
use pro_core::resolver::Resolver;
use pro_core::Lockfile;

#[derive(Args)]
pub struct UpdateCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Specific packages to update (updates all if not specified)
    /// Use package@version or package==version to pin to specific version
    #[arg()]
    pub packages: Vec<String>,

    /// Include development dependencies
    #[arg(long)]
    pub dev: bool,

    /// Show what would be updated without making changes
    #[arg(long)]
    pub dry_run: bool,
}

/// Parsed package specification from CLI
struct PackageSpec {
    name: String,
    version: Option<String>,
}

/// Parse package specification like "requests", "requests@2.31.0", or "requests==2.31.0"
fn parse_package_spec(spec: &str) -> PackageSpec {
    // Handle @ syntax: requests@2.31.0
    if let Some(at_pos) = spec.find('@') {
        return PackageSpec {
            name: spec[..at_pos].to_string(),
            version: Some(spec[at_pos + 1..].to_string()),
        };
    }

    // Handle == syntax: requests==2.31.0
    if let Some(eq_pos) = spec.find("==") {
        return PackageSpec {
            name: spec[..eq_pos].to_string(),
            version: Some(spec[eq_pos + 2..].to_string()),
        };
    }

    // Handle >= syntax: requests>=2.31.0 (just extract name)
    if let Some(pos) = spec.find(">=") {
        return PackageSpec {
            name: spec[..pos].to_string(),
            version: None,
        };
    }

    // Handle > syntax: requests>2.31.0 (just extract name)
    if let Some(pos) = spec.find('>') {
        return PackageSpec {
            name: spec[..pos].to_string(),
            version: None,
        };
    }

    // No version specified
    PackageSpec {
        name: spec.to_string(),
        version: None,
    }
}

impl UpdateCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let lockfile_path = project_dir.join("rx.lock");

        // Load existing lockfile
        let old_lockfile = Lockfile::load(&lockfile_path).with_context(|| {
            format!(
                "No rx.lock found in {:?}. Run 'rx lock' or 'rx add <package>' first.",
                project_dir
            )
        })?;

        // Load pyproject.toml
        let mut pyproject = PyProject::load(&project_dir).with_context(|| {
            format!(
                "No pyproject.toml found in {:?}. Run 'rx init' first.",
                project_dir
            )
        })?;

        info!("Updating dependencies for {:?}", pyproject.name());

        // Parse package specs and handle version pinning
        let package_specs: Vec<PackageSpec> = self
            .packages
            .iter()
            .map(|p| parse_package_spec(p))
            .collect();

        // Update pyproject.toml for packages with specific versions
        let mut pyproject_modified = false;
        for spec in &package_specs {
            if let Some(ref version) = spec.version {
                let new_constraint = format!("{}=={}", spec.name, version);
                println!("Pinning {} to version {}", spec.name, version);

                // Try to update in main dependencies first
                let is_dev = !pyproject.dependencies().iter().any(|d| {
                    d.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                        .next()
                        .map(|n| n.to_lowercase() == spec.name.to_lowercase())
                        .unwrap_or(false)
                });

                if is_dev || self.dev {
                    pyproject.add_dev_dependency(new_constraint);
                } else {
                    pyproject.add_dependency(new_constraint);
                }
                pyproject_modified = true;
            }
        }

        // Save pyproject.toml if modified
        if pyproject_modified {
            pyproject
                .save(&project_dir)
                .with_context(|| "Failed to update pyproject.toml")?;
            println!("✓ Updated pyproject.toml");
        }

        // Collect all dependencies (re-read after modifications)
        let mut all_requirements: Vec<Requirement> = pyproject
            .dependencies()
            .iter()
            .filter_map(|s| Requirement::parse(s).ok())
            .collect();

        // Add dev dependencies if requested
        if self.dev {
            for dep in pyproject.dev_dependencies() {
                if let Ok(req) = Requirement::parse(dep) {
                    all_requirements.push(req);
                }
            }
        }

        if all_requirements.is_empty() {
            println!("No dependencies to update.");
            return Ok(());
        }

        // Determine which packages to update (use just the name, not version)
        let packages_to_update: Vec<String> = if self.packages.is_empty() {
            // Update all packages
            old_lockfile.packages.keys().cloned().collect()
        } else {
            // Normalize package names for comparison
            package_specs
                .iter()
                .map(|p| p.name.to_lowercase().replace('_', "-"))
                .collect()
        };

        // Re-resolve dependencies
        println!("Resolving dependencies...");
        let resolver = Resolver::new();
        let resolution = resolver
            .resolve(&all_requirements)
            .await
            .with_context(|| "Failed to resolve dependencies")?;

        // Create new lockfile
        let new_lockfile = Lockfile::from_resolution(&resolution);

        // Compare and collect changes
        let changes = compare_lockfiles(&old_lockfile, &new_lockfile, &packages_to_update);

        if changes.is_empty() {
            println!("All packages are up to date.");
            return Ok(());
        }

        // Display changes
        println!();
        if self.dry_run {
            println!("Would update {} packages:", changes.len());
        } else {
            println!("Updating {} packages:", changes.len());
        }

        for change in &changes {
            match change {
                PackageChange::Updated {
                    name,
                    old_version,
                    new_version,
                } => {
                    println!("  {} {} → {}", name, old_version, new_version);
                }
                PackageChange::Added { name, version } => {
                    println!("  {} {} (new)", name, version);
                }
                PackageChange::Removed { name, version } => {
                    println!("  {} {} (removed)", name, version);
                }
            }
        }

        if self.dry_run {
            println!();
            println!("Run without --dry-run to apply updates.");
            return Ok(());
        }

        // Save updated lockfile
        new_lockfile
            .save(&lockfile_path)
            .with_context(|| "Failed to write rx.lock")?;

        println!();
        println!("✓ Updated rx.lock");
        println!();
        println!("Run 'rx sync' to install the updated packages.");

        Ok(())
    }
}

#[derive(Debug)]
enum PackageChange {
    Updated {
        name: String,
        old_version: String,
        new_version: String,
    },
    Added {
        name: String,
        version: String,
    },
    Removed {
        name: String,
        version: String,
    },
}

fn compare_lockfiles(
    old: &Lockfile,
    new: &Lockfile,
    packages_to_update: &[String],
) -> Vec<PackageChange> {
    let mut changes = Vec::new();

    // Check for updates and additions
    for (name, new_pkg) in &new.packages {
        let normalized_name = name.to_lowercase().replace('_', "-");
        let should_check = packages_to_update.is_empty()
            || packages_to_update.contains(&normalized_name)
            || packages_to_update.iter().any(|p| p == name);

        if !should_check {
            continue;
        }

        if let Some(old_pkg) = old.packages.get(name) {
            if old_pkg.version != new_pkg.version {
                changes.push(PackageChange::Updated {
                    name: name.clone(),
                    old_version: old_pkg.version.clone(),
                    new_version: new_pkg.version.clone(),
                });
            }
        } else {
            changes.push(PackageChange::Added {
                name: name.clone(),
                version: new_pkg.version.clone(),
            });
        }
    }

    // Check for removals
    for (name, old_pkg) in &old.packages {
        if !new.packages.contains_key(name) {
            changes.push(PackageChange::Removed {
                name: name.clone(),
                version: old_pkg.version.clone(),
            });
        }
    }

    // Sort changes by name for consistent output
    changes.sort_by(|a, b| {
        let name_a = match a {
            PackageChange::Updated { name, .. } => name,
            PackageChange::Added { name, .. } => name,
            PackageChange::Removed { name, .. } => name,
        };
        let name_b = match b {
            PackageChange::Updated { name, .. } => name,
            PackageChange::Added { name, .. } => name,
            PackageChange::Removed { name, .. } => name,
        };
        name_a.cmp(name_b)
    });

    changes
}
