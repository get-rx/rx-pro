//! Package installer with caching and parallel downloads

use std::path::Path;

use crate::{resolver::Resolution, Result};

/// Package installer
pub struct Installer {
    /// Cache directory for downloaded wheels
    cache_dir: std::path::PathBuf,
}

impl Installer {
    /// Create a new installer with the given cache directory
    pub fn new(cache_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
        }
    }

    /// Install resolved packages into a virtual environment
    pub async fn install(&self, resolution: &Resolution, venv_path: &Path) -> Result<()> {
        // TODO: Implement installation
        // - Download wheels in parallel
        // - Cache wheels (content-addressable)
        // - Unpack to venv site-packages
        // - Use hardlinks where possible

        tracing::info!(
            "Installing {} packages to {:?}",
            resolution.packages.len(),
            venv_path
        );

        Ok(())
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}
