use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use tracing::info;

use rx_core::Lockfile;

#[derive(Args)]
pub struct SyncCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Don't actually install, just show what would be installed
    #[arg(long)]
    pub dry_run: bool,
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
        let lockfile = Lockfile::load(&lockfile_path)
            .with_context(|| {
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
        } else {
            println!("Installing {} packages:", lockfile.len());
        }

        for (name, pkg) in &lockfile.packages {
            if self.dry_run {
                println!("  {} {}", name, pkg.version);
            } else {
                // TODO: Implement actual installation
                // - Create venv if not exists
                // - Download wheel/sdist
                // - Verify hash
                // - Install into venv
                print!("  {} {}...", name, pkg.version);

                // For now, just simulate
                if let Some(ref url) = pkg.url {
                    if !url.is_empty() {
                        println!(" ✓");
                    } else {
                        println!(" (no URL)");
                    }
                } else {
                    println!(" (no URL)");
                }
            }
        }

        if !self.dry_run {
            println!();
            println!("✓ Synchronized {} packages", lockfile.len());
            println!();
            println!("Note: Full installation not yet implemented.");
            println!("      Packages need to be installed manually for now.");
        }

        Ok(())
    }
}
