//! Workspace command - manage monorepo workspaces
//!
//! A workspace allows managing multiple related Python projects:
//! - Unified lockfile at workspace root
//! - Optional shared virtual environment
//! - Coordinated dependency resolution

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};

use rx_core::lockfile::Lockfile;
use rx_core::resolver::Resolver;
use rx_core::workspace::Workspace;
use rx_core::{default_cache_dir, install_path_dependency, load_path_dependencies, Installer, VenvManager};

#[derive(Args)]
pub struct WorkspaceCommand {
    #[command(subcommand)]
    pub command: WorkspaceSubcommand,
}

#[derive(Subcommand)]
pub enum WorkspaceSubcommand {
    /// Initialize a new workspace
    Init(WorkspaceInitCommand),

    /// Add a member project to the workspace
    Add(WorkspaceAddCommand),

    /// Remove a member from the workspace
    Remove(WorkspaceRemoveCommand),

    /// List workspace members
    List(WorkspaceListCommand),

    /// Generate unified lockfile for all members
    Lock(WorkspaceLockCommand),

    /// Sync all members with the unified lockfile
    Sync(WorkspaceSyncCommand),
}

impl WorkspaceCommand {
    pub async fn run(self) -> Result<()> {
        match self.command {
            WorkspaceSubcommand::Init(cmd) => cmd.run().await,
            WorkspaceSubcommand::Add(cmd) => cmd.run().await,
            WorkspaceSubcommand::Remove(cmd) => cmd.run().await,
            WorkspaceSubcommand::List(cmd) => cmd.run().await,
            WorkspaceSubcommand::Lock(cmd) => cmd.run().await,
            WorkspaceSubcommand::Sync(cmd) => cmd.run().await,
        }
    }
}

// ============================================================================
// Init Command
// ============================================================================

#[derive(Args)]
pub struct WorkspaceInitCommand {
    /// Directory to initialize (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Use a shared virtual environment for all members
    #[arg(long)]
    pub shared_venv: bool,
}

impl WorkspaceInitCommand {
    pub async fn run(self) -> Result<()> {
        let root = if self.path.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            std::fs::create_dir_all(&self.path)?;
            self.path.canonicalize()?
        };

        // Check if already a workspace
        if Workspace::is_workspace_root(&root) {
            bail!("Directory is already a workspace root: {}", root.display());
        }

        // Create workspace
        let workspace = Workspace::create(&root, self.shared_venv)
            .context("Failed to create workspace")?;

        println!("Initialized workspace at {}", root.display());
        println!();
        println!("Configuration:");
        println!("  Shared venv: {}", workspace.shared_venv);
        println!();
        println!("Add members with:");
        println!("  rx workspace add <path>");
        println!();
        println!("Or edit pyproject.toml:");
        println!("  [tool.rx.workspace]");
        println!("  members = [\"packages/*\", \"apps/myapp\"]");

        Ok(())
    }
}

// ============================================================================
// Add Command
// ============================================================================

#[derive(Args)]
pub struct WorkspaceAddCommand {
    /// Path to the member project (relative to workspace root)
    pub path: String,

    /// Workspace root directory (defaults to searching upward)
    #[arg(long)]
    pub root: Option<PathBuf>,
}

impl WorkspaceAddCommand {
    pub async fn run(self) -> Result<()> {
        let root = match self.root {
            Some(r) => r.canonicalize()?,
            None => Workspace::find_root(&std::env::current_dir()?)
                .context("Not in a workspace. Run 'rx workspace init' first.")?,
        };

        let mut workspace = Workspace::load_from_root(&root)
            .context("Failed to load workspace")?;

        workspace.add_member(&self.path)?;

        println!("Added '{}' to workspace", self.path);
        println!();
        println!("Members ({}):", workspace.members().len());
        for member in workspace.members() {
            let rel_path = member.strip_prefix(&root).unwrap_or(member);
            println!("  {}", rel_path.display());
        }

        Ok(())
    }
}

