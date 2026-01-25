//! Polylith architecture support
//!
//! Polylith is a software architecture that applies functional thinking
//! to the system level, organizing code into:
//!
//! - **bases/**: Entry points (CLI, web server, Lambda handler, etc.)
//! - **components/**: Reusable building blocks with well-defined interfaces
//! - **projects/**: Deployable artifacts that combine bases and components
//!
//! ```
//! workspace/
//! ├── bases/
//! │   └── myapp-cli/           # CLI entry point
//! │       ├── pyproject.toml
//! │       └── src/myapp_cli/
//! ├── components/
//! │   ├── user/                # User component
//! │   │   ├── pyproject.toml
//! │   │   └── src/user/
//! │   └── database/            # Database component
//! │       ├── pyproject.toml
//! │       └── src/database/
//! ├── projects/
//! │   └── myapp/               # Deployable project
//! │       └── pyproject.toml   # Combines base + components
//! └── pyproject.toml           # Workspace root
//! ```
//!
//! Benefits:
//! - Clear separation of concerns
//! - High code reuse across projects
//! - Independent testing of components
//! - Easy to reason about dependencies

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::pep::PyProject;
use crate::workspace::Workspace;
use crate::{Error, Result};

/// Polylith brick types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrickType {
    /// Entry point (CLI, API, etc.)
    Base,
    /// Reusable building block
    Component,
    /// Deployable artifact
    Project,
}

impl BrickType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BrickType::Base => "base",
            BrickType::Component => "component",
            BrickType::Project => "project",
        }
    }

    pub fn dir_name(&self) -> &'static str {
        match self {
            BrickType::Base => "bases",
            BrickType::Component => "components",
            BrickType::Project => "projects",
        }
    }

    pub fn plural(&self) -> &'static str {
        match self {
            BrickType::Base => "bases",
            BrickType::Component => "components",
            BrickType::Project => "projects",
        }
    }
}

/// A Polylith brick (base, component, or project)
#[derive(Debug, Clone)]
pub struct Brick {
    /// Brick type
    pub brick_type: BrickType,
    /// Brick name
    pub name: String,
    /// Path to the brick
    pub path: PathBuf,
    /// Dependencies on other bricks
    pub brick_deps: Vec<String>,
    /// External dependencies
    pub external_deps: Vec<String>,
}

/// Polylith workspace configuration
#[derive(Debug, Clone)]
pub struct Polylith {
    /// Workspace root
    pub root: PathBuf,
    /// Top namespace for all bricks
    pub top_namespace: String,
    /// All bases
    pub bases: Vec<Brick>,
    /// All components
    pub components: Vec<Brick>,
    /// All projects
    pub projects: Vec<Brick>,
}

