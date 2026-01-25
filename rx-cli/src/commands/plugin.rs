//! Plugin command - manage WebAssembly plugins
//!
//! Plugins extend rx functionality with custom hooks:
//!
//! ```bash
//! # List installed plugins
//! rx plugin list
//!
//! # Add a plugin from local file
//! rx plugin add my-plugin ./plugins/my-plugin.wasm
//!
//! # Add a plugin from URL
//! rx plugin add linter https://example.com/linter.wasm
//!
//! # Remove a plugin
//! rx plugin remove my-plugin
//!
//! # Enable/disable a plugin
//! rx plugin enable my-plugin
//! rx plugin disable my-plugin
//!
//! # Show plugin info
//! rx plugin info my-plugin
//!
//! # Run a specific hook manually
//! rx plugin run pre-build
//! ```
//!
//! Configuration in pyproject.toml:
//! ```toml
//! [tool.rx.plugins.my-plugin]
//! source = "./plugins/my-plugin.wasm"
//! enabled = true
//!
//! [tool.rx.plugins.my-plugin.settings]
//! verbose = true
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};

use rx_core::pep::PyProject;
use rx_plugin::{Hook, HookContext, PluginConfig, PluginHost};

#[derive(Args)]
pub struct PluginCommand {
    #[command(subcommand)]
    pub command: PluginSubcommand,
}

#[derive(Subcommand)]
pub enum PluginSubcommand {
    /// List installed plugins
    List(PluginListCommand),

    /// Add a plugin
    Add(PluginAddCommand),

    /// Remove a plugin
    Remove(PluginRemoveCommand),

    /// Show plugin information
    Info(PluginInfoCommand),

    /// Enable a disabled plugin
    Enable(PluginEnableCommand),

    /// Disable a plugin
    Disable(PluginDisableCommand),

    /// Run a hook manually
    Run(PluginRunCommand),

    /// Initialize plugin development template
    Init(PluginInitCommand),
}

impl PluginCommand {
    pub async fn run(self) -> Result<()> {
        match self.command {
            PluginSubcommand::List(cmd) => cmd.run().await,
            PluginSubcommand::Add(cmd) => cmd.run().await,
            PluginSubcommand::Remove(cmd) => cmd.run().await,
            PluginSubcommand::Info(cmd) => cmd.run().await,
            PluginSubcommand::Enable(cmd) => cmd.run().await,
            PluginSubcommand::Disable(cmd) => cmd.run().await,
            PluginSubcommand::Run(cmd) => cmd.run().await,
            PluginSubcommand::Init(cmd) => cmd.run().await,
        }
    }
}

// ============================================================================
// List Command
// ============================================================================

#[derive(Args)]
pub struct PluginListCommand {
    /// Project directory
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Show global plugins only
    #[arg(long)]
    pub global: bool,

    /// Show detailed information
    #[arg(long, short)]
    pub verbose: bool,
}

impl PluginListCommand {
    pub async fn run(self) -> Result<()> {
        let mut host = PluginHost::with_default_dir();

        // Load global plugins
        let plugin_dir = host.plugin_dir().to_path_buf();
        let _global_count = host.load_from_dir(&plugin_dir).unwrap_or(0);

        // Load project plugins if not global-only
        if !self.global {
            let project_dir = if self.project.as_os_str() == "." {
                std::env::current_dir()?
            } else {
                self.project.canonicalize()?
            };

            let configs = load_plugin_configs(&project_dir);
            host.load_from_config(&configs).ok();
        }

        let plugins = host.list_plugins();

        if plugins.is_empty() {
            println!("No plugins installed.");
            println!();
            println!("Add a plugin with:");
            println!("  rx plugin add <name> <path-or-url>");
            return Ok(());
        }

        println!("Installed plugins ({}):", plugins.len());
        println!();

        for plugin in plugins {
            let status = if plugin.enabled {
                "enabled"
            } else {
                "disabled"
            };

            if self.verbose {
                println!("  {} ({})", plugin.name, status);
                println!("    Version: {}", plugin.manifest.version);
                if !plugin.manifest.description.is_empty() {
                    println!("    Description: {}", plugin.manifest.description);
                }
                if let Some(ref author) = plugin.manifest.author {
                    println!("    Author: {}", author);
                }
                println!("    Hooks: {}", plugin.manifest.hooks.join(", "));
                println!();
            } else {
                let hooks_count = plugin.manifest.hooks.len();
                println!(
                    "  {} v{} ({}, {} hooks)",
                    plugin.name, plugin.manifest.version, status, hooks_count
                );
            }
        }

        if !self.verbose {
            println!();
            println!("Use --verbose for more details.");
        }

        Ok(())
    }
}

