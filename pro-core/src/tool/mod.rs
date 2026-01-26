//! Tool runner for ephemeral Python tool execution
//!
//! This module provides functionality for running Python tools without
//! permanent installation, similar to `uvx` or `pipx run`:
//! - Tool caching in ~/.local/share/rx/tools/
//! - Automatic installation on first run
//! - Cache management (list, clear)

mod cache;
mod runner;

pub use cache::{CachedTool, ToolCache};
pub use runner::ToolRunner;