impl Polylith {
    /// Initialize a new Polylith workspace
    pub fn init(root: &Path, top_namespace: &str) -> Result<Self> {
        // Create directory structure
        let bases_dir = root.join("bases");
        let components_dir = root.join("components");
        let projects_dir = root.join("projects");

        std::fs::create_dir_all(&bases_dir).map_err(Error::Io)?;
        std::fs::create_dir_all(&components_dir).map_err(Error::Io)?;
        std::fs::create_dir_all(&projects_dir).map_err(Error::Io)?;

        // Update or create pyproject.toml with polylith config
        let pyproject_path = root.join("pyproject.toml");
        let content = if pyproject_path.exists() {
            std::fs::read_to_string(&pyproject_path).map_err(Error::Io)?
        } else {
            format!(
                r#"[project]
name = "{}-workspace"
version = "0.0.0"
description = "Polylith workspace"
"#,
                top_namespace
            )
        };

        let mut doc: toml_edit::DocumentMut = content.parse().map_err(|e| {
            Error::Config(format!("Failed to parse pyproject.toml: {}", e))
        })?;

        // Ensure [tool.rx] exists
        if !doc.contains_key("tool") {
            doc["tool"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        if !doc["tool"].as_table().unwrap().contains_key("rx") {
            doc["tool"]["rx"] = toml_edit::Item::Table(toml_edit::Table::new());
        }

        // Add workspace config
        let rx_table = doc["tool"]["rx"].as_table_mut().unwrap();
        if !rx_table.contains_key("workspace") {
            rx_table["workspace"] = toml_edit::Item::Table(toml_edit::Table::new());
        }

        let workspace_table = rx_table["workspace"].as_table_mut().unwrap();

        // Set members to include all polylith directories
        let members = toml_edit::Array::from_iter([
            "bases/*",
            "components/*",
            "projects/*",
        ]);
        workspace_table["members"] = toml_edit::Item::Value(members.into());

        // Add polylith config
        if !rx_table.contains_key("polylith") {
            rx_table["polylith"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        let polylith_table = rx_table["polylith"].as_table_mut().unwrap();
        polylith_table["top-namespace"] = toml_edit::Item::Value(top_namespace.into());

        std::fs::write(&pyproject_path, doc.to_string()).map_err(Error::Io)?;

        Ok(Self {
            root: root.to_path_buf(),
            top_namespace: top_namespace.to_string(),
            bases: Vec::new(),
            components: Vec::new(),
            projects: Vec::new(),
        })
    }

    /// Load an existing Polylith workspace
    pub fn load(root: &Path) -> Result<Self> {
        let pyproject = PyProject::load(root)?;

        let rx_config = pyproject
            .tool
            .get("rx")
            .ok_or_else(|| Error::Config("No [tool.rx] section found".to_string()))?;

        let polylith_config = rx_config
            .get("polylith")
            .ok_or_else(|| Error::Config("No [tool.rx.polylith] section found".to_string()))?;

        let top_namespace = polylith_config
            .get("top-namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("app")
            .to_string();

        let mut polylith = Self {
            root: root.to_path_buf(),
            top_namespace,
            bases: Vec::new(),
            components: Vec::new(),
            projects: Vec::new(),
        };

        polylith.discover_bricks()?;

        Ok(polylith)
    }

    /// Check if a directory is a Polylith workspace
    pub fn is_polylith(root: &Path) -> bool {
        if let Ok(pyproject) = PyProject::load(root) {
            if let Some(rx_config) = pyproject.tool.get("rx") {
                return rx_config.get("polylith").is_some();
            }
        }
        false
    }

    /// Discover all bricks in the workspace
    fn discover_bricks(&mut self) -> Result<()> {
        self.bases = self.discover_brick_type(BrickType::Base)?;
        self.components = self.discover_brick_type(BrickType::Component)?;
        self.projects = self.discover_brick_type(BrickType::Project)?;
        Ok(())
    }

    /// Discover bricks of a specific type
    fn discover_brick_type(&self, brick_type: BrickType) -> Result<Vec<Brick>> {
        let dir = self.root.join(brick_type.dir_name());
        let mut bricks = Vec::new();

        if !dir.exists() {
            return Ok(bricks);
        }

        for entry in std::fs::read_dir(&dir).map_err(Error::Io)? {
            let entry = entry.map_err(Error::Io)?;
            let path = entry.path();

            if path.is_dir() && path.join("pyproject.toml").exists() {
                if let Ok(brick) = self.load_brick(&path, brick_type) {
                    bricks.push(brick);
                }
            }
        }

        bricks.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(bricks)
    }

    /// Load a brick from its directory
    fn load_brick(&self, path: &Path, brick_type: BrickType) -> Result<Brick> {
        let pyproject = PyProject::load(path)?;

        let name = pyproject
            .name()
            .ok_or_else(|| Error::Config("Brick has no name".to_string()))?
            .to_string();

        // Parse dependencies
        let mut brick_deps = Vec::new();
        let mut external_deps = Vec::new();

        for dep in pyproject.dependencies() {
            let dep_name = dep
                .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                .next()
                .unwrap_or("")
                .to_string();

            // Check if it's a brick dependency (starts with top namespace)
            if dep_name.starts_with(&self.top_namespace)
                || self.is_brick(&dep_name)
            {
                brick_deps.push(dep_name);
            } else {
                external_deps.push(dep_name);
            }
        }

        Ok(Brick {
            brick_type,
            name,
            path: path.to_path_buf(),
            brick_deps,
            external_deps,
        })
    }

    /// Check if a package name is a brick
    fn is_brick(&self, name: &str) -> bool {
        let normalized = name.to_lowercase().replace('-', "_");

        for brick in &self.bases {
            if brick.name.to_lowercase().replace('-', "_") == normalized {
                return true;
            }
        }
        for brick in &self.components {
            if brick.name.to_lowercase().replace('-', "_") == normalized {
                return true;
            }
        }
        false
    }

    /// Create a new brick
    pub fn create_brick(&mut self, brick_type: BrickType, name: &str) -> Result<Brick> {
        let dir = self.root.join(brick_type.dir_name()).join(name);

        if dir.exists() {
            return Err(Error::Config(format!(
                "{} '{}' already exists",
                brick_type.as_str(),
                name
            )));
        }

        // Create directory structure
        let src_dir = dir.join("src").join(name.replace('-', "_"));
        std::fs::create_dir_all(&src_dir).map_err(Error::Io)?;

        // Create __init__.py
        let init_content = format!(
            r#"""{}

This {} is part of the {} Polylith workspace.
"""

__version__ = "0.1.0"
"#,
            name.replace('-', "_"),
            brick_type.as_str(),
            self.top_namespace
        );
        std::fs::write(src_dir.join("__init__.py"), init_content).map_err(Error::Io)?;

        // Create interface module for components
        if brick_type == BrickType::Component {
            let interface_content = r#""""Public interface for this component.

Export only what should be used by other bricks.
"""

# Export public API here
# from .core import MyClass, my_function
"#;
            std::fs::write(src_dir.join("interface.py"), interface_content).map_err(Error::Io)?;

            let core_content = r#""""Core implementation.

Internal implementation details. Use interface.py for public API.
"""
"#;
            std::fs::write(src_dir.join("core.py"), core_content).map_err(Error::Io)?;
        }

        // Create pyproject.toml
        let pyproject_content = format!(
            r#"[project]
name = "{name}"
version = "0.1.0"
description = "A {brick_type} in the {namespace} workspace"
requires-python = ">=3.9"
dependencies = []

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/{package_name}"]
"#,
            name = name,
            brick_type = brick_type.as_str(),
            namespace = self.top_namespace,
            package_name = name.replace('-', "_"),
        );
        std::fs::write(dir.join("pyproject.toml"), pyproject_content).map_err(Error::Io)?;

        // Create tests directory
        let tests_dir = dir.join("tests");
        std::fs::create_dir_all(&tests_dir).map_err(Error::Io)?;

        let test_content = format!(
            r#""""Tests for {}."""

def test_placeholder():
    """Placeholder test."""
    assert True
"#,
            name
        );
        std::fs::write(
            tests_dir.join(format!("test_{}.py", name.replace('-', "_"))),
            test_content,
        )
        .map_err(Error::Io)?;

        let brick = Brick {
            brick_type,
            name: name.to_string(),
            path: dir,
            brick_deps: Vec::new(),
            external_deps: Vec::new(),
        };

        // Add to appropriate list
        match brick_type {
            BrickType::Base => self.bases.push(brick.clone()),
            BrickType::Component => self.components.push(brick.clone()),
            BrickType::Project => self.projects.push(brick.clone()),
        }

        Ok(brick)
    }

    /// Create a project that combines bases and components
    pub fn create_project(
        &mut self,
        name: &str,
        bases: &[String],
        components: &[String],
    ) -> Result<Brick> {
        let dir = self.root.join("projects").join(name);

        if dir.exists() {
            return Err(Error::Config(format!("Project '{}' already exists", name)));
        }

        std::fs::create_dir_all(&dir).map_err(Error::Io)?;

        // Build dependencies list
        let mut deps = Vec::new();
        for base in bases {
            deps.push(format!("{} @ {{root:uri}}/../bases/{}", base, base));
        }
        for component in components {
            deps.push(format!(
                "{} @ {{root:uri}}/../components/{}",
                component, component
            ));
        }

        let deps_str = deps
            .iter()
            .map(|d| format!("    \"{}\",", d))
            .collect::<Vec<_>>()
            .join("\n");

        let pyproject_content = format!(
            r#"[project]
name = "{name}"
version = "0.1.0"
description = "Deployable project combining bases and components"
requires-python = ">=3.9"
dependencies = [
{deps}
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.rx.project]
bases = {bases:?}
components = {components:?}
"#,
            name = name,
            deps = deps_str,
            bases = bases,
            components = components,
        );

        std::fs::write(dir.join("pyproject.toml"), pyproject_content).map_err(Error::Io)?;

        let brick = Brick {
            brick_type: BrickType::Project,
            name: name.to_string(),
            path: dir,
            brick_deps: bases
                .iter()
                .chain(components.iter())
                .cloned()
                .collect(),
            external_deps: Vec::new(),
        };

        self.projects.push(brick.clone());
        Ok(brick)
    }

    /// Get all bricks
    pub fn all_bricks(&self) -> Vec<&Brick> {
        let mut all = Vec::new();
        all.extend(self.bases.iter());
        all.extend(self.components.iter());
        all.extend(self.projects.iter());
        all
    }

    /// Check for dependency cycles
    pub fn check_cycles(&self) -> Result<()> {
        // Build dependency graph
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for brick in self.all_bricks() {
            if self.has_cycle(&brick.name, &mut visited, &mut rec_stack)? {
                return Err(Error::Config(format!(
                    "Dependency cycle detected involving '{}'",
                    brick.name
                )));
            }
        }

        Ok(())
    }

    fn has_cycle(
        &self,
        name: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> Result<bool> {
        if rec_stack.contains(name) {
            return Ok(true);
        }
        if visited.contains(name) {
            return Ok(false);
        }

        visited.insert(name.to_string());
        rec_stack.insert(name.to_string());

        // Find the brick
        let brick = self.all_bricks().into_iter().find(|b| b.name == name);

        if let Some(brick) = brick {
            for dep in &brick.brick_deps {
                if self.has_cycle(dep, visited, rec_stack)? {
                    return Ok(true);
                }
            }
        }

        rec_stack.remove(name);
        Ok(false)
    }

    /// Get the workspace for this polylith
    pub fn as_workspace(&self) -> Result<Workspace> {
        Workspace::load_from_root(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_polylith_init() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        let polylith = Polylith::init(root, "myapp").unwrap();

        assert_eq!(polylith.top_namespace, "myapp");
        assert!(root.join("bases").exists());
        assert!(root.join("components").exists());
        assert!(root.join("projects").exists());

        // Check pyproject.toml was created
        let content = std::fs::read_to_string(root.join("pyproject.toml")).unwrap();
        assert!(content.contains("[tool.rx.polylith]"));
        assert!(content.contains("top-namespace"));
    }

    #[test]
    fn test_create_component() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        let mut polylith = Polylith::init(root, "myapp").unwrap();
        let brick = polylith.create_brick(BrickType::Component, "user").unwrap();

        assert_eq!(brick.name, "user");
        assert_eq!(brick.brick_type, BrickType::Component);
        assert!(root.join("components/user/pyproject.toml").exists());
        assert!(root.join("components/user/src/user/__init__.py").exists());
        assert!(root.join("components/user/src/user/interface.py").exists());
    }

    #[test]
    fn test_create_base() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        let mut polylith = Polylith::init(root, "myapp").unwrap();
        let brick = polylith.create_brick(BrickType::Base, "cli").unwrap();

        assert_eq!(brick.name, "cli");
        assert_eq!(brick.brick_type, BrickType::Base);
        assert!(root.join("bases/cli/pyproject.toml").exists());
    }

    #[test]
    fn test_is_polylith() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        assert!(!Polylith::is_polylith(root));

        Polylith::init(root, "myapp").unwrap();

        assert!(Polylith::is_polylith(root));
    }
}
