use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use tracing::{debug, info};

use rx_core::pep::{PyProject, Requirement};
use rx_core::resolver::Resolver;
use rx_core::Lockfile;

#[derive(Args)]
pub struct AddCommand {
    /// Packages to add (e.g., "requests", "flask>=2.0")
    #[arg(required = true)]
    pub packages: Vec<String>,

    /// Add as development dependency
    #[arg(short, long)]
    pub dev: bool,

    /// Add as optional dependency group
    #[arg(short, long)]
    pub optional: Option<String>,

    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

impl AddCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Load existing pyproject.toml
        let mut pyproject = PyProject::load(&project_dir)
            .with_context(|| {
                format!(
                    "No pyproject.toml found in {:?}. Run 'rx init' first.",
                    project_dir
                )
            })?;

        let dep_type = if self.dev {
            "dev"
        } else if let Some(ref group) = self.optional {
            group.as_str()
        } else {
            "main"
        };

        info!("Adding {} dependencies: {:?}", dep_type, self.packages);

        // Parse and validate package specifiers
        let mut new_requirements = Vec::new();
        for pkg_spec in &self.packages {
            // Parse as requirement to validate
            let req = Requirement::parse(pkg_spec)
                .with_context(|| format!("Invalid package specifier: {}", pkg_spec))?;

            new_requirements.push(req);

            // Add to pyproject.toml
            if self.dev {
                pyproject.add_dev_dependency(pkg_spec.clone());
            } else {
                pyproject.add_dependency(pkg_spec.clone());
            }
        }

        // Collect all dependencies for resolution
        let mut all_requirements: Vec<Requirement> = pyproject
            .dependencies()
            .iter()
            .filter_map(|s| Requirement::parse(s).ok())
            .collect();

        // Add dev dependencies if resolving with dev
        if self.dev {
            for dep in pyproject.dev_dependencies() {
                if let Ok(req) = Requirement::parse(dep) {
                    all_requirements.push(req);
                }
            }
        }

        debug!("Resolving {} total dependencies", all_requirements.len());

        // Resolve dependencies
        println!("Resolving dependencies...");
        let resolver = Resolver::new();
        let resolution = resolver
            .resolve(&all_requirements)
            .await
            .with_context(|| "Failed to resolve dependencies")?;

        println!("Resolved {} packages:", resolution.len());
        for pkg in &resolution.packages {
            println!("  {} {}", pkg.name, pkg.version);
        }

        // Save pyproject.toml
        pyproject.save(&project_dir)
            .with_context(|| "Failed to update pyproject.toml")?;
        println!("✓ Updated pyproject.toml");

        // Create/update lockfile
        let lockfile = Lockfile::from_resolution(&resolution);
        lockfile.save(&project_dir.join("rx.lock"))
            .with_context(|| "Failed to update rx.lock")?;
        println!("✓ Updated rx.lock");

        println!();
        for pkg in &self.packages {
            println!("Added {}", pkg);
        }

        println!();
        println!("Run 'rx sync' to install packages.");

        Ok(())
    }
}
