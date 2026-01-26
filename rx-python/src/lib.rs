//! Python bindings for T-Rex package manager
//!
//! This module provides Python bindings via PyO3, allowing Python code
//! to use T-Rex's fast Rust implementation directly.
//!
//! ```python
//! from trex import resolve, sync, build, audit
//!
//! # Resolve dependencies
//! resolution = resolve(["requests>=2.28", "numpy"])
//!
//! # Sync to virtual environment
//! sync("./my-project")
//!
//! # Build wheel
//! result = build("./my-project", "./dist")
//!
//! # Security audit
//! vulnerabilities = audit("./my-project")
//! ```

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

/// Resolve Python package dependencies
///
/// Args:
///     requirements: List of requirement strings (e.g., ["requests>=2.28", "numpy"])
///
/// Returns:
///     List of resolved packages with versions and URLs
#[pyfunction]
fn resolve(requirements: Vec<String>) -> PyResult<Vec<(String, String, String)>> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;

    rt.block_on(async {
        let reqs: Vec<rx_core::pep::Requirement> = requirements
            .iter()
            .filter_map(|r| rx_core::pep::Requirement::parse(r).ok())
            .collect();

        let resolver = rx_core::resolver::Resolver::new();
        let resolution = resolver
            .resolve(&reqs)
            .await
            .map_err(|e| PyRuntimeError::new_err(format!("Resolution failed: {}", e)))?;

        let result: Vec<(String, String, String)> = resolution
            .packages
            .into_iter()
            .map(|pkg| (pkg.name, pkg.version, pkg.url))
            .collect();

        Ok(result)
    })
}

/// Synchronize a project's virtual environment with its lockfile
///
/// Args:
///     project_path: Path to the project directory
///     recreate: Whether to recreate the venv from scratch
///
/// Returns:
///     Number of packages installed
#[pyfunction]
#[pyo3(signature = (project_path, recreate=false))]
fn sync(project_path: &str, recreate: bool) -> PyResult<usize> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;

    let project_dir = PathBuf::from(project_path);

    rt.block_on(async {
        // Load lockfile
        let lockfile_path = project_dir.join("rx.lock");
        let lockfile = rx_core::Lockfile::load(&lockfile_path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to load lockfile: {}", e)))?;

        // Create venv manager
        let venv_path = project_dir.join(".venv");
        let venv = rx_core::VenvManager::new(&venv_path);

        if recreate && venv_path.exists() {
            std::fs::remove_dir_all(&venv_path)
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to remove venv: {}", e)))?;
        }

        // Create venv if needed (not async)
        if !venv_path.exists() {
            venv.create(None)
                .map_err(|e| PyRuntimeError::new_err(format!("Failed to create venv: {}", e)))?;
        }

        // Get site-packages path
        let site_packages = venv
            .site_packages()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to get site-packages: {}", e)))?;

        // Get cache directory
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("rx")
            .join("wheels");

        // Install packages
        let installer = rx_core::Installer::new(&cache_dir);
        let count = lockfile.packages.len();

        // Convert BTreeMap to HashMap for installer
        let packages: HashMap<_, _> = lockfile.packages.into_iter().collect();

        installer
            .install(&packages, &site_packages)
            .await
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to install packages: {}", e)))?;

        Ok(count)
    })
}

/// Build a Python package (wheel and/or sdist)
///
/// Args:
///     project_path: Path to the project directory
///     output_dir: Directory for built artifacts
///     wheel: Build wheel (default: True)
///     sdist: Build source distribution (default: True)
///
/// Returns:
///     Dictionary with paths to built artifacts
#[pyfunction]
#[pyo3(signature = (project_path, output_dir="dist", wheel=true, sdist=true))]
fn build(
    project_path: &str,
    output_dir: &str,
    wheel: bool,
    sdist: bool,
) -> PyResult<std::collections::HashMap<String, String>> {
    let project_dir = PathBuf::from(project_path);
    let out_dir = PathBuf::from(output_dir);

    let builder = rx_core::builder::Builder::new(&project_dir);
    let mut results = std::collections::HashMap::new();

    if wheel {
        let result = builder
            .build_wheel(&out_dir)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to build wheel: {}", e)))?;
        results.insert(
            "wheel".to_string(),
            result.path.to_string_lossy().to_string(),
        );
    }

    if sdist {
        let result = builder
            .build_sdist(&out_dir)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to build sdist: {}", e)))?;
        results.insert(
            "sdist".to_string(),
            result.path.to_string_lossy().to_string(),
        );
    }

    Ok(results)
}

/// Audit dependencies for security vulnerabilities
///
/// Args:
///     project_path: Path to the project directory
///
/// Returns:
///     List of vulnerabilities found (package, version, vuln_id, severity, summary)
#[pyfunction]
fn audit(project_path: &str) -> PyResult<Vec<(String, String, String, String, String)>> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;

    let project_dir = PathBuf::from(project_path);

    rt.block_on(async {
        // Load lockfile
        let lockfile_path = project_dir.join("rx.lock");
        let lockfile = rx_core::Lockfile::load(&lockfile_path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to load lockfile: {}", e)))?;

        // Create auditor
        let auditor = rx_core::audit::Auditor::new();

        // Audit the lockfile
        let report = auditor
            .audit_lockfile(&lockfile)
            .await
            .map_err(|e| PyRuntimeError::new_err(format!("Audit failed: {}", e)))?;

        // Collect all vulnerabilities from all packages
        let mut vulns = Vec::new();
        for pkg_result in report.packages {
            for vuln in pkg_result.vulnerabilities {
                vulns.push((
                    pkg_result.name.clone(),
                    pkg_result.version.clone(),
                    vuln.id,
                    format!("{:?}", vuln.severity),
                    vuln.summary,
                ));
            }
        }

        Ok(vulns)
    })
}

/// Get the version of T-Rex
#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Python module initialization
#[pymodule]
fn trex(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(resolve, m)?)?;
    m.add_function(wrap_pyfunction!(sync, m)?)?;
    m.add_function(wrap_pyfunction!(build, m)?)?;
    m.add_function(wrap_pyfunction!(audit, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
