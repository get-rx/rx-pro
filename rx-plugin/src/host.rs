//! Plugin host for loading and executing Wasm plugins

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use extism::{Manifest, Plugin, Wasm};

use crate::manifest::{PluginConfig, PluginManifest, PluginPermissions};
use crate::{Hook, HookContext, HookResult, PluginError, PluginResult};

/// Plugin host that manages Wasm plugins
pub struct PluginHost {
    /// Loaded plugins
    plugins: Vec<LoadedPlugin>,
    /// Plugin directory
    plugin_dir: PathBuf,
    /// Default permissions for plugins
    default_permissions: PluginPermissions,
}

/// A loaded plugin
pub struct LoadedPlugin {
    /// Plugin name
    pub name: String,
    /// Plugin manifest
    pub manifest: PluginManifest,
    /// Plugin config (from pyproject.toml)
    pub config: Option<PluginConfig>,
    /// Extism plugin instance
    instance: Plugin,
    /// Whether the plugin is enabled
    pub enabled: bool,
}

impl LoadedPlugin {
    /// Check if this plugin implements a hook
    pub fn has_hook(&self, hook: Hook) -> bool {
        self.manifest.has_hook(hook)
    }

    /// Call a hook function
    pub fn call_hook(&mut self, hook: Hook, context: &HookContext) -> PluginResult<HookResult> {
        let func_name = hook.function_name();

        // Check if plugin implements this hook
        if !self.has_hook(hook) {
            return Ok(HookResult::ok());
        }

        // Serialize context
        let input = context.to_bytes();

        // Call the Wasm function
        let output = self
            .instance
            .call::<&[u8], Vec<u8>>(func_name, &input)
            .map_err(|e| PluginError::ExecutionError(e.to_string()))?;

        // Deserialize result
        let result = HookResult::from_bytes(&output)?;
        Ok(result)
    }
}

