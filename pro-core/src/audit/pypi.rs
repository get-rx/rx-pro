//! PyPI API client for yanked version detection
//!
//! Yanked releases on PyPI often indicate security issues or critical bugs.
//! This module checks if installed packages are using yanked versions.

use std::collections::HashMap;

use serde::Deserialize;

use crate::{Error, Result};

/// PyPI JSON API base URL
const PYPI_API_URL: &str = "https://pypi.org/pypi";

/// PyPI API client
pub struct PyPIClient {
    client: reqwest::Client,
}

impl PyPIClient {
    /// Create a new PyPI client
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Pro/0.1.0")
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Check if a specific version of a package is yanked
    pub async fn is_yanked(&self, package: &str, version: &str) -> Result<bool> {
        let url = format!("{}/{}/json", PYPI_API_URL, package);

        let response = self.client.get(&url).send().await.map_err(Error::Network)?;

        if !response.status().is_success() {
            // Package not found or API error - assume not yanked
            return Ok(false);
        }

        let pkg_info: PyPIPackageInfo = response.json().await.map_err(Error::Network)?;

        // Check if the specific version is yanked
        if let Some(releases) = pkg_info.releases.get(version) {
            // A version is yanked if ALL its files are yanked
            // (empty releases are considered not yanked)
            if releases.is_empty() {
                return Ok(false);
            }
            return Ok(releases.iter().all(|r| r.yanked));
        }

        Ok(false)
    }

    /// Check multiple packages for yanked versions
    pub async fn check_yanked_batch(
        &self,
        packages: &[(&str, &str)], // (name, version)
    ) -> Result<Vec<YankedPackage>> {
        let mut yanked = Vec::new();

        // Check packages in parallel (limited concurrency)
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
        let mut handles = Vec::new();

        for (name, version) in packages {
            let name = name.to_string();
            let version = version.to_string();
            let client = self.client.clone();
            let semaphore = semaphore.clone();

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                check_single_package(&client, &name, &version).await
            });
            handles.push(handle);
        }

        for handle in handles {
            if let Ok(Ok(Some(yanked_pkg))) = handle.await {
                yanked.push(yanked_pkg);
            }
        }

        Ok(yanked)
    }
}

impl Default for PyPIClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Check a single package for yanked status
async fn check_single_package(
    client: &reqwest::Client,
    package: &str,
    version: &str,
) -> Result<Option<YankedPackage>> {
    let url = format!("{}/{}/json", PYPI_API_URL, package);

    let response = client.get(&url).send().await.map_err(Error::Network)?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let pkg_info: PyPIPackageInfo = response.json().await.map_err(Error::Network)?;

    if let Some(releases) = pkg_info.releases.get(version) {
        if !releases.is_empty() && releases.iter().all(|r| r.yanked) {
            // Get yanked reason from first file
            let reason = releases.first().and_then(|r| r.yanked_reason.clone());
            return Ok(Some(YankedPackage {
                name: package.to_string(),
                version: version.to_string(),
                reason,
            }));
        }
    }

    Ok(None)
}

/// A package version that has been yanked from PyPI
#[derive(Debug, Clone)]
pub struct YankedPackage {
    /// Package name
    pub name: String,
    /// Yanked version
    pub version: String,
    /// Reason for yanking (if provided)
    pub reason: Option<String>,
}

// PyPI API response types

#[derive(Debug, Deserialize)]
struct PyPIPackageInfo {
    releases: HashMap<String, Vec<PyPIRelease>>,
}

#[derive(Debug, Deserialize)]
struct PyPIRelease {
    yanked: bool,
    yanked_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network
    async fn test_check_not_yanked() {
        let client = PyPIClient::new();
        // requests 2.31.0 should not be yanked
        let yanked = client.is_yanked("requests", "2.31.0").await.unwrap();
        assert!(!yanked);
    }
}
