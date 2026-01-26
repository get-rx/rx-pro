//! Package installer with caching and parallel downloads
//!
//! Downloads wheels, verifies hashes, and unpacks to site-packages.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use futures::stream::{self, StreamExt};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::lockfile::LockedPackage;
use crate::{Error, Result};

/// Maximum concurrent downloads
const MAX_CONCURRENT_DOWNLOADS: usize = 8;

/// Package installer
pub struct Installer {
    /// Cache directory for downloaded wheels
    cache_dir: PathBuf,
    /// HTTP client
    client: reqwest::Client,
}

/// Result of installing a package
#[derive(Debug)]
pub struct InstallResult {
    /// Package name
    pub name: String,
    /// Whether it was installed (false if already cached)
    pub installed: bool,
    /// Whether it was cached (downloaded this run)
    pub downloaded: bool,
    /// Error message if failed
    pub error: Option<String>,
}

impl Installer {
    /// Create a new installer with the given cache directory
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            client: reqwest::Client::builder()
                .user_agent("Pro/0.1.0")
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Install packages from lockfile into site-packages
    pub async fn install(
        &self,
        packages: &HashMap<String, LockedPackage>,
        site_packages: &Path,
    ) -> Result<Vec<InstallResult>> {
        // Ensure cache directory exists
        fs::create_dir_all(&self.cache_dir).map_err(Error::Io)?;
        fs::create_dir_all(site_packages).map_err(Error::Io)?;

        // Download packages in parallel
        let download_tasks: Vec<_> = packages
            .iter()
            .map(|(name, pkg)| self.download_package(name, pkg))
            .collect();

        let download_results: Vec<_> = stream::iter(download_tasks)
            .buffer_unordered(MAX_CONCURRENT_DOWNLOADS)
            .collect()
            .await;

        // Install each downloaded package
        let mut results = Vec::new();
        for name in packages.keys() {
            let download_result = download_results.iter().find(|(n, _, _)| n == name);

            match download_result {
                Some((_, Some(cached_path), downloaded)) => {
                    match self.install_wheel(cached_path, site_packages) {
                        Ok(()) => {
                            results.push(InstallResult {
                                name: name.clone(),
                                installed: true,
                                downloaded: *downloaded,
                                error: None,
                            });
                        }
                        Err(e) => {
                            results.push(InstallResult {
                                name: name.clone(),
                                installed: false,
                                downloaded: *downloaded,
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
                Some((_, None, _)) => {
                    results.push(InstallResult {
                        name: name.clone(),
                        installed: false,
                        downloaded: false,
                        error: Some("No download URL available".into()),
                    });
                }
                None => {
                    results.push(InstallResult {
                        name: name.clone(),
                        installed: false,
                        downloaded: false,
                        error: Some("Download task not found".into()),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Download a package and return the cached path
    async fn download_package(
        &self,
        name: &str,
        pkg: &LockedPackage,
    ) -> (String, Option<PathBuf>, bool) {
        let url = match &pkg.url {
            Some(u) if !u.is_empty() => u,
            _ => return (name.to_string(), None, false),
        };

        // Determine cache path from URL
        let filename = url.rsplit('/').next().unwrap_or("package.whl");
        let cached_path = self.cache_dir.join(filename);

        // Check if already cached with valid hash
        if cached_path.exists() {
            if let Some(expected_hash) = &pkg.hash {
                if let Ok(actual_hash) = compute_file_hash(&cached_path) {
                    if verify_hash(&actual_hash, expected_hash) {
                        tracing::debug!("Using cached: {}", filename);
                        return (name.to_string(), Some(cached_path), false);
                    }
                }
            } else {
                // No hash to verify, assume cached is good
                tracing::debug!("Using cached (no hash): {}", filename);
                return (name.to_string(), Some(cached_path), false);
            }
        }

        // Download the file
        tracing::info!("Downloading {} from {}", name, url);
        match self.download_file(url, &cached_path).await {
            Ok(()) => {
                // Verify hash if provided
                if let Some(expected_hash) = &pkg.hash {
                    match compute_file_hash(&cached_path) {
                        Ok(actual_hash) => {
                            if !verify_hash(&actual_hash, expected_hash) {
                                tracing::error!(
                                    "Hash mismatch for {}: expected {}, got sha256:{}",
                                    name,
                                    expected_hash,
                                    actual_hash
                                );
                                let _ = fs::remove_file(&cached_path);
                                return (name.to_string(), None, false);
                            }
                            tracing::debug!("Hash verified for {}", name);
                        }
                        Err(e) => {
                            tracing::error!("Failed to compute hash for {}: {}", name, e);
                            return (name.to_string(), None, false);
                        }
                    }
                }
                (name.to_string(), Some(cached_path), true)
            }
            Err(e) => {
                tracing::error!("Failed to download {}: {}", name, e);
                (name.to_string(), None, false)
            }
        }
    }

    /// Download a file to the given path
    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        let response = self.client.get(url).send().await.map_err(Error::Network)?;

        if !response.status().is_success() {
            return Err(Error::Index(format!(
                "Failed to download {}: HTTP {}",
                url,
                response.status()
            )));
        }

        let bytes = response.bytes().await.map_err(Error::Network)?;

        let mut file = File::create(dest).map_err(Error::Io)?;
        file.write_all(&bytes).map_err(Error::Io)?;

        Ok(())
    }

    /// Install a wheel file to site-packages
    fn install_wheel(&self, wheel_path: &Path, site_packages: &Path) -> Result<()> {
        tracing::debug!("Installing wheel: {:?}", wheel_path);

        let file = File::open(wheel_path).map_err(Error::Io)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| Error::BuildError(format!("Invalid wheel: {}", e)))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| Error::BuildError(format!("Failed to read wheel entry: {}", e)))?;

            let entry_path = entry
                .enclosed_name()
                .ok_or_else(|| Error::BuildError("Invalid entry path in wheel".into()))?;

            let dest_path = site_packages.join(&entry_path);

            if entry.is_dir() {
                fs::create_dir_all(&dest_path).map_err(Error::Io)?;
            } else {
                // Create parent directories
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent).map_err(Error::Io)?;
                }

                // Extract file
                let mut outfile = File::create(&dest_path).map_err(Error::Io)?;
                io::copy(&mut entry, &mut outfile).map_err(Error::Io)?;

                // Set executable permissions for scripts
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if entry_path.starts_with("..")
                        || entry_path.to_string_lossy().contains("/bin/")
                    {
                        let mut perms = fs::metadata(&dest_path).map_err(Error::Io)?.permissions();
                        perms.set_mode(0o755);
                        fs::set_permissions(&dest_path, perms).map_err(Error::Io)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Clear the cache
    pub fn clear_cache(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).map_err(Error::Io)?;
        }
        Ok(())
    }
}

/// Compute SHA256 hash of a file
fn compute_file_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(Error::Io)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(Error::Io)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Verify a hash against expected (handles "sha256:..." prefix)
fn verify_hash(actual: &str, expected: &str) -> bool {
    let expected_hash = expected
        .strip_prefix("sha256:")
        .unwrap_or(expected)
        .to_lowercase();
    actual.to_lowercase() == expected_hash
}

/// Get the default cache directory
pub fn default_cache_dir() -> PathBuf {
    // Use XDG_CACHE_HOME or ~/.cache on Unix
    // Use %LOCALAPPDATA% on Windows
    #[cfg(unix)]
    {
        if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
            return PathBuf::from(xdg_cache).join("rx").join("wheels");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".cache").join("rx").join("wheels");
        }
    }

    #[cfg(windows)]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local_app_data)
                .join("rx")
                .join("cache")
                .join("wheels");
        }
    }

    // Fallback
    PathBuf::from("/tmp/rx-cache/wheels")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_compute_file_hash() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"hello world").unwrap();
        drop(file);

        let hash = compute_file_hash(&file_path).unwrap();
        // SHA256 of "hello world"
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_verify_hash() {
        let hash = "abc123def456";
        assert!(verify_hash(hash, "sha256:abc123def456"));
        assert!(verify_hash(hash, "ABC123DEF456"));
        assert!(!verify_hash(hash, "sha256:different"));
    }

    #[test]
    fn test_installer_new() {
        let installer = Installer::new("/tmp/test-cache");
        assert_eq!(installer.cache_dir(), Path::new("/tmp/test-cache"));
    }

    #[test]
    fn test_default_cache_dir() {
        let cache_dir = default_cache_dir();
        assert!(cache_dir.to_string_lossy().contains("rx"));
    }
}
