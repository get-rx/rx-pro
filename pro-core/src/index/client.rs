//! PyPI HTTP client with private registry support

use std::collections::HashMap;
use std::sync::Arc;

use reqwest::{Client, RequestBuilder};
use tokio::sync::RwLock;
use tracing::{debug, instrument};

use super::types::PackageMetadata;
use crate::pep::Version;
use crate::registry::{RegistryConfig, ResolvedCredentials};
use crate::{Error, Result};

/// Default PyPI index URL
pub const DEFAULT_INDEX_URL: &str = "https://pypi.org/pypi";

/// PyPI index client for fetching package metadata
#[derive(Clone)]
pub struct PyPIClient {
    /// HTTP client
    client: Client,
    /// Base URL for the index (JSON API)
    base_url: String,
    /// Resolved credentials for authentication
    credentials: Option<ResolvedCredentials>,
    /// Cache for package metadata
    cache: Arc<RwLock<HashMap<String, PackageMetadata>>>,
}

impl PyPIClient {
    /// Create a new PyPI client with default settings
    pub fn new() -> Self {
        Self::with_url(DEFAULT_INDEX_URL)
    }

    /// Create a new PyPI client with a custom index URL
    pub fn with_url(base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .user_agent(concat!("pro/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("failed to build HTTP client");

        Self {
            client,
            base_url: base_url.into(),
            credentials: None,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a client from a registry configuration
    pub fn from_registry(config: &RegistryConfig) -> Result<Self> {
        let client = Client::builder()
            .user_agent(concat!("pro/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("failed to build HTTP client");

        let credentials = if config.has_auth() {
            Some(config.resolve_credentials()?)
        } else {
            None
        };

        Ok(Self {
            client,
            base_url: config.api_url(),
            credentials,
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Set credentials for authentication
    pub fn with_credentials(mut self, credentials: ResolvedCredentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Apply authentication to a request
    fn apply_auth(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.credentials {
            Some(creds) => {
                if let Some(ref token) = creds.token {
                    // Bearer token authentication
                    request.bearer_auth(token)
                } else if let (Some(ref username), Some(ref password)) =
                    (&creds.username, &creds.password)
                {
                    // Basic authentication
                    request.basic_auth(username, Some(password))
                } else {
                    request
                }
            }
            None => request,
        }
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Fetch package metadata from PyPI
    #[instrument(skip(self), fields(package = %name))]
    pub async fn get_package(&self, name: &str) -> Result<PackageMetadata> {
        let normalized = Self::normalize_name(name);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(metadata) = cache.get(&normalized) {
                debug!("cache hit for {}", normalized);
                return Ok(metadata.clone());
            }
        }

        debug!("fetching metadata for {}", normalized);

        let url = format!("{}/{}/json", self.base_url, normalized);
        let request = self.apply_auth(self.client.get(&url));
        let response = request.send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::PackageNotFound {
                package: name.to_string(),
            });
        }

        let metadata: PackageMetadata = response.error_for_status()?.json().await?;

        // Cache the result
        {
            let mut cache = self.cache.write().await;
            cache.insert(normalized, metadata.clone());
        }

        Ok(metadata)
    }

    /// Fetch metadata for a specific version
    #[instrument(skip(self), fields(package = %name, version = %version))]
    pub async fn get_package_version(&self, name: &str, version: &str) -> Result<PackageMetadata> {
        let normalized = Self::normalize_name(name);

        debug!("fetching metadata for {}=={}", normalized, version);

        let url = format!("{}/{}/{}/json", self.base_url, normalized, version);
        let request = self.apply_auth(self.client.get(&url));
        let response = request.send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::VersionNotFound {
                package: name.to_string(),
                version: version.to_string(),
            });
        }

        response
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }

    /// Get all available versions for a package
    #[instrument(skip(self), fields(package = %name))]
    pub async fn get_versions(&self, name: &str) -> Result<Vec<Version>> {
        let metadata = self.get_package(name).await?;

        let mut versions: Vec<Version> = metadata
            .releases
            .keys()
            .filter_map(|v| Version::parse(v).ok())
            .collect();

        // Sort by version, newest first
        versions.sort_by(|a, b| b.cmp(a));

        Ok(versions)
    }

    /// Get available versions that have non-yanked files
    #[instrument(skip(self), fields(package = %name))]
    pub async fn get_available_versions(&self, name: &str) -> Result<Vec<Version>> {
        let metadata = self.get_package(name).await?;

        let mut versions: Vec<Version> = metadata
            .releases
            .iter()
            .filter(|(_, files)| {
                // Version is available if it has at least one non-yanked file
                files.iter().any(|f| !f.yanked)
            })
            .filter_map(|(v, _)| Version::parse(v).ok())
            .collect();

        // Sort by version, newest first
        versions.sort_by(|a, b| b.cmp(a));

        Ok(versions)
    }

    /// Fetch metadata for multiple packages concurrently
    #[instrument(skip(self, names))]
    pub async fn get_packages_concurrent(
        &self,
        names: &[String],
    ) -> HashMap<String, Result<PackageMetadata>> {
        use futures::future::join_all;

        let futures: Vec<_> = names
            .iter()
            .map(|name| {
                let name = name.clone();
                let client = self.clone();
                async move {
                    let result = client.get_package(&name).await;
                    (Self::normalize_name(&name), result)
                }
            })
            .collect();

        join_all(futures).await.into_iter().collect()
    }

    /// Clear the metadata cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Normalize a package name according to PEP 503
    fn normalize_name(name: &str) -> String {
        name.to_lowercase()
            .chars()
            .map(|c| match c {
                '_' | '.' => '-',
                c => c,
            })
            .collect()
    }
}

impl Default for PyPIClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_name() {
        assert_eq!(PyPIClient::normalize_name("requests"), "requests");
        assert_eq!(PyPIClient::normalize_name("Requests"), "requests");
        assert_eq!(PyPIClient::normalize_name("my_package"), "my-package");
        assert_eq!(
            PyPIClient::normalize_name("zope.interface"),
            "zope-interface"
        );
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_get_package() {
        let client = PyPIClient::new();
        let metadata = client.get_package("requests").await.unwrap();
        assert_eq!(metadata.info.name.to_lowercase(), "requests");
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_get_versions() {
        let client = PyPIClient::new();
        let versions = client.get_versions("requests").await.unwrap();
        assert!(!versions.is_empty());
        // Versions should be sorted newest first
        for i in 1..versions.len() {
            assert!(versions[i - 1] >= versions[i]);
        }
    }
}
