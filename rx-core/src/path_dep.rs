//! Local path dependencies support
//!
//! Path dependencies allow referencing local Python packages:
//!
//! ```toml
//! [tool.rx.dependencies]
//! my-lib = { path = "../my-lib" }
//! my-utils = { path = "./packages/utils", editable = true }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::pep::PyProject;
use crate::{Error, Result};

/// A local path dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathDependency {
    /// Package name
    pub name: String,
    /// Path to the package (relative to project root or absolute)
    pub path: PathBuf,
    /// Whether to install as editable (default: true)
    #[serde(default = "default_editable")]
    pub editable: bool,
    /// Optional extras to install
    #[serde(default)]
    pub extras: Vec<String>,
}

fn default_editable() -> bool {
    true
}

impl PathDependency {
    /// Create a new path dependency
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            editable: true,
            extras: Vec::new(),
        }
    }

    /// Set editable mode
    pub fn with_editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    /// Add extras
    pub fn with_extras(mut self, extras: Vec<String>) -> Self {
        self.extras = extras;
        self
    }

    /// Resolve the path relative to a base directory
    pub fn resolve_path(&self, base_dir: &Path) -> PathBuf {
        if self.path.is_absolute() {
            self.path.clone()
        } else {
            base_dir.join(&self.path)
        }
    }

    /// Check if the path dependency is valid (path exists and has pyproject.toml)
    pub fn validate(&self, base_dir: &Path) -> Result<()> {
        let resolved = self.resolve_path(base_dir);

        if !resolved.exists() {
            return Err(Error::Config(format!(
                "Path dependency '{}' not found: {}",
                self.name,
                resolved.display()
            )));
        }

        let pyproject_path = resolved.join("pyproject.toml");
        if !pyproject_path.exists() {
            return Err(Error::Config(format!(
                "Path dependency '{}' has no pyproject.toml: {}",
                self.name,
                resolved.display()
            )));
        }

        Ok(())
    }

    /// Get the package version from pyproject.toml
    pub fn get_version(&self, base_dir: &Path) -> Result<Option<String>> {
        let resolved = self.resolve_path(base_dir);
        let pyproject = PyProject::load(&resolved)?;
        Ok(pyproject.version().map(String::from))
    }

    /// Get transitive dependencies from the path dependency
    pub fn get_dependencies(&self, base_dir: &Path) -> Result<Vec<String>> {
        let resolved = self.resolve_path(base_dir);
        let pyproject = PyProject::load(&resolved)?;
        Ok(pyproject.dependencies().to_vec())
    }
}

/// Load path dependencies from pyproject.toml [tool.rx.dependencies]
pub fn load_path_dependencies(project_dir: &Path) -> Result<HashMap<String, PathDependency>> {
    let pyproject = PyProject::load(project_dir)?;

    let mut path_deps = HashMap::new();

    let rx_config = match pyproject.tool.get("rx") {
        Some(c) => c,
        None => return Ok(path_deps),
    };

    let deps_config = match rx_config.get("dependencies") {
        Some(c) => c,
        None => return Ok(path_deps),
    };

    let deps_table = match deps_config.as_table() {
        Some(t) => t,
        None => return Ok(path_deps),
    };

    for (name, value) in deps_table {
        // Only process table entries with a "path" key
        if let Some(table) = value.as_table() {
            if let Some(path_value) = table.get("path") {
                if let Some(path_str) = path_value.as_str() {
                    let editable = table
                        .get("editable")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);

                    let extras: Vec<String> = table
                        .get("extras")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();

                    let dep = PathDependency {
                        name: name.clone(),
                        path: PathBuf::from(path_str),
                        editable,
                        extras,
                    };

                    path_deps.insert(name.clone(), dep);
                }
            }
        }
    }

    Ok(path_deps)
}

/// Install a path dependency
pub async fn install_path_dependency(
    dep: &PathDependency,
    base_dir: &Path,
    site_packages: &Path,
) -> Result<()> {
    let resolved_path = dep.resolve_path(base_dir);

    // Validate the dependency
    dep.validate(base_dir)?;

    if dep.editable {
        // Editable install: create .pth file pointing to the source
        install_editable(&dep.name, &resolved_path, site_packages)?;
    } else {
        // Regular install: copy the package to site-packages
        install_copy(&dep.name, &resolved_path, site_packages)?;
    }

    Ok(())
}

