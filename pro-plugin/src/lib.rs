//! rx-plugin: Plugin SDK for Pro Python package manager
//!
//! This crate provides the plugin system for Pro:
//! - WebAssembly plugin host (Extism)
//! - Lifecycle hooks (pre-resolve, post-resolve, pre-build, post-build, pre-publish)
//! - Sandboxed execution with capability-based permissions
//!
//! ## Configuration
//!
//! Plugins can be configured in pyproject.toml:
//!
//! ```toml
//! [tool.rx.plugins.my-plugin]
//! source = "./plugins/my-plugin.wasm"
//! enabled = true
//!
//! [tool.rx.plugins.my-plugin.permissions]
//! read_files = true
//! network = false
//!
//! [tool.rx.plugins.my-plugin.settings]
//! option1 = "value"
//! ```

mod error;
mod hooks;
mod host;
mod manifest;

pub use error::{PluginError, PluginResult};
pub use hooks::{Hook, HookContext, HookResult};
pub use host::{LoadedPlugin, PluginHost};
pub use manifest::{PluginConfig, PluginManifest, PluginPermissions};
