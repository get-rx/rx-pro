//! Native Rust build backend for Python packages (PEP 517)

use std::path::{Path, PathBuf};

use crate::Result;

/// Build backend for creating wheels and sdists
pub struct Builder {
    /// Project root directory
    project_root: PathBuf,
}

impl Builder {
    /// Create a new builder for the given project
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }

    /// Build a wheel (PEP 427)
    pub async fn build_wheel(&self, output_dir: &Path) -> Result<PathBuf> {
        // TODO: Implement wheel building
        // - Read pyproject.toml
        // - Collect package files
        // - Generate METADATA (PEP 566)
        // - Generate WHEEL file
        // - Generate RECORD file
        // - Create zip archive with .whl extension

        tracing::info!("Building wheel to {:?}", output_dir);

        let wheel_name = "package-0.1.0-py3-none-any.whl";
        Ok(output_dir.join(wheel_name))
    }

    /// Build a source distribution (PEP 517)
    pub async fn build_sdist(&self, output_dir: &Path) -> Result<PathBuf> {
        // TODO: Implement sdist building
        // - Read pyproject.toml
        // - Collect all source files
        // - Generate PKG-INFO
        // - Create .tar.gz archive

        tracing::info!("Building sdist to {:?}", output_dir);

        let sdist_name = "package-0.1.0.tar.gz";
        Ok(output_dir.join(sdist_name))
    }

    /// Get the project root
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}
