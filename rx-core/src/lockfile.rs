//! Lockfile format for T-Rex (rx.lock)
//!
//! The lockfile captures the exact resolved versions of all dependencies,
//! including transitive dependencies, with download URLs and hashes.
//!
//! Features:
//! - Platform markers for OS/Python-specific dependencies
//! - Dependency graph tracking (which package depends on which)
//! - Multiple file variants per platform

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::resolver::{ResolvedPackage, Resolution};
use crate::{Error, Result};

/// Lockfile format version
pub const LOCKFILE_VERSION: &str = "2";

/// The lockfile (rx.lock)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    /// Lockfile format version
    pub version: String,

    /// Metadata about the resolution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<LockfileMetadata>,

    /// Locked packages (sorted by name for deterministic output)
    #[serde(default)]
    pub packages: BTreeMap<String, LockedPackage>,
}

/// Metadata about the lockfile resolution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockfileMetadata {
    /// Python version used for resolution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python_version: Option<String>,

    /// Platform used for resolution (e.g., "linux", "darwin", "win32")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,

    /// Resolution timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
}

/// A locked package with its exact version and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPackage {
    /// Exact version
    pub version: String,

    /// Download URL (default/universal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Hash for verification (format: "algorithm:hash")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,

    /// Direct dependencies of this package (normalized names)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,

    /// Platform markers (PEP 508 environment markers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markers: Option<String>,

    /// Platform-specific files (for packages with binary wheels)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<PlatformFile>,
}

/// Platform-specific file information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformFile {
    /// Download URL
    pub url: String,

    /// Hash for verification
    pub hash: String,

    /// Platform markers (e.g., "sys_platform == 'win32'")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markers: Option<String>,

    /// Python version constraint (e.g., ">=3.8")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python: Option<String>,

    /// Wheel tags (e.g., "cp311-cp311-manylinux_2_17_x86_64")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
}

impl Lockfile {
    /// Create a new empty lockfile
    pub fn new() -> Self {
        Self {
            version: LOCKFILE_VERSION.to_string(),
            metadata: None,
            packages: BTreeMap::new(),
        }
    }

