//! PEP 723 script support
//!
//! This module provides functionality for running Python scripts with
//! inline dependency specifications (PEP 723):
//! - Parsing inline metadata from script comments
//! - Creating ephemeral environments for script execution
//! - Caching environments by dependency hash

mod parser;
mod runner;

pub use parser::{might_have_metadata, parse_script_metadata, ScriptMetadata};
pub use runner::{is_pep723_script, ScriptRunner};
