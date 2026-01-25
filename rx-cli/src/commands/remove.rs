//! Remove command - remove dependencies from the project

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use rx_core::lockfile::Lockfile;
use rx_core::pep::PyProject;
use rx_core::resolver::Resolver;

#[derive(Args)]
pub struct RemoveCommand {
    /// Packages to remove
    #[arg(required = true)]
    pub packages: Vec<String>,

    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Remove from dev dependencies
    #[arg(long)]
    pub dev: bool,

    /// Remove from path dependencies ([tool.rx.dependencies])
    #[arg(long)]
    pub path: bool,

    /// Don't update the lockfile after removing
    #[arg(long)]
    pub no_lock: bool,

    /// Show what would be removed without making changes
    #[arg(long)]
    pub dry_run: bool,
}

impl RemoveCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let lockfile_path = project_dir.join("rx.lock");

        // Load pyproject.toml
        let mut pyproject = PyProject::load(&project_dir).with_context(|| {
            format!(
                "No pyproject.toml found in {:?}. Run 'rx init' first.",
                project_dir
            )
        })?;

        // Handle path dependency removal
        if self.path {
            return self.remove_path_dependencies(&project_dir, &mut pyproject);
        }

        // Normalize package names for comparison (lowercase, underscores to hyphens)
        let packages_to_remove: Vec<String> = self
            .packages
            .iter()
            .map(|p| normalize_package_name(p))
            .collect();

        // Get current dependencies
        let (removed, not_found) = if self.dev {
            remove_dev_dependencies(&mut pyproject, &packages_to_remove)?
        } else {
            remove_dependencies(&mut pyproject, &packages_to_remove)?
        };

        if removed.is_empty() && not_found.is_empty() {
            println!("No matching dependencies found to remove.");
            return Ok(());
        }

        // Report what wasn't found
        if !not_found.is_empty() {
            println!("Not found in dependencies: {}", not_found.join(", "));
        }

        if removed.is_empty() {
            return Ok(());
        }

        // Dry run - just show what would happen
        if self.dry_run {
            println!("Would remove:");
            for pkg in &removed {
                println!("  - {}", pkg);
            }
            return Ok(());
        }

        // Save updated pyproject.toml
        pyproject.save(&project_dir)?;
        println!("Removed {} package(s):", removed.len());
        for pkg in &removed {
            println!("  - {}", pkg);
        }

        // Update lockfile unless --no-lock
        if !self.no_lock {
            println!();
            println!("Updating lockfile...");

            // Get all remaining dependencies
            let deps = pyproject.all_dependencies();

            if deps.is_empty() {
                // No dependencies left, create empty lockfile
                let lockfile = Lockfile::new();
                lockfile.save(&lockfile_path)?;
                println!("Lockfile updated (now empty).");
            } else {
                // Re-resolve dependencies
                let resolver = Resolver::new();
                let resolution = resolver.resolve(&deps).await.with_context(|| {
                    "Failed to resolve dependencies after removal"
                })?;

                let lockfile = Lockfile::from_resolution(&resolution);
                lockfile.save(&lockfile_path)?;
                println!("Lockfile updated with {} packages.", lockfile.len());
            }

            println!();
            println!("Run 'rx sync' to update your virtual environment.");
        }

        Ok(())
    }

    /// Remove path dependencies from [tool.rx.dependencies]
    fn remove_path_dependencies(
        &self,
        project_dir: &PathBuf,
        pyproject: &mut PyProject,
    ) -> Result<()> {
        let mut removed = Vec::new();
        let mut not_found = Vec::new();

        for name in &self.packages {
            if pyproject.remove_path_dependency(name) {
                removed.push(name.clone());
            } else {
                not_found.push(name.clone());
            }
        }

        if removed.is_empty() && not_found.is_empty() {
            println!("No path dependencies found to remove.");
            return Ok(());
        }

        if !not_found.is_empty() {
            println!("Not found in path dependencies: {}", not_found.join(", "));
        }

        if removed.is_empty() {
            return Ok(());
        }

        if self.dry_run {
            println!("Would remove path dependencies:");
            for pkg in &removed {
                println!("  - {}", pkg);
            }
            return Ok(());
        }

        pyproject.save(project_dir)?;
        println!("Removed {} path dependency(ies):", removed.len());
        for pkg in &removed {
            println!("  - {}", pkg);
        }
        println!();
        println!("Run 'rx sync' to update your virtual environment.");

        Ok(())
    }
}

/// Normalize package name for comparison
fn normalize_package_name(name: &str) -> String {
    name.to_lowercase().replace('_', "-")
}

/// Remove packages from main dependencies
fn remove_dependencies(
    pyproject: &mut PyProject,
    packages: &[String],
) -> Result<(Vec<String>, Vec<String>)> {
    let mut removed = Vec::new();
    let mut not_found = Vec::new();

    // Get current dependencies
    let current_deps = pyproject.dependencies();

    for pkg_name in packages {
        // Find matching dependency
        let found = current_deps.iter().any(|dep| {
            let dep_name = extract_package_name(dep);
            normalize_package_name(&dep_name) == *pkg_name
        });

        if found {
            removed.push(pkg_name.clone());
        } else {
            not_found.push(pkg_name.clone());
        }
    }

    // Remove from pyproject
    if !removed.is_empty() {
        pyproject.remove_dependencies(&removed)?;
    }

    Ok((removed, not_found))
}

/// Remove packages from dev dependencies
fn remove_dev_dependencies(
    pyproject: &mut PyProject,
    packages: &[String],
) -> Result<(Vec<String>, Vec<String>)> {
    let mut removed = Vec::new();
    let mut not_found = Vec::new();

    // Get current dev dependencies
    let current_deps = pyproject.dev_dependencies();

    for pkg_name in packages {
        // Find matching dependency
        let found = current_deps.iter().any(|dep| {
            let dep_name = extract_package_name(dep);
            normalize_package_name(&dep_name) == *pkg_name
        });

        if found {
            removed.push(pkg_name.clone());
        } else {
            not_found.push(pkg_name.clone());
        }
    }

    // Remove from pyproject
    if !removed.is_empty() {
        pyproject.remove_dev_dependencies(&removed)?;
    }

    Ok((removed, not_found))
}

/// Extract package name from a dependency specifier (e.g., "requests>=2.0" -> "requests")
fn extract_package_name(dep: &str) -> String {
    // Find the first character that's not part of the package name
    let end = dep
        .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.')
        .unwrap_or(dep.len());
    dep[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_package_name() {
        assert_eq!(normalize_package_name("requests"), "requests");
        assert_eq!(normalize_package_name("Requests"), "requests");
        assert_eq!(normalize_package_name("my_package"), "my-package");
        assert_eq!(normalize_package_name("My_Package"), "my-package");
    }

    #[test]
    fn test_extract_package_name() {
        assert_eq!(extract_package_name("requests"), "requests");
        assert_eq!(extract_package_name("requests>=2.0"), "requests");
        assert_eq!(extract_package_name("requests[security]>=2.0"), "requests");
        assert_eq!(extract_package_name("my-package>=1.0,<2.0"), "my-package");
    }
}
