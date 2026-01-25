//! Run command - execute commands in the project's virtual environment

use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use clap::Args;
use tracing::debug;

use rx_core::pep::PyProject;
use rx_core::{load_dotenv, DotenvConfig};

#[derive(Args)]
pub struct RunCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Don't load .env file
    #[arg(long)]
    pub no_dotenv: bool,

    /// Command to run (e.g., python, pytest, mypy)
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

impl RunCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Check for virtual environment
        let venv_path = project_dir.join(".venv");
        if !venv_path.exists() {
            bail!(
                "No virtual environment found at {:?}. Run 'rx sync' first.",
                venv_path
            );
        }

        // Get venv bin directory
        #[cfg(unix)]
        let bin_dir = venv_path.join("bin");
        #[cfg(windows)]
        let bin_dir = venv_path.join("Scripts");

        if !bin_dir.exists() {
            bail!(
                "Virtual environment appears corrupted (no bin directory). Run 'rx sync --recreate'."
            );
        }

        // Load dotenv configuration
        let dotenv_config = if self.no_dotenv {
            DotenvConfig {
                enabled: false,
                ..Default::default()
            }
        } else {
            self.load_dotenv_config(&project_dir)
        };

        // Load environment variables from .env
        let dotenv_vars = load_dotenv(&project_dir, &dotenv_config).unwrap_or_default();
        if !dotenv_vars.is_empty() {
            debug!("Loaded {} variables from .env", dotenv_vars.len());
        }

        // Build the command
        let (program, args) = match self.command.split_first() {
            Some((prog, args)) => (prog, args),
            None => bail!("No command specified"),
        };

        // Check if the command exists in the venv bin directory first
        let program_path = {
            #[cfg(unix)]
            let venv_program = bin_dir.join(program);
            #[cfg(windows)]
            let venv_program = bin_dir.join(format!("{}.exe", program));

            if venv_program.exists() {
                venv_program
            } else {
                // Fall back to letting the shell find it via PATH
                PathBuf::from(program)
            }
        };

        // Set up environment
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!(
            "{}{}{}",
            bin_dir.display(),
            if cfg!(windows) { ";" } else { ":" },
            current_path
        );

        // Build command with environment
        let mut cmd = Command::new(&program_path);
        cmd.args(args)
            .current_dir(&project_dir)
            .env("PATH", &new_path)
            .env("VIRTUAL_ENV", &venv_path)
            .env_remove("PYTHONHOME")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // Add dotenv variables (don't override existing env vars unless configured)
        for (key, value) in &dotenv_vars {
            if dotenv_config.override_env || std::env::var(key).is_err() {
                cmd.env(key, value);
            }
        }

        // Execute the command
        let status = cmd
            .status()
            .with_context(|| format!("Failed to execute '{}'", program))?;

        // Exit with the same code as the child process
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }

        Ok(())
    }

    fn load_dotenv_config(&self, project_dir: &PathBuf) -> DotenvConfig {
        // Try to load from pyproject.toml [tool.rx.dotenv]
        if let Ok(pyproject) = PyProject::load(project_dir) {
            if let Some(rx_config) = pyproject.tool.get("rx") {
                if let Some(dotenv_table) = rx_config.get("dotenv") {
                    if let Some(table) = dotenv_table.as_table() {
                        return DotenvConfig::from_toml(table);
                    }
                }
            }
        }

        // Default configuration
        DotenvConfig::new()
    }
}
