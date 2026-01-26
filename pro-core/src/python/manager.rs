//! Python version manager
//!
//! Handles downloading, installing, and managing Python versions from
//! python-build-standalone releases.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Error, Result};

use super::platform::Platform;
use super::versions::{available_versions, find_matching_version, AvailableVersion, PythonVersion};

/// Base URL for python-build-standalone releases
const PBS_RELEASE_URL: &str =
    "https://github.com/astral-sh/python-build-standalone/releases/download";

/// Information about an installed Python version
#[derive(Debug, Clone)]
pub struct InstalledPython {
    pub version: PythonVersion,
    pub path: PathBuf,
}

impl InstalledPython {
    /// Get the path to the Python executable
    pub fn executable(&self) -> PathBuf {
        #[cfg(unix)]
        {
            self.path.join("bin").join("python3")
        }
        #[cfg(windows)]
        {
            self.path.join("python.exe")
        }
    }

    /// Check if this installation is valid (executable exists)
    pub fn is_valid(&self) -> bool {
        self.executable().exists()
    }
}

/// Python version manager
///
/// Manages Python installations in ~/.local/share/rx/python/
pub struct PythonManager {
    /// Base directory for Python installations
    install_dir: PathBuf,
    /// Config directory for global settings
    config_dir: PathBuf,
    /// Detected platform
    platform: Platform,
}

