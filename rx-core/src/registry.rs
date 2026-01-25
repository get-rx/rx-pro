//! Private registry configuration and authentication
//!
//! Supports multiple package registries with authentication:
//!
//! ```toml
//! # pyproject.toml
//! [[tool.rx.registries]]
//! name = "private"
//! url = "https://private.pypi.org/simple/"
//! username = "user"  # or use environment variable
//! password = "${PRIVATE_PYPI_TOKEN}"  # environment variable interpolation
//!
//! [[tool.rx.registries]]
//! name = "internal"
//! url = "https://internal.example.com/pypi/"
//! token = "${INTERNAL_TOKEN}"  # Bearer token auth
//! ```
//!
//! Or via ~/.rx/config.toml for global credentials:
//!
//! ```toml
//! [[registries]]
//! name = "private"
//! url = "https://private.pypi.org/simple/"
//! username = "user"
//! password = "secret"
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Default PyPI registry
pub const PYPI_URL: &str = "https://pypi.org/simple/";
pub const PYPI_API_URL: &str = "https://pypi.org/pypi";

/// Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Registry name (for reference)
    pub name: String,

    /// Registry URL (Simple API endpoint)
    pub url: String,

    /// Username for basic auth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Password for basic auth (supports ${ENV_VAR} interpolation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Bearer token (supports ${ENV_VAR} interpolation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Whether this is the default registry for publishing
    #[serde(default)]
    pub default: bool,

    /// Priority (lower = higher priority, default = 100)
    #[serde(default = "default_priority")]
    pub priority: u32,
}

fn default_priority() -> u32 {
    100
}

impl RegistryConfig {
    /// Create a new registry config
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            username: None,
            password: None,
            token: None,
            default: false,
            priority: default_priority(),
        }
    }

    /// Create the default PyPI registry
    pub fn pypi() -> Self {
        Self {
            name: "pypi".to_string(),
            url: PYPI_URL.to_string(),
            username: None,
            password: None,
            token: None,
            default: true,
            priority: 1000, // Lowest priority (fallback)
        }
    }

    /// Set basic auth credentials
    pub fn with_basic_auth(mut self, username: String, password: String) -> Self {
        self.username = Some(username);
        self.password = Some(password);
        self
    }

    /// Set bearer token auth
    pub fn with_token(mut self, token: String) -> Self {
        self.token = Some(token);
        self
    }

    /// Resolve environment variables in credentials
    pub fn resolve_credentials(&self) -> Result<ResolvedCredentials> {
        let username = self.username.as_ref().map(|u| resolve_env_var(u));
        let password = self.password.as_ref().map(|p| resolve_env_var(p));
        let token = self.token.as_ref().map(|t| resolve_env_var(t));

        Ok(ResolvedCredentials {
            username,
            password,
            token,
        })
    }

    /// Check if authentication is configured
    pub fn has_auth(&self) -> bool {
        self.username.is_some() || self.token.is_some()
    }

    /// Get the JSON API URL (converts simple URL to JSON API)
    pub fn api_url(&self) -> String {
        // Convert simple API URL to JSON API URL
        // https://private.pypi.org/simple/ -> https://private.pypi.org/pypi
        if self.url.ends_with("/simple/") || self.url.ends_with("/simple") {
            let base = self.url.trim_end_matches('/').trim_end_matches("simple");
            format!("{}pypi", base)
        } else {
            self.url.clone()
        }
    }
}

/// Resolved credentials with environment variables expanded
#[derive(Debug, Clone)]
pub struct ResolvedCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
}

impl ResolvedCredentials {
    /// Check if any credentials are available
    pub fn has_credentials(&self) -> bool {
        self.username.is_some() || self.token.is_some()
    }
}

/// Registry manager for handling multiple registries
#[derive(Debug, Clone, Default)]
pub struct RegistryManager {
    /// Configured registries
    registries: Vec<RegistryConfig>,
}

impl RegistryManager {
    /// Create a new registry manager with default PyPI
    pub fn new() -> Self {
        Self {
            registries: vec![RegistryConfig::pypi()],
        }
    }

    /// Create from a list of registry configs
    pub fn from_configs(mut configs: Vec<RegistryConfig>) -> Self {
        // Sort by priority
        configs.sort_by_key(|r| r.priority);

        // Add PyPI as fallback if not present
        if !configs.iter().any(|r| r.name == "pypi") {
            configs.push(RegistryConfig::pypi());
        }

        Self { registries: configs }
    }

