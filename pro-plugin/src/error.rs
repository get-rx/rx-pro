//! Plugin error types

use thiserror::Error;

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, PluginError>;

/// Plugin-specific errors
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("plugin not found: {path}")]
    NotFound { path: String },

    #[error("failed to load plugin: {0}")]
    LoadError(String),

    #[error("plugin execution failed: {0}")]
    ExecutionError(String),

    #[error("hook '{hook}' not found in plugin")]
    HookNotFound { hook: String },

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("invalid plugin manifest: {0}")]
    InvalidManifest(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
