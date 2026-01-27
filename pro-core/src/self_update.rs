//! Self-update functionality for the rx CLI
//!
//! Detects installation method and updates appropriately:
//! - pip: delegates to `pip install --upgrade rx-pro`
//! - cargo: delegates to `cargo install pro-cli`
//! - binary: downloads latest from GitHub releases

use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::python::{Arch, Os, Platform};
use crate::{Error, Result};

/// GitHub repository for releases
const GITHUB_REPO: &str = "pro-rx/rx";

/// How rx was installed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMethod {
    /// Installed via pip (rx-pro package)
    Pip,
    /// Installed via cargo (pro-cli crate)
    Cargo,
    /// Installed as standalone binary
    Binary,
}

impl std::fmt::Display for InstallMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallMethod::Pip => write!(f, "pip"),
            InstallMethod::Cargo => write!(f, "cargo"),
            InstallMethod::Binary => write!(f, "binary"),
        }
    }
}

/// Release information from GitHub
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub download_url: String,
    pub asset_name: String,
}

/// Self-updater for the rx CLI
pub struct SelfUpdater {
    platform: Platform,
    current_version: String,
    install_method: InstallMethod,
    exe_path: PathBuf,
}

impl SelfUpdater {
    /// Create a new self-updater
    pub fn new(current_version: &str) -> Result<Self> {
        let platform = Platform::current()?;
        let exe_path = env::current_exe()
            .map_err(|e| Error::Io(io::Error::new(io::ErrorKind::NotFound, e.to_string())))?;
        let install_method = Self::detect_install_method(&exe_path);

        Ok(Self {
            platform,
            current_version: current_version.to_string(),
            install_method,
            exe_path,
        })
    }

    /// Detect how rx was installed based on executable path
    fn detect_install_method(exe_path: &PathBuf) -> InstallMethod {
        let path_str = exe_path.to_string_lossy().to_lowercase();

        // Check for cargo install location (~/.cargo/bin/)
        if path_str.contains(".cargo") && path_str.contains("bin") {
            return InstallMethod::Cargo;
        }

        // Check for development/source builds (target/release or target/debug)
        if path_str.contains("target/release") || path_str.contains("target/debug") {
            return InstallMethod::Cargo;
        }

        // Check for pip install locations
        // - site-packages (direct install)
        // - venv bin directories with Python
        // - Scripts directory (Windows pip)
        // - Homebrew or system Python paths
        if path_str.contains("site-packages")
            || (path_str.contains("bin") && path_str.contains("python"))
            || (path_str.contains("scripts") && path_str.contains("python"))
            || path_str.contains("/lib/python")
            || path_str.contains("\\lib\\python")
        {
            return InstallMethod::Pip;
        }

        // Check if pip knows about this package AND we're in a pip-managed location
        // (not just that the package exists somewhere)
        if Self::check_pip_owns_binary(exe_path) {
            return InstallMethod::Pip;
        }

        // Default to binary install
        InstallMethod::Binary
    }

