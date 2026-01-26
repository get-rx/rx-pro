//! Build command - create wheel and sdist distributions

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use pro_core::builder::Builder;
use pro_core::pep::PyProject;

#[derive(Args)]
pub struct BuildCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".", global = true)]
    pub project: PathBuf,

    /// Output directory for built artifacts
    #[arg(short, long, default_value = "dist")]
    pub output: PathBuf,

    /// Build only wheel (skip sdist)
    #[arg(long)]
    pub wheel: bool,

    /// Build only source distribution (skip wheel)
    #[arg(long)]
    pub sdist: bool,
}

impl BuildCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Load pyproject.toml to get project info for display
        let pyproject = PyProject::load(&project_dir).with_context(|| {
            format!(
                "No pyproject.toml found in {:?}. Run 'rx init' first.",
                project_dir
            )
        })?;

        let project_name = pyproject.name().unwrap_or("unknown");
        let version = pyproject.version().unwrap_or("0.0.0");

        println!("Building {} v{}...", project_name, version);
        println!();

        // Resolve output directory relative to project
        let output_dir = if self.output.is_absolute() {
            self.output.clone()
        } else {
            project_dir.join(&self.output)
        };

        let builder = Builder::new(&project_dir);

        // Determine what to build
        let build_wheel = !self.sdist || self.wheel;
        let build_sdist = !self.wheel || self.sdist;

        // Build wheel
        if build_wheel {
            print!("  Building wheel...");
            match builder.build_wheel(&output_dir) {
                Ok(result) => {
                    println!(
                        " ✓ {} ({} bytes)",
                        result.path.file_name().unwrap().to_string_lossy(),
                        format_size(result.size)
                    );
                }
                Err(e) => {
                    println!(" ✗ Failed");
                    return Err(anyhow::anyhow!("Failed to build wheel: {}", e));
                }
            }
        }

        // Build sdist
        if build_sdist {
            print!("  Building sdist...");
            match builder.build_sdist(&output_dir) {
                Ok(result) => {
                    println!(
                        " ✓ {} ({} bytes)",
                        result.path.file_name().unwrap().to_string_lossy(),
                        format_size(result.size)
                    );
                }
                Err(e) => {
                    println!(" ✗ Failed");
                    return Err(anyhow::anyhow!("Failed to build sdist: {}", e));
                }
            }
        }

        println!();
        println!("✓ Built artifacts in {}", output_dir.display());

        Ok(())
    }
}

/// Format file size in human-readable format
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