/// Install as editable (create .pth file)
fn install_editable(name: &str, source_path: &Path, site_packages: &Path) -> Result<()> {
    // Find the package directory (either src/<name> or <name>)
    let package_dir = find_package_dir(name, source_path)?;

    // Create .pth file
    let pth_filename = format!("{}.pth", name.replace('-', "_"));
    let pth_path = site_packages.join(pth_filename);

    // The .pth file should contain the parent of the package directory
    let pth_content = package_dir
        .parent()
        .unwrap_or(&package_dir)
        .to_string_lossy()
        .to_string();

    std::fs::write(&pth_path, pth_content).map_err(Error::Io)?;

    // Also create egg-link for compatibility
    let egg_link_path = site_packages.join(format!("{}.egg-link", name.replace('-', "_")));
    let egg_link_content = format!(
        "{}\n.",
        package_dir.parent().unwrap_or(&package_dir).display()
    );
    std::fs::write(&egg_link_path, egg_link_content).map_err(Error::Io)?;

    tracing::info!("Installed {} (editable) from {}", name, source_path.display());

    Ok(())
}

/// Install by copying the package
fn install_copy(name: &str, source_path: &Path, site_packages: &Path) -> Result<()> {
    let package_dir = find_package_dir(name, source_path)?;
    let package_name = package_dir
        .file_name()
        .ok_or_else(|| Error::Config("Invalid package directory".to_string()))?;

    let dest_dir = site_packages.join(package_name);

    // Remove existing if present
    if dest_dir.exists() {
        std::fs::remove_dir_all(&dest_dir).map_err(Error::Io)?;
    }

    // Copy the package
    copy_dir_recursive(&package_dir, &dest_dir)?;

    tracing::info!("Installed {} (copied) from {}", name, source_path.display());

    Ok(())
}

/// Find the Python package directory within a project
fn find_package_dir(name: &str, project_path: &Path) -> Result<PathBuf> {
    let normalized_name = name.replace('-', "_");

    // Try common layouts:
    // 1. src/<name>/
    let src_layout = project_path.join("src").join(&normalized_name);
    if src_layout.exists() && src_layout.join("__init__.py").exists() {
        return Ok(src_layout);
    }

    // 2. <name>/
    let flat_layout = project_path.join(&normalized_name);
    if flat_layout.exists() && flat_layout.join("__init__.py").exists() {
        return Ok(flat_layout);
    }

    // 3. Check if project root itself is a package (single-file module)
    let root_init = project_path.join("__init__.py");
    if root_init.exists() {
        return Ok(project_path.to_path_buf());
    }

    // 4. Look for any directory with __init__.py
    if let Ok(entries) = std::fs::read_dir(project_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("__init__.py").exists() {
                return Ok(path);
            }
        }
    }

    // 5. Also check src/ directory
    let src_dir = project_path.join("src");
    if src_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&src_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("__init__.py").exists() {
                    return Ok(path);
                }
            }
        }
    }

    Err(Error::Config(format!(
        "Could not find Python package in {}. Expected src/{}/ or {}/",
        project_path.display(),
        normalized_name,
        normalized_name
    )))
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).map_err(Error::Io)?;

    for entry in std::fs::read_dir(src).map_err(Error::Io)? {
        let entry = entry.map_err(Error::Io)?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            // Skip common non-package directories
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == "__pycache__"
                || name_str == ".git"
                || name_str == ".venv"
                || name_str == "venv"
                || name_str.ends_with(".egg-info")
            {
                continue;
            }
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            // Skip .pyc files
            if src_path.extension().map_or(false, |e| e == "pyc") {
                continue;
            }
            std::fs::copy(&src_path, &dst_path).map_err(Error::Io)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_path_dependency_resolve() {
        let dep = PathDependency::new("my-lib", "../my-lib");
        let base = PathBuf::from("/workspace/app");
        let resolved = dep.resolve_path(&base);
        assert_eq!(resolved, PathBuf::from("/workspace/app/../my-lib"));
    }

    #[test]
    fn test_path_dependency_absolute() {
        let dep = PathDependency::new("my-lib", "/absolute/path/my-lib");
        let base = PathBuf::from("/workspace/app");
        let resolved = dep.resolve_path(&base);
        assert_eq!(resolved, PathBuf::from("/absolute/path/my-lib"));
    }

    #[test]
    fn test_find_package_dir_src_layout() {
        let temp = TempDir::new().unwrap();
        let project = temp.path();

        // Create src/my_lib/__init__.py
        let pkg_dir = project.join("src").join("my_lib");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("__init__.py"), "").unwrap();

        let found = find_package_dir("my-lib", project).unwrap();
        assert_eq!(found, pkg_dir);
    }

    #[test]
    fn test_find_package_dir_flat_layout() {
        let temp = TempDir::new().unwrap();
        let project = temp.path();

        // Create my_lib/__init__.py
        let pkg_dir = project.join("my_lib");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("__init__.py"), "").unwrap();

        let found = find_package_dir("my-lib", project).unwrap();
        assert_eq!(found, pkg_dir);
    }
}