    /// Check if pip installed the binary at this specific path
    fn check_pip_owns_binary(exe_path: &PathBuf) -> bool {
        // Get pip show output and check if the location matches
        let output = Command::new("pip")
            .args(["show", "-f", "rx-pro"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Check if Location is a parent of our exe_path
                for line in stdout.lines() {
                    if let Some(location) = line.strip_prefix("Location: ") {
                        let exe_str = exe_path.to_string_lossy();
                        if exe_str.to_lowercase().contains(&location.to_lowercase()) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Get the detected installation method
    pub fn install_method(&self) -> InstallMethod {
        self.install_method
    }

    /// Get the executable path
    pub fn exe_path(&self) -> &PathBuf {
        &self.exe_path
    }

    /// Get the asset name for the current platform
    fn asset_name(&self) -> String {
        let ext = match self.platform.os {
            Os::Windows => "zip",
            _ => "tar.gz",
        };
        format!("rx-{}.{}", self.platform.triple(), ext)
    }

    /// Check for the latest release
    pub async fn check_latest(&self) -> Result<Option<ReleaseInfo>> {
        let client = reqwest::Client::builder()
            .user_agent("rx-self-update")
            .build()
            .map_err(|e| Error::UpdateError(e.to_string()))?;

        let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::UpdateError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(Error::UpdateError(format!(
                "Failed to fetch release info: {}",
                response.status()
            )));
        }

        let release: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::UpdateError(e.to_string()))?;

        let tag_name = release["tag_name"]
            .as_str()
            .ok_or_else(|| Error::UpdateError("Missing tag_name in release".to_string()))?;

        // Strip 'v' prefix if present
        let version = tag_name.strip_prefix('v').unwrap_or(tag_name);

        // Check if we're already on the latest version
        if version == self.current_version {
            return Ok(None);
        }

        // Find the asset for our platform
        let asset_name = self.asset_name();
        let assets = release["assets"]
            .as_array()
            .ok_or_else(|| Error::UpdateError("Missing assets in release".to_string()))?;

        let asset = assets
            .iter()
            .find(|a| a["name"].as_str() == Some(&asset_name))
            .ok_or_else(|| {
                Error::UpdateError(format!(
                    "No release asset found for platform: {}",
                    self.platform.triple()
                ))
            })?;

        let download_url = asset["browser_download_url"]
            .as_str()
            .ok_or_else(|| Error::UpdateError("Missing download URL".to_string()))?;

        Ok(Some(ReleaseInfo {
            version: version.to_string(),
            tag_name: tag_name.to_string(),
            download_url: download_url.to_string(),
            asset_name,
        }))
    }

    /// Update via pip
    pub fn update_via_pip(&self) -> Result<()> {
        let status = Command::new("pip")
            .args(["install", "--upgrade", "rx-pro"])
            .status()
            .map_err(|e| Error::Io(e))?;

        if !status.success() {
            return Err(Error::UpdateError("pip upgrade failed".to_string()));
        }
        Ok(())
    }

    /// Update via cargo
    pub fn update_via_cargo(&self) -> Result<()> {
        let status = Command::new("cargo")
            .args(["install", "pro-cli"])
            .status()
            .map_err(|e| Error::Io(e))?;

        if !status.success() {
            return Err(Error::UpdateError("cargo install failed".to_string()));
        }
        Ok(())
    }

    /// Download and install the update (for binary installs)
    pub async fn update_binary(&self, release: &ReleaseInfo) -> Result<PathBuf> {
        let client = reqwest::Client::builder()
            .user_agent("rx-self-update")
            .build()
            .map_err(|e| Error::UpdateError(e.to_string()))?;

        // Download the archive
        let response = client
            .get(&release.download_url)
            .send()
            .await
            .map_err(|e| Error::UpdateError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(Error::UpdateError(format!(
                "Failed to download release: {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::UpdateError(e.to_string()))?;

        // Create temp directory for extraction
        let temp_dir = env::temp_dir().join(format!("rx-update-{}", release.version));
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir)?;
        }
        fs::create_dir_all(&temp_dir)?;

        // Save archive to temp
        let archive_path = temp_dir.join(&release.asset_name);
        let mut file = File::create(&archive_path)?;
        file.write_all(&bytes)?;
        drop(file);

        // Extract the archive
        let new_exe_path = if self.platform.os == Os::Windows {
            self.extract_zip(&archive_path, &temp_dir)?
        } else {
            self.extract_tar_gz(&archive_path, &temp_dir)?
        };

        // Replace the current executable
        self.replace_executable(&self.exe_path, &new_exe_path)?;

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);

        Ok(self.exe_path.clone())
    }

    /// Extract a tar.gz archive and return the path to the rx binary
    fn extract_tar_gz(&self, archive_path: &PathBuf, temp_dir: &PathBuf) -> Result<PathBuf> {
        let file = File::open(archive_path)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(temp_dir)?;

        // Find the rx binary in the extracted contents
        let rx_path = temp_dir.join("rx");
        if rx_path.exists() {
            return Ok(rx_path);
        }

        // Check if it's in a subdirectory
        for entry in fs::read_dir(temp_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let rx_in_dir = path.join("rx");
                if rx_in_dir.exists() {
                    return Ok(rx_in_dir);
                }
            }
        }

        Err(Error::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "rx binary not found in archive",
        )))
    }

    /// Extract a zip archive and return the path to the rx binary
    fn extract_zip(&self, archive_path: &PathBuf, temp_dir: &PathBuf) -> Result<PathBuf> {
        let file = File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| Error::Io(io::Error::new(io::ErrorKind::InvalidData, e.to_string())))?;

        archive
            .extract(temp_dir)
            .map_err(|e| Error::Io(io::Error::new(io::ErrorKind::InvalidData, e.to_string())))?;

        // Find the rx.exe binary
        let rx_path = temp_dir.join("rx.exe");
        if rx_path.exists() {
            return Ok(rx_path);
        }

        // Check if it's in a subdirectory
        for entry in fs::read_dir(temp_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let rx_in_dir = path.join("rx.exe");
                if rx_in_dir.exists() {
                    return Ok(rx_in_dir);
                }
            }
        }

