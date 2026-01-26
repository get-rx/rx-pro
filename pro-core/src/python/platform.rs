//! Platform detection for Python installations
//!
//! Detects the current operating system and architecture to determine
//! the correct python-build-standalone release to download.

use std::fmt;

// Error is only used in cfg blocks for unsupported platforms
#[allow(unused_imports)]
use crate::Error;
use crate::Result;

/// Operating system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    Linux,
    MacOS,
    Windows,
}

impl Os {
    /// Detect the current operating system
    pub fn current() -> Result<Self> {
        #[cfg(target_os = "linux")]
        return Ok(Os::Linux);

        #[cfg(target_os = "macos")]
        return Ok(Os::MacOS);

        #[cfg(target_os = "windows")]
        return Ok(Os::Windows);

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        return Err(Error::UnsupportedPlatform(format!(
            "unsupported operating system: {}",
            std::env::consts::OS
        )));
    }

    /// Get the OS string used in python-build-standalone releases
    pub fn as_pbs_str(&self) -> &'static str {
        match self {
            Os::Linux => "unknown-linux-gnu",
            Os::MacOS => "apple-darwin",
            Os::Windows => "pc-windows-msvc",
        }
    }
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Os::Linux => write!(f, "linux"),
            Os::MacOS => write!(f, "macos"),
            Os::Windows => write!(f, "windows"),
        }
    }
}

/// CPU architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arch {
    X86_64,
    Aarch64,
}

impl Arch {
    /// Detect the current architecture
    pub fn current() -> Result<Self> {
        #[cfg(target_arch = "x86_64")]
        return Ok(Arch::X86_64);

        #[cfg(target_arch = "aarch64")]
        return Ok(Arch::Aarch64);

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        return Err(Error::UnsupportedPlatform(format!(
            "unsupported architecture: {}",
            std::env::consts::ARCH
        )));
    }

    /// Get the architecture string used in python-build-standalone releases
    pub fn as_pbs_str(&self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arch::X86_64 => write!(f, "x86_64"),
            Arch::Aarch64 => write!(f, "aarch64"),
        }
    }
}

/// Platform (OS + Architecture combination)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    /// Create a new platform
    pub fn new(os: Os, arch: Arch) -> Self {
        Self { os, arch }
    }

    /// Detect the current platform
    pub fn current() -> Result<Self> {
        Ok(Self {
            os: Os::current()?,
            arch: Arch::current()?,
        })
    }

    /// Get the platform triple for python-build-standalone releases
    ///
    /// Example: "x86_64-unknown-linux-gnu", "aarch64-apple-darwin"
    pub fn triple(&self) -> String {
        format!("{}-{}", self.arch.as_pbs_str(), self.os.as_pbs_str())
    }

    /// Get the archive extension for this platform
    pub fn archive_ext(&self) -> &'static str {
        match self.os {
            Os::Windows => "zip",
            _ => "tar.zst",
        }
    }

    /// Check if this platform supports the pgo+lto optimized builds
    pub fn supports_optimized(&self) -> bool {
        // Most platforms support optimized builds now
        true
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.arch, self.os)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_current() {
        let platform = Platform::current();
        assert!(platform.is_ok(), "Should detect current platform");
    }

    #[test]
    fn test_platform_triple() {
        let platform = Platform::new(Os::Linux, Arch::X86_64);
        assert_eq!(platform.triple(), "x86_64-unknown-linux-gnu");

        let platform = Platform::new(Os::MacOS, Arch::Aarch64);
        assert_eq!(platform.triple(), "aarch64-apple-darwin");

        let platform = Platform::new(Os::Windows, Arch::X86_64);
        assert_eq!(platform.triple(), "x86_64-pc-windows-msvc");
    }

    #[test]
    fn test_archive_ext() {
        let linux = Platform::new(Os::Linux, Arch::X86_64);
        assert_eq!(linux.archive_ext(), "tar.zst");

        let windows = Platform::new(Os::Windows, Arch::X86_64);
        assert_eq!(windows.archive_ext(), "zip");
    }
}