impl PythonManager {
    /// Create a new Python manager with default directories
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| Error::Config("cannot determine data directory".into()))?;
        let config_dir = dirs::config_dir()
            .ok_or_else(|| Error::Config("cannot determine config directory".into()))?;

        Ok(Self {
            install_dir: data_dir.join("rx").join("python"),
            config_dir: config_dir.join("rx"),
            platform: Platform::current()?,
        })
    }

    /// Create a manager with custom directories (for testing)
    pub fn with_dirs(install_dir: PathBuf, config_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            install_dir,
            config_dir,
            platform: Platform::current()?,
        })
    }

    /// Get the installation directory
    pub fn install_dir(&self) -> &Path {
        &self.install_dir
    }

    /// Get the config directory
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Install a Python version
    ///
    /// If a short version like "3.12" is provided, installs the latest patch version.
    pub async fn install(&self, version_spec: &str) -> Result<InstalledPython> {
        let spec = PythonVersion::parse(version_spec)?;

        // Find the matching available version
        let available = find_matching_version(&spec).ok_or_else(|| Error::PythonVersionNotFound {
            version: version_spec.to_string(),
        })?;

        let install_path = self.install_dir.join(available.version.to_string_full());

        // Check if already installed
        if install_path.exists() {
            return Err(Error::PythonAlreadyInstalled {
                version: available.version.to_string_full(),
            });
        }

        tracing::info!("Installing Python {}...", available.version);

        // Download the archive
        let archive_data = self.download(&available).await?;

        // Extract to installation directory
        self.extract(&archive_data, &install_path)?;

        tracing::info!(
            "Python {} installed to {}",
            available.version,
            install_path.display()
        );

        Ok(InstalledPython {
            version: available.version.clone(),
            path: install_path,
        })
    }

    /// Uninstall a Python version
    pub fn uninstall(&self, version_spec: &str) -> Result<()> {
        let spec = PythonVersion::parse(version_spec)?;

        // Find the installed version matching the spec
        let installed = self
            .list_installed()?
            .into_iter()
            .find(|i| i.version.matches(&spec))
            .ok_or_else(|| Error::PythonVersionNotFound {
                version: version_spec.to_string(),
            })?;

        tracing::info!("Uninstalling Python {}...", installed.version);

        // Remove the directory
        fs::remove_dir_all(&installed.path).map_err(Error::Io)?;

        tracing::info!("Python {} uninstalled", installed.version);

        Ok(())
    }

    /// List installed Python versions
    pub fn list_installed(&self) -> Result<Vec<InstalledPython>> {
        let mut installed = Vec::new();

        if !self.install_dir.exists() {
            return Ok(installed);
        }

        for entry in fs::read_dir(&self.install_dir).map_err(Error::Io)? {
            let entry = entry.map_err(Error::Io)?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(version) = PythonVersion::parse(name) {
                        let python = InstalledPython {
                            version,
                            path: path.clone(),
                        };
                        if python.is_valid() {
                            installed.push(python);
                        }
                    }
                }
            }
        }

        // Sort by version descending
        installed.sort_by(|a, b| b.version.cmp(&a.version));

        Ok(installed)
    }

    /// Get a specific installed version
    pub fn get(&self, version_spec: &str) -> Result<Option<InstalledPython>> {
        let spec = PythonVersion::parse(version_spec)?;

        Ok(self
            .list_installed()?
            .into_iter()
            .find(|i| i.version.matches(&spec)))
    }

    /// Find an installed version matching a specification
    ///
    /// Returns the highest patch version that matches.
    pub fn find_matching(&self, version_spec: &str) -> Result<Option<InstalledPython>> {
        let spec = PythonVersion::parse(version_spec)?;

        let mut matches: Vec<_> = self
            .list_installed()?
            .into_iter()
            .filter(|i| i.version.matches(&spec))
            .collect();

        matches.sort_by(|a, b| b.version.cmp(&a.version));

        Ok(matches.into_iter().next())
    }

    /// Pin a Python version for a project by creating .python-version file
    pub fn pin(&self, version_spec: &str, project_dir: &Path) -> Result<()> {
        let spec = PythonVersion::parse(version_spec)?;
        let version_str = spec.to_string_short();

        let version_file = project_dir.join(".python-version");
        let mut file = fs::File::create(&version_file).map_err(Error::Io)?;
        writeln!(file, "{}", version_str).map_err(Error::Io)?;

        tracing::info!("Pinned Python {} in {}", version_str, version_file.display());

        Ok(())
    }

    /// Read the pinned version from .python-version file
    pub fn read_pin(&self, project_dir: &Path) -> Result<Option<PythonVersion>> {
        let version_file = project_dir.join(".python-version");

        if !version_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&version_file).map_err(Error::Io)?;
        let version_str = content.trim();

        if version_str.is_empty() {
            return Ok(None);
        }

        Ok(Some(PythonVersion::parse(version_str)?))
    }

    /// Set the global default Python version
    pub fn set_global(&self, version_spec: &str) -> Result<()> {
        let spec = PythonVersion::parse(version_spec)?;
        let version_str = spec.to_string_short();

        // Create config directory if needed
        fs::create_dir_all(&self.config_dir).map_err(Error::Io)?;

        let config_file = self.config_dir.join("config.toml");

        // Read existing config or create new
        let mut config: toml::Table = if config_file.exists() {
            let content = fs::read_to_string(&config_file).map_err(Error::Io)?;
            toml::from_str(&content).unwrap_or_default()
        } else {
            toml::Table::new()
        };

        // Set the default Python version
        config.insert(
            "default_python".to_string(),
            toml::Value::String(version_str.clone()),
        );

        // Write back
        let content = toml::to_string_pretty(&config)
            .map_err(|e| Error::Config(format!("failed to serialize config: {}", e)))?;
        fs::write(&config_file, content).map_err(Error::Io)?;

        tracing::info!("Set global Python to {}", version_str);

        Ok(())
    }

    /// Get the global default Python version
    pub fn get_global(&self) -> Result<Option<PythonVersion>> {
        let config_file = self.config_dir.join("config.toml");

        if !config_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&config_file).map_err(Error::Io)?;
        let config: toml::Table = toml::from_str(&content).map_err(Error::TomlParse)?;

        if let Some(version) = config.get("default_python").and_then(|v| v.as_str()) {
            Ok(Some(PythonVersion::parse(version)?))
        } else {
            Ok(None)
        }
    }

    /// Get a list of all available versions (not installed)
    pub fn list_available(&self) -> Vec<AvailableVersion> {
        available_versions()
    }

    /// Download a Python release archive
    async fn download(&self, version: &AvailableVersion) -> Result<Vec<u8>> {
        let url = self.build_download_url(version);

        tracing::info!("Downloading from {}", url);

        let response = reqwest::get(&url)
            .await
            .map_err(|e| Error::DownloadFailed(format!("request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::DownloadFailed(format!(
                "HTTP {}: {}",
                response.status(),
                url
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::DownloadFailed(format!("failed to read response: {}", e)))?;

        Ok(bytes.to_vec())
    }

    /// Build the download URL for a Python version
    fn build_download_url(&self, version: &AvailableVersion) -> String {
        // Example URL:
        // https://github.com/astral-sh/python-build-standalone/releases/download/
        //   20240814/cpython-3.12.5+20240814-aarch64-apple-darwin-pgo+lto-full.tar.zst

        let triple = self.platform.triple();
        let ext = self.platform.archive_ext();
        let opt = if self.platform.supports_optimized() {
            "pgo+lto"
        } else {
            "pgo"
        };

        format!(
            "{}/{}/cpython-{}+{}-{}-{}-full.{}",
            PBS_RELEASE_URL,
            version.release_tag,
            version.version.to_string_full(),
            version.release_tag,
            triple,
            opt,
            ext
        )
    }

    /// Extract the downloaded archive to the installation directory
    fn extract(&self, archive_data: &[u8], install_path: &Path) -> Result<()> {
        // Create parent directory
        if let Some(parent) = install_path.parent() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        match self.platform.archive_ext() {
            "tar.zst" => self.extract_tar_zst(archive_data, install_path),
            "zip" => self.extract_zip(archive_data, install_path),
            ext => Err(Error::ExtractionFailed(format!(
                "unsupported archive format: {}",
                ext
            ))),
        }
    }

    /// Extract a tar.zst archive
    fn extract_tar_zst(&self, archive_data: &[u8], install_path: &Path) -> Result<()> {
        use std::io::Cursor;

        // Decompress zstd
        let cursor = Cursor::new(archive_data);
        let decoder = zstd::Decoder::new(cursor)
            .map_err(|e| Error::ExtractionFailed(format!("zstd decode error: {}", e)))?;

        // Extract tar
        let mut archive = tar::Archive::new(decoder);

        // Create a temp directory for extraction
        let temp_dir = install_path.with_extension("tmp");
        fs::create_dir_all(&temp_dir).map_err(Error::Io)?;

        archive
            .unpack(&temp_dir)
            .map_err(|e| Error::ExtractionFailed(format!("tar extraction failed: {}", e)))?;

        // The archive contains python/install/* - move the install directory
        let extracted = temp_dir.join("python").join("install");
        if extracted.exists() {
            fs::rename(&extracted, install_path)
                .map_err(|e| Error::ExtractionFailed(format!("failed to move extracted files: {}", e)))?;
        } else {
            // Fallback: maybe the structure is different, try to find the python directory
            for entry in fs::read_dir(&temp_dir).map_err(Error::Io)? {
                let entry = entry.map_err(Error::Io)?;
                let path = entry.path();
                if path.is_dir() {
                    // Check if this looks like a Python installation
                    let bin = path.join("bin").join("python3");
                    let install_subdir = path.join("install");
                    if bin.exists() {
                        fs::rename(&path, install_path).map_err(|e| {
                            Error::ExtractionFailed(format!("failed to move extracted files: {}", e))
                        })?;
                        break;
                    } else if install_subdir.exists() {
                        fs::rename(&install_subdir, install_path).map_err(|e| {
                            Error::ExtractionFailed(format!("failed to move extracted files: {}", e))
                        })?;
                        break;
                    }
                }
            }
        }

        // Clean up temp directory
        let _ = fs::remove_dir_all(&temp_dir);

        Ok(())
    }

    /// Extract a zip archive (Windows)
    fn extract_zip(&self, archive_data: &[u8], install_path: &Path) -> Result<()> {
        use std::io::Cursor;

        let cursor = Cursor::new(archive_data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| Error::ExtractionFailed(format!("zip open error: {}", e)))?;

        // Create a temp directory for extraction
        let temp_dir = install_path.with_extension("tmp");
        fs::create_dir_all(&temp_dir).map_err(Error::Io)?;

        archive
            .extract(&temp_dir)
            .map_err(|e| Error::ExtractionFailed(format!("zip extraction failed: {}", e)))?;

        // Find and move the Python installation directory
        let extracted = temp_dir.join("python").join("install");
        if extracted.exists() {
            fs::rename(&extracted, install_path)
                .map_err(|e| Error::ExtractionFailed(format!("failed to move extracted files: {}", e)))?;
        } else {
            // Try alternative structures
            for entry in fs::read_dir(&temp_dir).map_err(Error::Io)? {
                let entry = entry.map_err(Error::Io)?;
                let path = entry.path();
                if path.is_dir() {
                    let python_exe = path.join("python.exe");
                    let install_subdir = path.join("install");
                    if python_exe.exists() {
                        fs::rename(&path, install_path).map_err(|e| {
                            Error::ExtractionFailed(format!("failed to move extracted files: {}", e))
                        })?;
                        break;
                    } else if install_subdir.exists() {
                        fs::rename(&install_subdir, install_path).map_err(|e| {
                            Error::ExtractionFailed(format!("failed to move extracted files: {}", e))
                        })?;
                        break;
                    }
                }
            }
        }

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);

        Ok(())
    }

    /// Resolve the Python executable to use for a project
    ///
    /// Checks (in order):
    /// 1. .python-version in project
    /// 2. Global config
    /// 3. System Python
    pub fn resolve_python(&self, project_dir: &Path) -> Result<PathBuf> {
        // Check project pin
        if let Some(version) = self.read_pin(project_dir)? {
            if let Some(installed) = self.find_matching(&version.to_string_full())? {
                return Ok(installed.executable());
            }
        }

        // Check global config
        if let Some(version) = self.get_global()? {
            if let Some(installed) = self.find_matching(&version.to_string_full())? {
                return Ok(installed.executable());
            }
        }

        // Fall back to system Python
        self.find_system_python()
    }

    /// Find system Python
    fn find_system_python(&self) -> Result<PathBuf> {
        let candidates = ["python3", "python"];

        for candidate in candidates {
            let output = Command::new("which")
                .arg(candidate)
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Ok(PathBuf::from(path));
                    }
                }
            }

            // Try running directly
            if let Ok(output) = Command::new(candidate).arg("--version").output() {
                if output.status.success() {
                    return Ok(PathBuf::from(candidate));
                }
            }
        }

        Err(Error::VenvError(
            "could not find Python interpreter".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_build_download_url() {
        // This test may need adjustment based on current platform
        if let Ok(manager) = PythonManager::new() {
            let version = AvailableVersion::new(
                PythonVersion::new(3, 12, Some(5)),
                "20240814",
            );
            let url = manager.build_download_url(&version);
            assert!(url.contains("cpython-3.12.5"));
            assert!(url.contains("20240814"));
        }
    }

    #[test]
    fn test_pin_and_read() {
        let temp_dir = tempdir().unwrap();
        let manager = PythonManager::with_dirs(
            temp_dir.path().join("python"),
            temp_dir.path().join("config"),
        )
        .unwrap();

        // Pin a version
        manager.pin("3.12", temp_dir.path()).unwrap();

        // Read it back
        let pinned = manager.read_pin(temp_dir.path()).unwrap();
        assert!(pinned.is_some());
        let version = pinned.unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 12);
    }

    #[test]
    fn test_global_config() {
        let temp_dir = tempdir().unwrap();
        let manager = PythonManager::with_dirs(
            temp_dir.path().join("python"),
            temp_dir.path().join("config"),
        )
        .unwrap();

        // Set global
        manager.set_global("3.11").unwrap();

        // Read it back
        let global = manager.get_global().unwrap();
        assert!(global.is_some());
        let version = global.unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 11);
    }

    #[test]
    fn test_list_available() {
        if let Ok(manager) = PythonManager::new() {
            let available = manager.list_available();
            assert!(!available.is_empty());

            // Should have Python 3.12
            assert!(available.iter().any(|v| v.version.minor == 12));
        }
    }
}
