use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use tracing::info;

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
        info!("Initializing project at {:?}", self.path);

        // TODO: Implement project initialization
        // - Create pyproject.toml with PEP 621 metadata
        // - Create virtual environment
        // - Generate rx.lock

        println!("Initialized T-Rex project at {:?}", self.path);
        Ok(())
    }
}
