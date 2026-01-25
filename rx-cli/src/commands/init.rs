use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use tracing::info;

use rx_core::pep::PyProject;
use rx_core::Lockfile;

#[derive(Args)]
pub struct InitCommand {
    /// Directory to initialize the project in
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Project name (defaults to directory name)
    #[arg(short, long)]
    pub name: Option<String>,

    /// Python version requirement
    #[arg(long, default_value = ">=3.8")]
    pub python: String,
}

impl InitCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.path.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            // Create directory if it doesn't exist
            if !self.path.exists() {
                std::fs::create_dir_all(&self.path)
                    .with_context(|| format!("Failed to create directory {:?}", self.path))?;
            }
            self.path.canonicalize()?
        };

        // Determine project name
        let name = self.name.unwrap_or_else(|| {
            project_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("myproject")
                .to_string()
        });

        // Validate project name (PEP 503 normalized)
        let normalized_name = name
            .to_lowercase()
            .replace(|c: char| !c.is_alphanumeric(), "-");

        info!(
            "Initializing project '{}' at {:?}",
            normalized_name, project_dir
        );

        // Check if pyproject.toml already exists
        let pyproject_path = project_dir.join("pyproject.toml");
        if pyproject_path.exists() {
            anyhow::bail!(
                "pyproject.toml already exists at {:?}. Use 'rx add' to add dependencies.",
                project_dir
            );
        }

        // Create pyproject.toml
        let pyproject = PyProject::new(&normalized_name, "0.1.0", &self.python);
        pyproject
            .save(&project_dir)
            .with_context(|| "Failed to create pyproject.toml")?;

        // Create empty lockfile
        let lockfile = Lockfile::new();
        lockfile
            .save(&project_dir.join("rx.lock"))
            .with_context(|| "Failed to create rx.lock")?;

        // Create src directory with __init__.py
        let src_dir = project_dir
            .join("src")
            .join(normalized_name.replace('-', "_"));
        std::fs::create_dir_all(&src_dir)
            .with_context(|| format!("Failed to create source directory {:?}", src_dir))?;

        let init_py = src_dir.join("__init__.py");
        if !init_py.exists() {
            std::fs::write(
                &init_py,
                format!(
                    "\"\"\"{}.\"\"\"\n\n__version__ = \"0.1.0\"\n",
                    normalized_name
                ),
            )
            .with_context(|| "Failed to create __init__.py")?;
        }

        // Create tests directory
        let tests_dir = project_dir.join("tests");
        std::fs::create_dir_all(&tests_dir).with_context(|| "Failed to create tests directory")?;

        let test_init = tests_dir.join("__init__.py");
        if !test_init.exists() {
            std::fs::write(&test_init, "").with_context(|| "Failed to create tests/__init__.py")?;
        }

        println!("✓ Created pyproject.toml");
        println!("✓ Created rx.lock");
        println!("✓ Created src/{}/", normalized_name.replace('-', "_"));
        println!("✓ Created tests/");
        println!();
        println!("Initialized T-Rex project '{}'", normalized_name);
        println!();
        println!("Next steps:");
        println!("  rx add <package>   Add a dependency");
        println!("  rx sync            Install dependencies");

        Ok(())
    }
}
