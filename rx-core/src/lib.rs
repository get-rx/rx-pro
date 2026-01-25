//! rx-core: Core library for T-Rex Python package manager
//!
//! This crate provides the core functionality for T-Rex:
//! - Dependency resolution (using pubgrub)
//! - Package installation with caching
//! - Native wheel/sdist building
//! - Virtual environment management
//! - PEP standards compliance

pub mod builder;
pub mod error;
pub mod index;
pub mod installer;
pub mod lockfile;
pub mod pep;
pub mod resolver;
pub mod semver;
pub mod venv;

pub use error::{Error, Result};
pub use installer::{default_cache_dir, InstallResult, Installer};
pub use lockfile::Lockfile;
pub use venv::VenvManager;
