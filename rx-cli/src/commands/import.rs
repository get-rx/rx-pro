//! Import command - migrate from other package managers
//!
//! Supports importing from:
//! - Poetry (pyproject.toml + poetry.lock)
//!
//! ```bash
//! # Import from Poetry project
//! rx import poetry
//!
//! # Import from specific directory
//! rx import poetry --project ../other-project
//!
//! # Dry run (show what would be imported)
//! rx import poetry --dry-run
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use serde::Deserialize;

use rx_core::lockfile::LockedPackage;
use rx_core::pep::PyProject;
use rx_core::Lockfile;

#[derive(Args)]
pub struct ImportCommand {
    #[command(subcommand)]
    pub command: ImportSubcommand,
}

#[derive(Subcommand)]
pub enum ImportSubcommand {
    /// Import from Poetry project
    Poetry(PoetryImportCommand),
}

impl ImportCommand {
    pub async fn run(self) -> Result<()> {
        match self.command {
            ImportSubcommand::Poetry(cmd) => cmd.run().await,
        }
    }
}

#[derive(Args)]
pub struct PoetryImportCommand {
    /// Project directory containing Poetry files
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Show what would be imported without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Keep Poetry files after import (don't suggest removal)
    #[arg(long)]
    pub keep_poetry: bool,
}

impl PoetryImportCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let pyproject_path = project_dir.join("pyproject.toml");
        if !pyproject_path.exists() {
            bail!("No pyproject.toml found in {}", project_dir.display());
        }

        println!("Importing from Poetry project: {}", project_dir.display());
        println!();

        // Read pyproject.toml
        let content = std::fs::read_to_string(&pyproject_path)?;
        let doc: toml::Value = toml::from_str(&content)?;

        // Check if this is a Poetry project
        let poetry_section = doc.get("tool").and_then(|t| t.get("poetry"));

        if poetry_section.is_none() {
            bail!("No [tool.poetry] section found. Is this a Poetry project?");
        }

        let poetry = poetry_section.unwrap();

        // Extract project metadata
        let name = poetry
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing project name"))?;

        let version = poetry
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.1.0");

        let description = poetry
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let python_requires = poetry
            .get("python")
            .and_then(|v| v.as_str())
            .map(convert_poetry_python_constraint)
            .unwrap_or_else(|| ">=3.8".to_string());

        println!("Project: {} v{}", name, version);
        println!("Python: {}", python_requires);
        println!();

        // Extract dependencies
        let mut main_deps = Vec::new();
        let mut dev_deps = Vec::new();

        if let Some(deps) = poetry.get("dependencies").and_then(|v| v.as_table()) {
            for (dep_name, constraint) in deps {
                if dep_name == "python" {
                    continue; // Skip python version constraint
                }
                let constraint_str = convert_poetry_constraint(constraint);
                main_deps.push(format!("{}{}", dep_name, constraint_str));
            }
        }

        // Dev dependencies from [tool.poetry.group.dev.dependencies] or [tool.poetry.dev-dependencies]
        if let Some(groups) = poetry.get("group").and_then(|v| v.as_table()) {
            if let Some(dev_group) = groups.get("dev").and_then(|v| v.as_table()) {
                if let Some(deps) = dev_group.get("dependencies").and_then(|v| v.as_table()) {
                    for (dep_name, constraint) in deps {
                        let constraint_str = convert_poetry_constraint(constraint);
                        dev_deps.push(format!("{}{}", dep_name, constraint_str));
                    }
                }
            }
        }

        // Also check legacy dev-dependencies
        if let Some(deps) = poetry.get("dev-dependencies").and_then(|v| v.as_table()) {
            for (dep_name, constraint) in deps {
                let constraint_str = convert_poetry_constraint(constraint);
                dev_deps.push(format!("{}{}", dep_name, constraint_str));
            }
        }

        println!("Dependencies ({}):", main_deps.len());
        for dep in &main_deps {
            println!("  - {}", dep);
        }

