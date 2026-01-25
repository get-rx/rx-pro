use anyhow::Result;
use clap::Args;
use tracing::info;

#[derive(Args)]
pub struct AddCommand {
    /// Packages to add
    #[arg(required = true)]
    pub packages: Vec<String>,

    /// Add as development dependency
    #[arg(short, long)]
    pub dev: bool,

    /// Add as optional dependency group
    #[arg(short, long)]
    pub optional: Option<String>,
}

impl AddCommand {
    pub async fn run(self) -> Result<()> {
        let dep_type = if self.dev {
            "dev"
        } else if let Some(ref group) = self.optional {
            group.as_str()
        } else {
            "main"
        };

        info!("Adding {} dependencies: {:?}", dep_type, self.packages);

        // TODO: Implement dependency addition
        // - Parse and validate package specifiers
        // - Resolve dependencies with pubgrub
        // - Update pyproject.toml
        // - Update rx.lock
        // - Sync virtual environment

        for pkg in &self.packages {
            println!("Added {pkg}");
        }
        Ok(())
    }
}
