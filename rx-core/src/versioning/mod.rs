//! Dynamic versioning from git tags
//!
//! Supports automatic version derivation from git tags, similar to
//! poetry-dynamic-versioning. Configuration via `[tool.rx.versioning]`.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Versioning configuration from `[tool.rx.versioning]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct VersioningConfig {
    /// Version source: "git-tag" or "pyproject" (default)
    #[serde(default = "default_source")]
    pub source: String,

    /// Tag pattern for extracting version (default: "v{version}")
    /// Supports: "v{version}", "{version}", "release-{version}"
    #[serde(default = "default_pattern")]
    pub pattern: String,

    /// Version style: "pep440" (default) or "semver"
    #[serde(default = "default_style")]
    pub style: String,

    /// Whether to include dev suffix for commits after tag
    #[serde(default = "default_true")]
    pub dev_suffix: bool,

    /// Whether to include commit hash in local version
    #[serde(default = "default_true")]
    pub commit_hash: bool,

    /// Fallback version if no git tag found
    #[serde(default = "default_fallback")]
    pub fallback: String,
}

fn default_source() -> String {
    "pyproject".to_string()
}

fn default_pattern() -> String {
    "v{version}".to_string()
}

fn default_style() -> String {
    "pep440".to_string()
}

fn default_true() -> bool {
    true
}

fn default_fallback() -> String {
    "0.0.0".to_string()
}

impl Default for VersioningConfig {
    fn default() -> Self {
        Self {
            source: default_source(),
            pattern: default_pattern(),
            style: default_style(),
            dev_suffix: default_true(),
            commit_hash: default_true(),
            fallback: default_fallback(),
        }
    }
}

/// Git version information
#[derive(Debug, Clone)]
pub struct GitVersion {
    /// The base version from the tag
    pub version: String,
    /// Number of commits since the tag
    pub distance: u32,
    /// Short commit hash
    pub commit: String,
    /// Whether the working tree is dirty
    pub dirty: bool,
    /// The full tag name
    pub tag: String,
}

/// Get the current version, either from git or pyproject.toml
pub fn get_version(project_dir: &Path, config: &VersioningConfig) -> Result<String> {
    match config.source.as_str() {
        "git-tag" | "git" => get_git_version(project_dir, config),
        "pyproject" | _ => {
            // Return None to indicate pyproject.toml should be used
            Err(Error::Version("Using pyproject.toml version".to_string()))
        }
    }
}

/// Get version from git tags
pub fn get_git_version(project_dir: &Path, config: &VersioningConfig) -> Result<String> {
    let git_info = get_git_info(project_dir, config)?;
    format_version(&git_info, config)
}

/// Get raw git version information
pub fn get_git_info(project_dir: &Path, config: &VersioningConfig) -> Result<GitVersion> {
    // Check if we're in a git repository
    let git_dir = project_dir.join(".git");
    if !git_dir.exists() {
        // Walk up to find .git
        let mut current = project_dir.to_path_buf();
        loop {
            if current.join(".git").exists() {
                break;
            }
            if !current.pop() {
                return Err(Error::Version("Not a git repository".to_string()));
            }
        }
    }

    // Get the most recent tag matching our pattern
    let describe_output = Command::new("git")
        .args(["describe", "--tags", "--long", "--match", &pattern_to_glob(&config.pattern)])
        .current_dir(project_dir)
        .output()
        .map_err(|e| Error::Version(format!("Failed to run git describe: {}", e)))?;

    if !describe_output.status.success() {
        // No tags found, use fallback
        return Ok(GitVersion {
            version: config.fallback.clone(),
            distance: get_commit_count(project_dir)?,
            commit: get_short_hash(project_dir)?,
            dirty: is_dirty(project_dir)?,
            tag: String::new(),
        });
    }

    let describe = String::from_utf8_lossy(&describe_output.stdout)
        .trim()
        .to_string();

    // Parse git describe output: "v1.2.3-5-gabc1234" or "v1.2.3-0-gabc1234"
    parse_git_describe(&describe, config)
}

/// Parse git describe output into GitVersion
fn parse_git_describe(describe: &str, config: &VersioningConfig) -> Result<GitVersion> {
    // Format: tag-distance-gcommit
    // Example: v1.2.3-5-gabc1234
    let parts: Vec<&str> = describe.rsplitn(3, '-').collect();

    if parts.len() < 3 {
        return Err(Error::Version(format!(
            "Invalid git describe output: {}",
            describe
        )));
    }

    let commit = parts[0].trim_start_matches('g').to_string();
    let distance: u32 = parts[1]
        .parse()
        .map_err(|_| Error::Version(format!("Invalid distance in: {}", describe)))?;
    let tag = parts[2].to_string();

    // Extract version from tag using pattern
    let version = extract_version_from_tag(&tag, &config.pattern)?;

    Ok(GitVersion {
        version,
        distance,
        commit,
        dirty: false, // Will be checked separately if needed
        tag,
    })
}

/// Extract version from tag using pattern
fn extract_version_from_tag(tag: &str, pattern: &str) -> Result<String> {
    // Convert pattern like "v{version}" to regex
    let regex_pattern = pattern
        .replace("{version}", r"(?P<version>.+)")
        .replace(".", r"\.");

    let re = regex::Regex::new(&format!("^{}$", regex_pattern))
        .map_err(|e| Error::Version(format!("Invalid pattern: {}", e)))?;

    if let Some(caps) = re.captures(tag) {
        if let Some(version) = caps.name("version") {
            return Ok(version.as_str().to_string());
        }
    }

    // Fallback: try simple prefix stripping
    if pattern.starts_with("v{version}") && tag.starts_with('v') {
        return Ok(tag[1..].to_string());
    }
    if pattern == "{version}" {
        return Ok(tag.to_string());
    }

    Err(Error::Version(format!(
        "Tag '{}' does not match pattern '{}'",
        tag, pattern
    )))
}

