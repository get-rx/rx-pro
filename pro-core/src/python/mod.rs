//! Python version management
//!
//! This module provides functionality for managing Python installations:
//! - Platform detection for downloading the correct binaries
//! - Version parsing and matching
//! - Downloading and installing Python from python-build-standalone
//! - Project-level version pinning (.python-version)
//! - Global default version configuration

mod manager;
mod platform;
mod versions;

pub use manager::{InstalledPython, PythonManager};
pub use platform::{Arch, Os, Platform};
pub use versions::{
    available_versions, find_matching_version, get_versions_for_minor, AvailableVersion,
    PythonVersion,
};
