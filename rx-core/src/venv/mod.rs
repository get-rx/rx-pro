//! Virtual environment management
//!
//! Creates and manages Python virtual environments natively without
//! requiring the venv module or virtualenv package.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Error, Result};

/// Virtual environment manager
pub struct VenvManager {
    /// Path to the virtual environment
    path: PathBuf,
}

impl VenvManager {
    /// Create a new venv manager for the given path
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Create a new virtual environment
    pub fn create(&self, python_path: Option<&Path>) -> Result<()> {
        let python = match python_path {
            Some(p) => p.to_path_buf(),
            None => find_python()?,
        };

        // Get Python version info
        let version_info = get_python_version(&python)?;
        let (major, minor) = version_info;

        tracing::info!(
            "Creating venv at {:?} with Python {}.{}",
            self.path,
            major,
            minor
        );

        // Create directory structure
        fs::create_dir_all(&self.path).map_err(Error::Io)?;

        #[cfg(unix)]
        {
            let bin_dir = self.path.join("bin");
            fs::create_dir_all(&bin_dir).map_err(Error::Io)?;

            let lib_dir = self
                .path
                .join("lib")
                .join(format!("python{}.{}", major, minor))
                .join("site-packages");
            fs::create_dir_all(&lib_dir).map_err(Error::Io)?;

            let include_dir = self.path.join("include");
            fs::create_dir_all(&include_dir).map_err(Error::Io)?;

            // Symlink Python executable
            let python_link = bin_dir.join("python");
            if !python_link.exists() {
                std::os::unix::fs::symlink(&python, &python_link).map_err(Error::Io)?;
            }

            // Create versioned symlinks
            let python_versioned = bin_dir.join(format!("python{}", major));
            if !python_versioned.exists() {
                std::os::unix::fs::symlink(&python, &python_versioned).map_err(Error::Io)?;
            }

            let python_full = bin_dir.join(format!("python{}.{}", major, minor));
            if !python_full.exists() {
                std::os::unix::fs::symlink(&python, &python_full).map_err(Error::Io)?;
            }

            // Create pip symlink if pip exists in base Python
            let base_bin = python.parent().unwrap_or(Path::new("/usr/bin"));
            let base_pip = base_bin.join("pip3");
            if base_pip.exists() {
                let pip_link = bin_dir.join("pip");
                if !pip_link.exists() {
                    std::os::unix::fs::symlink(&base_pip, &pip_link).map_err(Error::Io)?;
                }
                let pip3_link = bin_dir.join("pip3");
                if !pip3_link.exists() {
                    std::os::unix::fs::symlink(&base_pip, &pip3_link).map_err(Error::Io)?;
                }
            }

            // Create activation scripts
            create_activate_script(&bin_dir, &self.path)?;
        }

        #[cfg(windows)]
        {
            let scripts_dir = self.path.join("Scripts");
            fs::create_dir_all(&scripts_dir).map_err(Error::Io)?;

            let lib_dir = self.path.join("Lib").join("site-packages");
            fs::create_dir_all(&lib_dir).map_err(Error::Io)?;

            let include_dir = self.path.join("Include");
            fs::create_dir_all(&include_dir).map_err(Error::Io)?;

            // Copy Python executable on Windows
            let python_exe = scripts_dir.join("python.exe");
            if !python_exe.exists() {
                fs::copy(&python, &python_exe).map_err(Error::Io)?;
            }
        }

        // Write pyvenv.cfg
        write_pyvenv_cfg(&self.path, &python, major, minor)?;

        tracing::info!("Virtual environment created at {:?}", self.path);
        Ok(())
    }

    /// Check if the venv exists and is valid
    pub fn exists(&self) -> bool {
        self.path.join("pyvenv.cfg").exists()
    }

    /// Get the site-packages directory
    pub fn site_packages(&self) -> Result<PathBuf> {
        if !self.exists() {
            return Err(Error::VenvError(
                "Virtual environment does not exist".into(),
            ));
        }

        // Read pyvenv.cfg to get Python version
        let cfg_path = self.path.join("pyvenv.cfg");
        let content = fs::read_to_string(&cfg_path).map_err(Error::Io)?;

        let mut version = None;
        for line in content.lines() {
            if let Some(v) = line.strip_prefix("version = ") {
                version = Some(v.trim().to_string());
                break;
            }
        }

        let version =
            version.ok_or_else(|| Error::VenvError("Cannot determine Python version".into()))?;
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() < 2 {
            return Err(Error::VenvError("Invalid version in pyvenv.cfg".into()));
        }
        let major = parts[0];
        let minor = parts[1];

        #[cfg(unix)]
        {
            Ok(self
                .path
                .join("lib")
                .join(format!("python{}.{}", major, minor))
                .join("site-packages"))
        }

        #[cfg(windows)]
        {
            Ok(self.path.join("Lib").join("site-packages"))
        }
    }

    /// Get the bin/Scripts directory
    pub fn bin_dir(&self) -> PathBuf {
        #[cfg(unix)]
        {
            self.path.join("bin")
        }

        #[cfg(windows)]
        {
            self.path.join("Scripts")
        }
    }

