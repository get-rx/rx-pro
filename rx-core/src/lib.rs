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
pub mod docker;
pub mod builder;
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
pub use path_dep::{PathDependency, load_path_dependencies, install_path_dependency};
pub use affected::{
    AffectedConfig, AffectedResult, detect_affected, detect_affected_with_transitive,
    build_dependency_graph, get_transitive_affected,
};
pub use polylith::{Polylith, Brick, BrickType};
pub use docker::{DockerConfig, DockerfileGenerator, build_image};
pub use registry::{RegistryConfig, RegistryManager, ResolvedCredentials};
