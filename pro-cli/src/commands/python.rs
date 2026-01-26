//! Python version management command
//!
//! Provides commands for installing, listing, and managing Python versions:
//! - `rx python install <version>` - Install a Python version
//! - `rx python list` - List available and installed versions
//! - `rx python pin <version>` - Pin version for project
//! - `rx python use <version>` - Set global default
//! - `rx python uninstall <version>` - Remove installed version

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use pro_core::{PythonManager, PythonVersion};

#[derive(Args)]
pub struct PythonCommand {
    #[command(subcommand)]
    pub subcommand: PythonSubcommand,
}

#[derive(Subcommand)]
pub enum PythonSubcommand {
    /// Install a Python version
    Install(PythonInstallCommand),

    /// List available and installed Python versions
    List(PythonListCommand),

    /// Pin Python version for the current project
    Pin(PythonPinCommand),

    /// Set the global default Python version
    Use(PythonUseCommand),

    /// Uninstall a Python version
    Uninstall(PythonUninstallCommand),
}

#[derive(Args)]
pub struct PythonInstallCommand {
    /// Python version to install (e.g., 3.12, 3.11.8)
    pub version: String,

    /// Force reinstall even if already installed
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct PythonListCommand {
    /// Only show installed versions
    #[arg(long)]
    pub installed: bool,

    /// Only show available (not installed) versions
    #[arg(long)]
    pub available: bool,
}

#[derive(Args)]
pub struct PythonPinCommand {
    /// Python version to pin (e.g., 3.12)
    pub version: String,

    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

#[derive(Args)]
pub struct PythonUseCommand {
    /// Python version to set as global default
    pub version: String,
}

#[derive(Args)]
pub struct PythonUninstallCommand {
    /// Python version to uninstall
    pub version: String,
}

impl PythonCommand {
    pub async fn run(self) -> Result<()> {
        match self.subcommand {
            PythonSubcommand::Install(cmd) => cmd.run().await,
            PythonSubcommand::List(cmd) => cmd.run().await,
            PythonSubcommand::Pin(cmd) => cmd.run().await,
            PythonSubcommand::Use(cmd) => cmd.run().await,
            PythonSubcommand::Uninstall(cmd) => cmd.run().await,
        }
    }
}

impl PythonInstallCommand {
    pub async fn run(self) -> Result<()> {
        let manager = PythonManager::new().context("Failed to initialize Python manager")?;

        // Check if already installed
        if !self.force {
            if let Some(installed) = manager.find_matching(&self.version)? {
                println!(
                    "Python {} is already installed at {}",
                    installed.version,
                    installed.path.display()
                );
                println!("Use --force to reinstall.");
                return Ok(());
            }
        } else {
            // Force reinstall - uninstall first if exists
            if let Some(installed) = manager.find_matching(&self.version)? {
                println!(
                    "Removing existing installation of Python {}...",
                    installed.version
                );
                manager.uninstall(&installed.version.to_string())?;
            }
        }

        // Install
        println!("Installing Python {}...", self.version);
        let installed = manager.install(&self.version).await?;

        println!();
        println!("Installed Python {} to:", installed.version);
        println!("  {}", installed.path.display());
        println!();
        println!("Executable: {}", installed.executable().display());

        Ok(())
    }
}

impl PythonListCommand {
    pub async fn run(self) -> Result<()> {
        let manager = PythonManager::new().context("Failed to initialize Python manager")?;

        let installed = manager.list_installed()?;
        let available = manager.list_available();

        // If both flags are false, show both
        let show_installed = self.installed || !self.available;
        let show_available = self.available || !self.installed;

        // Get global default
        let global = manager.get_global()?;

        if show_installed {
            println!("Installed versions:");
            if installed.is_empty() {
                println!("  (none)");
            } else {
                for python in &installed {
                    let is_global = global
                        .as_ref()
                        .map(|g| python.version.matches(g))
                        .unwrap_or(false);

                    let marker = if is_global { " (default)" } else { "" };
                    println!("  {} {}{}", python.version, python.path.display(), marker);
                }
            }
            println!();
        }

        if show_available {
            println!("Available versions:");

            // Group by minor version
            let mut current_minor: Option<u32> = None;
            for version in &available {
                // Check if this version is installed
                let is_installed = installed.iter().any(|i| {
                    i.version.major == version.version.major
                        && i.version.minor == version.version.minor
                        && i.version.patch == version.version.patch
                });

                // Show minor version header
                if current_minor != Some(version.version.minor) {
                    current_minor = Some(version.version.minor);
                    println!();
                    println!("  Python 3.{}:", version.version.minor);
                }

                let marker = if is_installed { " (installed)" } else { "" };
                println!("    {}{}", version.version, marker);
            }
            println!();
        }

        // Show global default if set
        if let Some(ref global) = global {
            println!("Global default: Python {}", global);
        }

        Ok(())
    }
}

impl PythonPinCommand {
    pub async fn run(self) -> Result<()> {
        let manager = PythonManager::new().context("Failed to initialize Python manager")?;

        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Validate version format
        let version = PythonVersion::parse(&self.version).context("Invalid version format")?;

        // Check if we have this version installed
        if let Some(installed) = manager.find_matching(&self.version)? {
            println!(
                "Pinning Python {} (installed at {})",
                installed.version,
                installed.path.display()
            );
        } else {
            println!(
                "Warning: Python {} is not installed. Run 'rx python install {}' to install it.",
                version, version
            );
        }

        // Create .python-version file
        manager.pin(&self.version, &project_dir)?;

        let version_file = project_dir.join(".python-version");
        println!("Created {}", version_file.display());

        Ok(())
    }
}

impl PythonUseCommand {
    pub async fn run(self) -> Result<()> {
        let manager = PythonManager::new().context("Failed to initialize Python manager")?;

        // Validate version format
        let version = PythonVersion::parse(&self.version).context("Invalid version format")?;

        // Check if we have this version installed
        if let Some(installed) = manager.find_matching(&self.version)? {
            println!(
                "Setting global Python to {} (installed at {})",
                installed.version,
                installed.path.display()
            );
        } else {
            println!(
                "Warning: Python {} is not installed. Run 'rx python install {}' to install it.",
                version, version
            );
        }

        // Set global default
        manager.set_global(&self.version)?;

        println!("Global Python version set to {}", version);

        Ok(())
    }
}

impl PythonUninstallCommand {
    pub async fn run(self) -> Result<()> {
        let manager = PythonManager::new().context("Failed to initialize Python manager")?;

        // Check if installed
        let installed = manager
            .find_matching(&self.version)?
            .ok_or_else(|| anyhow::anyhow!("Python {} is not installed", self.version))?;

        println!("Uninstalling Python {}...", installed.version);

        manager.uninstall(&installed.version.to_string())?;

        println!("Python {} has been uninstalled.", installed.version);

        Ok(())
    }
}
