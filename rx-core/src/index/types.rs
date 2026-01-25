//! PyPI JSON API response types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response from PyPI JSON API for a package
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackageMetadata {
    /// Package information
    pub info: PackageInfo,
    /// Available releases, keyed by version string
    pub releases: HashMap<String, Vec<FileInfo>>,
}

/// Package metadata from the info field
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PackageInfo {
    /// Package name
    pub name: String,
    /// Latest version
    pub version: String,
    /// Package summary/description
    pub summary: Option<String>,
    /// Project homepage
    pub home_page: Option<String>,
    /// Author name
    pub author: Option<String>,
    /// Author email
    pub author_email: Option<String>,
    /// License
    pub license: Option<String>,
    /// Requires Python version
    pub requires_python: Option<String>,
    /// Dependencies (requires_dist)
    pub requires_dist: Option<Vec<String>>,
    /// Project URLs
    pub project_urls: Option<HashMap<String, String>>,
}

/// Information about a release file
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileInfo {
    /// Filename (e.g., "requests-2.28.0-py3-none-any.whl")
    pub filename: String,
    /// Download URL
    pub url: String,
    /// File size in bytes
    pub size: Option<u64>,
    /// File digests (hashes)
    pub digests: FileDigests,
    /// Package type (sdist, bdist_wheel, etc.)
    pub packagetype: String,
    /// Python version requirement for this file
    pub requires_python: Option<String>,
    /// Whether the file has been yanked
    #[serde(default)]
    pub yanked: bool,
    /// Reason for yanking (if yanked)
    pub yanked_reason: Option<String>,
}

/// File digest/hash information
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FileDigests {
    /// MD5 hash
    pub md5: Option<String>,
    /// SHA256 hash
    pub sha256: Option<String>,
    /// Blake2b-256 hash
    pub blake2b_256: Option<String>,
}

impl FileInfo {
    /// Check if this is a wheel file
    pub fn is_wheel(&self) -> bool {
        self.filename.ends_with(".whl")
    }

    /// Check if this is a source distribution
    pub fn is_sdist(&self) -> bool {
        self.filename.ends_with(".tar.gz")
            || self.filename.ends_with(".zip")
            || self.packagetype == "sdist"
    }

    /// Get the best available hash (prefer SHA256)
    pub fn best_hash(&self) -> Option<(&'static str, &str)> {
        if let Some(ref sha256) = self.digests.sha256 {
            return Some(("sha256", sha256));
        }
        if let Some(ref blake2b) = self.digests.blake2b_256 {
            return Some(("blake2b_256", blake2b));
        }
        if let Some(ref md5) = self.digests.md5 {
            return Some(("md5", md5));
        }
        None
    }

    /// Parse wheel filename to extract compatibility info
    /// Format: {distribution}-{version}(-{build tag})?-{python tag}-{abi tag}-{platform tag}.whl
    pub fn parse_wheel_tags(&self) -> Option<WheelTags> {
        if !self.is_wheel() {
            return None;
        }

        let name = self.filename.strip_suffix(".whl")?;
        let parts: Vec<&str> = name.split('-').collect();

        // Minimum: name-version-python-abi-platform
        if parts.len() < 5 {
            return None;
        }

        // Work backwards from the end
        let platform = parts[parts.len() - 1].to_string();
        let abi = parts[parts.len() - 2].to_string();
        let python = parts[parts.len() - 3].to_string();

        Some(WheelTags {
            python,
            abi,
            platform,
        })
    }
}

/// Wheel compatibility tags
#[derive(Debug, Clone)]
pub struct WheelTags {
    /// Python version tag (e.g., "py3", "cp311")
    pub python: String,
    /// ABI tag (e.g., "none", "abi3", "cp311")
    pub abi: String,
    /// Platform tag (e.g., "any", "manylinux_2_17_x86_64")
    pub platform: String,
}

impl WheelTags {
    /// Check if this wheel is compatible with the given Python version
    pub fn is_python_compatible(&self, major: u8, minor: u8) -> bool {
        // Handle py2, py3, py2.py3
        if self.python.contains("py3") || self.python.contains("py2.py3") {
            return major == 3;
        }
        if self.python.contains("py2") {
            return major == 2;
        }

        // Handle cpXY
        if let Some(rest) = self.python.strip_prefix("cp") {
            if rest.len() >= 2 {
                let py_major: u8 = rest[0..1].parse().unwrap_or(0);
                let py_minor: u8 = rest[1..].parse().unwrap_or(0);
                return py_major == major && py_minor <= minor;
            }
        }

        // Unknown tag, assume compatible
        true
    }

    /// Check if this is a universal wheel (works on any platform)
    pub fn is_universal(&self) -> bool {
        self.platform == "any" && self.abi == "none"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_wheel() {
        let file = FileInfo {
            filename: "requests-2.28.0-py3-none-any.whl".to_string(),
            url: "".to_string(),
            size: None,
            digests: FileDigests::default(),
            packagetype: "bdist_wheel".to_string(),
            requires_python: None,
            yanked: false,
            yanked_reason: None,
        };
        assert!(file.is_wheel());
        assert!(!file.is_sdist());
    }

    #[test]
    fn test_parse_wheel_tags() {
        let file = FileInfo {
            filename: "requests-2.28.0-py3-none-any.whl".to_string(),
            url: "".to_string(),
            size: None,
            digests: FileDigests::default(),
            packagetype: "bdist_wheel".to_string(),
            requires_python: None,
            yanked: false,
            yanked_reason: None,
        };

        let tags = file.parse_wheel_tags().unwrap();
        assert_eq!(tags.python, "py3");
        assert_eq!(tags.abi, "none");
        assert_eq!(tags.platform, "any");
    }

    #[test]
    fn test_best_hash() {
        let file = FileInfo {
            filename: "test.whl".to_string(),
            url: "".to_string(),
            size: None,
            digests: FileDigests {
                md5: Some("abc".to_string()),
                sha256: Some("def".to_string()),
                blake2b_256: None,
            },
            packagetype: "bdist_wheel".to_string(),
            requires_python: None,
            yanked: false,
            yanked_reason: None,
        };

        let (algo, hash) = file.best_hash().unwrap();
        assert_eq!(algo, "sha256");
        assert_eq!(hash, "def");
    }
}