        Err(Error::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "rx.exe binary not found in archive",
        )))
    }

    /// Replace the current executable with the new one
    fn replace_executable(&self, current: &PathBuf, new: &PathBuf) -> Result<()> {
        // Make the new binary executable (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(new)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(new, perms)?;
        }

        // On Windows, we can't replace a running executable directly
        // We need to rename the current one first
        #[cfg(windows)]
        {
            let backup = current.with_extension("exe.old");
            if backup.exists() {
                fs::remove_file(&backup)?;
            }
            fs::rename(current, &backup)?;
            fs::copy(new, current)?;
            // Try to remove the backup (might fail if still in use)
            let _ = fs::remove_file(&backup);
        }

        #[cfg(not(windows))]
        {
            // On Unix, we can just copy over the running binary
            fs::copy(new, current)?;
        }

        Ok(())
    }

    /// Compare versions to determine if update is newer
    pub fn is_newer(current: &str, latest: &str) -> bool {
        // Simple semver comparison
        let parse = |v: &str| -> (u32, u32, u32) {
            let parts: Vec<&str> = v.split('.').collect();
            let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
            let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            (major, minor, patch)
        };

        parse(latest) > parse(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(SelfUpdater::is_newer("0.1.11", "0.1.12"));
        assert!(SelfUpdater::is_newer("0.1.12", "0.2.0"));
        assert!(SelfUpdater::is_newer("0.1.12", "1.0.0"));
        assert!(!SelfUpdater::is_newer("0.1.12", "0.1.12"));
        assert!(!SelfUpdater::is_newer("0.1.12", "0.1.11"));
    }

    #[test]
    fn test_asset_name() {
        let updater = SelfUpdater {
            platform: Platform::new(Os::Linux, Arch::X86_64),
            current_version: "0.1.0".to_string(),
        };
        assert_eq!(updater.asset_name(), "rx-x86_64-unknown-linux-gnu.tar.gz");

        let updater = SelfUpdater {
            platform: Platform::new(Os::MacOS, Arch::Aarch64),
            current_version: "0.1.0".to_string(),
        };
        assert_eq!(updater.asset_name(), "rx-aarch64-apple-darwin.tar.gz");

        let updater = SelfUpdater {
            platform: Platform::new(Os::Windows, Arch::X86_64),
            current_version: "0.1.0".to_string(),
        };
        assert_eq!(updater.asset_name(), "rx-x86_64-pc-windows-msvc.zip");
    }
}