    /// Create a lockfile from a resolution
    pub fn from_resolution(resolution: &Resolution) -> Self {
        let mut packages = BTreeMap::new();

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
                    files: pkg.files.iter().map(|f| PlatformFile {
                        url: f.url.clone(),
                        hash: f.hash.clone(),
                        markers: f.markers.clone(),
                        python: f.python.clone(),
                        tags: f.tags.clone(),
                    }).collect(),
                },
            );
        }

        // Add metadata
        let metadata = LockfileMetadata {
            python_version: None, // Could be filled by resolver
            platform: Some(std::env::consts::OS.to_string()),
            resolved_at: Some(chrono::Utc::now().to_rfc3339()),
        };

        Self {
            version: LOCKFILE_VERSION.to_string(),
            metadata: Some(metadata),
            packages,
        }
    }

    /// Load a lockfile from disk
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Io(e))?;
        Self::parse(&content)
    }

    /// Parse lockfile content
    pub fn parse(content: &str) -> Result<Self> {
        toml::from_str(content).map_err(Error::TomlParse)
    }

    /// Save lockfile to disk
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = self.to_string()?;
        std::fs::write(path, content).map_err(Error::Io)
    }

    /// Convert to TOML string
    pub fn to_string(&self) -> Result<String> {
        // Add header comment
        let mut output = String::new();
        output.push_str("# This file is automatically generated by T-Rex.\n");
        output.push_str("# Do not edit manually.\n\n");

        let toml = toml::to_string_pretty(self).map_err(Error::TomlSerialize)?;
        output.push_str(&toml);

        Ok(output)
    }

    /// Convert back to a Resolution for installation
    pub fn to_resolution(&self) -> Resolution {
        use crate::resolver::ResolvedFile;

        let packages = self
            .packages
            .iter()
            .map(|(name, pkg)| ResolvedPackage {
                name: name.clone(),
                version: pkg.version.clone(),
                url: pkg.url.clone().unwrap_or_default(),
                hash: pkg.hash.clone().unwrap_or_default(),
                dependencies: pkg.dependencies.clone(),
                markers: pkg.markers.clone(),
                files: pkg.files.iter().map(|f| ResolvedFile {
                    url: f.url.clone(),
                    hash: f.hash.clone(),
                    markers: f.markers.clone(),
                    python: f.python.clone(),
                    tags: f.tags.clone(),
                }).collect(),
            })
            .collect();

        Resolution { packages }
    }

    /// Get the dependency graph as adjacency list
    pub fn dependency_graph(&self) -> BTreeMap<String, Vec<String>> {
        self.packages
            .iter()
            .map(|(name, pkg)| (name.clone(), pkg.dependencies.clone()))
            .collect()
    }

    /// Get reverse dependencies (which packages depend on a given package)
    pub fn reverse_dependencies(&self, package: &str) -> Vec<String> {
        self.packages
            .iter()
            .filter(|(_, pkg)| pkg.dependencies.contains(&package.to_string()))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get a locked package by name
    pub fn get(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.get(name)
    }

    /// Check if a package is locked
    pub fn contains(&self, name: &str) -> bool {
        self.packages.contains_key(name)
    }

    /// Number of locked packages
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_lockfile() {
        let lockfile = Lockfile::new();
        assert_eq!(lockfile.version, LOCKFILE_VERSION);
        assert!(lockfile.is_empty());
    }

    #[test]
    fn test_parse_lockfile() {
        let content = r#"
version = "1"

[packages.requests]
version = "2.28.0"
url = "https://example.com/requests-2.28.0.whl"
hash = "sha256:abc123"

[packages.urllib3]
version = "1.26.0"
"#;

        let lockfile = Lockfile::parse(content).unwrap();
        assert_eq!(lockfile.len(), 2);
        assert!(lockfile.contains("requests"));
        assert!(lockfile.contains("urllib3"));

        let requests = lockfile.get("requests").unwrap();
        assert_eq!(requests.version, "2.28.0");
        assert_eq!(requests.hash, Some("sha256:abc123".to_string()));
    }

    #[test]
    fn test_round_trip() {
        let mut lockfile = Lockfile::new();
        lockfile.packages.insert(
            "mypackage".to_string(),
            LockedPackage {
                version: "1.0.0".to_string(),
                url: Some("https://example.com/pkg.whl".to_string()),
                hash: Some("sha256:abc".to_string()),
                dependencies: vec!["urllib3".to_string()],
                markers: None,
                files: vec![],
            },
        );

        let content = lockfile.to_string().unwrap();
        let parsed = Lockfile::parse(&content).unwrap();

        assert_eq!(parsed.len(), 1);
        let pkg = parsed.get("mypackage").unwrap();
        assert_eq!(pkg.version, "1.0.0");
        assert_eq!(pkg.dependencies, vec!["urllib3".to_string()]);
    }

    #[test]
    fn test_dependency_graph() {
        let mut lockfile = Lockfile::new();
        lockfile.packages.insert(
            "requests".to_string(),
            LockedPackage {
                version: "2.28.0".to_string(),
                url: None,
                hash: None,
                dependencies: vec!["urllib3".to_string(), "certifi".to_string()],
                markers: None,
                files: vec![],
            },
        );
        lockfile.packages.insert(
            "urllib3".to_string(),
            LockedPackage {
                version: "1.26.0".to_string(),
                url: None,
                hash: None,
                dependencies: vec![],
                markers: None,
                files: vec![],
            },
        );

        let graph = lockfile.dependency_graph();
        assert_eq!(graph.get("requests").unwrap().len(), 2);
        assert!(graph.get("requests").unwrap().contains(&"urllib3".to_string()));

        let reverse = lockfile.reverse_dependencies("urllib3");
        assert!(reverse.contains(&"requests".to_string()));
    }

    #[test]
    fn test_platform_files() {
        let mut lockfile = Lockfile::new();
        lockfile.packages.insert(
            "numpy".to_string(),
            LockedPackage {
                version: "1.24.0".to_string(),
                url: Some("https://example.com/numpy-universal.whl".to_string()),
                hash: Some("sha256:abc".to_string()),
                dependencies: vec![],
                markers: None,
                files: vec![
                    PlatformFile {
                        url: "https://example.com/numpy-win.whl".to_string(),
                        hash: "sha256:win".to_string(),
                        markers: Some("sys_platform == 'win32'".to_string()),
                        python: Some(">=3.8".to_string()),
                        tags: Some("cp311-cp311-win_amd64".to_string()),
                    },
                    PlatformFile {
                        url: "https://example.com/numpy-linux.whl".to_string(),
                        hash: "sha256:linux".to_string(),
                        markers: Some("sys_platform == 'linux'".to_string()),
                        python: Some(">=3.8".to_string()),
                        tags: Some("cp311-cp311-manylinux_2_17_x86_64".to_string()),
                    },
                ],
            },
        );

        let content = lockfile.to_string().unwrap();
        let parsed = Lockfile::parse(&content).unwrap();

        let numpy = parsed.get("numpy").unwrap();
        assert_eq!(numpy.files.len(), 2);
        assert!(numpy.files[0].markers.is_some());
    }
}
