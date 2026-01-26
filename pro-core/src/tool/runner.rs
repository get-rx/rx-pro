//! Tool runner for executing tools in ephemeral virtual environments
//!
//! Provides functionality similar to `uvx` or `pipx run` - running Python tools
//! without permanently installing them.

use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

use crate::installer::{default_cache_dir, Installer};
use crate::lockfile::LockedPackage;
use crate::pep::Requirement;
use crate::resolver::Resolver;
use crate::venv::VenvManager;
use crate::{Error, Result};

use super::cache::{CachedTool, ToolCache};

/// Tool runner for executing Python tools
pub struct ToolRunner {
    cache: ToolCache,
    /// Optional Python executable to use
    python: Option<PathBuf>,
}

impl ToolRunner {
    /// Create a new tool runner
    pub fn new() -> Result<Self> {
        Ok(Self {
            cache: ToolCache::new()?,
            python: None,
        })
    }

    /// Create a tool runner with a specific Python executable
    pub fn with_python(python: PathBuf) -> Result<Self> {
        Ok(Self {
            cache: ToolCache::new()?,
            python: Some(python),
        })
    }

    /// Create a tool runner with a custom cache
    pub fn with_cache(cache: ToolCache) -> Self {
        Self {
            cache,
            python: None,
        }
    }

    /// Get the tool cache
    pub fn cache(&self) -> &ToolCache {
        &self.cache
    }

    /// Run a tool with the given arguments
    ///
    /// The package will be installed if not already cached.
    /// The tool executable is determined by the package name (e.g., "black" -> "black").
    pub async fn run(&self, package: &str, args: &[String]) -> Result<ExitStatus> {
        self.run_with_command(package, package, args).await
    }

    /// Run a specific command from a package
    ///
    /// Useful when the command name differs from the package name
    /// (e.g., package "Pillow" provides command "PIL").
    pub async fn run_with_command(
        &self,
        package: &str,
        command: &str,
        args: &[String],
    ) -> Result<ExitStatus> {
        // Check cache first
        let tool = if let Some(cached) = self.cache.get(package) {
            if cached.has_executable(command) {
                tracing::debug!("Using cached tool: {}", package);
                cached
            } else {
                // Cache exists but doesn't have the command - reinstall
                self.install_tool(package).await?
            }
        } else {
            // Not cached - install
            self.install_tool(package).await?
        };

        // Execute the tool
        self.execute(&tool, command, args)
    }

    /// Install a tool into the cache
    async fn install_tool(&self, package: &str) -> Result<CachedTool> {
        tracing::info!("Installing {}...", package);

        // Prepare cache directory
        let venv_path = self.cache.prepare(package)?;

        // Create virtual environment
        let venv = VenvManager::new(&venv_path);
        venv.create(self.python.as_deref())?;

        // Parse package as requirement
        let requirement = Requirement::parse(package).map_err(|e| {
            Error::ToolExecutionFailed(format!("invalid package name {}: {}", package, e))
        })?;

        // Resolve the package
        let resolver = Resolver::new();
        let resolution = resolver.resolve(&[requirement]).await?;

        // Convert to LockedPackage format for installer
        let mut packages = std::collections::HashMap::new();
        for pkg in &resolution.packages {
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

        // Install into the venv
        let site_packages = venv.site_packages()?;
        let installer = Installer::new(default_cache_dir());
        let results = installer.install(&packages, &site_packages).await?;
        tracing::debug!("Installed {} packages for {}", results.len(), package);

        // Record the installed version
        if let Some(pkg) = resolution.packages.iter().find(|p| {
            p.name.to_lowercase() == package.to_lowercase()
                || p.name.to_lowercase().replace('-', "_")
                    == package.to_lowercase().replace('-', "_")
        }) {
            self.cache.record_version(package, &pkg.version)?;
        }

        // Return the cached tool info
        self.cache.get(package).ok_or_else(|| {
            Error::ToolExecutionFailed(format!("failed to install tool: {}", package))
        })
    }

    /// Execute a tool
    fn execute(&self, tool: &CachedTool, command: &str, args: &[String]) -> Result<ExitStatus> {
        let executable = tool.executable(command);

        if !executable.exists() {
            // Try to find similar executables
            let bin_dir = tool.bin_dir();
            let available: Vec<_> = std::fs::read_dir(&bin_dir)
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter_map(|e| e.file_name().into_string().ok())
                        .filter(|n| !n.starts_with("python") && !n.starts_with("pip") && !n.starts_with("activate"))
                        .collect()
                })
                .unwrap_or_default();

            return Err(Error::ToolNotFound {
                tool: format!(
                    "{} (available: {})",
                    command,
                    if available.is_empty() {
                        "none".to_string()
                    } else {
                        available.join(", ")
                    }
                ),
            });
        }

        // Set up environment
        let bin_dir = tool.bin_dir();
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!(
            "{}{}{}",
            bin_dir.display(),
            if cfg!(windows) { ";" } else { ":" },
            current_path
        );

        let status = Command::new(&executable)
            .args(args)
            .env("PATH", &new_path)
            .env("VIRTUAL_ENV", &tool.venv_path)
            .env_remove("PYTHONHOME")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| Error::ToolExecutionFailed(format!("failed to run {}: {}", command, e)))?;

        Ok(status)
    }

    /// Check if a tool is cached
    pub fn is_cached(&self, package: &str) -> bool {
        self.cache.get(package).is_some()
    }

    /// List cached tools
    pub fn list_cached(&self) -> Result<Vec<CachedTool>> {
        self.cache.list()
    }

    /// Clear a specific tool from the cache
    pub fn clear(&self, package: &str) -> Result<bool> {
        self.cache.clear(package)
    }

    /// Clear all cached tools
    pub fn clear_all(&self) -> Result<usize> {
        self.cache.clear_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_tool_runner_new() {
        // Should not fail on most systems
        let runner = ToolRunner::new();
        assert!(runner.is_ok());
    }

    #[test]
    fn test_is_cached_false() {
        let temp_dir = tempdir().unwrap();
        let cache = ToolCache::with_dir(temp_dir.path().to_path_buf());
        let runner = ToolRunner::with_cache(cache);

        assert!(!runner.is_cached("nonexistent"));
    }
}
