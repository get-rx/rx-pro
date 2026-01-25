//! PEP 621 - Storing project metadata in pyproject.toml

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Root pyproject.toml structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PyProject {
    /// PEP 621 project metadata
    pub project: Option<ProjectMetadata>,

    /// Build system configuration (PEP 517)
    pub build_system: Option<BuildSystem>,

    /// Tool-specific configuration
    #[serde(default)]
    pub tool: HashMap<String, toml::Value>,
}

/// PEP 621 project metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectMetadata {
    /// Project name
    pub name: String,

    /// Project version
    pub version: Option<String>,

    /// Short description
    pub description: Option<String>,

    /// Project readme file or text
    pub readme: Option<Readme>,

    /// Required Python version
    pub requires_python: Option<String>,

    /// Project license
    pub license: Option<License>,

    /// Project authors
    #[serde(default)]
    pub authors: Vec<Person>,

    /// Project maintainers
    #[serde(default)]
    pub maintainers: Vec<Person>,

    /// Project keywords
    #[serde(default)]
    pub keywords: Vec<String>,

    /// Trove classifiers
    #[serde(default)]
    pub classifiers: Vec<String>,

    /// Project URLs
    #[serde(default)]
    pub urls: HashMap<String, String>,

    /// Project dependencies
    #[serde(default)]
    pub dependencies: Vec<String>,

    /// Optional dependencies
    #[serde(default)]
    pub optional_dependencies: HashMap<String, Vec<String>>,

    /// Entry points
    #[serde(default)]
    pub scripts: HashMap<String, String>,

    /// GUI scripts
    #[serde(default)]
    pub gui_scripts: HashMap<String, String>,

    /// Entry point groups
    #[serde(default)]
    pub entry_points: HashMap<String, HashMap<String, String>>,

    /// Dynamic fields (computed at build time)
    #[serde(default)]
    pub dynamic: Vec<String>,
}

/// Readme specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Readme {
    /// Path to readme file
    Path(String),
    /// Inline readme with content type
    Inline {
        file: Option<String>,
        text: Option<String>,
        content_type: Option<String>,
    },
}

/// License specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum License {
    /// SPDX identifier
    Text { text: String },
    /// Path to license file
    File { file: String },
}

/// Person (author or maintainer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// PEP 517 build system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BuildSystem {
    /// Build dependencies
    pub requires: Vec<String>,

    /// Build backend module
    pub build_backend: Option<String>,

    /// Backend path
    pub backend_path: Option<Vec<String>>,
}

impl PyProject {
    /// Load pyproject.toml from a directory
    pub fn load(project_dir: &Path) -> Result<Self> {
        let path = project_dir.join("pyproject.toml");
        let content = std::fs::read_to_string(&path).map_err(|_| Error::PyProjectNotFound)?;
        Self::parse(&content)
    }

    /// Parse pyproject.toml content
    pub fn parse(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(Error::TomlParse)
    }

    /// Get project name
    pub fn name(&self) -> Option<&str> {
        self.project.as_ref().map(|p| p.name.as_str())
    }

    /// Get project version
    pub fn version(&self) -> Option<&str> {
        self.project
            .as_ref()
            .and_then(|p| p.version.as_deref())
    }

    /// Get dependencies
    pub fn dependencies(&self) -> &[String] {
        self.project
            .as_ref()
            .map(|p| p.dependencies.as_slice())
            .unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal() {
        let content = r#"
[project]
name = "mypackage"
version = "0.1.0"
"#;

        let pyproject = PyProject::parse(content).unwrap();
        assert_eq!(pyproject.name(), Some("mypackage"));
        assert_eq!(pyproject.version(), Some("0.1.0"));
    }

    #[test]
    fn test_parse_with_dependencies() {
        let content = r#"
[project]
name = "mypackage"
version = "0.1.0"
dependencies = [
    "requests>=2.0",
    "click",
]
"#;

        let pyproject = PyProject::parse(content).unwrap();
        assert_eq!(pyproject.dependencies().len(), 2);
    }
}