/// Convert pattern to git glob for matching
fn pattern_to_glob(pattern: &str) -> String {
    pattern.replace("{version}", "*")
}

/// Format version according to config
fn format_version(git: &GitVersion, config: &VersioningConfig) -> Result<String> {
    if git.distance == 0 && !git.dirty {
        // Exactly on a tag, return clean version
        return Ok(git.version.clone());
    }

    let mut version = git.version.clone();

    if config.dev_suffix && git.distance > 0 {
        // Add dev suffix: 1.2.3.dev5
        version = format!("{}.dev{}", version, git.distance);
    }

    if config.commit_hash && (git.distance > 0 || git.dirty) {
        // Add local version with commit hash: +gabc1234
        let dirty_suffix = if git.dirty { ".dirty" } else { "" };
        version = format!("{}+g{}{}", version, git.commit, dirty_suffix);
    } else if git.dirty {
        version = format!("{}+dirty", version);
    }

    Ok(version)
}

/// Get total commit count in repository
fn get_commit_count(project_dir: &Path) -> Result<u32> {
    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| Error::Version(format!("Failed to get commit count: {}", e)))?;

    if !output.status.success() {
        return Ok(0);
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .map_err(|_| Error::Version("Invalid commit count".to_string()))
}

/// Get short commit hash
fn get_short_hash(project_dir: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| Error::Version(format!("Failed to get commit hash: {}", e)))?;

    if !output.status.success() {
        return Ok("0000000".to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if working tree is dirty
fn is_dirty(project_dir: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| Error::Version(format!("Failed to check git status: {}", e)))?;

    Ok(!output.stdout.is_empty())
}

/// Bump a version string
pub fn bump_version(version: &str, part: &str) -> Result<String> {
    let parts: Vec<&str> = version.split('.').collect();

    if parts.len() < 3 {
        return Err(Error::Version(format!("Invalid version format: {}", version)));
    }

    let major: u32 = parts[0]
        .parse()
        .map_err(|_| Error::Version("Invalid major version".to_string()))?;
    let minor: u32 = parts[1]
        .parse()
        .map_err(|_| Error::Version("Invalid minor version".to_string()))?;
    // Handle versions like "1.2.3-alpha" or "1.2.3.dev1"
    let patch_str = parts[2].split(|c: char| !c.is_ascii_digit()).next().unwrap_or("0");
    let patch: u32 = patch_str
        .parse()
        .map_err(|_| Error::Version("Invalid patch version".to_string()))?;

    match part {
        "major" => Ok(format!("{}.0.0", major + 1)),
        "minor" => Ok(format!("{}.{}.0", major, minor + 1)),
        "patch" => Ok(format!("{}.{}.{}", major, minor, patch + 1)),
        "pre" | "prerelease" => {
            // Check if already has prerelease suffix
            if version.contains("-alpha") || version.contains("-beta") || version.contains("-rc") {
                // Increment the prerelease number
                if let Some(idx) = version.rfind(|c: char| c.is_ascii_digit()) {
                    let (prefix, num_str) = version.split_at(idx);
                    if let Some(first_digit) = num_str.chars().next() {
                        if first_digit.is_ascii_digit() {
                            let num: u32 = num_str.split(|c: char| !c.is_ascii_digit())
                                .next()
                                .unwrap_or("0")
                                .parse()
                                .unwrap_or(0);
                            return Ok(format!("{}{}", prefix, num + 1));
                        }
                    }
                }
            }
            // Start new alpha prerelease
            Ok(format!("{}.{}.{}-alpha.1", major, minor, patch + 1))
        }
        _ => Err(Error::Version(format!("Unknown version part: {}", part))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version_v_prefix() {
        let version = extract_version_from_tag("v1.2.3", "v{version}").unwrap();
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_extract_version_no_prefix() {
        let version = extract_version_from_tag("1.2.3", "{version}").unwrap();
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_extract_version_custom_prefix() {
        let version = extract_version_from_tag("release-1.2.3", "release-{version}").unwrap();
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_bump_major() {
        assert_eq!(bump_version("1.2.3", "major").unwrap(), "2.0.0");
    }

    #[test]
    fn test_bump_minor() {
        assert_eq!(bump_version("1.2.3", "minor").unwrap(), "1.3.0");
    }

    #[test]
    fn test_bump_patch() {
        assert_eq!(bump_version("1.2.3", "patch").unwrap(), "1.2.4");
    }

    #[test]
    fn test_format_version_on_tag() {
        let git = GitVersion {
            version: "1.2.3".to_string(),
            distance: 0,
            commit: "abc1234".to_string(),
            dirty: false,
            tag: "v1.2.3".to_string(),
        };
        let config = VersioningConfig::default();
        assert_eq!(format_version(&git, &config).unwrap(), "1.2.3");
    }

    #[test]
    fn test_format_version_after_tag() {
        let git = GitVersion {
            version: "1.2.3".to_string(),
            distance: 5,
            commit: "abc1234".to_string(),
            dirty: false,
            tag: "v1.2.3".to_string(),
        };
        let config = VersioningConfig::default();
        assert_eq!(
            format_version(&git, &config).unwrap(),
            "1.2.3.dev5+gabc1234"
        );
    }

    #[test]
    fn test_pattern_to_glob() {
        assert_eq!(pattern_to_glob("v{version}"), "v*");
        assert_eq!(pattern_to_glob("{version}"), "*");
        assert_eq!(pattern_to_glob("release-{version}"), "release-*");
    }
}
