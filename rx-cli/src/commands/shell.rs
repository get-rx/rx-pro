//! Shell command - spawn an interactive subshell with venv activated
//!
//! Detects the user's shell and spawns a subshell with:
//! - VIRTUAL_ENV set
//! - PATH modified to include venv bin directory
//! - PS1 modified to show the venv name (for supported shells)

use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use clap::Args;

#[derive(Args)]
pub struct ShellCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Shell to use (auto-detected if not specified)
    #[arg(long, short)]
    pub shell: Option<String>,
}

impl ShellCommand {
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

        // Detect shell
        let shell_path = self.detect_shell()?;
        let shell_name = shell_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sh");

        // Get venv name for prompt
        let venv_name = venv_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(".venv");

        // Set up environment
        let current_path = std::env::var("PATH").unwrap_or_default();
        let path_sep = if cfg!(windows) { ";" } else { ":" };
        let new_path = format!("{}{}{}", bin_dir.display(), path_sep, current_path);

        println!("Spawning shell with virtual environment activated...");
        println!("  Shell: {}", shell_name);
        println!("  Venv: {:?}", venv_path);
        println!();
        println!("Type 'exit' or press Ctrl+D to return to your normal shell.");
        println!();

        // Build and execute the shell command
        let status = self.spawn_shell(
            &shell_path,
            shell_name,
            &project_dir,
            &venv_path,
            &bin_dir,
            &new_path,
            venv_name,
        )?;

        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }

        Ok(())
    }

    fn detect_shell(&self) -> Result<PathBuf> {
        // Use explicitly provided shell
        if let Some(ref shell) = self.shell {
            return Ok(PathBuf::from(shell));
        }

        // Try SHELL environment variable (Unix)
        #[cfg(unix)]
        if let Ok(shell) = std::env::var("SHELL") {
            return Ok(PathBuf::from(shell));
        }

        // Try COMSPEC on Windows
        #[cfg(windows)]
        if let Ok(comspec) = std::env::var("COMSPEC") {
            return Ok(PathBuf::from(comspec));
        }

        // Check for PowerShell on Windows
        #[cfg(windows)]
        {
            // Try pwsh (PowerShell Core) first
            if which_exists("pwsh") {
                return Ok(PathBuf::from("pwsh"));
            }
            // Then try powershell
            if which_exists("powershell") {
                return Ok(PathBuf::from("powershell"));
            }
        }

        // Fallback
        #[cfg(unix)]
        return Ok(PathBuf::from("/bin/sh"));

        #[cfg(windows)]
        return Ok(PathBuf::from("cmd.exe"));
    }

    fn spawn_shell(
        &self,
        shell_path: &PathBuf,
        shell_name: &str,
        project_dir: &PathBuf,
        venv_path: &PathBuf,
        bin_dir: &PathBuf,
        new_path: &str,
        venv_name: &str,
    ) -> Result<std::process::ExitStatus> {
        let mut cmd = Command::new(shell_path);

        // Set common environment
        cmd.current_dir(project_dir)
            .env("PATH", new_path)
            .env("VIRTUAL_ENV", venv_path)
            .env_remove("PYTHONHOME")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // Shell-specific configuration
        match shell_name {
            "bash" => {
                // Create a temporary rcfile that sources user's bashrc and activates venv
                let rcfile = self.create_bash_rcfile(venv_path, venv_name)?;
                cmd.arg("--rcfile").arg(&rcfile);
            }
            "zsh" => {
                // For zsh, we set ZDOTDIR to a temp directory with our .zshrc
                let zdotdir = self.create_zsh_zdotdir(venv_path, venv_name)?;
                cmd.env("ZDOTDIR", &zdotdir);
                // Also pass original ZDOTDIR for sourcing user config
                if let Ok(original) = std::env::var("ZDOTDIR") {
                    cmd.env("_RX_ORIGINAL_ZDOTDIR", original);
                } else if let Ok(home) = std::env::var("HOME") {
                    cmd.env("_RX_ORIGINAL_ZDOTDIR", home);
                }
            }
            "fish" => {
                // Fish uses a different activation script
                let activate_fish = bin_dir.join("activate.fish");
                if activate_fish.exists() {
                    cmd.arg("-C")
                        .arg(format!("source {}", activate_fish.display()));
                } else {
                    // Manual activation for fish
                    cmd.arg("-C").arg(format!(
                        "set -gx VIRTUAL_ENV {}; set -gx PATH {} $PATH",
                        venv_path.display(),
                        bin_dir.display()
                    ));
                }
            }
            "pwsh" | "powershell" => {
                // PowerShell activation
                let activate_ps1 = bin_dir.join("Activate.ps1");
                if activate_ps1.exists() {
                    cmd.arg("-NoExit")
                        .arg("-Command")
                        .arg(format!("& '{}'", activate_ps1.display()));
                } else {
                    // Manual activation
                    cmd.arg("-NoExit").arg("-Command").arg(format!(
                        "$env:VIRTUAL_ENV='{}'; $env:PATH='{}'+';'+$env:PATH; function prompt {{ '({}) ' + (Get-Location) + '> ' }}",
                        venv_path.display(),
                        bin_dir.display(),
                        venv_name
                    ));
                }
            }
            #[cfg(windows)]
            "cmd" | "cmd.exe" => {
                // CMD activation via batch file
                let activate_bat = bin_dir.join("activate.bat");
                if activate_bat.exists() {
                    cmd.arg("/K").arg(&activate_bat);
                } else {
                    // Manual - just set the environment, prompt will be basic
                    cmd.arg("/K")
                        .arg(format!("set VIRTUAL_ENV={}", venv_path.display()));
                }
            }
            _ => {
                // Generic POSIX shell - try sourcing activate script
                let activate = bin_dir.join("activate");
                if activate.exists() {
                    // For generic shells, just set environment (prompt may not update)
                    cmd.env("PS1", format!("({}) $ ", venv_name));
                }
            }
        }

        cmd.status()
            .with_context(|| format!("Failed to spawn shell: {}", shell_name))
    }

    fn create_bash_rcfile(&self, venv_path: &PathBuf, venv_name: &str) -> Result<PathBuf> {
        let temp_dir = std::env::temp_dir();
        let rcfile = temp_dir.join(format!("rx-bash-{}.rc", std::process::id()));

        let content = format!(
            r#"# T-Rex shell activation
# Source user's bashrc if it exists
if [ -f ~/.bashrc ]; then
    source ~/.bashrc
fi

# Activate virtual environment
export VIRTUAL_ENV="{venv_path}"
export PATH="{venv_path}/bin:$PATH"
unset PYTHONHOME

# Update prompt
PS1="({venv_name}) $PS1"
"#,
            venv_path = venv_path.display(),
            venv_name = venv_name
        );

        std::fs::write(&rcfile, content)?;
        Ok(rcfile)
    }

    fn create_zsh_zdotdir(&self, venv_path: &PathBuf, venv_name: &str) -> Result<PathBuf> {
        let temp_dir = std::env::temp_dir();
        let zdotdir = temp_dir.join(format!("rx-zsh-{}", std::process::id()));
        std::fs::create_dir_all(&zdotdir)?;

        let zshrc = zdotdir.join(".zshrc");
        let content = format!(
            r#"# T-Rex shell activation
# Source user's zshrc if it exists
if [ -n "$_RX_ORIGINAL_ZDOTDIR" ] && [ -f "$_RX_ORIGINAL_ZDOTDIR/.zshrc" ]; then
    source "$_RX_ORIGINAL_ZDOTDIR/.zshrc"
elif [ -f ~/.zshrc ]; then
    source ~/.zshrc
fi

# Activate virtual environment
export VIRTUAL_ENV="{venv_path}"
export PATH="{venv_path}/bin:$PATH"
unset PYTHONHOME

# Update prompt
PROMPT="({venv_name}) $PROMPT"
"#,
            venv_path = venv_path.display(),
            venv_name = venv_name
        );

        std::fs::write(&zshrc, content)?;
        Ok(zdotdir)
    }
}

/// Check if a command exists in PATH
#[cfg(windows)]
fn which_exists(cmd: &str) -> bool {
    Command::new("where")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell_with_explicit() {
        let cmd = ShellCommand {
            project: PathBuf::from("."),
            shell: Some("/bin/zsh".to_string()),
        };
        let shell = cmd.detect_shell().unwrap();
        assert_eq!(shell, PathBuf::from("/bin/zsh"));
    }

    #[test]
    fn test_detect_shell_from_env() {
        let cmd = ShellCommand {
            project: PathBuf::from("."),
            shell: None,
        };
        // This should succeed on any system
        let shell = cmd.detect_shell();
        assert!(shell.is_ok());
    }
}
