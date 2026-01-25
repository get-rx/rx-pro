//! Task runner - execute predefined tasks with dependencies
//!
//! Tasks are defined in [tool.rx.tasks] with optional dependencies:
//! ```toml
//! [tool.rx.tasks.build]
//! cmd = "python -m build"
//! description = "Build the package"
//!
//! [tool.rx.tasks.test]
//! cmd = "pytest -v tests/"
//! depends = ["build"]
//!
//! [tool.rx.tasks.check]
//! depends = ["lint", "test"]
//! description = "Run all checks"
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::Args;
use tracing::debug;

use rx_core::pep::PyProject;
use rx_core::{load_dotenv, DotenvConfig};

/// A task definition
#[derive(Debug, Clone)]
pub struct TaskDef {
    /// Task name
    pub name: String,
    /// Command to execute (optional if task only aggregates dependencies)
    pub cmd: Option<String>,
    /// Task description
    pub description: Option<String>,
    /// Dependencies (other task names)
    pub depends: Vec<String>,
}

#[derive(Args)]
pub struct TaskCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Don't load .env file
    #[arg(long)]
    pub no_dotenv: bool,

    /// List available tasks
    #[arg(long)]
    pub list: bool,

    /// Run tasks sequentially (disable parallel execution)
    #[arg(long)]
    pub sequential: bool,

    /// Task name to run
    #[arg()]
    pub task: Option<String>,
}

