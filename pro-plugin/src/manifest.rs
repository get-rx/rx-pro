//! Plugin manifest and configuration

use serde::{Deserialize, Serialize};

use crate::hooks::Hook;

/// Plugin manifest (plugin.toml or embedded in wasm)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin name
    pub name: String,

    /// Plugin version
    pub version: String,

    /// Plugin description
    #[serde(default)]
    pub description: String,

    /// Plugin author
    #[serde(default)]
    pub author: Option<String>,

    /// License
    #[serde(default)]
    pub license: Option<String>,

    /// Homepage/repository URL
    #[serde(default)]
    pub homepage: Option<String>,

    /// Minimum rx version required
    #[serde(default)]
    pub min_rx_version: Option<String>,

    /// Hooks this plugin implements
    #[serde(default)]
    pub hooks: Vec<String>,

    /// Permissions requested by the plugin
    #[serde(default)]
    pub permissions: PluginPermissions,
}

impl PluginManifest {
    /// Create a new manifest
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            description: String::new(),
            author: None,
            license: None,
            homepage: None,
            min_rx_version: None,
            hooks: Vec::new(),
            permissions: PluginPermissions::default(),
        }
    }

    /// Check if the plugin implements a specific hook
    pub fn has_hook(&self, hook: Hook) -> bool {
        self.hooks.contains(&hook.function_name().to_string())
    }

    /// Parse from TOML string
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    /// Serialize to TOML string
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}

/// Permissions that a plugin can request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginPermissions {
    /// Can read files in project directory
    #[serde(default)]
    pub read_files: bool,

    /// Can write files in project directory
    #[serde(default)]
    pub write_files: bool,

    /// Can make network requests
    #[serde(default)]
    pub network: bool,

    /// Can read environment variables
    #[serde(default)]
    pub env_vars: bool,

    /// Can execute shell commands
    #[serde(default)]
    pub execute: bool,

    /// Allowed file patterns for read access
    #[serde(default)]
    pub allowed_read_paths: Vec<String>,

    /// Allowed file patterns for write access
    #[serde(default)]
    pub allowed_write_paths: Vec<String>,

    /// Allowed network hosts
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
}

impl PluginPermissions {
    /// Create permissions with no access
    pub fn none() -> Self {
        Self::default()
    }

    /// Create permissions with read-only access
    pub fn read_only() -> Self {
        Self {
            read_files: true,
            ..Default::default()
        }
    }

    /// Create permissions with full file access
    pub fn full_file_access() -> Self {
        Self {
            read_files: true,
            write_files: true,
            ..Default::default()
        }
    }
}

/// Plugin configuration from pyproject.toml [tool.rx.plugins]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin source (local path or URL)
    pub source: String,

    /// Override permissions (grant more or deny)
    #[serde(default)]
    pub permissions: Option<PluginPermissions>,

    /// Plugin-specific settings
    #[serde(default)]
    pub settings: serde_json::Value,

    /// Whether the plugin is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl PluginConfig {
    /// Create a new plugin config from source
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            permissions: None,
            settings: serde_json::Value::Null,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_from_toml() {
        let toml = r#"
name = "my-plugin"
version = "1.0.0"
description = "A test plugin"
hooks = ["pre_build", "post_build"]

[permissions]
read_files = true
write_files = false
"#;

        let manifest = PluginManifest::from_toml(toml).unwrap();
        assert_eq!(manifest.name, "my-plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert!(manifest.has_hook(Hook::PreBuild));
        assert!(manifest.has_hook(Hook::PostBuild));
        assert!(!manifest.has_hook(Hook::PreResolve));
        assert!(manifest.permissions.read_files);
        assert!(!manifest.permissions.write_files);
    }

    #[test]
    fn test_manifest_to_toml() {
        let manifest = PluginManifest::new("test", "0.1.0");
        let toml = manifest.to_toml().unwrap();
        assert!(toml.contains("name = \"test\""));
    }
}
