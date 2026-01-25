//! Dependency resolver using pubgrub algorithm

mod package;
mod provider;

pub use package::Package;
pub use provider::{PyPIProvider, ROOT_PACKAGE};

use std::sync::Arc;

use pubgrub::error::PubGrubError;
use pubgrub::report::{DefaultStringReporter, Reporter};
use pubgrub::solver::resolve;
use tracing::{debug, info, instrument, warn};

use crate::index::{FileInfo, PyPIClient};
use crate::pep::{Requirement, Version};
use crate::{Error, Result};

/// Dependency resolver for Python packages
pub struct Resolver {
    /// PyPI client for fetching metadata
    client: Arc<PyPIClient>,
}

impl Resolver {
    /// Create a new resolver with default PyPI client
    pub fn new() -> Self {
        Self {
            client: Arc::new(PyPIClient::new()),
        }
    }

    /// Create a new resolver with a custom client
    pub fn with_client(client: PyPIClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }

    /// Resolve dependencies for a list of requirements
    #[instrument(skip(self, requirements))]
    pub async fn resolve(&self, requirements: &[Requirement]) -> Result<Resolution> {
        if requirements.is_empty() {
            return Ok(Resolution { packages: vec![] });
        }

        info!("resolving {} requirements", requirements.len());

        // Phase 1: Pre-fetch metadata for all direct requirements
        debug!("fetching metadata for direct dependencies");
        let mut provider = PyPIProvider::build(&self.client, requirements).await?;

        // Phase 2: Pre-crawl dependency graph to discover all needed packages
        // We need all metadata BEFORE running pubgrub
        let mut iteration = 0;
        const MAX_ITERATIONS: usize = 20;

        loop {
            iteration += 1;
            if iteration > MAX_ITERATIONS {
                return Err(Error::Resolution(
                    "too many iterations discovering dependencies".to_string(),
                ));
            }

            // Discover all packages referenced in dependencies
            let missing = provider.discover_all_packages();
            if missing.is_empty() {
                break;
            }

            debug!(
                "iteration {}: fetching {} missing packages",
                iteration,
                missing.len()
            );

            // Fetch missing metadata
            let results = self.client.get_packages_concurrent(&missing).await;
            for (name, result) in results {
                match result {
                    Ok(meta) => provider.add_metadata(name, meta),
                    Err(Error::PackageNotFound { .. }) => {
                        warn!("dependency {} not found on PyPI, skipping", name);
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        debug!("all dependencies discovered, running pubgrub solver");

        // Phase 3: Run pubgrub solver with complete metadata
        let root = Package::new(ROOT_PACKAGE);
        let solution = match resolve(&provider, root.clone(), Version::new(vec![1, 0, 0])) {
            Ok(sol) => sol,
            Err(PubGrubError::NoSolution(mut tree)) => {
                tree.collapse_no_versions();
                let msg = DefaultStringReporter::report(&tree);
                return Err(Error::Resolution(msg));
            }
            Err(PubGrubError::ErrorChoosingPackageVersion(e)) => {
                return Err(Error::Resolution(format!("error choosing version: {}", e)));
            }
            Err(PubGrubError::ErrorRetrievingDependencies {
                package,
                version,
                source,
            }) => {
                return Err(Error::Resolution(format!(
                    "error getting dependencies for {} {}: {}",
                    package, version, source
                )));
            }
            Err(PubGrubError::SelfDependency { package, version }) => {
                return Err(Error::Resolution(format!(
                    "package {} {} depends on itself",
                    package, version
                )));
            }
            Err(PubGrubError::DependencyOnTheEmptySet {
                package,
                version,
                dependent,
            }) => {
                return Err(Error::Resolution(format!(
                    "package {} {} has impossible dependency on {}",
                    package, version, dependent
                )));
            }
            Err(PubGrubError::Failure(msg)) => {
                return Err(Error::Resolution(msg));
            }
            Err(PubGrubError::ErrorInShouldCancel(e)) => {
                return Err(Error::Resolution(format!("resolution cancelled: {}", e)));
            }
        };

        self.build_resolution(&solution).await
    }

    /// Build the final resolution from the pubgrub solution
    async fn build_resolution(
        &self,
        solution: &pubgrub::type_aliases::SelectedDependencies<Package, Version>,
    ) -> Result<Resolution> {
        let mut packages = Vec::new();

        for (package, version) in solution {
            // Skip the root package
            if package.name == ROOT_PACKAGE {
                continue;
            }

            // Fetch metadata for this specific version to get file info
            let metadata = self.client.get_package(&package.name).await?;
            let version_str = version.to_string();

            // Find the best file for this version
            let files = metadata
                .releases
                .get(&version_str)
                .cloned()
                .unwrap_or_default();
            let file = Self::select_best_file(&files);

            let (url, hash) = match file {
                Some(f) => {
                    let hash = f
                        .best_hash()
                        .map(|(algo, h)| format!("{}:{}", algo, h))
                        .unwrap_or_default();
                    (f.url.clone(), hash)
                }
                None => {
                    warn!("no suitable file found for {}=={}", package.name, version);
                    (String::new(), String::new())
                }
            };

            packages.push(ResolvedPackage {
                name: package.name.clone(),
                version: version_str,
                url,
                hash,
            });
        }

        // Sort packages alphabetically for consistent output
        packages.sort_by(|a, b| a.name.cmp(&b.name));

        info!("resolved {} packages", packages.len());
        Ok(Resolution { packages })
    }

    /// Select the best file from available files
    /// Prefers wheels over sdists, and universal wheels over platform-specific
    fn select_best_file(files: &[FileInfo]) -> Option<&FileInfo> {
        // Filter out yanked files
        let available: Vec<_> = files.iter().filter(|f| !f.yanked).collect();

        if available.is_empty() {
            return None;
        }

        // First, try to find a universal wheel (py3-none-any)
        for file in &available {
            if file.is_wheel() {
                if let Some(tags) = file.parse_wheel_tags() {
                    if tags.is_universal() && tags.python.contains("py3") {
                        return Some(file);
                    }
                }
            }
        }

        // Next, try any wheel
        for file in &available {
            if file.is_wheel() {
                return Some(file);
            }
        }

        // Finally, fall back to sdist
        for file in &available {
            if file.is_sdist() {
                return Some(file);
            }
        }

        // Return first available as last resort
        available.first().copied()
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of dependency resolution
#[derive(Debug, Clone)]
pub struct Resolution {
    /// Resolved packages with their versions
    pub packages: Vec<ResolvedPackage>,
}

impl Resolution {
    /// Get a package by name
    pub fn get(&self, name: &str) -> Option<&ResolvedPackage> {
        let normalized = Package::new(name).name;
        self.packages
            .iter()
            .find(|p| Package::new(&p.name).name == normalized)
    }

    /// Check if a package is in the resolution
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Number of resolved packages
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    /// Check if resolution is empty
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

/// A resolved package with its locked version
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    /// Package name (normalized)
    pub name: String,
    /// Resolved version
    pub version: String,
    /// Download URL
    pub url: String,
    /// Hash (format: "algorithm:hash")
    pub hash: String,
}

impl ResolvedPackage {
    /// Parse the hash into algorithm and value
    pub fn parse_hash(&self) -> Option<(&str, &str)> {
        self.hash.split_once(':')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolution_get() {
        let resolution = Resolution {
            packages: vec![ResolvedPackage {
                name: "requests".to_string(),
                version: "2.28.0".to_string(),
                url: "".to_string(),
                hash: "sha256:abc".to_string(),
            }],
        };

        assert!(resolution.contains("requests"));
        assert!(resolution.contains("Requests")); // Normalized
        assert!(!resolution.contains("urllib3"));
    }

    #[test]
    fn test_parse_hash() {
        let pkg = ResolvedPackage {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            url: "".to_string(),
            hash: "sha256:abc123".to_string(),
        };

        let (algo, hash) = pkg.parse_hash().unwrap();
        assert_eq!(algo, "sha256");
        assert_eq!(hash, "abc123");
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_resolve_requests() {
        let resolver = Resolver::new();
        let requirements = vec![Requirement::parse("requests>=2.28.0").unwrap()];

        let resolution = resolver.resolve(&requirements).await.unwrap();

        assert!(resolution.contains("requests"));
        // requests should pull in urllib3, charset-normalizer, idna, certifi
        assert!(resolution.contains("urllib3"));
        assert!(resolution.contains("certifi"));
    }
}
