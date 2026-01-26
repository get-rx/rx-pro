//! Workspace support for monorepo management
//!
//! A workspace is a collection of related Python projects that share:
//! - A unified lockfile (rx.lock) at the workspace root
//! - Optionally, a shared virtual environment
//!
//! Configuration is stored in pyproject.toml:
//! ```toml
//! [tool.rx.workspace]
//! members = ["packages/*", "apps/myapp"]
//! shared-venv = true  # optional, default false
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::pep::PyProject;
use crate::{Error, Result};

/// Workspace configuration
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Root directory of the workspace
    pub root: PathBuf,
    /// Member patterns (glob patterns or paths)
    pub member_patterns: Vec<String>,
    /// Whether to use a shared venv for all members
    pub shared_venv: bool,
    /// Resolved member paths
    members: Vec<PathBuf>,
}

impl Workspace {
    /// Load workspace from a directory (searches upward for workspace root)
    pub fn load(start_dir: &Path) -> Result<Self> {
        let root = Self::find_root(start_dir)?;
        Self::load_from_root(&root)
    }

    /// Load workspace from a known root directory
    pub fn load_from_root(root: &Path) -> Result<Self> {
        let pyproject = PyProject::load(root)?;

        let rx_config = pyproject
            .tool
            .get("rx")
            .ok_or_else(|| Error::WorkspaceNotFound)?;

        let workspace_config = rx_config
            .get("workspace")
            .ok_or_else(|| Error::WorkspaceNotFound)?;

        let members: Vec<String> = workspace_config
            .get("members")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let shared_venv = workspace_config
            .get("shared-venv")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut workspace = Self {
            root: root.to_path_buf(),
            member_patterns: members,
            shared_venv,
            members: Vec::new(),
        };

        workspace.resolve_members()?;

        Ok(workspace)
    }

    /// Find workspace root by searching upward for [tool.rx.workspace]
    pub fn find_root(start_dir: &Path) -> Result<PathBuf> {
        let mut current = start_dir.to_path_buf();

        loop {
            let pyproject_path = current.join("pyproject.toml");
            if pyproject_path.exists() {
                if let Ok(pyproject) = PyProject::load(&current) {
                    if let Some(rx_config) = pyproject.tool.get("rx") {
                        if rx_config.get("workspace").is_some() {
                            return Ok(current);
                        }
                    }
                }
            }

            if !current.pop() {
                return Err(Error::WorkspaceNotFound);
            }
        }
    }

    /// Check if a directory is a workspace root
    pub fn is_workspace_root(dir: &Path) -> bool {
        if let Ok(pyproject) = PyProject::load(dir) {
            if let Some(rx_config) = pyproject.tool.get("rx") {
                return rx_config.get("workspace").is_some();
            }
        }
        false
    }

