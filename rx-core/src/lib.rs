//! rx-core: Core library for T-Rex Python package manager
//!
//! This crate provides the core functionality for T-Rex:
//! - Dependency resolution (using pubgrub)
//! - Package installation with caching
//! - Native wheel/sdist building
//! - Virtual environment management
//! - Security auditing (CVE checking)
//! - PEP standards compliance

pub mod affected;
pub mod audit;
pub mod builder;
pub mod docker;
pub mod dotenv;
pub mod error;
pub mod index;
pub mod installer;
pub mod lockfile;
pub mod path_dep;
pub mod pep;
pub mod polylith;
pub mod registry;
pub mod resolver;
pub mod semver;
pub mod venv;
pub mod versioning;
pub mod workspace;

pub use affected::{
    build_dependency_graph, detect_affected, detect_affected_with_transitive,
    get_transitive_affected, AffectedConfig, AffectedResult,
};
pub use audit::{
    AuditConfig, AuditReport, Auditor, FixRecommendation, FixResult, IgnoredVulnerability,
    Severity, Vulnerability,
};
pub use docker::{build_image, DockerConfig, DockerfileGenerator};
pub use dotenv::{load_dotenv, DotenvConfig};
pub use error::{Error, Result};
pub use installer::{default_cache_dir, InstallResult, Installer};
pub use lockfile::Lockfile;
pub use path_dep::{install_path_dependency, load_path_dependencies, PathDependency};
pub use polylith::{Brick, BrickType, Polylith};
pub use registry::{RegistryConfig, RegistryManager, ResolvedCredentials};
pub use venv::VenvManager;
pub use versioning::{bump_version, get_git_version, get_version, VersioningConfig};
pub use workspace::{MemberInfo, Workspace};