        if !dev_deps.is_empty() {
            println!();
            println!("Dev dependencies ({}):", dev_deps.len());
            for dep in &dev_deps {
                println!("  - {}", dep);
            }
        }

        // Try to import poetry.lock if it exists
        let lockfile_path = project_dir.join("poetry.lock");
        let lockfile = if lockfile_path.exists() {
            println!();
            println!("Found poetry.lock, importing locked versions...");
            Some(import_poetry_lock(&lockfile_path)?)
        } else {
            None
        };

        if self.dry_run {
            println!();
            println!("Dry run - no changes made.");
            return Ok(());
        }

        // Create new pyproject.toml for rx
        let mut pyproject = PyProject::new(name, version, &python_requires);
        if let Some(ref mut proj) = pyproject.project {
            proj.description = if description.is_empty() {
                None
            } else {
                Some(description.to_string())
            };
        }

        // Add dependencies
        for dep in &main_deps {
            pyproject.add_dependency(dep.clone());
        }
        for dep in &dev_deps {
            pyproject.add_dev_dependency(dep.clone());
        }

        // Extract scripts/entry points
        if let Some(scripts) = poetry.get("scripts").and_then(|v| v.as_table()) {
            if let Some(ref mut proj) = pyproject.project {
                for (name, value) in scripts {
                    if let Some(entry) = value.as_str() {
                        proj.scripts.insert(name.clone(), entry.to_string());
                    }
                }
            }
        }

        // Add build system
        pyproject.build_system = Some(rx_core::pep::pep621::BuildSystem {
            requires: vec!["hatchling".to_string()],
            build_backend: Some("hatchling.build".to_string()),
            backend_path: None,
        });

        // Save new pyproject.toml
        pyproject.save(&project_dir)?;
        println!();
        println!("✓ Updated pyproject.toml for T-Rex");

        // Save lockfile if we imported one
        if let Some(lockfile) = lockfile {
            let rx_lock_path = project_dir.join("rx.lock");
            lockfile.save(&rx_lock_path)?;
            println!("✓ Created rx.lock from poetry.lock");
        }

        println!();
        println!("Migration complete!");
        println!();
        println!("Next steps:");
        println!("  rx sync      # Install dependencies");

        if !self.keep_poetry {
            println!();
            println!("You can now remove Poetry files:");
            println!("  rm poetry.lock");
            println!("  # Edit pyproject.toml to remove [tool.poetry] section");
        }

        Ok(())
    }
}

