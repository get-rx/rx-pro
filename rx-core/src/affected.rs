//! Affected detection for workspace members
//!
//! Detects which workspace members have changed based on git diff.
//! Useful for CI/CD pipelines to only build/test affected packages.
//!
//! ```bash
//! # List affected packages
//! rx affected
//!
//! # Run tests only on affected packages
//! rx run --affected pytest
//! ```

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::workspace::Workspace;
use crate::{Error, Result};

/// Configuration for affected detection
#[derive(Debug, Clone)]
pub struct AffectedConfig {
    /// Base ref to compare against (default: main or master)
    pub base: String,
    /// Head ref to compare (default: HEAD)
    pub head: String,
    /// Include uncommitted changes
    pub uncommitted: bool,
    /// Include untracked files
    pub untracked: bool,
}

impl Default for AffectedConfig {
    fn default() -> Self {
        Self {
            base: "main".to_string(),
            head: "HEAD".to_string(),
            uncommitted: true,
            untracked: true,
        }
    }
}

impl AffectedConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    pub fn with_head(mut self, head: impl Into<String>) -> Self {
        self.head = head.into();
        self
    }
}

/// Result of affected detection
#[derive(Debug, Clone)]
pub struct AffectedResult {
    /// Directly affected members (files changed in their directory)
    pub direct: Vec<PathBuf>,
    /// All affected members including transitive (depends on changed packages)
    pub all: Vec<PathBuf>,
    /// Changed files that triggered the detection
    pub changed_files: Vec<PathBuf>,
}

/// Detect affected workspace members based on git changes
pub fn detect_affected(workspace: &Workspace, config: &AffectedConfig) -> Result<AffectedResult> {
    let root = &workspace.root;

    // Get changed files
    let changed_files = get_changed_files(root, config)?;

    if changed_files.is_empty() {
        return Ok(AffectedResult {
            direct: Vec::new(),
            all: Vec::new(),
            changed_files: Vec::new(),
        });
    }

    // Map files to members
    let members = workspace.members();
    let mut directly_affected: HashSet<PathBuf> = HashSet::new();

    for file in &changed_files {
        // Make file path relative to workspace root if absolute
        let relative_file = if file.is_absolute() {
            file.strip_prefix(root).unwrap_or(file)
        } else {
            file.as_path()
        };

        // Check which member this file belongs to
        for member in members {
            let relative_member = member.strip_prefix(root).unwrap_or(member);

            if relative_file.starts_with(relative_member) {
                directly_affected.insert(member.clone());
                break;
            }
        }
    }

    // For now, all affected = directly affected
    // Future: Add transitive dependency detection
    let direct: Vec<PathBuf> = directly_affected.iter().cloned().collect();
    let all = direct.clone();

    Ok(AffectedResult {
        direct,
        all,
        changed_files,
    })
}

/// Get list of changed files from git
fn get_changed_files(repo_root: &Path, config: &AffectedConfig) -> Result<Vec<PathBuf>> {
    let mut changed_files = HashSet::new();

    // Detect default branch if "main" doesn't exist
    let base = detect_base_branch(repo_root, &config.base)?;

    // Get committed changes between base and head
    let diff_output = Command::new("git")
        .args(["diff", "--name-only", &format!("{}...{}", base, config.head)])
        .current_dir(repo_root)
        .output()
        .map_err(|e| Error::Config(format!("Failed to run git diff: {}", e)))?;

    if diff_output.status.success() {
        let stdout = String::from_utf8_lossy(&diff_output.stdout);
        for line in stdout.lines() {
            if !line.is_empty() {
                changed_files.insert(PathBuf::from(line));
            }
        }
    }

    // Include uncommitted changes (staged + unstaged)
    if config.uncommitted {
        // Staged changes
        let staged_output = Command::new("git")
            .args(["diff", "--name-only", "--cached"])
            .current_dir(repo_root)
            .output()
            .map_err(|e| Error::Config(format!("Failed to run git diff --cached: {}", e)))?;

        if staged_output.status.success() {
            let stdout = String::from_utf8_lossy(&staged_output.stdout);
            for line in stdout.lines() {
                if !line.is_empty() {
                    changed_files.insert(PathBuf::from(line));
                }
            }
        }

        // Unstaged changes
        let unstaged_output = Command::new("git")
            .args(["diff", "--name-only"])
            .current_dir(repo_root)
            .output()
            .map_err(|e| Error::Config(format!("Failed to run git diff: {}", e)))?;

        if unstaged_output.status.success() {
            let stdout = String::from_utf8_lossy(&unstaged_output.stdout);
            for line in stdout.lines() {
                if !line.is_empty() {
                    changed_files.insert(PathBuf::from(line));
                }
            }
        }
    }

    // Include untracked files
    if config.untracked {
        let untracked_output = Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(repo_root)
            .output()
            .map_err(|e| Error::Config(format!("Failed to run git ls-files: {}", e)))?;

        if untracked_output.status.success() {
            let stdout = String::from_utf8_lossy(&untracked_output.stdout);
            for line in stdout.lines() {
                if !line.is_empty() {
                    changed_files.insert(PathBuf::from(line));
                }
            }
        }
    }

    let mut result: Vec<PathBuf> = changed_files.into_iter().collect();
    result.sort();
    Ok(result)
}

