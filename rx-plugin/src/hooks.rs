//! Plugin lifecycle hooks

use serde::{Deserialize, Serialize};

/// Available lifecycle hooks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hook {
    /// Called before dependency resolution
    PreResolve,
    /// Called after dependency resolution
    PostResolve,
    /// Called before building
    PreBuild,
    /// Called after building
    PostBuild,
    /// Called before publishing
    PrePublish,
}

impl Hook {
    /// Get the function name to call in the Wasm module
    pub fn function_name(&self) -> &'static str {
        match self {
            Hook::PreResolve => "pre_resolve",
            Hook::PostResolve => "post_resolve",
            Hook::PreBuild => "pre_build",
            Hook::PostBuild => "post_build",
            Hook::PrePublish => "pre_publish",
        }
    }
}

/// Context passed to hook functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    /// Project root directory
    pub project_root: String,

    /// Current operation details
    pub operation: String,

    /// Additional context data (JSON)
    #[serde(default)]
    pub data: serde_json::Value,
}

impl HookContext {
    /// Create a new hook context
    pub fn new(project_root: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            project_root: project_root.into(),
            operation: operation.into(),
            data: serde_json::Value::Null,
        }
    }

    /// Add additional data to the context
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    /// Serialize to bytes for Wasm
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// Result returned from hook execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookResult {
    /// Whether to continue with the operation
    pub continue_operation: bool,

    /// Messages to display to the user
    #[serde(default)]
    pub messages: Vec<String>,

    /// Modified data (if any)
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

impl HookResult {
    /// Create a successful result
    pub fn ok() -> Self {
        Self {
            continue_operation: true,
            messages: vec![],
            data: None,
        }
    }

    /// Create a result that stops the operation
    pub fn stop(message: impl Into<String>) -> Self {
        Self {
            continue_operation: false,
            messages: vec![message.into()],
            data: None,
        }
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Merge another result into this one
    pub fn merge(&mut self, other: HookResult) {
        self.continue_operation = self.continue_operation && other.continue_operation;
        self.messages.extend(other.messages);
        if other.data.is_some() {
            self.data = other.data;
        }
    }
}