// ============================================================================
// Remove Command
// ============================================================================

#[derive(Args)]
pub struct WorkspaceRemoveCommand {
    /// Path to the member project to remove
    pub path: String,

    /// Workspace root directory (defaults to searching upward)
    #[arg(long)]
    pub root: Option<PathBuf>,
}

impl WorkspaceRemoveCommand {
    pub async fn run(self) -> Result<()> {
        let root = match self.root {
            Some(r) => r.canonicalize()?,
            None => Workspace::find_root(&std::env::current_dir()?)
                .context("Not in a workspace. Run 'rx workspace init' first.")?,
        };

        let mut workspace = Workspace::load_from_root(&root)
            .context("Failed to load workspace")?;

        if workspace.remove_member(&self.path)? {
            println!("Removed '{}' from workspace", self.path);
        } else {
            println!("'{}' was not a workspace member", self.path);
        }

        Ok(())
    }
}

// ============================================================================
// List Command
// ============================================================================

#[derive(Args)]
pub struct WorkspaceListCommand {
    /// Workspace root directory (defaults to searching upward)
    #[arg(long)]
    pub root: Option<PathBuf>,

    /// Show detailed information
    #[arg(long, short)]
    pub verbose: bool,
}

impl WorkspaceListCommand {
    pub async fn run(self) -> Result<()> {
        let root = match self.root {
            Some(r) => r.canonicalize()?,
            None => Workspace::find_root(&std::env::current_dir()?)
                .context("Not in a workspace. Run 'rx workspace init' first.")?,
        };

        let workspace = Workspace::load_from_root(&root)
            .context("Failed to load workspace")?;

        println!("Workspace: {}", root.display());
        println!("Shared venv: {}", workspace.shared_venv);
        println!();

        let members = workspace.members();
        if members.is_empty() {
            println!("No members configured.");
            println!();
            println!("Add members with: rx workspace add <path>");
            return Ok(());
        }

        println!("Members ({}):", members.len());
        println!();

        if self.verbose {
            let info = workspace.member_info()?;
            for member in info {
                let name = member.name.as_deref().unwrap_or("<unnamed>");
                let version = member.version.as_deref().unwrap_or("0.0.0");
                println!("  {} ({}@{})", member.path, name, version);
                println!("    Dependencies: {}", member.dependency_count);
            }
        } else {
            for member in members {
                let rel_path = member.strip_prefix(&root).unwrap_or(member);
                println!("  {}", rel_path.display());
            }
        }

        Ok(())
    }
}

// ============================================================================
// Lock Command
// ============================================================================

#[derive(Args)]
pub struct WorkspaceLockCommand {
    /// Workspace root directory (defaults to searching upward)
    #[arg(long)]
    pub root: Option<PathBuf>,
}

impl WorkspaceLockCommand {
    pub async fn run(self) -> Result<()> {
        let root = match self.root {
            Some(r) => r.canonicalize()?,
            None => Workspace::find_root(&std::env::current_dir()?)
                .context("Not in a workspace. Run 'rx workspace init' first.")?,
        };

        let workspace = Workspace::load_from_root(&root)
            .context("Failed to load workspace")?;

        let members = workspace.members();
        if members.is_empty() {
            bail!("No members in workspace. Add members with 'rx workspace add <path>'.");
        }

        println!("Collecting dependencies from {} members...", members.len());

        // Collect all dependencies
        let all_deps = workspace.all_dependencies()?;
        println!("Found {} unique dependencies", all_deps.len());

        if all_deps.is_empty() {
            println!("No dependencies to lock.");
            return Ok(());
        }

        // Resolve
        println!("Resolving dependencies...");
        let resolver = Resolver::new();
        let resolution = resolver.resolve(&all_deps).await
            .context("Failed to resolve dependencies")?;

        // Create lockfile
        let lockfile = Lockfile::from_resolution(&resolution);
        let lockfile_path = workspace.lockfile_path();
        lockfile.save(&lockfile_path)?;

        println!();
        println!("Lockfile created at {}", lockfile_path.display());
        println!("Locked {} packages", lockfile.len());

        Ok(())
    }
}

