//! Polylith command - manage Polylith architecture
//!
//! Polylith organizes code into:
//! - **bases/**: Entry points (CLI, API, Lambda, etc.)
//! - **components/**: Reusable building blocks
//! - **projects/**: Deployable artifacts
//!
//! ```bash
//! # Initialize a Polylith workspace
//! rx polylith init myapp
//!
//! # Create a component
//! rx polylith create component user
//!
//! # Create a base (entry point)
//! rx polylith create base cli
//!
//! # Create a project combining bases and components
//! rx polylith create project myapp-service --base cli --component user --component database
//!
//! # List all bricks
//! rx polylith list
//!
//! # Check for issues (cycles, missing deps)
//! rx polylith check
//! ```

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};

use pro_core::{BrickType, Polylith};

#[derive(Args)]
pub struct PolylithCommand {
    #[command(subcommand)]
    pub command: PolylithSubcommand,
}

#[derive(Subcommand)]
pub enum PolylithSubcommand {
    /// Initialize a new Polylith workspace
    Init(PolylithInitCommand),

    /// Create a new brick (base, component, or project)
    Create(PolylithCreateCommand),

    /// List all bricks in the workspace
    List(PolylithListCommand),

    /// Check workspace for issues
    Check(PolylithCheckCommand),

    /// Show information about a brick
    Info(PolylithInfoCommand),
}

impl PolylithCommand {
    pub async fn run(self) -> Result<()> {
        match self.command {
            PolylithSubcommand::Init(cmd) => cmd.run().await,
            PolylithSubcommand::Create(cmd) => cmd.run().await,
            PolylithSubcommand::List(cmd) => cmd.run().await,
            PolylithSubcommand::Check(cmd) => cmd.run().await,
            PolylithSubcommand::Info(cmd) => cmd.run().await,
        }
    }
}

// ============================================================================
// Init Command
// ============================================================================

#[derive(Args)]
pub struct PolylithInitCommand {
    /// Top-level namespace for all bricks (e.g., "myapp")
    pub namespace: String,

    /// Directory to initialize (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
}

impl PolylithInitCommand {
    pub async fn run(self) -> Result<()> {
        let root = if self.path.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            std::fs::create_dir_all(&self.path)?;
            self.path.canonicalize()?
        };

        if Polylith::is_polylith(&root) {
            bail!("Directory is already a Polylith workspace");
        }

        let polylith = Polylith::init(&root, &self.namespace)
            .context("Failed to initialize Polylith workspace")?;

        println!("Initialized Polylith workspace");
        println!();
        println!("  Root: {}", root.display());
        println!("  Namespace: {}", polylith.top_namespace);
        println!();
        println!("Directory structure created:");
        println!("  bases/       - Entry points (CLI, API, etc.)");
        println!("  components/  - Reusable building blocks");
        println!("  projects/    - Deployable artifacts");
        println!();
        println!("Next steps:");
        println!("  rx polylith create component <name>  - Create a component");
        println!("  rx polylith create base <name>       - Create an entry point");
        println!("  rx polylith list                     - List all bricks");

        Ok(())
    }
}

// ============================================================================
// Create Command
// ============================================================================

#[derive(Args)]
pub struct PolylithCreateCommand {
    /// Type of brick to create (base, component, project)
    #[arg(value_parser = parse_brick_type)]
    pub brick_type: BrickType,

    /// Name of the brick
    pub name: String,

    /// Workspace root (defaults to searching upward)
    #[arg(long)]
    pub root: Option<PathBuf>,

    /// Bases to include (for projects only)
    #[arg(long = "base", short = 'b')]
    pub bases: Vec<String>,

    /// Components to include (for projects only)
    #[arg(long = "component", short = 'c')]
    pub components: Vec<String>,
}

fn parse_brick_type(s: &str) -> Result<BrickType, String> {
    match s.to_lowercase().as_str() {
        "base" | "bases" => Ok(BrickType::Base),
        "component" | "components" | "comp" => Ok(BrickType::Component),
        "project" | "projects" | "proj" => Ok(BrickType::Project),
        _ => Err(format!(
            "Invalid brick type '{}'. Use: base, component, or project",
            s
        )),
    }
}

