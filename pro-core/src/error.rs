//! Error types for rx-core

use thiserror::Error;

/// Result type for rx-core operations
pub type Result<T> = std::result::Result<T, Error>;

/// Core error type for rx-core
#[derive(Error, Debug)]
pub enum Error {
    #[error("package '{package}' not found on index")]
    PackageNotFound { package: String },

    #[error("version '{version}' not found for package '{package}'")]
    VersionNotFound { package: String, version: String },

    #[error("dependency conflict: {package} requires {required}, but {found} is installed")]
    DependencyConflict {
        package: String,
        required: String,
        found: String,
    },

    #[error("invalid version specifier: {0}")]
    InvalidVersion(String),

    #[error("invalid version specifier: {0}")]
    InvalidSpecifier(String),

    #[error("invalid dependency specifier: {0}")]
    InvalidDependency(String),

    #[error("pyproject.toml not found")]
    PyProjectNotFound,

    #[error("invalid pyproject.toml: {0}")]
    InvalidPyProject(String),

    #[error("virtual environment error: {0}")]
    VenvError(String),

    #[error("build error: {0}")]
    BuildError(String),

    #[error("resolution error: {0}")]
    Resolution(String),

    #[error("index error: {0}")]
    Index(String),

    #[error("no compatible version found for {package}")]
    NoCompatibleVersion { package: String },

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("version error: {0}")]
    Version(String),

    #[error("zip error: {0}")]
    Zip(String),

    #[error("tar error: {0}")]
    Tar(String),

    #[error("missing project metadata in pyproject.toml")]
    MissingProjectMetadata,

    #[error("missing version in pyproject.toml")]
    MissingVersion,

    #[error("workspace not found")]
    WorkspaceNotFound,

    #[error("configuration error: {0}")]
    Config(String),
}
