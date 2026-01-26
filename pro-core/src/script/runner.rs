//! Script runner for executing Python scripts with inline dependencies
//!
//! Handles PEP 723 scripts by creating ephemeral virtual environments
//! and installing the declared dependencies before execution.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use crate::installer::{default_cache_dir, Installer};
use crate::lockfile::LockedPackage;
use crate::pep::Requirement;
use crate::resolver::Resolver;
use crate::venv::VenvManager;
use crate::{Error, Result};

use super::parser::{parse_script_metadata, ScriptMetadata};

/// Script runner for PEP 723 scripts
pub struct ScriptRunner {
    /// Cache directory for script venvs
    cache_dir: PathBuf,
    /// Optional Python executable to use
    python: Option<PathBuf>,
}

impl ScriptRunner {
    /// Create a new script runner with default cache directory
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| Error::Config("cannot determine cache directory".into()))?;

        Ok(Self {
            cache_dir: cache_dir.join("rx").join("scripts"),
            python: None,
        })
    }

    /// Create a script runner with a specific Python executable
    pub fn with_python(python: PathBuf) -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| Error::Config("cannot determine cache directory".into()))?;

        Ok(Self {
            cache_dir: cache_dir.join("rx").join("scripts"),
            python: Some(python),
        })
    }

    /// Create a script runner with a custom cache directory
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            python: None,
        }
    }

    /// Run a Python script, handling PEP 723 dependencies if present
    pub async fn run(&self, script_path: &Path, args: &[String]) -> Result<ExitStatus> {
        // Read the script
        let content = fs::read_to_string(script_path).map_err(|e| {
            Error::ScriptExecutionFailed(format!(
                "failed to read script {}: {}",
                script_path.display(),
                e
            ))
        })?;

        // Parse metadata
        let metadata = parse_script_metadata(&content)?;

        if metadata.is_empty() {
            // No dependencies, run with system/project Python
            return self.run_simple(script_path, args);
        }

        // Has dependencies - set up environment
        self.run_with_deps(script_path, args, &metadata).await
    }

    /// Run a script without dependencies (simple execution)
    fn run_simple(&self, script_path: &Path, args: &[String]) -> Result<ExitStatus> {
        let python = self
            .python
            .clone()
            .unwrap_or_else(|| PathBuf::from("python3"));

        let status = Command::new(&python)
            .arg(script_path)
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| {
                Error::ScriptExecutionFailed(format!(
                    "failed to execute script: {}",
                    e
                ))
            })?;

        Ok(status)
    }

    /// Run a script with PEP 723 dependencies
    async fn run_with_deps(
        &self,
        script_path: &Path,
        args: &[String],
        metadata: &ScriptMetadata,
    ) -> Result<ExitStatus> {
        // Get or create the venv for this script's dependencies
        let venv_path = self.get_or_create_venv(metadata).await?;

        // Get the Python executable from the venv
        let python = {
            #[cfg(unix)]
            {
                venv_path.join("bin").join("python")
            }
            #[cfg(windows)]
            {
                venv_path.join("Scripts").join("python.exe")
            }
        };

        // Set up environment
        let bin_dir = {
            #[cfg(unix)]
            {
                venv_path.join("bin")
            }
            #[cfg(windows)]
            {
                venv_path.join("Scripts")
            }
        };

        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!(
            "{}{}{}",
            bin_dir.display(),
            if cfg!(windows) { ";" } else { ":" },
            current_path
        );

        // Execute the script
        let status = Command::new(&python)
            .arg(script_path)
            .args(args)
            .env("PATH", &new_path)
            .env("VIRTUAL_ENV", &venv_path)
            .env_remove("PYTHONHOME")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| {
                Error::ScriptExecutionFailed(format!(
                    "failed to execute script: {}",
                    e
                ))
            })?;

        Ok(status)
    }

    /// Get or create a virtual environment for the given dependencies
    async fn get_or_create_venv(&self, metadata: &ScriptMetadata) -> Result<PathBuf> {
        let hash = metadata.dependency_hash();
        let venv_path = self.cache_dir.join(&hash);

        // Check if venv already exists and is valid
        if self.is_venv_valid(&venv_path) {
            tracing::debug!("Using cached script environment: {}", hash);
            return Ok(venv_path);
        }

        tracing::info!(
            "Creating script environment for {} dependencies...",
            metadata.dependencies.len()
        );

        // Create the venv
        fs::create_dir_all(&self.cache_dir).map_err(Error::Io)?;

        // Remove any incomplete venv
        if venv_path.exists() {
            fs::remove_dir_all(&venv_path).map_err(Error::Io)?;
        }

        // Create virtual environment
        let venv = VenvManager::new(&venv_path);
        venv.create(self.python.as_deref())?;

        // Parse dependencies into Requirements
        let requirements: Vec<Requirement> = metadata
            .dependencies
            .iter()
            .filter_map(|dep| Requirement::parse(dep).ok())
            .collect();

        if requirements.is_empty() && !metadata.dependencies.is_empty() {
            return Err(Error::ScriptMetadataError(
                "failed to parse script dependencies".into(),
            ));
        }

        // Resolve dependencies
        let resolver = Resolver::new();
        let resolution = resolver.resolve(&requirements).await?;

        // Convert to LockedPackage format for installer
        let mut packages = std::collections::HashMap::new();
        for pkg in resolution.packages {
            packages.insert(
                pkg.name.clone(),
                LockedPackage {
                    version: pkg.version.clone(),
                    url: if pkg.url.is_empty() {
                        None
                    } else {
                        Some(pkg.url.clone())
                    },
                    hash: if pkg.hash.is_empty() {
                        None
                    } else {
                        Some(pkg.hash.clone())
                    },
                    dependencies: pkg.dependencies.clone(),
                    markers: pkg.markers.clone(),
                    files: vec![],
                },
            );
        }

        // Install packages
        let site_packages = venv.site_packages()?;
        let installer = Installer::new(default_cache_dir());
        let results = installer.install(&packages, &site_packages).await?;
        tracing::debug!("Installed {} packages", results.len());

        Ok(venv_path)
    }

    /// Check if a cached venv is valid
    fn is_venv_valid(&self, venv_path: &Path) -> bool {
        if !venv_path.exists() {
            return false;
        }

        // Check for pyvenv.cfg
        if !venv_path.join("pyvenv.cfg").exists() {
            return false;
        }

        // Check for Python executable
        #[cfg(unix)]
        let python = venv_path.join("bin").join("python");
        #[cfg(windows)]
        let python = venv_path.join("Scripts").join("python.exe");

        python.exists()
    }

    /// Clear the script cache
    pub fn clear_cache(&self) -> Result<usize> {
        if !self.cache_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        for entry in fs::read_dir(&self.cache_dir).map_err(Error::Io)? {
            let entry = entry.map_err(Error::Io)?;
            if entry.path().is_dir() {
                fs::remove_dir_all(entry.path()).map_err(Error::Io)?;
                count += 1;
            }
        }

        Ok(count)
    }

    /// List cached script environments
    pub fn list_cached(&self) -> Result<Vec<PathBuf>> {
        if !self.cache_dir.exists() {
            return Ok(Vec::new());
        }

        let mut cached = Vec::new();
        for entry in fs::read_dir(&self.cache_dir).map_err(Error::Io)? {
            let entry = entry.map_err(Error::Io)?;
            if entry.path().is_dir() {
                cached.push(entry.path());
            }
        }

        Ok(cached)
    }
}

/// Check if a file is a Python script that might have PEP 723 metadata
pub fn is_pep723_script(path: &Path) -> Result<bool> {
    // Must have .py extension
    if path.extension().map_or(true, |e| e != "py") {
        return Ok(false);
    }

    // Read first few KB to check for metadata block
    let content = fs::read_to_string(path).map_err(Error::Io)?;

    Ok(super::parser::might_have_metadata(&content))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_script_runner_new() {
        let runner = ScriptRunner::new();
        assert!(runner.is_ok());
    }

    #[test]
    fn test_is_venv_valid_nonexistent() {
        let temp_dir = tempdir().unwrap();
        let runner = ScriptRunner::with_cache_dir(temp_dir.path().to_path_buf());

        assert!(!runner.is_venv_valid(&temp_dir.path().join("nonexistent")));
    }

    #[test]
    fn test_list_cached_empty() {
        let temp_dir = tempdir().unwrap();
        let runner = ScriptRunner::with_cache_dir(temp_dir.path().to_path_buf());

        let cached = runner.list_cached().unwrap();
        assert!(cached.is_empty());
    }
}
