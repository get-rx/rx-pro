//! Self-update command
//!
//! Updates the rx CLI to the latest version.
//! Detects installation method and uses the appropriate update mechanism:
//! - pip: `pip install --upgrade rx-pro`
//! - cargo: `cargo install pro-cli`
//! - binary: downloads from GitHub releases

use anyhow::{Context, Result};
use clap::Args;

use pro_core::{InstallMethod, SelfUpdater};

#[derive(Args)]
pub struct SelfUpdateCommand {
    /// Check for updates without installing
    #[arg(long)]
    pub check: bool,

    /// Force update even if already on latest version
    #[arg(long)]
    pub force: bool,
}

impl SelfUpdateCommand {
    pub async fn run(self) -> Result<()> {
        let current_version = env!("CARGO_PKG_VERSION");

        let updater =
            SelfUpdater::new(current_version).context("Failed to initialize self-updater")?;

        let install_method = updater.install_method();

        println!("Current version: {}", current_version);
        println!("Install method:  {}", install_method);
        println!("Location:        {}", updater.exe_path().display());
        println!();

        // For pip/cargo, we can check PyPI/crates.io or just run the update
        match install_method {
            InstallMethod::Pip => {
                if self.check {
                    println!("To update, run: pip install --upgrade rx-pro");
                    return Ok(());
                }

                println!("Updating via pip...");
                updater
                    .update_via_pip()
                    .context("Failed to update via pip")?;

                println!();
                println!("✓ Successfully updated rx-pro via pip");
                println!("  Run 'rx --version' to verify the new version.");
            }

            InstallMethod::Cargo => {
                if self.check {
                    println!("To update, run: cargo install pro-cli");
                    return Ok(());
                }

                println!("Updating via cargo...");
                updater
                    .update_via_cargo()
                    .context("Failed to update via cargo")?;

                println!();
                println!("✓ Successfully updated pro-cli via cargo");
                println!("  Run 'rx --version' to verify the new version.");
            }

            InstallMethod::Binary => {
                println!("Checking for updates...");

                let release = updater
                    .check_latest()
                    .await
                    .context("Failed to check for updates")?;

                match release {
                    Some(release) => {
                        if !SelfUpdater::is_newer(current_version, &release.version) && !self.force
                        {
                            println!("Already on the latest version ({}).", current_version);
                            return Ok(());
                        }

                        println!(
                            "New version available: {} -> {}",
                            current_version, release.version
                        );

                        if self.check {
                            println!();
                            println!("Run 'rx self-update' to install the update.");
                            return Ok(());
                        }

                        println!("Downloading {}...", release.asset_name);

                        let installed_path = updater
                            .update_binary(&release)
                            .await
                            .context("Failed to install update")?;

                        println!();
                        println!("✓ Successfully updated to version {}", release.version);
                        println!("  Installed at: {}", installed_path.display());
                    }
                    None => {
                        println!("Already on the latest version ({}).", current_version);
                    }
                }
            }
        }

        Ok(())
    }
}