impl PluginHost {
    /// Create a new plugin host
    pub fn new(plugin_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins: vec![],
            plugin_dir: plugin_dir.into(),
            default_permissions: PluginPermissions::read_only(),
        }
    }

    /// Create with default plugin directory (~/.rx/plugins)
    pub fn with_default_dir() -> Self {
        let plugin_dir = dirs::home_dir()
            .map(|h| h.join(".rx").join("plugins"))
            .unwrap_or_else(|| PathBuf::from(".rx/plugins"));

        Self::new(plugin_dir)
    }

    /// Set default permissions for plugins
    pub fn set_default_permissions(&mut self, permissions: PluginPermissions) {
        self.default_permissions = permissions;
    }

    /// Get the plugin directory
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// Ensure plugin directory exists
    pub fn ensure_plugin_dir(&self) -> PluginResult<()> {
        std::fs::create_dir_all(&self.plugin_dir)
            .map_err(|e| PluginError::LoadError(format!("Failed to create plugin directory: {}", e)))
    }

    /// Load a plugin from a Wasm file
    pub fn load(&mut self, name: &str, wasm_path: &Path) -> PluginResult<()> {
        self.load_with_config(name, wasm_path, None)
    }

    /// Load a plugin with specific configuration
    pub fn load_with_config(
        &mut self,
        name: &str,
        wasm_path: &Path,
        config: Option<PluginConfig>,
    ) -> PluginResult<()> {
        if !wasm_path.exists() {
            return Err(PluginError::NotFound {
                path: wasm_path.display().to_string(),
            });
        }

        tracing::info!("Loading plugin '{}' from {:?}", name, wasm_path);

        // Read the Wasm file
        let wasm_bytes = std::fs::read(wasm_path)
            .map_err(|e| PluginError::LoadError(format!("Failed to read Wasm file: {}", e)))?;

        // Try to extract manifest from custom section or use default
        let manifest = self.extract_or_create_manifest(name, wasm_path, &wasm_bytes)?;

        // Determine permissions
        let permissions = config
            .as_ref()
            .and_then(|c| c.permissions.clone())
            .unwrap_or_else(|| manifest.permissions.clone());

        // Create Extism manifest with permissions
        let extism_manifest = self.create_extism_manifest(&wasm_bytes, &permissions)?;

        // Create the plugin instance
        let instance = Plugin::new(&extism_manifest, [], true)
            .map_err(|e| PluginError::LoadError(format!("Failed to create plugin: {}", e)))?;

        let enabled = config.as_ref().map(|c| c.enabled).unwrap_or(true);

        self.plugins.push(LoadedPlugin {
            name: name.to_string(),
            manifest,
            config,
            instance,
            enabled,
        });

        tracing::info!("Successfully loaded plugin '{}'", name);
        Ok(())
    }

    /// Extract manifest from Wasm or create a default one
    fn extract_or_create_manifest(
        &self,
        name: &str,
        wasm_path: &Path,
        _wasm_bytes: &[u8],
    ) -> PluginResult<PluginManifest> {
        // First, try to load manifest from adjacent .toml file
        let manifest_path = wasm_path.with_extension("toml");
        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)
                .map_err(|e| PluginError::InvalidManifest(format!("Failed to read manifest: {}", e)))?;
            return PluginManifest::from_toml(&content)
                .map_err(|e| PluginError::InvalidManifest(format!("Invalid manifest TOML: {}", e)));
        }

        // TODO: Extract from Wasm custom section "rx_manifest"
        // For now, create a default manifest
        Ok(PluginManifest {
            name: name.to_string(),
            version: "0.0.0".to_string(),
            description: String::new(),
            author: None,
            license: None,
            homepage: None,
            min_rx_version: None,
            hooks: vec![
                "pre_resolve".to_string(),
                "post_resolve".to_string(),
                "pre_build".to_string(),
                "post_build".to_string(),
                "pre_publish".to_string(),
            ],
            permissions: self.default_permissions.clone(),
        })
    }

    /// Create Extism manifest with appropriate permissions
    fn create_extism_manifest(
        &self,
        wasm_bytes: &[u8],
        permissions: &PluginPermissions,
    ) -> PluginResult<Manifest> {
        let wasm = Wasm::data(wasm_bytes.to_vec());
        let mut manifest = Manifest::new([wasm]);

        // Configure allowed hosts for network access
        if permissions.network && !permissions.allowed_hosts.is_empty() {
            manifest = manifest.with_allowed_hosts(
                permissions.allowed_hosts.iter().cloned(),
            );
        }

        // Note: File system access is handled by host functions, not Extism directly
        // We'll need to implement custom host functions for file I/O

        Ok(manifest)
    }

    /// Load all plugins from a directory
    pub fn load_from_dir(&mut self, dir: &Path) -> PluginResult<usize> {
        if !dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        for entry in std::fs::read_dir(dir)
            .map_err(|e| PluginError::LoadError(format!("Failed to read plugin directory: {}", e)))?
        {
            let entry = entry
                .map_err(|e| PluginError::LoadError(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "wasm") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");

                match self.load(name, &path) {
                    Ok(_) => count += 1,
                    Err(e) => {
                        tracing::warn!("Failed to load plugin {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(count)
    }

    /// Load plugins from pyproject.toml configuration
    pub fn load_from_config(&mut self, configs: &HashMap<String, PluginConfig>) -> PluginResult<usize> {
        let mut count = 0;

        for (name, config) in configs {
            if !config.enabled {
                tracing::debug!("Skipping disabled plugin '{}'", name);
                continue;
            }

            let path = if config.source.starts_with("http://") || config.source.starts_with("https://") {
                // Download from URL
                match self.download_plugin(name, &config.source) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!("Failed to download plugin '{}': {}", name, e);
                        continue;
                    }
                }
            } else {
                // Local path
                PathBuf::from(&config.source)
            };

            match self.load_with_config(name, &path, Some(config.clone())) {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::warn!("Failed to load plugin '{}': {}", name, e);
                }
            }
        }

        Ok(count)
    }

    /// Download a plugin from URL
    fn download_plugin(&self, name: &str, url: &str) -> PluginResult<PathBuf> {
        self.ensure_plugin_dir()?;

        let dest_path = self.plugin_dir.join(format!("{}.wasm", name));

        // Use blocking reqwest for simplicity (this should be async in production)
        let response = reqwest::blocking::get(url)
            .map_err(|e| PluginError::LoadError(format!("Failed to download plugin: {}", e)))?;

        if !response.status().is_success() {
            return Err(PluginError::LoadError(format!(
                "Failed to download plugin: HTTP {}",
                response.status()
            )));
        }

        let bytes = response.bytes()
            .map_err(|e| PluginError::LoadError(format!("Failed to read response: {}", e)))?;

        std::fs::write(&dest_path, &bytes)
            .map_err(|e| PluginError::LoadError(format!("Failed to save plugin: {}", e)))?;

        Ok(dest_path)
    }

    /// Execute a hook on all enabled plugins that implement it
    pub fn execute_hook(&mut self, hook: Hook, context: &HookContext) -> PluginResult<HookResult> {
        tracing::debug!("Executing hook {:?}", hook);

        let mut combined_result = HookResult::ok();

        for plugin in &mut self.plugins {
            if !plugin.enabled {
                continue;
            }

            if !plugin.has_hook(hook) {
                continue;
            }

            tracing::trace!("Running hook {:?} on plugin '{}'", hook, plugin.name);

            match plugin.call_hook(hook, context) {
                Ok(result) => {
                    // Print any messages
                    for msg in &result.messages {
                        println!("[{}] {}", plugin.name, msg);
                    }

                    combined_result.merge(result);

                    // Stop if plugin requested to halt
                    if !combined_result.continue_operation {
                        tracing::info!("Plugin '{}' stopped operation at {:?}", plugin.name, hook);
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Plugin '{}' hook {:?} failed: {}", plugin.name, hook, e);
                    // Continue with other plugins unless it's a critical error
                }
            }
        }

        Ok(combined_result)
    }

    /// Get the number of loaded plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get the number of enabled plugins
    pub fn enabled_count(&self) -> usize {
        self.plugins.iter().filter(|p| p.enabled).count()
    }

    /// List all loaded plugins
    pub fn list_plugins(&self) -> Vec<&LoadedPlugin> {
        self.plugins.iter().collect()
    }

    /// Get a plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<&LoadedPlugin> {
        self.plugins.iter().find(|p| p.name == name)
    }

    /// Remove a plugin by name
    pub fn remove_plugin(&mut self, name: &str) -> bool {
        let len_before = self.plugins.len();
        self.plugins.retain(|p| p.name != name);
        self.plugins.len() < len_before
    }

    /// Enable a plugin
    pub fn enable_plugin(&mut self, name: &str) -> bool {
        if let Some(plugin) = self.plugins.iter_mut().find(|p| p.name == name) {
            plugin.enabled = true;
            true
        } else {
            false
        }
    }

    /// Disable a plugin
    pub fn disable_plugin(&mut self, name: &str) -> bool {
        if let Some(plugin) = self.plugins.iter_mut().find(|p| p.name == name) {
            plugin.enabled = false;
            true
        } else {
            false
        }
    }
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::with_default_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_host_creation() {
        let host = PluginHost::with_default_dir();
        assert_eq!(host.plugin_count(), 0);
    }

    #[test]
    fn test_load_nonexistent_plugin() {
        let mut host = PluginHost::with_default_dir();
        let result = host.load("test", Path::new("/nonexistent/plugin.wasm"));
        assert!(result.is_err());
    }
}