    /// Create a new workspace
    pub fn create(root: &Path, shared_venv: bool) -> Result<Self> {
        // Load or create pyproject.toml
        let pyproject_path = root.join("pyproject.toml");

        let content = if pyproject_path.exists() {
            std::fs::read_to_string(&pyproject_path).map_err(Error::Io)?
        } else {
            // Create minimal pyproject.toml
            r#"[project]
name = "workspace-root"
version = "0.0.0"
description = "Workspace root - not a package"
"#
            .to_string()
        };

        // Parse and update
        let mut doc: toml_edit::DocumentMut = content
            .parse()
            .map_err(|e| Error::Config(format!("Failed to parse pyproject.toml: {}", e)))?;

        // Ensure [tool.rx.workspace] exists
        if !doc.contains_key("tool") {
            doc["tool"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        if !doc["tool"].as_table().unwrap().contains_key("rx") {
            doc["tool"]["rx"] = toml_edit::Item::Table(toml_edit::Table::new());
        }

        let rx_table = doc["tool"]["rx"].as_table_mut().unwrap();
        if !rx_table.contains_key("workspace") {
            rx_table["workspace"] = toml_edit::Item::Table(toml_edit::Table::new());
        }

        let workspace_table = rx_table["workspace"].as_table_mut().unwrap();

        // Set members array if not exists
        if !workspace_table.contains_key("members") {
            workspace_table["members"] = toml_edit::Item::Value(toml_edit::Array::new().into());
        }

        // Set shared-venv
        workspace_table["shared-venv"] = toml_edit::Item::Value(shared_venv.into());

        // Write back
        std::fs::write(&pyproject_path, doc.to_string()).map_err(Error::Io)?;

        Ok(Self {
            root: root.to_path_buf(),
            member_patterns: Vec::new(),
            shared_venv,
            members: Vec::new(),
        })
    }

    /// Add a member to the workspace
    pub fn add_member(&mut self, path: &str) -> Result<()> {
        // Verify the path exists and has a pyproject.toml
        let member_path = self.root.join(path);
        if !member_path.exists() {
            return Err(Error::Config(format!(
                "Member path does not exist: {}",
                member_path.display()
            )));
        }

        let member_pyproject = member_path.join("pyproject.toml");
        if !member_pyproject.exists() {
            return Err(Error::Config(format!(
                "Member does not have pyproject.toml: {}",
                member_path.display()
            )));
        }

        // Add to member patterns if not already present
        let path_str = path.to_string();
        if !self.member_patterns.contains(&path_str) {
            self.member_patterns.push(path_str);
        }

        // Update pyproject.toml
        self.save()?;

        // Re-resolve members
        self.resolve_members()?;

        Ok(())
    }

    /// Remove a member from the workspace
    pub fn remove_member(&mut self, path: &str) -> Result<bool> {
        let path_str = path.to_string();
        let initial_len = self.member_patterns.len();

        self.member_patterns.retain(|p| p != &path_str);

        if self.member_patterns.len() < initial_len {
            self.save()?;
            self.resolve_members()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Save workspace configuration to pyproject.toml
    pub fn save(&self) -> Result<()> {
        let pyproject_path = self.root.join("pyproject.toml");
        let content = std::fs::read_to_string(&pyproject_path).map_err(Error::Io)?;

        let mut doc: toml_edit::DocumentMut = content
            .parse()
            .map_err(|e| Error::Config(format!("Failed to parse pyproject.toml: {}", e)))?;

        // Update members array
        let members_array: toml_edit::Array = self
            .member_patterns
            .iter()
            .map(|s| toml_edit::Value::from(s.as_str()))
            .collect();

        doc["tool"]["rx"]["workspace"]["members"] = toml_edit::Item::Value(members_array.into());
        doc["tool"]["rx"]["workspace"]["shared-venv"] =
            toml_edit::Item::Value(self.shared_venv.into());

        std::fs::write(&pyproject_path, doc.to_string()).map_err(Error::Io)?;

        Ok(())
    }

    /// Resolve member patterns to actual paths
    fn resolve_members(&mut self) -> Result<()> {
        let mut members = HashSet::new();

        for pattern in &self.member_patterns {
            // Check if it's a glob pattern
            if pattern.contains('*') {
                // Use glob to expand pattern
                let full_pattern = self.root.join(pattern);
                let pattern_str = full_pattern.to_string_lossy();

                if let Ok(paths) = glob::glob(&pattern_str) {
                    for entry in paths.flatten() {
                        // Only include directories with pyproject.toml
                        if entry.is_dir() && entry.join("pyproject.toml").exists() {
                            members.insert(entry);
                        }
                    }
                }
            } else {
                // Direct path
                let member_path = self.root.join(pattern);
                if member_path.is_dir() && member_path.join("pyproject.toml").exists() {
                    members.insert(member_path);
                }
            }
        }

        self.members = members.into_iter().collect();
        self.members.sort();

        Ok(())
    }

    /// Get resolved member paths
    pub fn members(&self) -> &[PathBuf] {
        &self.members
    }

    /// Get lockfile path (at workspace root)
    pub fn lockfile_path(&self) -> PathBuf {
        self.root.join("rx.lock")
    }

    /// Get venv path
    pub fn venv_path(&self) -> PathBuf {
        self.root.join(".venv")
    }

    /// Collect all dependencies from all members
    pub fn all_dependencies(&self) -> Result<Vec<crate::pep::Requirement>> {
        let mut all_reqs = Vec::new();
        let mut seen_names = HashSet::new();

        for member_path in &self.members {
            let pyproject = PyProject::load(member_path)?;

            for dep in pyproject.dependencies() {
                if let Ok(req) = crate::pep::Requirement::parse(dep) {
                    let name_lower = req.name.to_lowercase();
                    if !seen_names.contains(&name_lower) {
                        seen_names.insert(name_lower);
                        all_reqs.push(req);
                    }
                }
            }

            for dep in pyproject.dev_dependencies() {
                if let Ok(req) = crate::pep::Requirement::parse(dep) {
                    let name_lower = req.name.to_lowercase();
                    if !seen_names.contains(&name_lower) {
                        seen_names.insert(name_lower);
                        all_reqs.push(req);
                    }
                }
            }
        }

        Ok(all_reqs)
    }

    /// Get member info for display
    pub fn member_info(&self) -> Result<Vec<MemberInfo>> {
        let mut info = Vec::new();

        for member_path in &self.members {
            let pyproject = PyProject::load(member_path)?;
            let relative_path = member_path
                .strip_prefix(&self.root)
                .unwrap_or(member_path)
                .to_string_lossy()
                .to_string();

            info.push(MemberInfo {
                path: relative_path,
                name: pyproject.name().map(String::from),
                version: pyproject.version().map(String::from),
                dependency_count: pyproject.dependencies().len()
                    + pyproject.dev_dependencies().len(),
            });
        }

        Ok(info)
    }
}

/// Information about a workspace member
#[derive(Debug, Clone)]
pub struct MemberInfo {
    /// Relative path from workspace root
    pub path: String,
    /// Project name
    pub name: Option<String>,
    /// Project version
    pub version: Option<String>,
    /// Total number of dependencies
    pub dependency_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_workspace_create() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        let workspace = Workspace::create(root, false).unwrap();
        assert_eq!(workspace.members().len(), 0);
        assert!(!workspace.shared_venv);

        // Verify pyproject.toml was created
        let content = std::fs::read_to_string(root.join("pyproject.toml")).unwrap();
        assert!(content.contains("[tool.rx.workspace]"));
    }

    #[test]
    fn test_is_workspace_root() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // Not a workspace initially
        assert!(!Workspace::is_workspace_root(root));

        // Create workspace
        Workspace::create(root, false).unwrap();

        // Now it should be detected
        assert!(Workspace::is_workspace_root(root));
    }
}
