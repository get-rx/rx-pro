//! Plugin host for loading and executing Wasm plugins

use std::path::Path;

use crate::{Hook, HookContext, HookResult, PluginError, PluginResult};

/// Plugin host that manages Wasm plugins
pub struct PluginHost {
    /// Loaded plugins
    plugins: Vec<LoadedPlugin>,
}

/// A loaded plugin
struct LoadedPlugin {
    /// Plugin name
    name: String,
    /// Plugin path
    #[allow(dead_code)]
    path: String,
    // TODO: Add extism::Plugin instance
}

impl PluginHost {
    /// Create a new plugin host
    pub fn new() -> Self {
        Self { plugins: vec![] }
    }

    /// Load a plugin from a Wasm file
    pub fn load(&mut self, name: &str, path: &Path) -> PluginResult<()> {
        // TODO: Implement actual Wasm loading with Extism
        // - Create manifest with capabilities
        // - Load and validate Wasm module
        // - Register available hooks

        tracing::info!("Loading plugin '{}' from {:?}", name, path);

        if !path.exists() {
            return Err(PluginError::NotFound {
                path: path.display().to_string(),
            });
        }

        self.plugins.push(LoadedPlugin {
            name: name.to_string(),
            path: path.display().to_string(),
        });

        Ok(())
    }

    /// Execute a hook on all plugins that implement it
    pub async fn execute_hook(&self, hook: Hook, _context: &HookContext) -> PluginResult<HookResult> {
        tracing::debug!("Executing hook {:?}", hook);

        let result = HookResult::default();

        for plugin in &self.plugins {
            tracing::trace!("Running hook on plugin '{}'", plugin.name);

            // TODO: Actually call the Wasm function
            // let output = plugin.instance.call(hook.function_name(), context.to_bytes())?;
            // result.merge(HookResult::from_bytes(&output)?);
        }

        Ok(result)
    }

    /// Get the number of loaded plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}
