use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use tracing::{debug, info};

use pro_core::pep::{PyProject, Requirement};
use pro_core::resolver::Resolver;
use pro_core::Lockfile;

#[derive(Args)]
pub struct LockCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Include development dependencies
    #[arg(long)]
    pub dev: bool,
}

impl LockCommand {
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

        info!("Locking dependencies for {:?}", pyproject.name());

        // Collect all dependencies
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
            println!("No dependencies to lock.");
            // Still create an empty lockfile
            let lockfile = Lockfile::new();
            lockfile
                .save(&project_dir.join("rx.lock"))
                .with_context(|| "Failed to write rx.lock")?;
            println!("✓ Created empty rx.lock");
            return Ok(());
        }

        debug!("Resolving {} dependencies", all_requirements.len());

        // Resolve dependencies
        println!("Resolving {} dependencies...", all_requirements.len());
        let resolver = Resolver::new();
        let resolution = resolver
            .resolve(&all_requirements)
            .await
            .with_context(|| "Failed to resolve dependencies")?;

        println!("Locked {} packages:", resolution.len());
        for pkg in &resolution.packages {
            println!("  {} {}", pkg.name, pkg.version);
        }

        // Create lockfile
        let lockfile = Lockfile::from_resolution(&resolution);
        lockfile
            .save(&project_dir.join("rx.lock"))
            .with_context(|| "Failed to write rx.lock")?;

        println!();
        println!("✓ Wrote rx.lock");

        Ok(())
    }
}
