//! Affected command - detect changed workspace members
//!
//! Uses git to detect which packages have changed compared to a base branch.
//! Useful for CI/CD pipelines to only build/test affected packages.
//!
//! ```bash
//! # List affected packages
//! rx affected
//!
//! # Compare against a specific branch
//! rx affected --base develop
//!
//! # Show only package names (for scripting)
//! rx affected --names-only
//!
//! # Include transitive dependencies
//! rx affected --include-dependents
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use pro_core::workspace::Workspace;
use pro_core::{detect_affected, detect_affected_with_transitive, AffectedConfig};

#[derive(Args)]
pub struct AffectedCommand {
    /// Workspace root directory (defaults to searching upward)
    #[arg(long)]
    pub root: Option<PathBuf>,

    /// Base branch/commit to compare against (default: main)
    #[arg(long, default_value = "main")]
    pub base: String,

    /// Head branch/commit to compare (default: HEAD)
    #[arg(long, default_value = "HEAD")]
    pub head: String,

    /// Include packages that depend on changed packages
    #[arg(long)]
    pub include_dependents: bool,

    /// Only output package names (for scripting)
    #[arg(long)]
    pub names_only: bool,

    /// Only output package paths (for scripting)
    #[arg(long)]
    pub paths_only: bool,

    /// Show changed files
    #[arg(long)]
    pub files: bool,

    /// Don't include uncommitted changes
    #[arg(long)]
    pub committed_only: bool,

    /// Don't include untracked files
    #[arg(long)]
    pub no_untracked: bool,
}

impl AffectedCommand {
    pub async fn run(self) -> Result<()> {
        let root = match self.root {
            Some(r) => r.canonicalize()?,
            None => Workspace::find_root(&std::env::current_dir()?)
                .context("Not in a workspace. Run 'rx workspace init' first.")?,
        };

        let workspace = Workspace::load_from_root(&root).context("Failed to load workspace")?;

        let members = workspace.members();
        if members.is_empty() {
            if !self.names_only && !self.paths_only {
                println!("No members in workspace.");
            }
            return Ok(());
        }

        // Configure affected detection
        let config = AffectedConfig {
            base: self.base.clone(),
            head: self.head.clone(),
            uncommitted: !self.committed_only,
            untracked: !self.no_untracked,
        };

        // Detect affected packages
        let result = if self.include_dependents {
            detect_affected_with_transitive(&workspace, &config)?
        } else {
            detect_affected(&workspace, &config)?
        };

        // Output based on flags
        if self.files {
            if self.names_only || self.paths_only {
                for file in &result.changed_files {
                    println!("{}", file.display());
                }
            } else {
                println!("Changed files ({}):", result.changed_files.len());
                for file in &result.changed_files {
                    println!("  {}", file.display());
                }
            }
            return Ok(());
        }

        if result.all.is_empty() {
            if !self.names_only && !self.paths_only {
                println!("No affected packages detected.");
                println!();
                println!("Compared {} against {}", self.head, self.base);
            }
            return Ok(());
        }

        if self.paths_only {
            for member in &result.all {
                let rel = member.strip_prefix(&root).unwrap_or(member);
                println!("{}", rel.display());
            }
            return Ok(());
        }

        if self.names_only {
            for member in &result.all {
                if let Ok(pyproject) = pro_core::pep::PyProject::load(member) {
                    if let Some(name) = pyproject.name() {
                        println!("{}", name);
                    }
                }
            }
            return Ok(());
        }

        // Full output
        println!("Affected packages ({}):", result.all.len());
        println!();

        for member in &result.all {
            let rel = member.strip_prefix(&root).unwrap_or(member);
            let is_direct = result.direct.contains(member);

            if let Ok(pyproject) = pro_core::pep::PyProject::load(member) {
                let name = pyproject.name().unwrap_or("<unnamed>");
                let version = pyproject.version().unwrap_or("0.0.0");

                if is_direct {
                    println!(
                        "  {} ({}@{}) - directly changed",
                        rel.display(),
                        name,
                        version
                    );
                } else {
                    println!(
                        "  {} ({}@{}) - depends on changed",
                        rel.display(),
                        name,
                        version
                    );
                }
            } else if is_direct {
                println!("  {} - directly changed", rel.display());
            } else {
                println!("  {} - depends on changed", rel.display());
            }
        }

        println!();
        println!("Compared {} against {}", self.head, self.base);

        if !self.include_dependents && result.direct.len() < members.len() {
            println!();
            println!("Tip: Use --include-dependents to also show packages that depend on changed packages.");
        }

        Ok(())
    }
}