// ============================================================================
// Add Command
// ============================================================================

#[derive(Args)]
pub struct PluginAddCommand {
    /// Plugin name
    pub name: String,

    /// Plugin source (local path or URL)
    pub source: String,

    /// Install globally (to ~/.rx/plugins)
    #[arg(long)]
    pub global: bool,

    /// Project directory (for local install)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

impl PluginAddCommand {
    pub async fn run(self) -> Result<()> {
        if self.global {
            self.add_global().await
        } else {
            self.add_project().await
        }
    }

    async fn add_global(&self) -> Result<()> {
        let mut host = PluginHost::with_default_dir();
        host.ensure_plugin_dir()?;

        let plugin_path =
            if self.source.starts_with("http://") || self.source.starts_with("https://") {
                // Download
                println!("Downloading plugin from {}...", self.source);
                let dest = host.plugin_dir().join(format!("{}.wasm", self.name));

                let response = reqwest::get(&self.source)
                    .await
                    .context("Failed to download plugin")?;

                if !response.status().is_success() {
                    bail!("Failed to download plugin: HTTP {}", response.status());
                }

                let bytes = response.bytes().await?;
                std::fs::write(&dest, &bytes)?;
                dest
            } else {
                // Copy local file
                let source_path = PathBuf::from(&self.source);
                if !source_path.exists() {
                    bail!("Plugin file not found: {}", self.source);
                }

                let dest = host.plugin_dir().join(format!("{}.wasm", self.name));
                std::fs::copy(&source_path, &dest)?;

                // Also copy manifest if it exists
                let manifest_src = source_path.with_extension("toml");
                if manifest_src.exists() {
                    let manifest_dest = dest.with_extension("toml");
                    std::fs::copy(&manifest_src, &manifest_dest)?;
                }

                dest
            };

        // Try to load to validate
        host.load(&self.name, &plugin_path)?;

        println!("Installed plugin '{}' globally", self.name);
        println!("  Location: {}", plugin_path.display());

        Ok(())
    }

