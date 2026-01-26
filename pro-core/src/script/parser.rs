//! PEP 723 inline script metadata parser
//!
//! Parses inline dependency specifications from Python scripts following
//! PEP 723: https://peps.python.org/pep-0723/
//!
//! Example script metadata:
//! ```python
//! # /// script
//! # requires-python = ">=3.11"
//! # dependencies = [
//! #   "requests",
//! #   "rich>=10.0",
//! # ]
//! # ///
//! ```

use crate::{Error, Result};

/// Metadata extracted from a PEP 723 script
#[derive(Debug, Clone, Default)]
pub struct ScriptMetadata {
    /// Required Python version (PEP 440 specifier)
    pub requires_python: Option<String>,
    /// Package dependencies
    pub dependencies: Vec<String>,
}

impl ScriptMetadata {
    /// Check if the script has any metadata
    pub fn is_empty(&self) -> bool {
        self.requires_python.is_none() && self.dependencies.is_empty()
    }

    /// Check if the script has dependencies
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }

    /// Generate a hash of the dependencies for caching
    pub fn dependency_hash(&self) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();

        // Include requires-python in hash
        if let Some(ref req) = self.requires_python {
            hasher.update(req.as_bytes());
        }

        // Include sorted dependencies
        let mut deps = self.dependencies.clone();
        deps.sort();
        for dep in deps {
            hasher.update(dep.as_bytes());
        }

        let result = hasher.finalize();
        hex::encode(&result[..8]) // Use first 8 bytes (16 hex chars)
    }
}

/// Parse PEP 723 inline script metadata from script content
///
/// Looks for a block of the form:
/// ```
/// # /// script
/// # key = "value"
/// # ///
/// ```
pub fn parse_script_metadata(content: &str) -> Result<ScriptMetadata> {
    // Find the script metadata block
    let block = extract_metadata_block(content)?;

    if block.is_none() {
        return Ok(ScriptMetadata::default());
    }

    let block = block.unwrap();

    // Parse as TOML
    let toml_content = block.join("\n");
    parse_metadata_toml(&toml_content)
}

/// Extract the metadata block lines from script content
fn extract_metadata_block(content: &str) -> Result<Option<Vec<String>>> {
    let mut in_block = false;
    let mut block_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for block start
        if !in_block {
            if trimmed == "# /// script" {
                in_block = true;
                continue;
            }
            // Also handle variations without space
            if trimmed == "#/// script" {
                in_block = true;
                continue;
            }
        } else {
            // Check for block end
            if trimmed == "# ///" || trimmed == "#///" {
                return Ok(Some(block_lines));
            }

            // Extract the content after "# "
            if let Some(content) = trimmed.strip_prefix("# ") {
                block_lines.push(content.to_string());
            } else if let Some(content) = trimmed.strip_prefix("#") {
                // Handle lines without space after #
                block_lines.push(content.to_string());
            } else if trimmed.is_empty() {
                // Preserve empty lines within the block
                block_lines.push(String::new());
            } else {
                // Non-comment line inside block is an error
                return Err(Error::ScriptMetadataError(format!(
                    "unexpected non-comment line in script block: {}",
                    line
                )));
            }
        }
    }

    // If we're still in the block, it wasn't closed
    if in_block {
        return Err(Error::ScriptMetadataError(
            "script metadata block not closed (missing # ///)".into(),
        ));
    }

    Ok(None)
}

/// Parse the extracted metadata as TOML
fn parse_metadata_toml(content: &str) -> Result<ScriptMetadata> {
    let table: toml::Table = toml::from_str(content).map_err(|e| {
        Error::ScriptMetadataError(format!("invalid TOML in script metadata: {}", e))
    })?;

    let mut metadata = ScriptMetadata::default();

    // Extract requires-python
    if let Some(value) = table.get("requires-python") {
        metadata.requires_python = value
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::ScriptMetadataError("requires-python must be a string".into()))?
            .into();
    }

    // Extract dependencies
    if let Some(value) = table.get("dependencies") {
        let deps = value.as_array().ok_or_else(|| {
            Error::ScriptMetadataError("dependencies must be an array".into())
        })?;

        for dep in deps {
            let dep_str = dep.as_str().ok_or_else(|| {
                Error::ScriptMetadataError("dependency must be a string".into())
            })?;
            metadata.dependencies.push(dep_str.to_string());
        }
    }

    Ok(metadata)
}

/// Check if a file looks like it might have PEP 723 metadata
///
/// This is a quick check that doesn't fully parse the metadata.
pub fn might_have_metadata(content: &str) -> bool {
    content.contains("# /// script") || content.contains("#/// script")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_metadata() {
        let content = r#"#!/usr/bin/env python
# /// script
# requires-python = ">=3.11"
# dependencies = ["requests"]
# ///

import requests
print("Hello")
"#;

        let metadata = parse_script_metadata(content).unwrap();
        assert_eq!(metadata.requires_python, Some(">=3.11".to_string()));
        assert_eq!(metadata.dependencies, vec!["requests"]);
    }

    #[test]
    fn test_parse_multiple_dependencies() {
        let content = r#"# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "requests>=2.28",
#   "rich",
#   "click>=8.0",
# ]
# ///
"#;

        let metadata = parse_script_metadata(content).unwrap();
        assert_eq!(metadata.requires_python, Some(">=3.10".to_string()));
        assert_eq!(
            metadata.dependencies,
            vec!["requests>=2.28", "rich", "click>=8.0"]
        );
    }

    #[test]
    fn test_no_metadata() {
        let content = r#"#!/usr/bin/env python
import sys
print(sys.version)
"#;

        let metadata = parse_script_metadata(content).unwrap();
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_unclosed_block() {
        let content = r#"# /// script
# dependencies = ["requests"]
import requests
"#;

        let result = parse_script_metadata(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_dependency_only() {
        let content = r#"# /// script
# dependencies = ["numpy", "pandas"]
# ///
import numpy
"#;

        let metadata = parse_script_metadata(content).unwrap();
        assert!(metadata.requires_python.is_none());
        assert_eq!(metadata.dependencies, vec!["numpy", "pandas"]);
    }

    #[test]
    fn test_might_have_metadata() {
        assert!(might_have_metadata("# /// script\n# ///"));
        assert!(might_have_metadata("#/// script"));
        assert!(!might_have_metadata("import sys"));
    }

    #[test]
    fn test_dependency_hash() {
        let meta1 = ScriptMetadata {
            requires_python: Some(">=3.11".to_string()),
            dependencies: vec!["requests".to_string(), "rich".to_string()],
        };

        let meta2 = ScriptMetadata {
            requires_python: Some(">=3.11".to_string()),
            dependencies: vec!["rich".to_string(), "requests".to_string()], // Different order
        };

        // Same deps (sorted) should produce same hash
        assert_eq!(meta1.dependency_hash(), meta2.dependency_hash());

        // Different deps should produce different hash
        let meta3 = ScriptMetadata {
            requires_python: Some(">=3.11".to_string()),
            dependencies: vec!["requests".to_string()],
        };
        assert_ne!(meta1.dependency_hash(), meta3.dependency_hash());
    }
}