/// Detect the default base branch (main, master, or specified)
fn detect_base_branch(repo_root: &Path, preferred: &str) -> Result<String> {
    // Check if preferred branch exists
    let check = Command::new("git")
        .args(["rev-parse", "--verify", preferred])
        .current_dir(repo_root)
        .output();

    if let Ok(output) = check {
        if output.status.success() {
            return Ok(preferred.to_string());
        }
    }

    // Try common alternatives
    let alternatives = ["main", "master", "develop", "dev"];
    for alt in alternatives {
        if alt == preferred {
            continue;
        }

        let check = Command::new("git")
            .args(["rev-parse", "--verify", alt])
            .current_dir(repo_root)
            .output();

        if let Ok(output) = check {
            if output.status.success() {
                return Ok(alt.to_string());
            }
        }
    }

    // Try to get the default branch from origin
    let remote_check = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(repo_root)
        .output();

    if let Ok(output) = remote_check {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(branch) = stdout.trim().strip_prefix("refs/remotes/origin/") {
                return Ok(branch.to_string());
            }
        }
    }

    // Fall back to HEAD~1 if nothing else works
    Ok("HEAD~1".to_string())
}

/// Build dependency graph for workspace members
/// Returns a map of member path -> paths it depends on
pub fn build_dependency_graph(workspace: &Workspace) -> Result<HashMap<PathBuf, Vec<PathBuf>>> {
    use crate::pep::PyProject;

    let mut graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let members = workspace.members();

    // Build a map of package names to member paths
    let mut name_to_path: HashMap<String, PathBuf> = HashMap::new();
    for member in members {
        if let Ok(pyproject) = PyProject::load(member) {
            if let Some(name) = pyproject.name() {
                name_to_path.insert(name.to_lowercase(), member.clone());
                // Also add with underscores converted to dashes and vice versa
                name_to_path.insert(name.to_lowercase().replace('_', "-"), member.clone());
                name_to_path.insert(name.to_lowercase().replace('-', "_"), member.clone());
            }
        }
    }

    // Build the dependency graph
    for member in members {
        let mut deps = Vec::new();

        if let Ok(pyproject) = PyProject::load(member) {
            // Check regular dependencies
            for dep in pyproject.dependencies() {
                let dep_name = dep.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                    .next()
                    .unwrap_or("")
                    .to_lowercase();

                if let Some(dep_path) = name_to_path.get(&dep_name) {
                    if dep_path != member {
                        deps.push(dep_path.clone());
                    }
                }
            }

            // Check dev dependencies
            for dep in pyproject.dev_dependencies() {
                let dep_name = dep.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                    .next()
                    .unwrap_or("")
                    .to_lowercase();

                if let Some(dep_path) = name_to_path.get(&dep_name) {
                    if dep_path != member {
                        deps.push(dep_path.clone());
                    }
                }
            }

            // Check path dependencies
            if let Ok(path_deps) = crate::load_path_dependencies(member) {
                for (_name, path_dep) in path_deps {
                    let resolved = path_dep.resolve_path(member);
                    if members.contains(&resolved) && &resolved != member {
                        deps.push(resolved);
                    }
                }
            }
        }

        graph.insert(member.clone(), deps);
    }

    Ok(graph)
}

/// Get transitively affected members
/// If A depends on B, and B changed, then A is also affected
pub fn get_transitive_affected(
    directly_affected: &[PathBuf],
    dep_graph: &HashMap<PathBuf, Vec<PathBuf>>,
) -> Vec<PathBuf> {
    let mut all_affected: HashSet<PathBuf> = directly_affected.iter().cloned().collect();
    let mut changed = true;

    // Keep iterating until no new packages are found
    while changed {
        changed = false;
        let current_affected: Vec<PathBuf> = all_affected.iter().cloned().collect();

        for (member, deps) in dep_graph {
            if all_affected.contains(member) {
                continue;
            }

            // If this member depends on any affected package, it's also affected
            for dep in deps {
                if current_affected.contains(dep) {
                    all_affected.insert(member.clone());
                    changed = true;
                    break;
                }
            }
        }
    }

    let mut result: Vec<PathBuf> = all_affected.into_iter().collect();
    result.sort();
    result
}

/// Detect affected with transitive dependencies
pub fn detect_affected_with_transitive(
    workspace: &Workspace,
    config: &AffectedConfig,
) -> Result<AffectedResult> {
    let mut result = detect_affected(workspace, config)?;

    if !result.direct.is_empty() {
        let dep_graph = build_dependency_graph(workspace)?;
        result.all = get_transitive_affected(&result.direct, &dep_graph);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_affected_config_default() {
        let config = AffectedConfig::default();
        assert_eq!(config.base, "main");
        assert_eq!(config.head, "HEAD");
        assert!(config.uncommitted);
        assert!(config.untracked);
    }

    #[test]
    fn test_affected_config_builder() {
        let config = AffectedConfig::new()
            .with_base("develop")
            .with_head("feature-branch");

        assert_eq!(config.base, "develop");
        assert_eq!(config.head, "feature-branch");
    }

    #[test]
    fn test_transitive_affected() {
        let mut graph = HashMap::new();

        // A depends on B
        // B depends on C
        // D has no dependencies
        graph.insert(PathBuf::from("/workspace/a"), vec![PathBuf::from("/workspace/b")]);
        graph.insert(PathBuf::from("/workspace/b"), vec![PathBuf::from("/workspace/c")]);
        graph.insert(PathBuf::from("/workspace/c"), vec![]);
        graph.insert(PathBuf::from("/workspace/d"), vec![]);

        // If C changed, A and B should also be affected
        let directly_affected = vec![PathBuf::from("/workspace/c")];
        let all_affected = get_transitive_affected(&directly_affected, &graph);

        assert!(all_affected.contains(&PathBuf::from("/workspace/a")));
        assert!(all_affected.contains(&PathBuf::from("/workspace/b")));
        assert!(all_affected.contains(&PathBuf::from("/workspace/c")));
        assert!(!all_affected.contains(&PathBuf::from("/workspace/d")));
    }
}