impl TaskCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Load tasks from pyproject.toml
        let tasks = self.load_tasks(&project_dir)?;

        // Handle --list flag
        if self.list {
            return self.list_tasks(&tasks);
        }

        // Require a task name
        let task_name = match &self.task {
            Some(name) => name.clone(),
            None => {
                if !tasks.is_empty() {
                    println!("No task specified. Available tasks:");
                    println!();
                    for (name, task) in &tasks {
                        let desc = task.description.as_deref().unwrap_or("");
                        if desc.is_empty() {
                            println!("  {}", name);
                        } else {
                            println!("  {} - {}", name, desc);
                        }
                    }
                    println!();
                }
                bail!("No task specified. Use 'rx task <name>' or 'rx task --list'");
            }
        };

        // Validate task exists
        if !tasks.contains_key(&task_name) {
            bail!(
                "Unknown task '{}'. Use 'rx task --list' to see available tasks.",
                task_name
            );
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

        // Load dotenv
        let dotenv_config = if self.no_dotenv {
            DotenvConfig {
                enabled: false,
                ..Default::default()
            }
        } else {
            self.load_dotenv_config(&project_dir)
        };
        let dotenv_vars = load_dotenv(&project_dir, &dotenv_config).unwrap_or_default();

        // Build execution plan
        let execution_order = self.build_execution_plan(&task_name, &tasks)?;

        println!("Running task '{}'...", task_name);
        if execution_order.len() > 1 {
            println!("  Execution plan: {}", execution_order.join(" → "));
        }
        println!();

        let start_time = Instant::now();

        // Execute tasks
        if self.sequential {
            self.run_sequential(
                &execution_order,
                &tasks,
                &project_dir,
                &venv_path,
                &bin_dir,
                &dotenv_vars,
                &dotenv_config,
            )?;
        } else {
            self.run_parallel(
                &execution_order,
                &tasks,
                &project_dir,
                &venv_path,
                &bin_dir,
                &dotenv_vars,
                &dotenv_config,
            )?;
        }

        let elapsed = start_time.elapsed();
        println!();
        println!(
            "✓ Task '{}' completed in {:.2}s",
            task_name,
            elapsed.as_secs_f64()
        );

        Ok(())
    }

    fn list_tasks(&self, tasks: &HashMap<String, TaskDef>) -> Result<()> {
        if tasks.is_empty() {
            println!("No tasks defined.");
            println!();
            println!("Define tasks in pyproject.toml:");
            println!();
            println!("  [tool.rx.tasks.build]");
            println!("  cmd = \"python -m build\"");
            println!("  description = \"Build the package\"");
            println!();
            println!("  [tool.rx.tasks.test]");
            println!("  cmd = \"pytest -v tests/\"");
            println!("  depends = [\"build\"]");
        } else {
            println!("Available tasks:");
            println!();

            let mut task_list: Vec<_> = tasks.iter().collect();
            task_list.sort_by(|a, b| a.0.cmp(b.0));

            for (name, task) in task_list {
                let desc = task.description.as_deref().unwrap_or("");
                let deps = if task.depends.is_empty() {
                    String::new()
                } else {
                    format!(" [depends: {}]", task.depends.join(", "))
                };

                if desc.is_empty() {
                    if let Some(cmd) = &task.cmd {
                        println!("  {} → {}{}", name, cmd, deps);
                    } else {
                        println!("  {}{}", name, deps);
                    }
                } else {
                    println!("  {} - {}{}", name, desc, deps);
                    if let Some(cmd) = &task.cmd {
                        println!("      → {}", cmd);
                    }
                }
            }
            println!();
            println!("Run with: rx task <name>");
        }
        Ok(())
    }

    fn load_tasks(&self, project_dir: &Path) -> Result<HashMap<String, TaskDef>> {
        let mut tasks = HashMap::new();

        let pyproject = PyProject::load(project_dir).ok();

        if let Some(pyproject) = pyproject {
            if let Some(rx_config) = pyproject.tool.get("rx") {
                if let Some(tasks_table) = rx_config.get("tasks") {
                    if let Some(table) = tasks_table.as_table() {
                        for (name, value) in table {
                            let task = self.parse_task(name, value)?;
                            tasks.insert(name.clone(), task);
                        }
                    }
                }
            }
        }

        Ok(tasks)
    }

    fn parse_task(&self, name: &str, value: &toml::Value) -> Result<TaskDef> {
        match value {
            toml::Value::String(cmd) => {
                // Simple form: task = "command"
                Ok(TaskDef {
                    name: name.to_string(),
                    cmd: Some(cmd.clone()),
                    description: None,
                    depends: vec![],
                })
            }
            toml::Value::Table(table) => {
                // Full form with options
                let cmd = table.get("cmd").and_then(|v| v.as_str()).map(String::from);
                let description = table
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let depends = table
                    .get("depends")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                Ok(TaskDef {
                    name: name.to_string(),
                    cmd,
                    description,
                    depends,
                })
            }
            _ => bail!("Invalid task definition for '{}'", name),
        }
    }

    fn build_execution_plan(
        &self,
        task_name: &str,
        tasks: &HashMap<String, TaskDef>,
    ) -> Result<Vec<String>> {
        // Topological sort with cycle detection
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();
        let mut order = Vec::new();

        self.visit_task(task_name, tasks, &mut visited, &mut in_stack, &mut order)?;

        Ok(order)
    }

    fn visit_task(
        &self,
        name: &str,
        tasks: &HashMap<String, TaskDef>,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<()> {
        if in_stack.contains(name) {
            bail!("Circular dependency detected involving task '{}'", name);
        }

        if visited.contains(name) {
            return Ok(());
        }

        let task = tasks
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown task '{}' referenced in dependencies", name))?;

        in_stack.insert(name.to_string());

        for dep in &task.depends {
            self.visit_task(dep, tasks, visited, in_stack, order)?;
        }

        in_stack.remove(name);
        visited.insert(name.to_string());
        order.push(name.to_string());

        Ok(())
    }

    fn run_sequential(
        &self,
        order: &[String],
        tasks: &HashMap<String, TaskDef>,
        project_dir: &Path,
        venv_path: &Path,
        bin_dir: &Path,
        dotenv_vars: &HashMap<String, String>,
        dotenv_config: &DotenvConfig,
    ) -> Result<()> {
        for task_name in order {
            let task = tasks.get(task_name).unwrap();
            if let Some(cmd) = &task.cmd {
                self.run_task_cmd(
                    task_name,
                    cmd,
                    project_dir,
                    venv_path,
                    bin_dir,
                    dotenv_vars,
                    dotenv_config,
                )?;
            } else {
                debug!("Task '{}' has no command (aggregator task)", task_name);
            }
        }
        Ok(())
    }

    fn run_parallel(
        &self,
        order: &[String],
        tasks: &HashMap<String, TaskDef>,
        project_dir: &Path,
        venv_path: &Path,
        bin_dir: &Path,
        dotenv_vars: &HashMap<String, String>,
        dotenv_config: &DotenvConfig,
    ) -> Result<()> {
        // For now, use a simple approach: run tasks level by level
        // Tasks at the same "depth" can run in parallel

        let mut completed: HashSet<String> = HashSet::new();
        let mut remaining: VecDeque<String> = order.iter().cloned().collect();

        while !remaining.is_empty() {
            // Find all tasks whose dependencies are satisfied
            let ready: Vec<String> = remaining
                .iter()
                .filter(|name| {
                    let task = tasks.get(*name).unwrap();
                    task.depends.iter().all(|dep| completed.contains(dep))
                })
                .cloned()
                .collect();

            if ready.is_empty() && !remaining.is_empty() {
                bail!("Deadlock detected - no tasks can proceed");
            }

            // Remove ready tasks from remaining
            remaining.retain(|name| !ready.contains(name));

            // Run ready tasks (in parallel if more than one)
            if ready.len() == 1 {
                let task_name = &ready[0];
                let task = tasks.get(task_name).unwrap();
                if let Some(cmd) = &task.cmd {
                    self.run_task_cmd(
                        task_name,
                        cmd,
                        project_dir,
                        venv_path,
                        bin_dir,
                        dotenv_vars,
                        dotenv_config,
                    )?;
                }
                completed.insert(task_name.clone());
            } else if ready.len() > 1 {
                // Run in parallel using threads
                let results = self.run_tasks_parallel(
                    &ready,
                    tasks,
                    project_dir,
                    venv_path,
                    bin_dir,
                    dotenv_vars,
                    dotenv_config,
                )?;

                for (task_name, result) in results {
                    result.with_context(|| format!("Task '{}' failed", task_name))?;
                    completed.insert(task_name);
                }
            }
        }

        Ok(())
    }

    fn run_tasks_parallel(
        &self,
        task_names: &[String],
        tasks: &HashMap<String, TaskDef>,
        project_dir: &Path,
        venv_path: &Path,
        bin_dir: &Path,
        dotenv_vars: &HashMap<String, String>,
        dotenv_config: &DotenvConfig,
    ) -> Result<Vec<(String, Result<()>)>> {
        use std::thread;

        let project_dir = project_dir.to_path_buf();
        let venv_path = venv_path.to_path_buf();
        let bin_dir = bin_dir.to_path_buf();
        let dotenv_vars = dotenv_vars.clone();
        let dotenv_config = dotenv_config.clone();

        // Collect task commands
        let task_cmds: Vec<(String, Option<String>)> = task_names
            .iter()
            .map(|name| {
                let task = tasks.get(name).unwrap();
                (name.clone(), task.cmd.clone())
            })
            .collect();

        let results: Arc<Mutex<Vec<(String, Result<()>)>>> = Arc::new(Mutex::new(Vec::new()));

        let handles: Vec<_> = task_cmds
            .into_iter()
            .map(|(name, cmd)| {
                let project_dir = project_dir.clone();
                let venv_path = venv_path.clone();
                let bin_dir = bin_dir.clone();
                let dotenv_vars = dotenv_vars.clone();
                let dotenv_config = dotenv_config.clone();
                let results = Arc::clone(&results);

                thread::spawn(move || {
                    let result = if let Some(cmd) = cmd {
                        run_task_cmd_static(
                            &name,
                            &cmd,
                            &project_dir,
                            &venv_path,
                            &bin_dir,
                            &dotenv_vars,
                            &dotenv_config,
                        )
                    } else {
                        Ok(())
                    };
                    results.lock().unwrap().push((name, result));
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            handle
                .join()
                .map_err(|_| anyhow::anyhow!("Thread panicked"))?;
        }

        let results = Arc::try_unwrap(results)
            .map_err(|_| anyhow::anyhow!("Failed to unwrap results"))?
            .into_inner()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;

        Ok(results)
    }

    fn run_task_cmd(
        &self,
        task_name: &str,
        cmd: &str,
        project_dir: &Path,
        venv_path: &Path,
        bin_dir: &Path,
        dotenv_vars: &HashMap<String, String>,
        dotenv_config: &DotenvConfig,
    ) -> Result<()> {
        run_task_cmd_static(
            task_name,
            cmd,
            project_dir,
            venv_path,
            bin_dir,
            dotenv_vars,
            dotenv_config,
        )
    }

    fn load_dotenv_config(&self, project_dir: &Path) -> DotenvConfig {
        if let Ok(pyproject) = PyProject::load(project_dir) {
            if let Some(rx_config) = pyproject.tool.get("rx") {
                if let Some(dotenv_table) = rx_config.get("dotenv") {
                    if let Some(table) = dotenv_table.as_table() {
                        return DotenvConfig::from_toml(table);
                    }
                }
            }
        }
        DotenvConfig::new()
    }
}

/// Static function for running task commands (needed for threading)
fn run_task_cmd_static(
    task_name: &str,
    cmd: &str,
    project_dir: &Path,
    venv_path: &Path,
    bin_dir: &Path,
    dotenv_vars: &HashMap<String, String>,
    dotenv_config: &DotenvConfig,
) -> Result<()> {
    println!("▶ {}: {}", task_name, cmd);

    // Parse command
    let parts = parse_command(cmd);
    if parts.is_empty() {
        bail!("Task '{}' has empty command", task_name);
    }

    let (program, args) = parts.split_first().unwrap();

    // Find program
    #[cfg(unix)]
    let venv_program = bin_dir.join(program);
    #[cfg(windows)]
    let venv_program = bin_dir.join(format!("{}.exe", program));

    let program_path = if venv_program.exists() {
        venv_program
    } else {
        PathBuf::from(program)
    };

    // Set up PATH
    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!(
        "{}{}{}",
        bin_dir.display(),
        if cfg!(windows) { ";" } else { ":" },
        current_path
    );

    // Build command
    let mut command = Command::new(&program_path);
    command
        .args(args)
        .current_dir(project_dir)
        .env("PATH", &new_path)
        .env("VIRTUAL_ENV", venv_path)
        .env_remove("PYTHONHOME")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // Add dotenv vars
    for (key, value) in dotenv_vars {
        if dotenv_config.override_env || std::env::var(key).is_err() {
            command.env(key, value);
        }
    }

    // Execute
    let status = command
        .status()
        .with_context(|| format!("Failed to execute task '{}'", task_name))?;

    if !status.success() {
        bail!(
            "Task '{}' failed with exit code {}",
            task_name,
            status.code().unwrap_or(-1)
        );
    }

    Ok(())
}

/// Parse command string into parts
fn parse_command(cmd: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = cmd.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    result.push(current);
                    current = String::new();
                }
            }
            '\\' if in_double_quote => {
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
            _ => current.push(c),
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
    fn test_parse_command() {
        assert_eq!(
            parse_command("pytest -v tests/"),
            vec!["pytest", "-v", "tests/"]
        );
        assert_eq!(
            parse_command("python -c \"print('hi')\""),
            vec!["python", "-c", "print('hi')"]
        );
        assert_eq!(
            parse_command("echo 'hello world'"),
            vec!["echo", "hello world"]
        );
    }

    #[test]
    fn test_parse_task_simple() {
        let cmd = TaskCommand {
            project: PathBuf::from("."),
            no_dotenv: false,
            list: false,
            sequential: false,
            task: None,
        };

        let value = toml::Value::String("pytest -v".to_string());
        let task = cmd.parse_task("test", &value).unwrap();

        assert_eq!(task.name, "test");
        assert_eq!(task.cmd, Some("pytest -v".to_string()));
        assert!(task.depends.is_empty());
    }

    #[test]
    fn test_parse_task_full() {
        let cmd = TaskCommand {
            project: PathBuf::from("."),
            no_dotenv: false,
            list: false,
            sequential: false,
            task: None,
        };

        let value: toml::Value = toml::from_str(
            r#"
            cmd = "pytest -v"
            description = "Run tests"
            depends = ["build", "lint"]
        "#,
        )
        .unwrap();

        let task = cmd.parse_task("test", &value).unwrap();

        assert_eq!(task.name, "test");
        assert_eq!(task.cmd, Some("pytest -v".to_string()));
        assert_eq!(task.description, Some("Run tests".to_string()));
        assert_eq!(task.depends, vec!["build", "lint"]);
    }

    #[test]
    fn test_execution_plan() {
        let cmd = TaskCommand {
            project: PathBuf::from("."),
            no_dotenv: false,
            list: false,
            sequential: false,
            task: None,
        };

        let mut tasks = HashMap::new();
        tasks.insert(
            "build".to_string(),
            TaskDef {
                name: "build".to_string(),
                cmd: Some("echo build".to_string()),
                description: None,
                depends: vec![],
            },
        );
        tasks.insert(
            "test".to_string(),
            TaskDef {
                name: "test".to_string(),
                cmd: Some("echo test".to_string()),
                description: None,
                depends: vec!["build".to_string()],
            },
        );
        tasks.insert(
            "deploy".to_string(),
            TaskDef {
                name: "deploy".to_string(),
                cmd: Some("echo deploy".to_string()),
                description: None,
                depends: vec!["test".to_string()],
            },
        );

        let plan = cmd.build_execution_plan("deploy", &tasks).unwrap();
        assert_eq!(plan, vec!["build", "test", "deploy"]);
    }

    #[test]
    fn test_circular_dependency_detection() {
        let cmd = TaskCommand {
            project: PathBuf::from("."),
            no_dotenv: false,
            list: false,
            sequential: false,
            task: None,
        };

        let mut tasks = HashMap::new();
        tasks.insert(
            "a".to_string(),
            TaskDef {
                name: "a".to_string(),
                cmd: Some("echo a".to_string()),
                description: None,
                depends: vec!["b".to_string()],
            },
        );
        tasks.insert(
            "b".to_string(),
            TaskDef {
                name: "b".to_string(),
                cmd: Some("echo b".to_string()),
                description: None,
                depends: vec!["a".to_string()],
            },
        );

        let result = cmd.build_execution_plan("a", &tasks);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency"));
    }
}
