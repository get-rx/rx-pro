//! Tool cache management
//!
//! Caches installed tools in ephemeral virtual environments for fast re-execution.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::{Error, Result};

/// Metadata about a cached tool
#[derive(Debug, Clone)]
pub struct CachedTool {
    /// Package name
    pub package: String,
    /// Version installed
    pub version: Option<String>,
    /// Path to the virtual environment
    pub venv_path: PathBuf,
    /// When the tool was cached
    pub cached_at: SystemTime,
}

impl CachedTool {
    /// Get the bin directory for this tool
    pub fn bin_dir(&self) -> PathBuf {
        #[cfg(unix)]
        {
            self.venv_path.join("bin")
        }
        #[cfg(windows)]
        {
            self.venv_path.join("Scripts")
        }
    }

    /// Get the path to the tool's executable
    pub fn executable(&self, name: &str) -> PathBuf {
        #[cfg(unix)]
        {
            self.bin_dir().join(name)
        }
        #[cfg(windows)]
        {
            self.bin_dir().join(format!("{}.exe", name))
        }
    }

    /// Check if the tool executable exists
    pub fn has_executable(&self, name: &str) -> bool {
        self.executable(name).exists()
    }
}

/// Tool cache manager
///
/// Manages cached tool installations in ~/.local/share/rx/tools/
pub struct ToolCache {
    /// Base directory for tool caches
    cache_dir: PathBuf,
}

impl ToolCache {
    /// Create a new tool cache with default directory
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| Error::Config("cannot determine data directory".into()))?;

        Ok(Self {
            cache_dir: data_dir.join("rx").join("tools"),
        })
    }

    /// Create a tool cache with a custom directory
    pub fn with_dir(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get a cached tool by package name
    pub fn get(&self, package: &str) -> Option<CachedTool> {
        let venv_path = self.tool_dir(package);

        if !venv_path.exists() {
            return None;
        }

        // Check if the venv is valid
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

        if !bin_dir.exists() {
            return None;
        }

        // Try to get metadata
        let cached_at = fs::metadata(&venv_path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        // Try to read version from marker file
        let version = self.read_version(package);

        Some(CachedTool {
            package: package.to_string(),
            version,
            venv_path,
            cached_at,
        })
    }

    /// Store a tool in the cache
    ///
    /// Returns the path where the venv should be created
    pub fn prepare(&self, package: &str) -> Result<PathBuf> {
        let venv_path = self.tool_dir(package);

        // Create parent directories
        fs::create_dir_all(&self.cache_dir).map_err(Error::Io)?;

        // Remove existing if present
        if venv_path.exists() {
            fs::remove_dir_all(&venv_path).map_err(Error::Io)?;
        }

        Ok(venv_path)
    }

    /// Record the installed version for a tool
    pub fn record_version(&self, package: &str, version: &str) -> Result<()> {
        let marker = self.version_marker(package);
        fs::write(&marker, version).map_err(Error::Io)?;
        Ok(())
    }

    /// Read the recorded version for a tool
    fn read_version(&self, package: &str) -> Option<String> {
        let marker = self.version_marker(package);
        fs::read_to_string(&marker)
            .ok()
            .map(|s| s.trim().to_string())
    }

    /// List all cached tools
    pub fn list(&self) -> Result<Vec<CachedTool>> {
        let mut tools = Vec::new();

        if !self.cache_dir.exists() {
            return Ok(tools);
        }

        for entry in fs::read_dir(&self.cache_dir).map_err(Error::Io)? {
            let entry = entry.map_err(Error::Io)?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(tool) = self.get(name) {
                        tools.push(tool);
                    }
                }
            }
        }

        // Sort by name
        tools.sort_by(|a, b| a.package.cmp(&b.package));

        Ok(tools)
    }

    /// Clear a specific tool from the cache
    pub fn clear(&self, package: &str) -> Result<bool> {
        let venv_path = self.tool_dir(package);

        if venv_path.exists() {
            fs::remove_dir_all(&venv_path).map_err(Error::Io)?;

            // Also remove version marker
            let marker = self.version_marker(package);
            let _ = fs::remove_file(&marker);

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Clear all cached tools
    pub fn clear_all(&self) -> Result<usize> {
        let tools = self.list()?;
        let count = tools.len();

        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).map_err(Error::Io)?;
        }

        Ok(count)
    }

    /// Get the directory for a specific tool
    fn tool_dir(&self, package: &str) -> PathBuf {
        // Normalize package name (lowercase, replace - with _)
        let normalized = package.to_lowercase().replace('-', "_");
        self.cache_dir.join(&normalized)
    }

    /// Get the version marker file path
    fn version_marker(&self, package: &str) -> PathBuf {
        let normalized = package.to_lowercase().replace('-', "_");
        self.cache_dir.join(format!(".{}.version", normalized))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_tool_dir_normalization() {
        let temp_dir = tempdir().unwrap();
        let cache = ToolCache::with_dir(temp_dir.path().to_path_buf());

        let path1 = cache.tool_dir("black");
        let path2 = cache.tool_dir("Black");
        let path3 = cache.tool_dir("my-tool");
        let path4 = cache.tool_dir("my_tool");

        assert_eq!(path1.file_name().unwrap(), "black");
        assert_eq!(path2.file_name().unwrap(), "black");
        assert_eq!(path3.file_name().unwrap(), "my_tool");
        assert_eq!(path4.file_name().unwrap(), "my_tool");
    }

    #[test]
    fn test_list_empty() {
        let temp_dir = tempdir().unwrap();
        let cache = ToolCache::with_dir(temp_dir.path().to_path_buf());

        let tools = cache.list().unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_get_nonexistent() {
        let temp_dir = tempdir().unwrap();
        let cache = ToolCache::with_dir(temp_dir.path().to_path_buf());

        assert!(cache.get("nonexistent").is_none());
    }
}