    async fn add_project(&self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let pyproject_path = project_dir.join("pyproject.toml");
        if !pyproject_path.exists() {
            bail!("No pyproject.toml found. Initialize a project first or use --global.");
        }

        // Update pyproject.toml
        let content = std::fs::read_to_string(&pyproject_path)?;
        let mut doc: toml_edit::DocumentMut =
            content.parse().context("Failed to parse pyproject.toml")?;

        // Ensure [tool.rx.plugins] exists
        if !doc.contains_key("tool") {
            doc["tool"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        if !doc["tool"].as_table().unwrap().contains_key("rx") {
            doc["tool"]["rx"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        if !doc["tool"]["rx"]
            .as_table()
            .unwrap()
            .contains_key("plugins")
        {
            doc["tool"]["rx"]["plugins"] = toml_edit::Item::Table(toml_edit::Table::new());
        }

        // Add plugin config
        let plugins = doc["tool"]["rx"]["plugins"].as_table_mut().unwrap();
        let mut plugin_table = toml_edit::Table::new();
        plugin_table.insert("source", toml_edit::value(&self.source));
        plugin_table.insert("enabled", toml_edit::value(true));
        plugins.insert(&self.name, toml_edit::Item::Table(plugin_table));

        std::fs::write(&pyproject_path, doc.to_string())?;

        println!("Added plugin '{}' to project", self.name);
        println!("  Source: {}", self.source);
        println!();
        println!("Configuration added to pyproject.toml:");
        println!("  [tool.rx.plugins.{}]", self.name);
        println!("  source = \"{}\"", self.source);

        Ok(())
    }
}

// ============================================================================
// Remove Command
// ============================================================================

#[derive(Args)]
pub struct PluginRemoveCommand {
    /// Plugin name
    pub name: String,

    /// Remove from global plugins
    #[arg(long)]
    pub global: bool,

    /// Project directory
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

impl PluginRemoveCommand {
    pub async fn run(self) -> Result<()> {
        if self.global {
            let host = PluginHost::with_default_dir();
            let plugin_path = host.plugin_dir().join(format!("{}.wasm", self.name));

            if !plugin_path.exists() {
                bail!("Plugin '{}' not found in global plugins", self.name);
            }

            std::fs::remove_file(&plugin_path)?;

            // Also remove manifest if exists
            let manifest_path = plugin_path.with_extension("toml");
            if manifest_path.exists() {
                std::fs::remove_file(&manifest_path)?;
            }

            println!("Removed plugin '{}' from global plugins", self.name);
        } else {
            let project_dir = if self.project.as_os_str() == "." {
                std::env::current_dir()?
            } else {
                self.project.canonicalize()?
            };

            let pyproject_path = project_dir.join("pyproject.toml");
            if !pyproject_path.exists() {
                bail!("No pyproject.toml found");
            }

            let content = std::fs::read_to_string(&pyproject_path)?;
            let mut doc: toml_edit::DocumentMut = content.parse()?;

            // Remove plugin from config
            let tool = doc
                .get_mut("tool")
                .and_then(|t: &mut toml_edit::Item| t.as_table_mut());
            if let Some(tool) = tool {
                let rx = tool
                    .get_mut("rx")
                    .and_then(|t: &mut toml_edit::Item| t.as_table_mut());
                if let Some(rx) = rx {
                    let plugins = rx
                        .get_mut("plugins")
                        .and_then(|t: &mut toml_edit::Item| t.as_table_mut());
                    if let Some(plugins) = plugins {
                        if plugins.remove(&self.name).is_some() {
                            std::fs::write(&pyproject_path, doc.to_string())?;
                            println!("Removed plugin '{}' from project", self.name);
                        } else {
                            bail!("Plugin '{}' not found in project", self.name);
                        }
                    } else {
                        bail!("No plugins configured in project");
                    }
                }
            }
        }

        Ok(())
    }
}

// ============================================================================
// Info Command
// ============================================================================

#[derive(Args)]
pub struct PluginInfoCommand {
    /// Plugin name
    pub name: String,

    /// Project directory
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

impl PluginInfoCommand {
    pub async fn run(self) -> Result<()> {
        let mut host = PluginHost::with_default_dir();

        // Try global first
        let global_path = host.plugin_dir().join(format!("{}.wasm", self.name));
        if global_path.exists() {
            host.load(&self.name, &global_path)?;
        } else {
            // Try project plugins
            let project_dir = if self.project.as_os_str() == "." {
                std::env::current_dir()?
            } else {
                self.project.canonicalize()?
            };

            let configs = load_plugin_configs(&project_dir);
            if let Some(config) = configs.get(&self.name) {
                let path = PathBuf::from(&config.source);
                if path.exists() {
                    host.load_with_config(&self.name, &path, Some(config.clone()))?;
                } else {
                    bail!("Plugin '{}' source not found: {}", self.name, config.source);
                }
            } else {
                bail!("Plugin '{}' not found", self.name);
            }
        }

        let plugin = host
            .get_plugin(&self.name)
            .ok_or_else(|| anyhow::anyhow!("Failed to load plugin"))?;

        println!("Plugin: {}", plugin.name);
        println!();
        println!("  Version:     {}", plugin.manifest.version);
        println!("  Enabled:     {}", plugin.enabled);

        if !plugin.manifest.description.is_empty() {
            println!("  Description: {}", plugin.manifest.description);
        }
        if let Some(ref author) = plugin.manifest.author {
            println!("  Author:      {}", author);
        }
        if let Some(ref license) = plugin.manifest.license {
            println!("  License:     {}", license);
        }
        if let Some(ref homepage) = plugin.manifest.homepage {
            println!("  Homepage:    {}", homepage);
        }

        println!();
        println!("  Hooks:");
        for hook in &plugin.manifest.hooks {
            println!("    - {}", hook);
        }

        println!();
        println!("  Permissions:");
        let perms = &plugin.manifest.permissions;
        println!("    Read files:  {}", perms.read_files);
        println!("    Write files: {}", perms.write_files);
        println!("    Network:     {}", perms.network);
        println!("    Env vars:    {}", perms.env_vars);
        println!("    Execute:     {}", perms.execute);

        Ok(())
    }
}

// ============================================================================
// Enable/Disable Commands
// ============================================================================

#[derive(Args)]
pub struct PluginEnableCommand {
    /// Plugin name
    pub name: String,

    /// Project directory
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

impl PluginEnableCommand {
    pub async fn run(self) -> Result<()> {
        update_plugin_enabled(&self.project, &self.name, true).await?;
        println!("Enabled plugin '{}'", self.name);
        Ok(())
    }
}

#[derive(Args)]
pub struct PluginDisableCommand {
    /// Plugin name
    pub name: String,

    /// Project directory
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

impl PluginDisableCommand {
    pub async fn run(self) -> Result<()> {
        update_plugin_enabled(&self.project, &self.name, false).await?;
        println!("Disabled plugin '{}'", self.name);
        Ok(())
    }
}

async fn update_plugin_enabled(project: &PathBuf, name: &str, enabled: bool) -> Result<()> {
    let project_dir = if project.as_os_str() == "." {
        std::env::current_dir()?
    } else {
        project.canonicalize()?
    };

    let pyproject_path = project_dir.join("pyproject.toml");
    if !pyproject_path.exists() {
        bail!("No pyproject.toml found");
    }

    let content = std::fs::read_to_string(&pyproject_path)?;
    let mut doc: toml_edit::DocumentMut = content.parse()?;

    let tool = doc
        .get_mut("tool")
        .and_then(|t: &mut toml_edit::Item| t.as_table_mut());
    if let Some(tool) = tool {
        let rx = tool
            .get_mut("rx")
            .and_then(|t: &mut toml_edit::Item| t.as_table_mut());
        if let Some(rx) = rx {
            let plugins = rx
                .get_mut("plugins")
                .and_then(|t: &mut toml_edit::Item| t.as_table_mut());
            if let Some(plugins) = plugins {
                let plugin = plugins
                    .get_mut(name)
                    .and_then(|t: &mut toml_edit::Item| t.as_table_mut());
                if let Some(plugin) = plugin {
                    plugin.insert("enabled", toml_edit::value(enabled));
                    std::fs::write(&pyproject_path, doc.to_string())?;
                    return Ok(());
                }
            }
        }
    }

    bail!("Plugin '{}' not found in project", name);
}

// ============================================================================
// Run Command
// ============================================================================

#[derive(Args)]
pub struct PluginRunCommand {
    /// Hook to run (pre-resolve, post-resolve, pre-build, post-build, pre-publish)
    pub hook: String,

    /// Project directory
    #[arg(long, default_value = ".")]
    pub project: PathBuf,
}

impl PluginRunCommand {
    pub async fn run(self) -> Result<()> {
        let hook = match self.hook.as_str() {
            "pre-resolve" | "pre_resolve" => Hook::PreResolve,
            "post-resolve" | "post_resolve" => Hook::PostResolve,
            "pre-build" | "pre_build" => Hook::PreBuild,
            "post-build" | "post_build" => Hook::PostBuild,
            "pre-publish" | "pre_publish" => Hook::PrePublish,
            _ => bail!(
                "Unknown hook '{}'. Available: pre-resolve, post-resolve, pre-build, post-build, pre-publish",
                self.hook
            ),
        };

        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let mut host = PluginHost::with_default_dir();

        // Load global plugins
        let plugin_dir = host.plugin_dir().to_path_buf();
        host.load_from_dir(&plugin_dir).ok();

        // Load project plugins
        let configs = load_plugin_configs(&project_dir);
        host.load_from_config(&configs).ok();

        if host.enabled_count() == 0 {
            println!("No enabled plugins found.");
            return Ok(());
        }

        println!("Running hook: {}", self.hook);
        println!();

        let context = HookContext::new(
            project_dir.display().to_string(),
            format!("manual:{}", self.hook),
        );

        let result = host.execute_hook(hook, &context)?;

        if result.continue_operation {
            println!();
            println!("Hook completed successfully.");
        } else {
            println!();
            println!("Hook stopped the operation.");
            std::process::exit(1);
        }

        Ok(())
    }
}

// ============================================================================
// Init Command (Plugin Development)
// ============================================================================

#[derive(Args)]
pub struct PluginInitCommand {
    /// Plugin name
    pub name: String,

    /// Output directory
    #[arg(long, default_value = ".")]
    pub output: PathBuf,

    /// Language for plugin template
    #[arg(long, default_value = "rust")]
    pub lang: String,
}

impl PluginInitCommand {
    pub async fn run(self) -> Result<()> {
        let output_dir = self.output.join(&self.name);

        if output_dir.exists() {
            bail!("Directory already exists: {}", output_dir.display());
        }

        std::fs::create_dir_all(&output_dir)?;

        match self.lang.as_str() {
            "rust" => self.create_rust_template(&output_dir)?,
            _ => bail!("Unsupported language: {}. Supported: rust", self.lang),
        }

        println!("Created plugin template: {}", output_dir.display());
        println!();
        println!("Next steps:");
        println!("  cd {}", self.name);
        println!("  cargo build --release --target wasm32-wasi");
        println!(
            "  rx plugin add {} ./target/wasm32-wasi/release/{}.wasm",
            self.name,
            self.name.replace('-', "_")
        );

        Ok(())
    }

    fn create_rust_template(&self, dir: &PathBuf) -> Result<()> {
        // Create Cargo.toml
        let cargo_toml = format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
extism-pdk = "1.2"
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"

[profile.release]
lto = true
opt-level = "s"
"#,
            self.name
        );
        std::fs::write(dir.join("Cargo.toml"), cargo_toml)?;

        // Create src/lib.rs
        let lib_rs = format!(
            r#"//! {} - rx plugin
//!
//! Build with: cargo build --release --target wasm32-wasi

use extism_pdk::*;
use serde::{{Deserialize, Serialize}};

/// Context passed to hook functions
#[derive(Debug, Deserialize)]
struct HookContext {{
    project_root: String,
    operation: String,
    #[serde(default)]
    data: serde_json::Value,
}}

/// Result returned from hook execution
#[derive(Debug, Serialize)]
struct HookResult {{
    continue_operation: bool,
    messages: Vec<String>,
    data: Option<serde_json::Value>,
}}

impl HookResult {{
    fn ok() -> Self {{
        Self {{
            continue_operation: true,
            messages: vec![],
            data: None,
        }}
    }}

    fn with_message(mut self, msg: impl Into<String>) -> Self {{
        self.messages.push(msg.into());
        self
    }}
}}

#[plugin_fn]
pub fn pre_resolve(input: Vec<u8>) -> FnResult<Vec<u8>> {{
    let context: HookContext = serde_json::from_slice(&input)?;

    let result = HookResult::ok()
        .with_message(format!("pre_resolve: Processing {{}}", context.project_root));

    Ok(serde_json::to_vec(&result)?)
}}

#[plugin_fn]
pub fn post_resolve(input: Vec<u8>) -> FnResult<Vec<u8>> {{
    let _context: HookContext = serde_json::from_slice(&input)?;
    let result = HookResult::ok();
    Ok(serde_json::to_vec(&result)?)
}}

#[plugin_fn]
pub fn pre_build(input: Vec<u8>) -> FnResult<Vec<u8>> {{
    let _context: HookContext = serde_json::from_slice(&input)?;
    let result = HookResult::ok();
    Ok(serde_json::to_vec(&result)?)
}}

#[plugin_fn]
pub fn post_build(input: Vec<u8>) -> FnResult<Vec<u8>> {{
    let _context: HookContext = serde_json::from_slice(&input)?;
    let result = HookResult::ok();
    Ok(serde_json::to_vec(&result)?)
}}

#[plugin_fn]
pub fn pre_publish(input: Vec<u8>) -> FnResult<Vec<u8>> {{
    let _context: HookContext = serde_json::from_slice(&input)?;
    let result = HookResult::ok();
    Ok(serde_json::to_vec(&result)?)
}}
"#,
            self.name
        );
        std::fs::create_dir_all(dir.join("src"))?;
        std::fs::write(dir.join("src").join("lib.rs"), lib_rs)?;

        // Create plugin manifest
        let manifest = format!(
            r#"name = "{}"
version = "0.1.0"
description = "An rx plugin"
hooks = ["pre_resolve", "post_resolve", "pre_build", "post_build", "pre_publish"]

[permissions]
read_files = true
write_files = false
network = false
"#,
            self.name
        );
        std::fs::write(
            dir.join(format!("{}.toml", self.name.replace('-', "_"))),
            manifest,
        )?;

        // Create README
        let readme = format!(
            r#"# {}

An rx plugin.

## Building

```bash
# Install wasm32-wasi target
rustup target add wasm32-wasi

# Build
cargo build --release --target wasm32-wasi
```

## Installing

```bash
rx plugin add {} ./target/wasm32-wasi/release/{}.wasm
```

## Hooks

This plugin implements the following hooks:

- `pre_resolve` - Called before dependency resolution
- `post_resolve` - Called after dependency resolution
- `pre_build` - Called before building
- `post_build` - Called after building
- `pre_publish` - Called before publishing
"#,
            self.name,
            self.name,
            self.name.replace('-', "_")
        );
        std::fs::write(dir.join("README.md"), readme)?;

        Ok(())
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn load_plugin_configs(project_dir: &PathBuf) -> HashMap<String, PluginConfig> {
    let mut configs = HashMap::new();

    if let Ok(pyproject) = PyProject::load(project_dir) {
        if let Some(rx_config) = pyproject.tool.get("rx") {
            if let Some(plugins) = rx_config.get("plugins") {
                if let Some(table) = plugins.as_table() {
                    for (name, value) in table {
                        if let Some(plugin_table) = value.as_table() {
                            let source = plugin_table
                                .get("source")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            let enabled = plugin_table
                                .get("enabled")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(true);

                            let settings = plugin_table
                                .get("settings")
                                .map(toml_to_json)
                                .unwrap_or(serde_json::Value::Null);

                            configs.insert(
                                name.clone(),
                                PluginConfig {
                                    source,
                                    permissions: None,
                                    settings,
                                    enabled,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    configs
}

fn toml_to_json(value: &toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Array(arr) => serde_json::Value::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(table) => {
            let map: serde_json::Map<String, serde_json::Value> = table
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
    }
}
