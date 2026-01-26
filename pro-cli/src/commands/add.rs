use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Args;
use tracing::{debug, info};

use pro_core::pep::{PyProject, Requirement};
use pro_core::resolver::Resolver;
use pro_core::{Lockfile, PathDependency};

#[derive(Args)]
pub struct AddCommand {
    /// Packages to add (e.g., "requests", "flask>=2.0", or paths with -e)
    #[arg(required = true)]
    pub packages: Vec<String>,

    /// Add as development dependency
    #[arg(short, long)]
    pub dev: bool,

    /// Add as editable path dependency (for local packages)
    #[arg(short = 'e', long)]
    pub editable: bool,

    /// Add as non-editable path dependency (copy mode)
    #[arg(long, conflicts_with = "editable")]
    pub path: bool,

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
        let mut pyproject = PyProject::load(&project_dir).with_context(|| {
            format!(
                "No pyproject.toml found in {:?}. Run 'rx init' first.",
                project_dir
            )
        })?;

        // Handle path dependencies (editable or copy mode)
        if self.editable || self.path {
            return self
                .add_path_dependencies(&project_dir, &mut pyproject)
                .await;
        }

        // Regular PyPI dependencies
        self.add_pypi_dependencies(&project_dir, &mut pyproject)
            .await
    }

    /// Add path dependencies (editable or copy mode)
    async fn add_path_dependencies(
        &self,
        project_dir: &PathBuf,
        pyproject: &mut PyProject,
    ) -> Result<()> {
        let editable = self.editable; // true for -e, false for --path
        let mode = if editable { "editable" } else { "copy" };

        info!("Adding {} path dependencies: {:?}", mode, self.packages);

        for path_str in &self.packages {
            let dep_path = PathBuf::from(path_str);

            // Resolve relative to project directory
            let absolute_path = if dep_path.is_absolute() {
                dep_path.clone()
            } else {
                project_dir.join(&dep_path)
            };

            // Validate the path exists
            if !absolute_path.exists() {
                bail!("Path not found: {}", absolute_path.display());
            }

            // Check for pyproject.toml in the path
            let dep_pyproject_path = absolute_path.join("pyproject.toml");
            if !dep_pyproject_path.exists() {
                bail!(
                    "No pyproject.toml found at {}. Is this a Python package?",
                    absolute_path.display()
                );
            }

            // Load the dependency's pyproject.toml to get its name
            let dep_pyproject = PyProject::load(&absolute_path)
                .with_context(|| format!("Failed to load pyproject.toml from {}", path_str))?;

            let name = dep_pyproject
                .name()
                .ok_or_else(|| {
                    anyhow::anyhow!("Package at {} has no name in pyproject.toml", path_str)
                })?
                .to_string();

            // Validate as a path dependency
            let path_dep = PathDependency::new(&name, &dep_path).with_editable(editable);
            path_dep.validate(project_dir)?;

            // Add to pyproject.toml [tool.rx.dependencies]
            pyproject.add_path_dependency(name.clone(), path_str.clone(), editable);

            println!("Added {} ({}) from {}", name, mode, path_str);
        }

        // Save pyproject.toml
        pyproject
            .save(project_dir)
            .with_context(|| "Failed to update pyproject.toml")?;
        println!("✓ Updated pyproject.toml");

        println!();
        println!("Run 'rx sync' to install path dependencies.");

        Ok(())
    }

    /// Add regular PyPI dependencies
    async fn add_pypi_dependencies(
        &self,
        project_dir: &PathBuf,
        pyproject: &mut PyProject,
    ) -> Result<()> {
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
        pyproject
            .save(project_dir)
            .with_context(|| "Failed to update pyproject.toml")?;
        println!("✓ Updated pyproject.toml");

        // Create/update lockfile
        let lockfile = Lockfile::from_resolution(&resolution);
        lockfile
            .save(&project_dir.join("rx.lock"))
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
