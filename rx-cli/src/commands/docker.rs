//! Docker command - build and manage Docker images
//!
//! Generate Dockerfiles from configuration and build images directly.
//!
//! ```bash
//! # Generate Dockerfile from pyproject.toml config
//! rx docker generate
//!
//! # Build Docker image
//! rx docker build
//!
//! # Build with custom tag
//! rx docker build --tag myapp:latest
//!
//! # Generate and show Dockerfile without writing
//! rx docker generate --stdout
//! ```
//!
//! Configuration in pyproject.toml:
//! ```toml
//! [tool.rx.docker]
//! base-image = "python:3.11-slim"
//! entrypoint = ["python", "-m", "myapp"]
//! expose = [8000]
//! env = { APP_ENV = "production" }
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};

use rx_core::pep::PyProject;
use rx_core::{build_image, DockerConfig, DockerfileGenerator};

#[derive(Args)]
pub struct DockerCommand {
    #[command(subcommand)]
    pub command: DockerSubcommand,
}

#[derive(Subcommand)]
pub enum DockerSubcommand {
    /// Generate Dockerfile from configuration
    Generate(DockerGenerateCommand),

    /// Build Docker image
    Build(DockerBuildCommand),

    /// Show current Docker configuration
    Config(DockerConfigCommand),
}

impl DockerCommand {
    pub async fn run(self) -> Result<()> {
        match self.command {
            DockerSubcommand::Generate(cmd) => cmd.run().await,
            DockerSubcommand::Build(cmd) => cmd.run().await,
            DockerSubcommand::Config(cmd) => cmd.run().await,
        }
    }
}

// ============================================================================
// Generate Command
// ============================================================================

#[derive(Args)]
pub struct DockerGenerateCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Output to stdout instead of writing Dockerfile
    #[arg(long)]
    pub stdout: bool,

    /// Output file path (default: Dockerfile)
    #[arg(long, short)]
    pub output: Option<PathBuf>,

    /// Also generate .dockerignore
    #[arg(long)]
    pub dockerignore: bool,

    /// Force overwrite existing files
    #[arg(long, short)]
    pub force: bool,
}

impl DockerGenerateCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let generator = DockerfileGenerator::from_project(&project_dir)
            .context("Failed to load Docker configuration")?;

        let dockerfile_content = generator.generate();

        if self.stdout {
            println!("{}", dockerfile_content);
            return Ok(());
        }

        // Write Dockerfile
        let output_path = self.output.unwrap_or_else(|| project_dir.join("Dockerfile"));

        if output_path.exists() && !self.force {
            bail!(
                "Dockerfile already exists at {}. Use --force to overwrite.",
                output_path.display()
            );
        }

        std::fs::write(&output_path, &dockerfile_content)
            .context("Failed to write Dockerfile")?;

        println!("Generated Dockerfile at {}", output_path.display());

        // Optionally generate .dockerignore
        if self.dockerignore {
            let dockerignore_path = project_dir.join(".dockerignore");

            if dockerignore_path.exists() && !self.force {
                println!(
                    "Warning: .dockerignore already exists, skipping (use --force to overwrite)"
                );
            } else {
                let dockerignore_content = generator.generate_dockerignore();
                std::fs::write(&dockerignore_path, dockerignore_content)
                    .context("Failed to write .dockerignore")?;
                println!("Generated .dockerignore");
            }
        }

        println!();
        println!("Next steps:");
        println!("  rx docker build              # Build the image");
        println!("  docker run -it <image>       # Run the container");

        Ok(())
    }
}

// ============================================================================
// Build Command
// ============================================================================

#[derive(Args)]
pub struct DockerBuildCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Image tag (default: <project-name>:latest)
    #[arg(long, short)]
    pub tag: Option<String>,

    /// Additional tags
    #[arg(long)]
    pub additional_tag: Vec<String>,

    /// Build argument (KEY=VALUE)
    #[arg(long = "build-arg")]
    pub build_args: Vec<String>,

    /// Don't use cache when building
    #[arg(long)]
    pub no_cache: bool,

    /// Path to Dockerfile (generates one if not specified and none exists)
    #[arg(long, short)]
    pub file: Option<PathBuf>,

    /// Push image after building
    #[arg(long)]
    pub push: bool,

    /// Don't generate Dockerfile, fail if missing
    #[arg(long)]
    pub no_generate: bool,
}

