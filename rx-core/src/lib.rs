//! rx-core: Core library for T-Rex Python package manager
//!
//! This crate provides the core functionality for T-Rex:
//! - Dependency resolution (using pubgrub)
//! - Package installation with caching
//! - Native wheel/sdist building
//! - Virtual environment management
//! - Security auditing (CVE checking)
//! - PEP standards compliance

pub mod audit;
pub mod builder;
pub mod dotenv;
pub mod error;
pub mod index;
pub mod installer;
pub mod lockfile;
pub mod pep;
pub mod resolver;
pub mod semver;
pub mod venv;
pub mod versioning;
pub mod workspace;

pub use audit::{
    AuditConfig, AuditReport, Auditor, FixRecommendation, FixResult, IgnoredVulnerability,
    Severity, Vulnerability,
};
pub use dotenv::{DotenvConfig, load_dotenv};
pub use error::{Error, Result};
pub use installer::{default_cache_dir, InstallResult, Installer};
pub use lockfile::Lockfile;
pub use venv::VenvManager;
pub use versioning::{VersioningConfig, get_version, get_git_version, bump_version};
pub use workspace::{Workspace, MemberInfo};
