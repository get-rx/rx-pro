//! Tool runner command
//!
//! Provides commands for running Python tools in ephemeral environments:
//! - `rx tool run <package> [args...]` - Run a tool
//! - `rx tool list` - List cached tools
//! - `rx tool clear [package]` - Clear tool cache

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use pro_core::ToolRunner;

#[derive(Args)]
pub struct ToolCommand {
    #[command(subcommand)]
    pub subcommand: ToolSubcommand,
}

#[derive(Subcommand)]
pub enum ToolSubcommand {
    /// Run a Python tool (installs if not cached)
    Run(ToolRunCommand),

    /// List cached tools
    List(ToolListCommand),

    /// Clear tool cache
    Clear(ToolClearCommand),
}

#[derive(Args)]
pub struct ToolRunCommand {
    /// Package name to run
    pub package: String,

    /// Arguments to pass to the tool
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,

    /// Use a specific command from the package (if different from package name)
    #[arg(long)]
    pub command: Option<String>,
}

#[derive(Args)]
pub struct ToolListCommand {
    /// Show detailed information
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct ToolClearCommand {
    /// Package to clear from cache (clears all if not specified)
    pub package: Option<String>,
}

impl ToolCommand {
    pub async fn run(self) -> Result<()> {
        match self.subcommand {
            ToolSubcommand::Run(cmd) => cmd.run().await,
            ToolSubcommand::List(cmd) => cmd.run().await,
            ToolSubcommand::Clear(cmd) => cmd.run().await,
        }
    }
}

impl ToolRunCommand {
    pub async fn run(self) -> Result<()> {
        let runner = ToolRunner::new()
            .context("Failed to initialize tool runner")?;

        let command = self.command.as_deref().unwrap_or(&self.package);

        let status = runner
            .run_with_command(&self.package, command, &self.args)
            .await?;

        // Exit with the same code as the tool
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }

        Ok(())
    }
}

impl ToolListCommand {
    pub async fn run(self) -> Result<()> {
        let runner = ToolRunner::new()
            .context("Failed to initialize tool runner")?;

        let tools = runner.list_cached()?;

        if tools.is_empty() {
            println!("No tools cached.");
            println!();
            println!("Run a tool with: rx tool run <package> [args...]");
            println!("Example: rx tool run black --version");
            return Ok(());
        }

        println!("Cached tools:");
        println!();

        for tool in tools {
            if self.verbose {
                let version = tool.version.as_deref().unwrap_or("unknown");
                let age = tool
                    .cached_at
                    .elapsed()
                    .map(|d| format_duration(d))
                    .unwrap_or_else(|_| "unknown".to_string());

                println!("  {} (v{})", tool.package, version);
                println!("    Path: {}", tool.venv_path.display());
                println!("    Cached: {} ago", age);
            } else {
                let version = tool
                    .version
                    .as_ref()
                    .map(|v| format!(" ({})", v))
                    .unwrap_or_default();
                println!("  {}{}", tool.package, version);
            }
        }

        Ok(())
    }
}

impl ToolClearCommand {
    pub async fn run(self) -> Result<()> {
        let runner = ToolRunner::new()
            .context("Failed to initialize tool runner")?;

        match self.package {
            Some(package) => {
                if runner.clear(&package)? {
                    println!("Cleared {} from cache.", package);
                } else {
                    println!("{} was not in the cache.", package);
                }
            }
            None => {
                let count = runner.clear_all()?;
                if count > 0 {
                    println!("Cleared {} tool(s) from cache.", count);
                } else {
                    println!("Cache was already empty.");
                }
            }
        }

        Ok(())
    }
}

/// Format a duration in a human-readable way
fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();

    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}
