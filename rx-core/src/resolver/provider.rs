//! pubgrub DependencyProvider implementation for PyPI packages

use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::error::Error as StdError;

use pubgrub::range::Range;
use pubgrub::solver::{Dependencies, DependencyProvider};

use crate::index::{PackageMetadata, PyPIClient};
use crate::pep::{Requirement, Version, VersionSpecifiers};
use crate::resolver::Package;
use crate::Error;

/// Special package representing the root project
/// Note: This name is chosen to survive Package::normalize() unchanged
pub const ROOT_PACKAGE: &str = "root-project";

/// PyPI-based dependency provider for pubgrub
pub struct PyPIProvider {
    /// Cached package metadata (package name -> metadata)
    metadata_cache: HashMap<String, PackageMetadata>,
    /// Cached parsed versions (package name -> sorted versions, newest first)
    versions_cache: HashMap<String, Vec<Version>>,
    /// Packages we've seen during resolution (interior mutability for get_dependencies)
    seen_packages: RefCell<HashSet<String>>,
    /// Root package dependencies
    root_deps: Vec<(Package, Range<Version>)>,
}

impl PyPIProvider {
    /// Create a new provider with pre-fetched metadata
    pub fn new(
        metadata: HashMap<String, PackageMetadata>,
        root_deps: Vec<(Package, Range<Version>)>,
    ) -> Self {
        let mut versions_cache = HashMap::new();

        // Pre-parse versions for all packages
        for (name, meta) in &metadata {
            let mut versions: Vec<Version> = meta
                .releases
                .iter()
                .filter(|(_, files)| files.iter().any(|f| !f.yanked && !files.is_empty()))
                .filter_map(|(v, _)| Version::parse(v).ok())
                .collect();

            // Sort newest first (pubgrub expects this for choose_version)
            versions.sort_by(|a, b| b.cmp(a));
            versions_cache.insert(name.clone(), versions);
        }

        // Initialize seen packages with root deps
        let mut seen = HashSet::new();
        for (pkg, _) in &root_deps {
            seen.insert(pkg.name.clone());
        }

        Self {
            metadata_cache: metadata,
            versions_cache,
            seen_packages: RefCell::new(seen),
            root_deps,
        }
    }

