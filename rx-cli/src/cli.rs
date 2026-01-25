use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands::{AddCommand, AuditCommand, BuildCommand, ExportCommand, InitCommand, LockCommand, PublishCommand, ReleaseCommand, RunCommand, SyncCommand, UpdateCommand, VersionCommand};

#[derive(Parser)]
#[command(
    name = "rx",
    version,
    about = "T-Rex: A fast Python package manager written in Rust",
    long_about = "T-Rex is a unified Python package manager and build tool that combines \
                  Rust-level performance with Poetry-like UX and WebAssembly plugin extensibility."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Python project
    Init(InitCommand),

    /// Add dependencies to the project
    Add(AddCommand),

    /// Generate/update the lockfile without installing
    Lock(LockCommand),

    /// Synchronize the virtual environment with the lockfile
    Sync(SyncCommand),

    /// Run a command in the project's virtual environment
    Run(RunCommand),

    /// Update dependencies to latest versions within constraints
    Update(UpdateCommand),

    /// Build the project (wheel and sdist)
    Build(BuildCommand),

    /// Export lockfile to requirements.txt or constraints.txt
    Export(ExportCommand),

    /// Check for security vulnerabilities in dependencies
    Audit(AuditCommand),

    /// Show or manage project version
    Version(VersionCommand),

    /// Create a release (bump version, changelog, tag)
    Release(ReleaseCommand),

    /// Publish package to PyPI
    Publish(PublishCommand),
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Commands::Init(cmd) => cmd.run().await,
            Commands::Add(cmd) => cmd.run().await,
            Commands::Lock(cmd) => cmd.run().await,
            Commands::Sync(cmd) => cmd.run().await,
            Commands::Run(cmd) => cmd.run().await,
            Commands::Update(cmd) => cmd.run().await,
            Commands::Build(cmd) => cmd.run().await,
            Commands::Export(cmd) => cmd.run().await,
            Commands::Audit(cmd) => cmd.run().await,
            Commands::Version(cmd) => cmd.run().await,
            Commands::Release(cmd) => cmd.run().await,
            Commands::Publish(cmd) => cmd.run().await,
        }
    }
}