impl PolylithCreateCommand {
    pub async fn run(self) -> Result<()> {
        let root = self.find_root()?;
        let mut polylith = Polylith::load(&root).context("Failed to load Polylith workspace")?;

        let brick = if self.brick_type == BrickType::Project {
            // Projects require bases and/or components
            if self.bases.is_empty() && self.components.is_empty() {
                bail!(
                    "Projects require at least one base or component.\n\
                     Use: rx polylith create project {} --base <name> --component <name>",
                    self.name
                );
            }

            polylith.create_project(&self.name, &self.bases, &self.components)?
        } else {
            polylith.create_brick(self.brick_type, &self.name)?
        };

        let rel_path = brick.path.strip_prefix(&root).unwrap_or(&brick.path);

        println!("Created {} '{}'", self.brick_type.as_str(), self.name);
        println!();
        println!("  Path: {}", rel_path.display());

        match self.brick_type {
            BrickType::Component => {
                println!();
                println!("Structure:");
                println!("  src/{}/", self.name.replace('-', "_"));
                println!("    __init__.py    - Package init");
                println!("    interface.py   - Public API (export here)");
                println!("    core.py        - Implementation");
                println!("  tests/");
                println!("    test_{}.py", self.name.replace('-', "_"));
            }
            BrickType::Base => {
                println!();
                println!("Structure:");
                println!("  src/{}/", self.name.replace('-', "_"));
                println!("    __init__.py    - Entry point");
                println!("  tests/");
                println!("    test_{}.py", self.name.replace('-', "_"));
            }
            BrickType::Project => {
                println!();
                println!("Includes:");
                for base in &self.bases {
                    println!("  base: {}", base);
                }
                for comp in &self.components {
                    println!("  component: {}", comp);
                }
            }
        }

        Ok(())
    }

    fn find_root(&self) -> Result<PathBuf> {
        if let Some(ref root) = self.root {
            return root.canonicalize().context("Invalid root path");
        }

        // Search upward for Polylith workspace
        let mut current = std::env::current_dir()?;
        loop {
            if Polylith::is_polylith(&current) {
                return Ok(current);
            }
            if !current.pop() {
                bail!(
                    "Not in a Polylith workspace.\n\
                     Initialize with: rx polylith init <namespace>"
                );
            }
        }
    }
}

// ============================================================================
// List Command
// ============================================================================

#[derive(Args)]
pub struct PolylithListCommand {
    /// Workspace root (defaults to searching upward)
    #[arg(long)]
    pub root: Option<PathBuf>,

    /// Show only bases
    #[arg(long)]
    pub bases: bool,

    /// Show only components
    #[arg(long)]
    pub components: bool,

    /// Show only projects
    #[arg(long)]
    pub projects: bool,

    /// Show detailed information
    #[arg(long, short)]
    pub verbose: bool,
}

impl PolylithListCommand {
    pub async fn run(self) -> Result<()> {
        let root = self.find_root()?;
        let polylith = Polylith::load(&root).context("Failed to load Polylith workspace")?;

        println!("Polylith workspace: {}", polylith.top_namespace);
        println!();

        let show_all = !self.bases && !self.components && !self.projects;

        if show_all || self.bases {
            self.print_bricks("Bases", &polylith.bases, &root);
        }

        if show_all || self.components {
            self.print_bricks("Components", &polylith.components, &root);
        }

        if show_all || self.projects {
            self.print_bricks("Projects", &polylith.projects, &root);
        }

        if show_all {
            let total = polylith.bases.len() + polylith.components.len() + polylith.projects.len();
            println!("Total: {} bricks", total);
        }

        Ok(())
    }

    fn print_bricks(&self, title: &str, bricks: &[pro_core::Brick], root: &PathBuf) {
        println!("{} ({}):", title, bricks.len());

        if bricks.is_empty() {
            println!("  (none)");
        } else {
            for brick in bricks {
                let rel_path = brick.path.strip_prefix(root).unwrap_or(&brick.path);

                if self.verbose {
                    println!("  {} ({})", brick.name, rel_path.display());
                    if !brick.brick_deps.is_empty() {
                        println!("    deps: {}", brick.brick_deps.join(", "));
                    }
                    if !brick.external_deps.is_empty() {
                        println!("    external: {}", brick.external_deps.join(", "));
                    }
                } else {
                    println!("  {}", brick.name);
                }
            }
        }
        println!();
    }

    fn find_root(&self) -> Result<PathBuf> {
        if let Some(ref root) = self.root {
            return root.canonicalize().context("Invalid root path");
        }

        let mut current = std::env::current_dir()?;
        loop {
            if Polylith::is_polylith(&current) {
                return Ok(current);
            }
            if !current.pop() {
                bail!("Not in a Polylith workspace");
            }
        }
    }
}

// ============================================================================
// Check Command
// ============================================================================

#[derive(Args)]
pub struct PolylithCheckCommand {
    /// Workspace root (defaults to searching upward)
    #[arg(long)]
    pub root: Option<PathBuf>,
}