impl DockerBuildCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Determine image tag
        let tag = match self.tag {
            Some(t) => t,
            None => {
                let pyproject = PyProject::load(&project_dir)
                    .context("Failed to load pyproject.toml")?;
                let name = pyproject.name().unwrap_or("app");
                let version = pyproject.version().unwrap_or("latest");
                format!("{}:{}", name, version)
            }
        };

        // Check for or generate Dockerfile
        let dockerfile_path = self.file.clone().unwrap_or_else(|| project_dir.join("Dockerfile"));

        if !dockerfile_path.exists() {
            if self.no_generate {
                bail!(
                    "Dockerfile not found at {}. Generate one with 'rx docker generate' or remove --no-generate.",
                    dockerfile_path.display()
                );
            }

            println!("Generating Dockerfile...");
            let generator = DockerfileGenerator::from_project(&project_dir)?;
            let dockerfile_content = generator.generate();
            std::fs::write(&dockerfile_path, &dockerfile_content)?;

            // Also generate .dockerignore if missing
            let dockerignore_path = project_dir.join(".dockerignore");
            if !dockerignore_path.exists() {
                let dockerignore_content = generator.generate_dockerignore();
                std::fs::write(&dockerignore_path, dockerignore_content)?;
            }
        }

        // Parse build args
        let mut build_args_map = HashMap::new();
        for arg in &self.build_args {
            let parts: Vec<&str> = arg.splitn(2, '=').collect();
            if parts.len() != 2 {
                bail!("Invalid build arg format: {}. Use KEY=VALUE", arg);
            }
            build_args_map.insert(parts[0].to_string(), parts[1].to_string());
        }

        // Build the image
        println!("Building Docker image: {}", tag);
        println!();

        build_image(
            &project_dir,
            &tag,
            Some(&dockerfile_path),
            &build_args_map,
            self.no_cache,
        )
        .context("Docker build failed")?;

        println!();
        println!("Successfully built: {}", tag);

        // Tag with additional tags
        for additional_tag in &self.additional_tag {
            tag_image(&tag, additional_tag)?;
            println!("Tagged: {}", additional_tag);
        }

        // Push if requested
        if self.push {
            println!();
            println!("Pushing image...");
            push_image(&tag)?;
            println!("Pushed: {}", tag);

            for additional_tag in &self.additional_tag {
                push_image(additional_tag)?;
                println!("Pushed: {}", additional_tag);
            }
        }

        println!();
        println!("Run with:");
        println!("  docker run -it {}", tag);

        Ok(())
    }
}

// ============================================================================
// Config Command
// ============================================================================

#[derive(Args)]
pub struct DockerConfigCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

impl DockerConfigCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let config = DockerConfig::load(&project_dir)
            .context("Failed to load Docker configuration")?;

        println!("Docker Configuration");
        println!();
        println!("  Base image:      {}", config.base_image);
        println!("  Python version:  {}", config.python_version);
        println!("  Working dir:     {}", config.workdir);
        println!("  Multi-stage:     {}", config.multi_stage);

        if let Some(ref user) = config.user {
            println!("  User:            {}", user);
        }

        if !config.expose.is_empty() {
            println!(
                "  Expose ports:    {}",
                config
                    .expose
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        if let Some(ref entrypoint) = config.entrypoint {
            println!("  Entrypoint:      {:?}", entrypoint);
        }

        if let Some(ref cmd) = config.cmd {
            println!("  CMD:             {:?}", cmd);
        }

        if !config.env.is_empty() {
            println!();
            println!("  Environment:");
            for (key, value) in &config.env {
                println!("    {}={}", key, value);
            }
        }

        if !config.apt_packages.is_empty() {
            println!();
            println!("  APT packages:    {}", config.apt_packages.join(", "));
        }

        if !config.labels.is_empty() {
            println!();
            println!("  Labels:");
            for (key, value) in &config.labels {
                println!("    {}={}", key, value);
            }
        }

        println!();
        println!("Configure in pyproject.toml:");
        println!();
        println!("  [tool.rx.docker]");
        println!("  base-image = \"python:3.11-slim\"");
        println!("  entrypoint = [\"python\", \"-m\", \"myapp\"]");
        println!("  expose = [8000]");

        Ok(())
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn tag_image(source: &str, target: &str) -> Result<()> {
    use std::process::Command;

    let status = Command::new("docker")
        .args(["tag", source, target])
        .status()
        .context("Failed to run docker tag")?;

    if !status.success() {
        bail!("Failed to tag image");
    }

    Ok(())
}

fn push_image(tag: &str) -> Result<()> {
    use std::process::Command;

    let status = Command::new("docker")
        .args(["push", tag])
        .status()
        .context("Failed to run docker push")?;

    if !status.success() {
        bail!("Failed to push image");
    }

    Ok(())
}
