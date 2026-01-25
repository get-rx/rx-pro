use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands::{AddCommand, BuildCommand, InitCommand, RunCommand, SyncCommand};

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

    /// Synchronize the virtual environment with the lockfile
    Sync(SyncCommand),

    /// Run a command in the project's virtual environment
    Run(RunCommand),

    /// Build the project (wheel and sdist)
    Build(BuildCommand),
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Commands::Init(cmd) => cmd.run().await,
            Commands::Add(cmd) => cmd.run().await,
            Commands::Sync(cmd) => cmd.run().await,
            Commands::Run(cmd) => cmd.run().await,
            Commands::Build(cmd) => cmd.run().await,
        }
    }
}