impl PolylithCheckCommand {
    pub async fn run(self) -> Result<()> {
        let root = self.find_root()?;
        let polylith = Polylith::load(&root).context("Failed to load Polylith workspace")?;

        println!("Checking Polylith workspace...");
        println!();

        let mut issues = 0;

        // Check for dependency cycles
        print!("  Checking for dependency cycles... ");
        match polylith.check_cycles() {
            Ok(_) => println!("OK"),
            Err(e) => {
                println!("FAIL");
                println!("    {}", e);
                issues += 1;
            }
        }

        // Check that components don't depend on bases
        print!("  Checking component dependencies... ");
        let mut comp_issues = 0;
        for comp in &polylith.components {
            for dep in &comp.brick_deps {
                if polylith.bases.iter().any(|b| &b.name == dep) {
                    if comp_issues == 0 {
                        println!("FAIL");
                    }
                    println!(
                        "    Component '{}' depends on base '{}' (not allowed)",
                        comp.name, dep
                    );
                    comp_issues += 1;
                }
            }
        }
        if comp_issues == 0 {
            println!("OK");
        } else {
            issues += comp_issues;
        }

        // Check that bases only depend on components
        print!("  Checking base dependencies... ");
        let mut base_issues = 0;
        for base in &polylith.bases {
            for dep in &base.brick_deps {
                if polylith
                    .bases
                    .iter()
                    .any(|b| &b.name == dep && b.name != base.name)
                {
                    if base_issues == 0 {
                        println!("FAIL");
                    }
                    println!(
                        "    Base '{}' depends on another base '{}' (not recommended)",
                        base.name, dep
                    );
                    base_issues += 1;
                }
            }
        }
        if base_issues == 0 {
            println!("OK");
        } else {
            issues += base_issues;
        }

        // Summary
        println!();
        if issues == 0 {
            println!("All checks passed!");
        } else {
            println!("{} issue(s) found.", issues);
            std::process::exit(1);
        }

        Ok(())
    }

    fn find_root(&self) -> Result<PathBuf> {
        if let Some(ref root) = self.root {
            return root.canonicalize().context("Invalid root path");
        }

        let mut current = std::env::current_dir()?;
        loop {
            if Polylith::is_polylith(&current) {
                return Ok(current);
            }
            if !current.pop() {
                bail!("Not in a Polylith workspace");
            }
        }
    }
}

// ============================================================================
// Info Command
// ============================================================================

#[derive(Args)]
pub struct PolylithInfoCommand {
    /// Name of the brick to show info for
    pub name: String,

    /// Workspace root (defaults to searching upward)
    #[arg(long)]
    pub root: Option<PathBuf>,
}

impl PolylithInfoCommand {
    pub async fn run(self) -> Result<()> {
        let root = self.find_root()?;
        let polylith = Polylith::load(&root).context("Failed to load Polylith workspace")?;

        // Find the brick
        let brick = polylith
            .all_bricks()
            .into_iter()
            .find(|b| b.name == self.name)
            .ok_or_else(|| anyhow::anyhow!("Brick '{}' not found", self.name))?;

        let rel_path = brick.path.strip_prefix(&root).unwrap_or(&brick.path);

        println!("Brick: {}", brick.name);
        println!("Type: {}", brick.brick_type.as_str());
        println!("Path: {}", rel_path.display());
        println!();

        if !brick.brick_deps.is_empty() {
            println!("Brick dependencies:");
            for dep in &brick.brick_deps {
                println!("  {}", dep);
            }
            println!();
        }

        if !brick.external_deps.is_empty() {
            println!("External dependencies:");
            for dep in &brick.external_deps {
                println!("  {}", dep);
            }
            println!();
        }

        // Show which projects use this brick
        if brick.brick_type != BrickType::Project {
            let used_by: Vec<_> = polylith
                .projects
                .iter()
                .filter(|p| p.brick_deps.contains(&brick.name))
                .collect();

            if !used_by.is_empty() {
                println!("Used by projects:");
                for proj in used_by {
                    println!("  {}", proj.name);
                }
            }
        }

        Ok(())
    }

    fn find_root(&self) -> Result<PathBuf> {
        if let Some(ref root) = self.root {
            return root.canonicalize().context("Invalid root path");
        }

        let mut current = std::env::current_dir()?;
        loop {
            if Polylith::is_polylith(&current) {
                return Ok(current);
            }
            if !current.pop() {
                bail!("Not in a Polylith workspace");
            }
        }
    }
}
