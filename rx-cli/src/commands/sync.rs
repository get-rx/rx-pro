//! Sync command - install packages from lockfile into venv

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use tracing::info;

use rx_core::{
    default_cache_dir, install_path_dependency, load_path_dependencies, Installer, Lockfile,
    VenvManager,
};

#[derive(Args)]
pub struct SyncCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Don't actually install, just show what would be installed
    #[arg(long)]
    pub dry_run: bool,

    /// Recreate the virtual environment from scratch
    #[arg(long)]
    pub recreate: bool,

    /// Path to Python interpreter to use
    #[arg(long)]
    pub python: Option<PathBuf>,
}

impl SyncCommand {
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
            println!("No packages to install (lockfile is empty).");
            return Ok(());
        }

        info!("Syncing {} packages", lockfile.len());

        if self.dry_run {
            println!("Would install {} packages:", lockfile.len());
            for (name, pkg) in &lockfile.packages {
                println!("  {} {}", name, pkg.version);
            }
            return Ok(());
        }

        // Setup virtual environment
        let venv_path = project_dir.join(".venv");
        let venv = VenvManager::new(&venv_path);

        if self.recreate && venv.exists() {
            println!("Recreating virtual environment...");
            std::fs::remove_dir_all(&venv_path)?;
        }

        if !venv.exists() {
            println!("Creating virtual environment...");
            venv.create(self.python.as_deref())
                .context("Failed to create virtual environment")?;
            println!("  Created .venv");
        }

        let site_packages = venv
            .site_packages()
            .context("Failed to determine site-packages directory")?;

        // Install packages
        println!("Installing {} packages:", lockfile.len());

        let cache_dir = default_cache_dir();
        let installer = Installer::new(&cache_dir);

        // Convert BTreeMap to HashMap for installer
        let packages: std::collections::HashMap<_, _> = lockfile
            .packages
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let results = installer
            .install(&packages, &site_packages)
            .await
            .context("Installation failed")?;

        // Report results
        let mut success_count = 0;
        let mut cached_count = 0;
        let mut error_count = 0;

        for result in &results {
            if result.installed {
                if result.downloaded {
                    println!(
                        "  {} {} ✓",
                        result.name,
                        lockfile
                            .packages
                            .get(&result.name)
                            .map(|p| p.version.as_str())
                            .unwrap_or("")
                    );
                    success_count += 1;
                } else {
                    println!(
                        "  {} {} (cached)",
                        result.name,
                        lockfile
                            .packages
                            .get(&result.name)
                            .map(|p| p.version.as_str())
                            .unwrap_or("")
                    );
                    cached_count += 1;
                }
            } else if let Some(ref err) = result.error {
                println!("  {} - FAILED: {}", result.name, err);
                error_count += 1;
            }
        }

        println!();
        if error_count > 0 {
            println!(
                "⚠ Completed with errors: {} installed, {} cached, {} failed",
                success_count, cached_count, error_count
            );
        } else {
            println!(
                "✓ Synchronized {} packages ({} downloaded, {} cached)",
                success_count + cached_count,
                success_count,
                cached_count
            );
        }

        // Install path dependencies
        let path_deps = load_path_dependencies(&project_dir).unwrap_or_default();
        if !path_deps.is_empty() {
            println!();
            println!("Installing {} path dependencies:", path_deps.len());

            for (name, dep) in &path_deps {
                let mode = if dep.editable { "editable" } else { "copy" };
                match install_path_dependency(dep, &project_dir, &site_packages).await {
                    Ok(_) => {
                        println!("  {} ({}) ✓", name, mode);
                    }
                    Err(e) => {
                        println!("  {} - FAILED: {}", name, e);
                    }
                }
            }
        }

        // Show activation hint
        println!();
        println!("To activate the virtual environment:");
        #[cfg(unix)]
        println!("  source .venv/bin/activate");
        #[cfg(windows)]
        println!("  .venv\\Scripts\\activate");

        Ok(())
    }
}
