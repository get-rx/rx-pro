//! rx-plugin: Plugin SDK for T-Rex Python package manager
//!
//! This crate provides the plugin system for T-Rex:
//! - WebAssembly plugin host (Extism)
//! - Lifecycle hooks (pre-resolve, post-resolve, pre-build, post-build, pre-publish)
//! - Sandboxed execution with capability-based permissions

mod error;
mod host;
mod hooks;

pub use error::{PluginError, PluginResult};
pub use host::PluginHost;
pub use hooks::{Hook, HookContext, HookResult};