// ============================================================================
// Sync Command
// ============================================================================

#[derive(Args)]
pub struct WorkspaceSyncCommand {
    /// Workspace root directory (defaults to searching upward)
    #[arg(long)]
    pub root: Option<PathBuf>,

    /// Recreate virtual environment from scratch
    #[arg(long)]
    pub recreate: bool,

    /// Python interpreter to use
    #[arg(long)]
    pub python: Option<PathBuf>,
}

impl WorkspaceSyncCommand {
    pub async fn run(self) -> Result<()> {
        let root = match self.root {
            Some(r) => r.canonicalize()?,
            None => Workspace::find_root(&std::env::current_dir()?)
                .context("Not in a workspace. Run 'rx workspace init' first.")?,
        };

        let workspace = Workspace::load_from_root(&root)
            .context("Failed to load workspace")?;

        let lockfile_path = workspace.lockfile_path();
        if !lockfile_path.exists() {
            bail!("No lockfile found. Run 'rx workspace lock' first.");
        }

        let lockfile = Lockfile::load(&lockfile_path)
            .context("Failed to load lockfile")?;

        if lockfile.is_empty() {
            println!("Lockfile is empty. Nothing to sync.");
            return Ok(());
        }

        // Determine venv path
        let venv_path = if workspace.shared_venv {
            workspace.venv_path()
        } else {
            // For non-shared, we sync each member separately
            // For now, just use workspace root
            workspace.venv_path()
        };

        // Setup virtual environment
        let venv = VenvManager::new(&venv_path);

        // Handle recreate
        if self.recreate && venv.exists() {
            println!("Removing existing virtual environment...");
            std::fs::remove_dir_all(&venv_path)?;
        }

        // Create venv if needed
        if !venv.exists() {
            println!("Creating virtual environment...");
            venv.create(self.python.as_deref())
                .context("Failed to create virtual environment")?;
        }

        let site_packages = venv
            .site_packages()
            .context("Failed to determine site-packages directory")?;

        // Install packages
        println!("Installing {} packages...", lockfile.len());

        let cache_dir = default_cache_dir();
        let installer = Installer::new(&cache_dir);

        let packages: std::collections::HashMap<_, _> = lockfile
            .packages
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let results = installer
            .install(&packages, &site_packages)
            .await
            .context("Installation failed")?;

        // Count results
        let mut installed = 0;
        let mut cached = 0;
        let mut downloaded = 0;

        for result in &results {
            if result.installed {
                installed += 1;
                if result.downloaded {
                    downloaded += 1;
                } else {
                    cached += 1;
                }
            }
        }

        println!();
        println!("Installed {} packages ({} downloaded, {} cached)",
            installed, downloaded, cached
        );

        // Install path dependencies from all workspace members
        let members = workspace.members();
        let mut total_path_deps = 0;

        for member in members {
            let path_deps = load_path_dependencies(member).unwrap_or_default();
            if !path_deps.is_empty() {
                if total_path_deps == 0 {
                    println!();
                    println!("Installing path dependencies:");
                }

                for (name, dep) in &path_deps {
                    let mode = if dep.editable { "editable" } else { "copy" };
                    match install_path_dependency(dep, member, &site_packages).await {
                        Ok(_) => {
                            println!("  {} ({}) ✓", name, mode);
                            total_path_deps += 1;
                        }
                        Err(e) => {
                            println!("  {} - FAILED: {}", name, e);
                        }
                    }
                }
            }
        }

        if total_path_deps > 0 {
            println!();
            println!("Installed {} path dependencies", total_path_deps);
        }

        if workspace.shared_venv {
            println!();
            println!("Shared virtual environment: {}", venv_path.display());
            println!();
            println!("All workspace members can use this venv with:");
            println!("  rx run --project <member> <command>");
        }

        Ok(())
    }
}
