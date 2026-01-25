//! Version command - show and manage project version

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};

use rx_core::pep::PyProject;
use rx_core::versioning::{VersioningConfig, get_git_version, bump_version};

#[derive(Args)]
pub struct VersionCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".", global = true)]
    pub project: PathBuf,

    #[command(subcommand)]
    pub action: Option<VersionAction>,
}

#[derive(Subcommand)]
pub enum VersionAction {
    /// Bump version (major, minor, patch, or pre)
    Bump {
        /// Part to bump: major, minor, patch, or pre
        part: String,

        /// Don't actually modify files, just show what would happen
        #[arg(long)]
        dry_run: bool,
    },

    /// Set version to a specific value
    Set {
        /// Version to set
        version: String,

        /// Don't actually modify files, just show what would happen
        #[arg(long)]
        dry_run: bool,
    },
}

impl VersionCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Load pyproject.toml
        let pyproject = PyProject::load(&project_dir).with_context(|| {
            format!(
                "No pyproject.toml found in {:?}. Run 'rx init' first.",
                project_dir
            )
        })?;

        // Get versioning config from [tool.rx.versioning] if present
        let config = get_versioning_config(&pyproject);

        match self.action {
            None => {
                // Show current version
                show_version(&project_dir, &pyproject, &config)?;
            }
            Some(VersionAction::Bump { part, dry_run }) => {
                bump_project_version(&project_dir, &pyproject, &part, dry_run)?;
            }
            Some(VersionAction::Set { version, dry_run }) => {
                set_project_version(&project_dir, &pyproject, &version, dry_run)?;
            }
        }

        Ok(())
    }
}

fn get_versioning_config(pyproject: &PyProject) -> VersioningConfig {
    if let Some(rx_config) = pyproject.tool.get("rx") {
        if let Some(versioning) = rx_config.get("versioning") {
            if let Ok(config) = versioning.clone().try_into::<VersioningConfig>() {
                return config;
            }
        }
    }
    VersioningConfig::default()
}

fn show_version(
    project_dir: &std::path::Path,
    pyproject: &PyProject,
    config: &VersioningConfig,
) -> Result<()> {
    let name = pyproject.name().unwrap_or("unknown");

    // Try git version first if configured
    if config.source == "git-tag" || config.source == "git" {
        match get_git_version(project_dir, config) {
            Ok(version) => {
                println!("{} {}", name, version);
                println!("  source: git tag");
                return Ok(());
            }
            Err(e) => {
                tracing::debug!("Git version failed: {}", e);
            }
        }
    }

    // Fall back to pyproject.toml version
    if let Some(version) = pyproject.version() {
        println!("{} {}", name, version);
        println!("  source: pyproject.toml");
    } else {
        // Try dynamic version from git even if not configured
        match get_git_version(project_dir, config) {
            Ok(version) => {
                println!("{} {}", name, version);
                println!("  source: git tag (auto-detected)");
            }
            Err(_) => {
                bail!("No version found in pyproject.toml or git tags");
            }
        }
    }

    Ok(())
}

fn bump_project_version(
    project_dir: &std::path::Path,
    pyproject: &PyProject,
    part: &str,
    dry_run: bool,
) -> Result<()> {
    let current = pyproject
        .version()
        .context("No version in pyproject.toml to bump")?;

    let new_version = bump_version(current, part)
        .with_context(|| format!("Failed to bump {} version", part))?;

    if dry_run {
        println!("Would bump {} → {}", current, new_version);
        return Ok(());
    }

    // Update pyproject.toml
    let mut updated = pyproject.clone();
    if let Some(ref mut project) = updated.project {
        project.version = Some(new_version.clone());
    }
    updated.save(project_dir)?;

    println!("Bumped {} → {}", current, new_version);

    Ok(())
}

fn set_project_version(
    project_dir: &std::path::Path,
    pyproject: &PyProject,
    version: &str,
    dry_run: bool,
) -> Result<()> {
    let current = pyproject.version().unwrap_or("(none)");

    if dry_run {
        println!("Would set version: {} → {}", current, version);
        return Ok(());
    }

    // Update pyproject.toml
    let mut updated = pyproject.clone();
    if let Some(ref mut project) = updated.project {
        project.version = Some(version.to_string());
    }
    updated.save(project_dir)?;

    println!("Set version: {} → {}", current, version);

    Ok(())
}
