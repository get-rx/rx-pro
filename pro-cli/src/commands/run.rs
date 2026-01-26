//! Run command - execute commands in the project's virtual environment
//!
//! Supports script aliases defined in [tool.rx.scripts]:
//! ```toml
//! [tool.rx.scripts]
//! test = "pytest -v tests/"
//! lint = "ruff check ."
//! dev = "python -m myapp --debug"
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use clap::Args;
use tracing::debug;

use pro_core::pep::PyProject;
use pro_core::workspace::Workspace;
use pro_core::{
    detect_affected_with_transitive, is_pep723_script, load_dotenv, AffectedConfig, DotenvConfig,
    ScriptRunner,
};

#[derive(Args)]
pub struct RunCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Don't load .env file
    #[arg(long)]
    pub no_dotenv: bool,

    /// List available scripts
    #[arg(long)]
    pub list: bool,

    /// Run only on affected workspace members (based on git changes)
    #[arg(long)]
    pub affected: bool,

    /// Base branch for affected detection (default: main)
    #[arg(long, default_value = "main")]
    pub base: String,

    /// Command or script to run (e.g., python, pytest, or a script alias)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

impl RunCommand {
    pub async fn run(self) -> Result<()> {
        // Handle --affected flag for workspace
        if self.affected {
            return self.run_affected().await;
        }

        // Check if the first argument is a .py file with PEP 723 metadata
        if let Some(first_arg) = self.command.first() {
            let script_path = Path::new(first_arg);
            if script_path.extension().is_some_and(|e| e == "py") && script_path.exists() {
                // Check for PEP 723 metadata
                if is_pep723_script(script_path).unwrap_or(false) {
                    return self.run_pep723_script(script_path).await;
                }
            }
        }

        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Load scripts from pyproject.toml
        let scripts = self.load_scripts(&project_dir);

        // Handle --list flag
        if self.list {
            return self.list_scripts(&scripts);
        }

        // Require a command if not listing
        if self.command.is_empty() {
            if !scripts.is_empty() {
                println!("No command specified. Available scripts:");
                for (name, cmd) in &scripts {
                    println!("  {} → {}", name, cmd);
                }
                println!();
                println!("Run with: rx run <script>");
            }
            bail!("No command specified. Use 'rx run <command>' or 'rx run --list'");
        }

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

        // Resolve script alias or use command directly
        let (program, args) = self.resolve_command(&scripts)?;

        // Check if the command exists in the venv bin directory first
        let program_path = self.find_program(&program, &bin_dir);

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
        cmd.args(&args)
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

    fn list_scripts(&self, scripts: &HashMap<String, String>) -> Result<()> {
        if scripts.is_empty() {
            println!("No scripts defined.");
            println!();
            println!("Define scripts in pyproject.toml:");
            println!();
            println!("  [tool.rx.scripts]");
            println!("  test = \"pytest -v tests/\"");
            println!("  lint = \"ruff check .\"");
            println!("  dev = \"python -m myapp\"");
        } else {
            println!("Available scripts:");
            println!();

            // Sort scripts by name for consistent output
            let mut script_list: Vec<_> = scripts.iter().collect();
            script_list.sort_by(|a, b| a.0.cmp(b.0));

            for (name, cmd) in script_list {
                println!("  {} → {}", name, cmd);
            }
            println!();
            println!("Run with: rx run <script> [args...]");
        }
        Ok(())
    }

    fn load_scripts(&self, project_dir: &Path) -> HashMap<String, String> {
        let mut scripts = HashMap::new();

        if let Ok(pyproject) = PyProject::load(project_dir) {
            if let Some(rx_config) = pyproject.tool.get("rx") {
                if let Some(scripts_table) = rx_config.get("scripts") {
                    if let Some(table) = scripts_table.as_table() {
                        for (name, value) in table {
                            if let Some(cmd) = value.as_str() {
                                scripts.insert(name.clone(), cmd.to_string());
                            }
                        }
                    }
                }
            }
        }

        scripts
    }

    fn resolve_command(&self, scripts: &HashMap<String, String>) -> Result<(String, Vec<String>)> {
        let (first, rest) = match self.command.split_first() {
            Some((f, r)) => (f, r),
            None => bail!("No command specified"),
        };

        // Check if it's a script alias
        if let Some(script_cmd) = scripts.get(first) {
            debug!("Resolved script '{}' → '{}'", first, script_cmd);

            // Parse the script command
            let parsed = parse_script_command(script_cmd);
            if parsed.is_empty() {
                bail!("Script '{}' has empty command", first);
            }

            let (program, mut script_args) = parsed
                .split_first()
                .map(|(p, a)| (p.clone(), a.to_vec()))
                .unwrap();

            // Append any additional arguments passed by user
            script_args.extend(rest.iter().cloned());

            Ok((program, script_args))
        } else {
            // Not a script, use as-is
            Ok((first.clone(), rest.to_vec()))
        }
    }

    fn find_program(&self, program: &str, bin_dir: &Path) -> PathBuf {
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
    }

    fn load_dotenv_config(&self, project_dir: &Path) -> DotenvConfig {
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

    /// Run a PEP 723 script with inline dependencies
    async fn run_pep723_script(&self, script_path: &Path) -> Result<()> {
        let runner = ScriptRunner::new()
            .context("Failed to initialize script runner")?;

        // Get remaining arguments (after the script name)
        let args: Vec<String> = self.command.iter().skip(1).cloned().collect();

        debug!("Running PEP 723 script: {:?}", script_path);

        let status = runner.run(script_path, &args).await?;

        // Exit with the same code as the script
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }

        Ok(())
    }

    /// Run command on affected workspace members only
    async fn run_affected(&self) -> Result<()> {
        if self.command.is_empty() {
            bail!("No command specified. Use 'rx run --affected <command>'");
        }

        // Find workspace root
        let start_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let workspace = Workspace::load(&start_dir)
            .context("Not in a workspace. --affected only works in workspaces.")?;

        // Detect affected packages
        let config = AffectedConfig::new().with_base(&self.base);
        let result = detect_affected_with_transitive(&workspace, &config)?;

        if result.all.is_empty() {
            println!("No affected packages detected. Nothing to run.");
            return Ok(());
        }

        println!("Running on {} affected packages:", result.all.len());
        println!();

        let mut any_failed = false;

        for member in &result.all {
            let rel_path = member.strip_prefix(&workspace.root).unwrap_or(member);
            let name = if let Ok(pyproject) = PyProject::load(member) {
                pyproject.name().unwrap_or("<unnamed>").to_string()
            } else {
                rel_path.to_string_lossy().to_string()
            };

            println!("▸ {} ({})", name, rel_path.display());

            // Run the command in this member's directory
            match self.run_in_project(member).await {
                Ok(_) => {
                    println!("  ✓ Success");
                }
                Err(e) => {
                    println!("  ✗ Failed: {}", e);
                    any_failed = true;
                }
            }
            println!();
        }

        if any_failed {
            bail!("Some packages failed");
        }

        println!(
            "All {} affected packages completed successfully.",
            result.all.len()
        );
        Ok(())
    }

    /// Run the command in a specific project directory
    async fn run_in_project(&self, project_dir: &Path) -> Result<()> {
        // Check for virtual environment (try workspace venv or local)
        let workspace_venv = Workspace::find_root(project_dir)
            .ok()
            .map(|root| root.join(".venv"));

        let local_venv = project_dir.join(".venv");

        let venv_path = if local_venv.exists() {
            local_venv
        } else if let Some(ref ws_venv) = workspace_venv {
            if ws_venv.exists() {
                ws_venv.clone()
            } else {
                bail!("No virtual environment found. Run 'rx sync' or 'rx workspace sync' first.");
            }
        } else {
            bail!("No virtual environment found. Run 'rx sync' first.");
        };

        // Get venv bin directory
        #[cfg(unix)]
        let bin_dir = venv_path.join("bin");
        #[cfg(windows)]
        let bin_dir = venv_path.join("Scripts");

        // Load scripts from this project
        let scripts = self.load_scripts(project_dir);

        // Resolve command (may be a script alias)
        let (program, args) = self.resolve_command(&scripts)?;

        // Find program
        let program_path = self.find_program(&program, &bin_dir);

        // Set up environment
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!(
            "{}{}{}",
            bin_dir.display(),
            if cfg!(windows) { ";" } else { ":" },
            current_path
        );

        // Load dotenv
        let dotenv_config = if self.no_dotenv {
            DotenvConfig {
                enabled: false,
                ..Default::default()
            }
        } else {
            self.load_dotenv_config(project_dir)
        };
        let dotenv_vars = load_dotenv(project_dir, &dotenv_config).unwrap_or_default();

        // Build and execute command
        let mut cmd = Command::new(&program_path);
        cmd.args(&args)
            .current_dir(project_dir)
            .env("PATH", &new_path)
            .env("VIRTUAL_ENV", &venv_path)
            .env_remove("PYTHONHOME")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        for (key, value) in &dotenv_vars {
            if dotenv_config.override_env || std::env::var(key).is_err() {
                cmd.env(key, value);
            }
        }

        let status = cmd
            .status()
            .with_context(|| format!("Failed to execute '{}'", program))?;

        if !status.success() {
            bail!("Command exited with code {}", status.code().unwrap_or(1));
        }

        Ok(())
    }
}

/// Parse a script command string into program and arguments
/// Handles basic quoting (single and double quotes)
fn parse_script_command(cmd: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = cmd.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    result.push(current);
                    current = String::new();
                }
            }
            '\\' if in_double_quote => {
                // Handle escape sequences in double quotes
                if let Some(&next) = chars.peek() {
                    if next == '"' || next == '\\' || next == '$' {
                        chars.next();
                        current.push(next);
                    } else {
                        current.push(c);
                    }
                } else {
                    current.push(c);
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        result.push(current);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_command() {
        let result = parse_script_command("pytest -v tests/");
        assert_eq!(result, vec!["pytest", "-v", "tests/"]);
    }

    #[test]
    fn test_parse_quoted_command() {
        let result = parse_script_command("python -c \"print('hello')\"");
        assert_eq!(result, vec!["python", "-c", "print('hello')"]);
    }

    #[test]
    fn test_parse_single_quoted() {
        let result = parse_script_command("echo 'hello world'");
        assert_eq!(result, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_parse_mixed_quotes() {
        let result = parse_script_command("python -c \"print('test')\" --flag");
        assert_eq!(result, vec!["python", "-c", "print('test')", "--flag"]);
    }

    #[test]
    fn test_parse_empty() {
        let result = parse_script_command("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_with_equals() {
        let result = parse_script_command("mypy --config-file=mypy.ini src/");
        assert_eq!(result, vec!["mypy", "--config-file=mypy.ini", "src/"]);
    }
}
