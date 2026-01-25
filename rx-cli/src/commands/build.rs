use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use tracing::info;

#[derive(Args)]
pub struct BuildCommand {
    /// Output directory for built artifacts
    #[arg(short, long, default_value = "dist")]
    pub output: PathBuf,

    /// Build wheel
    #[arg(long, default_value = "true")]
    pub wheel: bool,

    /// Build source distribution
    #[arg(long)]
    pub sdist: bool,
}

impl BuildCommand {
    pub async fn run(self) -> Result<()> {
        info!("Building project to {:?}", self.output);

        // TODO: Implement build using rx-core
        // - Read pyproject.toml metadata
        // - Collect package files
        // - Generate METADATA, WHEEL, RECORD
        // - Create .whl archive
        // - Optionally create .tar.gz sdist

        println!("Built wheel to {:?}", self.output);
        Ok(())
    }
}