    /// Load registry configuration from pyproject.toml and global config
    pub fn load(project_dir: &Path) -> Result<Self> {
        let mut configs = Vec::new();

        // Load from global config (~/.rx/config.toml)
        if let Some(global_configs) = load_global_config()? {
            configs.extend(global_configs);
        }

        // Load from pyproject.toml [tool.rx.registries]
        if let Some(project_configs) = load_project_config(project_dir)? {
            // Project configs override global
            for config in project_configs {
                // Remove any existing config with same name
                configs.retain(|c| c.name != config.name);
                configs.push(config);
            }
        }

        Ok(Self::from_configs(configs))
    }

    /// Add a registry
    pub fn add(&mut self, config: RegistryConfig) {
        // Remove existing with same name
        self.registries.retain(|r| r.name != config.name);
        self.registries.push(config);
        self.registries.sort_by_key(|r| r.priority);
    }

    /// Get all registries (sorted by priority)
    pub fn registries(&self) -> &[RegistryConfig] {
        &self.registries
    }

    /// Get a registry by name
    pub fn get(&self, name: &str) -> Option<&RegistryConfig> {
        self.registries.iter().find(|r| r.name == name)
    }

    /// Get the default registry for publishing
    pub fn default_registry(&self) -> Option<&RegistryConfig> {
        self.registries.iter().find(|r| r.default)
    }

    /// Get the primary registry (highest priority)
    pub fn primary(&self) -> Option<&RegistryConfig> {
        self.registries.first()
    }
}

/// Load global registry config from ~/.rx/config.toml
fn load_global_config() -> Result<Option<Vec<RegistryConfig>>> {
    let config_path = dirs::home_dir()
        .map(|h| h.join(".rx").join("config.toml"))
        .unwrap_or_else(|| PathBuf::from(".rx/config.toml"));

    if !config_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path).map_err(Error::Io)?;

    #[derive(Deserialize)]
    struct GlobalConfig {
        #[serde(default)]
        registries: Vec<RegistryConfig>,
    }

    let config: GlobalConfig = toml::from_str(&content).map_err(Error::TomlParse)?;
    Ok(Some(config.registries))
}

/// Load registry config from pyproject.toml
fn load_project_config(project_dir: &Path) -> Result<Option<Vec<RegistryConfig>>> {
    let pyproject_path = project_dir.join("pyproject.toml");
    if !pyproject_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&pyproject_path).map_err(Error::Io)?;
    let doc: toml::Value = toml::from_str(&content).map_err(Error::TomlParse)?;

    let registries = doc
        .get("tool")
        .and_then(|t| t.get("rx"))
        .and_then(|r| r.get("registries"))
        .and_then(|r| r.as_array());

    match registries {
        Some(arr) => {
            let configs: Vec<RegistryConfig> = arr
                .iter()
                .filter_map(|v| {
                    let s = toml::to_string(v).ok()?;
                    toml::from_str(&s).ok()
                })
                .collect();
            Ok(Some(configs))
        }
        None => Ok(None),
    }
}

/// Resolve environment variable references in a string
/// Supports ${VAR} and $VAR syntax
fn resolve_env_var(value: &str) -> String {
    let mut result = value.to_string();

    // Handle ${VAR} syntax
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let replacement = std::env::var(var_name).unwrap_or_default();
            result = format!("{}{}{}", &result[..start], replacement, &result[start + end + 1..]);
        } else {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_config() {
        let config = RegistryConfig::new("private", "https://private.pypi.org/simple/")
            .with_basic_auth("user".to_string(), "pass".to_string());

        assert_eq!(config.name, "private");
        assert!(config.has_auth());
        assert_eq!(config.api_url(), "https://private.pypi.org/pypi");
    }

    #[test]
    fn test_resolve_env_var() {
        std::env::set_var("TEST_VAR", "test_value");
        assert_eq!(resolve_env_var("${TEST_VAR}"), "test_value");
        assert_eq!(resolve_env_var("prefix_${TEST_VAR}_suffix"), "prefix_test_value_suffix");
        std::env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_registry_manager() {
        let mut manager = RegistryManager::new();
        assert_eq!(manager.registries().len(), 1);
        assert_eq!(manager.primary().unwrap().name, "pypi");

        manager.add(RegistryConfig::new("private", "https://private.pypi.org/simple/"));
        assert!(manager.get("private").is_some());
    }
}
