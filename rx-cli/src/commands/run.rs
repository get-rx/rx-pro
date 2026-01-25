//! Run command - execute commands in the project's virtual environment

use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use clap::Args;

#[derive(Args)]
pub struct RunCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

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

        // Execute the command
        let status = Command::new(&program_path)
            .args(args)
            .current_dir(&project_dir)
            .env("PATH", &new_path)
            .env("VIRTUAL_ENV", &venv_path)
            .env_remove("PYTHONHOME") // Can interfere with venv
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| format!("Failed to execute '{}'", program))?;

        // Exit with the same code as the child process
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }

        Ok(())
    }
}