    /// Build the provider by pre-fetching all required metadata
    pub async fn build(
        client: &PyPIClient,
        requirements: &[Requirement],
    ) -> Result<Self, Error> {
        // Collect all package names we need to fetch
        let names: Vec<String> = requirements.iter().map(|r| r.name.clone()).collect();

        // Fetch all metadata concurrently
        let results = client.get_packages_concurrent(&names).await;

        // Collect successful fetches and errors
        let mut metadata = HashMap::new();
        for (name, result) in results {
            match result {
                Ok(meta) => {
                    metadata.insert(name, meta);
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        // Parse root dependencies
        let root_deps = requirements
            .iter()
            .map(|r| {
                let pkg = Package::new(&r.name);
                let range = if let Some(ref spec) = r.specifier {
                    VersionSpecifiers::parse(spec)
                        .map(|s| s.to_pubgrub_range())
                        .unwrap_or_else(|_| Range::any())
                } else {
                    Range::any()
                };
                (pkg, range)
            })
            .collect();

        Ok(Self::new(metadata, root_deps))
    }

    /// Add more metadata to the provider (for transitive dependencies)
    pub fn add_metadata(&mut self, name: String, metadata: PackageMetadata) {
        let mut versions: Vec<Version> = metadata
            .releases
            .iter()
            .filter(|(_, files)| files.iter().any(|f| !f.yanked && !files.is_empty()))
            .filter_map(|(v, _)| Version::parse(v).ok())
            .collect();

        versions.sort_by(|a, b| b.cmp(a));
        self.versions_cache.insert(name.clone(), versions);
        self.metadata_cache.insert(name, metadata);
    }

    /// Get all packages that need metadata fetched
    pub fn missing_packages(&self) -> Vec<String> {
        let seen = self.seen_packages.borrow();
        let mut missing: Vec<String> = seen
            .iter()
            .filter(|name| !self.metadata_cache.contains_key(*name))
            .cloned()
            .collect();

        missing.sort();
        missing
    }

    /// Pre-crawl the dependency graph to find all packages we need
    /// This must be called before running pubgrub to ensure all metadata is available
    pub fn discover_all_packages(&mut self) -> Vec<String> {
        let mut to_process: Vec<String> = self
            .seen_packages
            .borrow()
            .iter()
            .cloned()
            .collect();
        let mut all_seen: HashSet<String> = to_process.iter().cloned().collect();

        while let Some(pkg_name) = to_process.pop() {
            let Some(metadata) = self.metadata_cache.get(&pkg_name) else {
                continue;
            };

            // Get requires_dist from package info
            let requires_dist = metadata.info.requires_dist.clone().unwrap_or_default();

            for req_str in &requires_dist {
                if let Ok(req) = Requirement::parse(req_str) {
                    // Skip requirements with markers
                    if req.marker.is_some() {
                        continue;
                    }
                    // Skip extras
                    if !req.extras.is_empty() {
                        continue;
                    }

                    let normalized = Package::new(&req.name).name;
                    if !all_seen.contains(&normalized) {
                        all_seen.insert(normalized.clone());
                        to_process.push(normalized);
                    }
                }
            }
        }

        // Update seen_packages
        *self.seen_packages.borrow_mut() = all_seen;

        // Return missing packages
        self.missing_packages()
    }

    /// Parse dependencies for a specific package version
    fn parse_dependencies(
        &self,
        package: &str,
        version: &Version,
    ) -> Vec<(Package, Range<Version>)> {
        let Some(metadata) = self.metadata_cache.get(package) else {
            return vec![];
        };

        let version_str = version.to_string();
        let Some(_files) = metadata.releases.get(&version_str) else {
            return vec![];
        };

        // Get requires_dist from package info
        let requires_dist = metadata.info.requires_dist.clone().unwrap_or_default();

        let deps: Vec<_> = requires_dist
            .iter()
            .filter_map(|req_str| {
                // Parse the requirement
                let req = Requirement::parse(req_str).ok()?;

                // Skip requirements with markers (MVP simplification)
                // In the future, we'd evaluate markers against current environment
                if req.marker.is_some() {
                    return None;
                }

                // Skip extras (MVP simplification)
                if !req.extras.is_empty() {
                    return None;
                }

                let pkg = Package::new(&req.name);
                let range = if let Some(ref spec) = req.specifier {
                    VersionSpecifiers::parse(spec)
                        .map(|s| s.to_pubgrub_range())
                        .unwrap_or_else(|_| Range::any())
                } else {
                    Range::any()
                };

                Some((pkg, range))
            })
            .collect();

        // Track all packages we've seen (interior mutability)
        {
            let mut seen = self.seen_packages.borrow_mut();
            for (pkg, _) in &deps {
                seen.insert(pkg.name.clone());
            }
        }

        deps
    }

    /// Get versions for a package
    fn get_versions(&self, package: &str) -> Option<&Vec<Version>> {
        self.versions_cache.get(package)
    }
}

impl DependencyProvider<Package, Version> for PyPIProvider {
    fn choose_package_version<T: Borrow<Package>, U: Borrow<Range<Version>>>(
        &self,
        potential_packages: impl Iterator<Item = (T, U)>,
    ) -> Result<(T, Option<Version>), Box<dyn StdError>> {
        // Strategy: pick the package with fewest available versions (faster resolution)
        let mut best: Option<(T, U, usize)> = None;

        for (package, range) in potential_packages {
            let pkg = package.borrow();

            // Root package always has exactly one version
            if pkg.name == ROOT_PACKAGE {
                return Ok((package, Some(Version::new(vec![1, 0, 0]))));
            }

            // Count compatible versions
            let count = match self.get_versions(&pkg.name) {
                Some(versions) => versions.iter().filter(|v| range.borrow().contains(v)).count(),
                None => 0,
            };

            match &best {
                None => best = Some((package, range, count)),
                Some((_, _, best_count)) if count < *best_count => {
                    best = Some((package, range, count));
                }
                _ => {}
            }
        }

        let (package, range, _) = best.expect("at least one package");
        let pkg = package.borrow();

        // Find the highest compatible version
        let version = self
            .get_versions(&pkg.name)
            .and_then(|versions| {
                versions
                    .iter()
                    .find(|v| range.borrow().contains(v))
                    .cloned()
            });

        Ok((package, version))
    }

    fn get_dependencies(
        &self,
        package: &Package,
        version: &Version,
    ) -> Result<Dependencies<Package, Version>, Box<dyn StdError>> {
        // Root package returns the user's requirements
        if package.name == ROOT_PACKAGE {
            let deps: pubgrub::type_aliases::Map<Package, Range<Version>> =
                self.root_deps.iter().cloned().collect();
            return Ok(Dependencies::Known(deps));
        }

        // Get dependencies for the package version
        let deps = self.parse_dependencies(&package.name, version);

        let deps_map: pubgrub::type_aliases::Map<Package, Range<Version>> =
            deps.into_iter().collect();

        Ok(Dependencies::Known(deps_map))
    }
}

// Implement required traits for Package to work with pubgrub
impl Borrow<str> for Package {
    fn borrow(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_package() {
        let provider = PyPIProvider::new(HashMap::new(), vec![]);
        let root = Package::new(ROOT_PACKAGE);
        let range = Range::any();

        let (_, version) = provider
            .choose_package_version(vec![(&root, &range)].into_iter())
            .unwrap();
        assert!(version.is_some());
    }
}