/// Convert Poetry version constraint to PEP 440
fn convert_poetry_constraint(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => convert_poetry_version_string(s),
        toml::Value::Table(t) => {
            // Handle complex constraints like { version = "^1.0", extras = ["dev"] }
            if let Some(version) = t.get("version").and_then(|v| v.as_str()) {
                let base = convert_poetry_version_string(version);
                if let Some(extras) = t.get("extras").and_then(|v| v.as_array()) {
                    let extras_str: Vec<_> = extras.iter().filter_map(|e| e.as_str()).collect();
                    if !extras_str.is_empty() {
                        return format!("[{}]{}", extras_str.join(","), base);
                    }
                }
                base
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

/// Convert Poetry version string to PEP 440
fn convert_poetry_version_string(version: &str) -> String {
    let v = version.trim();

    if v == "*" {
        return String::new(); // Any version
    }

    if let Some(ver) = v.strip_prefix('^') {
        // Caret requirement: ^1.2.3 means >=1.2.3 <2.0.0
        let parts: Vec<&str> = ver.split('.').collect();
        match parts.as_slice() {
            [major, ..] if *major != "0" => {
                let next_major: u32 = major.parse().unwrap_or(1) + 1;
                format!(">={},<{}.0.0", ver, next_major)
            }
            [_, minor, ..] if *minor != "0" => {
                let next_minor: u32 = minor.parse().unwrap_or(0) + 1;
                format!(">={}.<0.{}.0", ver, next_minor)
            }
            _ => format!(">={}", ver),
        }
    } else if let Some(ver) = v.strip_prefix('~') {
        // Tilde requirement: ~1.2.3 means >=1.2.3 <1.3.0
        let parts: Vec<&str> = ver.split('.').collect();
        if parts.len() >= 2 {
            let major = parts[0];
            let minor: u32 = parts[1].parse().unwrap_or(0) + 1;
            format!(">={},<{}.{}.0", ver, major, minor)
        } else {
            format!(">={}", ver)
        }
    } else if v.contains("||") {
        // OR constraint
        v.split("||")
            .map(|p| convert_poetry_version_string(p.trim()))
            .collect::<Vec<_>>()
            .join(" || ")
    } else if !v.starts_with('>')
        && !v.starts_with('<')
        && !v.starts_with('=')
        && !v.starts_with('!')
    {
        // Plain version: 1.2.3 means ==1.2.3
        format!("=={}", v)
    } else {
        v.to_string()
    }
}

/// Convert Poetry python constraint to PEP 440
fn convert_poetry_python_constraint(constraint: &str) -> String {
    // Poetry uses ^3.8 style, convert to >=3.8
    let v = constraint.trim();
    if v.starts_with('^') {
        format!(">={}", &v[1..])
    } else if v.starts_with('~') {
        format!(">={}", &v[1..])
    } else {
        v.to_string()
    }
}

/// Import poetry.lock into rx.lock format
fn import_poetry_lock(path: &PathBuf) -> Result<Lockfile> {
    let content = std::fs::read_to_string(path)?;

    // Poetry lock is TOML
    #[derive(Deserialize)]
    struct PoetryLock {
        #[serde(default)]
        package: Vec<PoetryPackage>,
    }

    #[derive(Deserialize)]
    struct PoetryPackage {
        name: String,
        version: String,
        #[serde(default)]
        dependencies: HashMap<String, toml::Value>,
        #[serde(default)]
        files: Vec<PoetryFile>,
    }

    #[derive(Deserialize)]
    struct PoetryFile {
        file: String,
        hash: String,
    }

    let poetry_lock: PoetryLock =
        toml::from_str(&content).context("Failed to parse poetry.lock")?;

    let mut lockfile = Lockfile::new();

    for pkg in poetry_lock.package {
        // Extract dependencies
        let dependencies: Vec<String> = pkg
            .dependencies
            .keys()
            .map(|k| k.to_lowercase().replace('_', "-"))
            .collect();

        // Get first file's hash if available
        let (url, hash) = if let Some(file) = pkg.files.first() {
            // Poetry doesn't store URLs, so we construct PyPI URL
            let name_normalized = pkg.name.to_lowercase().replace('_', "-");
            let url = format!(
                "https://files.pythonhosted.org/packages/{}/{}",
                name_normalized, file.file
            );
            let hash = format!("sha256:{}", file.hash.trim_start_matches("sha256:"));
            (Some(url), Some(hash))
        } else {
            (None, None)
        };

        lockfile.packages.insert(
            pkg.name.to_lowercase().replace('_', "-"),
            LockedPackage {
                version: pkg.version,
                url,
                hash,
                dependencies,
                markers: None,
                files: vec![],
            },
        );
    }

    Ok(lockfile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_poetry_caret() {
        assert_eq!(convert_poetry_version_string("^1.2.3"), ">=1.2.3,<2.0.0");
        assert_eq!(convert_poetry_version_string("^2.0"), ">=2.0,<3.0.0");
    }

    #[test]
    fn test_convert_poetry_tilde() {
        assert_eq!(convert_poetry_version_string("~1.2.3"), ">=1.2.3,<1.3.0");
    }

    #[test]
    fn test_convert_poetry_exact() {
        assert_eq!(convert_poetry_version_string("1.2.3"), "==1.2.3");
    }

    #[test]
    fn test_convert_poetry_any() {
        assert_eq!(convert_poetry_version_string("*"), "");
    }
}