    /// Get the venv path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the Python executable path in the venv
    pub fn python(&self) -> PathBuf {
        #[cfg(unix)]
        {
            self.bin_dir().join("python")
        }

        #[cfg(windows)]
        {
            self.bin_dir().join("python.exe")
        }
    }
}

/// Find Python interpreter on the system
fn find_python() -> Result<PathBuf> {
    // Try common Python paths in order of preference
    let candidates = [
        "python3",
        "python",
        "/usr/bin/python3",
        "/usr/local/bin/python3",
        "/opt/homebrew/bin/python3",
    ];

    for candidate in candidates {
        if let Ok(output) = Command::new("which").arg(candidate).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Ok(PathBuf::from(path));
                }
            }
        }

        // Also try running the command directly
        if let Ok(output) = Command::new(candidate).arg("--version").output() {
            if output.status.success() {
                return Ok(PathBuf::from(candidate));
            }
        }
    }

    Err(Error::VenvError(
        "Could not find Python interpreter. Please install Python 3.8+".into(),
    ))
}

/// Get Python version as (major, minor)
fn get_python_version(python: &Path) -> Result<(u32, u32)> {
    let output = Command::new(python)
        .args([
            "-c",
            "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')",
        ])
        .output()
        .map_err(|e| Error::VenvError(format!("Failed to run Python: {}", e)))?;

    if !output.status.success() {
        return Err(Error::VenvError("Failed to get Python version".into()));
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parts: Vec<&str> = version.split('.').collect();

    if parts.len() < 2 {
        return Err(Error::VenvError(format!(
            "Invalid Python version: {}",
            version
        )));
    }

    let major: u32 = parts[0]
        .parse()
        .map_err(|_| Error::VenvError(format!("Invalid major version: {}", parts[0])))?;
    let minor: u32 = parts[1]
        .parse()
        .map_err(|_| Error::VenvError(format!("Invalid minor version: {}", parts[1])))?;

    Ok((major, minor))
}

/// Write pyvenv.cfg file
fn write_pyvenv_cfg(venv_path: &Path, python: &Path, major: u32, minor: u32) -> Result<()> {
    let cfg_path = venv_path.join("pyvenv.cfg");

    // Get the base Python home directory
    let python_home = python
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(Path::new("/usr"));

    let content = format!(
        "home = {}\n\
         include-system-site-packages = false\n\
         version = {}.{}\n",
        python_home.display(),
        major,
        minor
    );

    let mut file = fs::File::create(&cfg_path).map_err(Error::Io)?;
    file.write_all(content.as_bytes()).map_err(Error::Io)?;

    Ok(())
}

/// Create activation script for bash/zsh
#[cfg(unix)]
fn create_activate_script(bin_dir: &Path, venv_path: &Path) -> Result<()> {
    let activate_path = bin_dir.join("activate");
    let venv_name = venv_path.file_name().unwrap_or_default().to_string_lossy();

    let content = format!(
        r#"# This file must be used with "source bin/activate" *from bash*
# You cannot run it directly

deactivate () {{
    if [ -n "${{_OLD_VIRTUAL_PATH:-}}" ] ; then
        PATH="${{_OLD_VIRTUAL_PATH:-}}"
        export PATH
        unset _OLD_VIRTUAL_PATH
    fi

    if [ -n "${{_OLD_VIRTUAL_PYTHONHOME:-}}" ] ; then
        PYTHONHOME="${{_OLD_VIRTUAL_PYTHONHOME:-}}"
        export PYTHONHOME
        unset _OLD_VIRTUAL_PYTHONHOME
    fi

    if [ -n "${{_OLD_VIRTUAL_PS1:-}}" ] ; then
        PS1="${{_OLD_VIRTUAL_PS1:-}}"
        export PS1
        unset _OLD_VIRTUAL_PS1
    fi

    unset VIRTUAL_ENV
    if [ ! "${{1:-}}" = "nondestructive" ] ; then
        unset -f deactivate
    fi
}}

deactivate nondestructive

VIRTUAL_ENV="{venv_path}"
export VIRTUAL_ENV

_OLD_VIRTUAL_PATH="$PATH"
PATH="$VIRTUAL_ENV/bin:$PATH"
export PATH

if [ -z "${{VIRTUAL_ENV_DISABLE_PROMPT:-}}" ] ; then
    _OLD_VIRTUAL_PS1="${{PS1:-}}"
    PS1="({name}) ${{PS1:-}}"
    export PS1
fi
"#,
        venv_path = venv_path.display(),
        name = venv_name
    );

    let mut file = fs::File::create(&activate_path).map_err(Error::Io)?;
    file.write_all(content.as_bytes()).map_err(Error::Io)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_python() {
        let python = find_python();
        assert!(python.is_ok(), "Should find Python on most systems");
    }

    #[test]
    fn test_get_python_version() {
        if let Ok(python) = find_python() {
            let version = get_python_version(&python);
            assert!(version.is_ok());
            let (major, minor) = version.unwrap();
            assert!(major >= 3, "Should be Python 3+");
            assert!(minor >= 8 || major > 3, "Should be Python 3.8+");
        }
    }

    #[test]
    fn test_venv_manager_new() {
        let manager = VenvManager::new("/tmp/test-venv");
        assert_eq!(manager.path(), Path::new("/tmp/test-venv"));
    }
}
