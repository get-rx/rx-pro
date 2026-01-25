//! Bundle command - package project for deployment
//!
//! Creates deployment-ready bundles:
//! - Standalone: Directory with dependencies
//! - Lambda: AWS Lambda deployment zip
//! - Docker: Docker-ready directory with Dockerfile

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, ValueEnum};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use rx_core::pep::PyProject;
use rx_core::{Lockfile, VenvManager};

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum BundleTarget {
    /// Standalone directory with dependencies
    #[default]
    Standalone,
    /// AWS Lambda deployment zip
    Lambda,
    /// Docker-ready directory with Dockerfile
    Docker,
}

#[derive(Args)]
pub struct BundleCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Output path for the bundle
    #[arg(long, short)]
    pub output: Option<PathBuf>,

    /// Target deployment format
    #[arg(long, short, value_enum, default_value = "standalone")]
    pub target: BundleTarget,

    /// Include development dependencies
    #[arg(long)]
    pub dev: bool,

    /// Lambda handler entry point (e.g., "app.handler")
    #[arg(long)]
    pub handler: Option<String>,

    /// Python version for Docker (e.g., "3.11")
    #[arg(long, default_value = "3.11")]
    pub python_version: String,

    /// Don't include the project source code (dependencies only)
    #[arg(long)]
    pub deps_only: bool,
}

impl BundleCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Load pyproject.toml for project metadata
        let pyproject = PyProject::load(&project_dir).with_context(|| {
            format!(
                "No pyproject.toml found in {:?}. Run 'rx init' first.",
                project_dir
            )
        })?;

        let project_name = pyproject.name().unwrap_or("app").to_string();

        // Check for lockfile
        let lockfile_path = project_dir.join("rx.lock");
        let lockfile = Lockfile::load(&lockfile_path).with_context(|| {
            format!(
                "No rx.lock found in {:?}. Run 'rx lock' or 'rx sync' first.",
                project_dir
            )
        })?;

        // Check for venv
        let venv_path = project_dir.join(".venv");
        let venv = VenvManager::new(&venv_path);

        if !venv.exists() {
            bail!(
                "No virtual environment found at {:?}. Run 'rx sync' first.",
                venv_path
            );
        }

        let site_packages = venv
            .site_packages()
            .context("Failed to determine site-packages directory")?;

        if !site_packages.exists() {
            bail!(
                "site-packages not found at {:?}. Run 'rx sync' first.",
                site_packages
            );
        }

        // Determine output path
        let output_path = self.get_output_path(&project_dir, &project_name)?;

        match self.target {
            BundleTarget::Standalone => {
                self.bundle_standalone(&project_dir, &site_packages, &output_path, &project_name)?;
            }
            BundleTarget::Lambda => {
                self.bundle_lambda(&project_dir, &site_packages, &output_path, &project_name)?;
            }
            BundleTarget::Docker => {
                self.bundle_docker(
                    &project_dir,
                    &lockfile,
                    &output_path,
                    &project_name,
                    &pyproject,
                )?;
            }
        }

        Ok(())
    }

    fn get_output_path(&self, project_dir: &Path, project_name: &str) -> Result<PathBuf> {
        if let Some(ref output) = self.output {
            return Ok(output.clone());
        }

        let dist_dir = project_dir.join("dist");
        fs::create_dir_all(&dist_dir)?;

        match self.target {
            BundleTarget::Standalone => Ok(dist_dir.join(format!("{}-bundle", project_name))),
            BundleTarget::Lambda => Ok(dist_dir.join(format!("{}-lambda.zip", project_name))),
            BundleTarget::Docker => Ok(dist_dir.join(format!("{}-docker", project_name))),
        }
    }

    fn bundle_standalone(
        &self,
        project_dir: &Path,
        site_packages: &Path,
        output_path: &Path,
        project_name: &str,
    ) -> Result<()> {
        println!("Creating standalone bundle...");

        // Clean and create output directory
        if output_path.exists() {
            fs::remove_dir_all(output_path)?;
        }
        fs::create_dir_all(output_path)?;

        // Copy site-packages (dependencies)
        let deps_dir = output_path.join("lib");
        println!("  Copying dependencies...");
        copy_dir_contents(site_packages, &deps_dir)?;

        // Copy project source
        if !self.deps_only {
            let src_dir = project_dir.join("src");
            if src_dir.exists() {
                println!("  Copying project source...");
                copy_dir_contents(&src_dir, output_path)?;
            } else {
                // Try copying package directory directly
                let pkg_dir = project_dir.join(project_name);
                if pkg_dir.exists() {
                    let dest = output_path.join(project_name);
                    copy_dir_contents(&pkg_dir, &dest)?;
                }
            }
        }

        // Create a simple run script
        let run_script = output_path.join("run.sh");
        let script_content = r#"#!/bin/bash
# Run script for standalone bundle
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
export PYTHONPATH="$SCRIPT_DIR/lib:$SCRIPT_DIR:$PYTHONPATH"
exec python "$@"
"#;
        fs::write(&run_script, script_content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&run_script)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&run_script, perms)?;
        }

        let dep_count = count_packages(&deps_dir);
        println!();
        println!(
            "✓ Created standalone bundle at {:?} ({} packages)",
            output_path, dep_count
        );
        println!();
        println!("Usage:");
        println!("  cd {:?}", output_path);
        println!("  ./run.sh -m {}", project_name);

        Ok(())
    }

    fn bundle_lambda(
        &self,
        project_dir: &Path,
        site_packages: &Path,
        output_path: &Path,
        project_name: &str,
    ) -> Result<()> {
        println!("Creating Lambda deployment package...");

        // Create zip file
        let file = File::create(output_path)?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        let exec_options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        // Add dependencies (flat structure for Lambda)
        println!("  Adding dependencies...");
        add_dir_to_zip(&mut zip, site_packages, "", &options, &exec_options)?;

        // Add project source
        if !self.deps_only {
            println!("  Adding project source...");
            let src_dir = project_dir.join("src");
            if src_dir.exists() {
                // Add contents of src/ directly to root
                add_dir_to_zip(&mut zip, &src_dir, "", &options, &exec_options)?;
            } else {
                // Add package directory
                let pkg_dir = project_dir.join(project_name);
                if pkg_dir.exists() {
                    add_dir_to_zip(&mut zip, &pkg_dir, project_name, &options, &exec_options)?;
                }
            }
        }

        zip.finish()?;

        let metadata = fs::metadata(output_path)?;
        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

        println!();
        println!(
            "✓ Created Lambda package at {:?} ({:.1} MB)",
            output_path, size_mb
        );

        if size_mb > 50.0 {
            println!();
            println!("⚠ Warning: Package exceeds 50 MB. Consider:");
            println!("  - Using Lambda layers for large dependencies");
            println!("  - Excluding unnecessary files with --exclude");
        }

        if let Some(handler) = &self.handler {
            println!();
            println!("Handler: {}", handler);
        } else {
            println!();
            println!("Tip: Specify handler with --handler (e.g., --handler app.handler)");
        }

        Ok(())
    }

    fn bundle_docker(
        &self,
        project_dir: &Path,
        lockfile: &Lockfile,
        output_path: &Path,
        project_name: &str,
        pyproject: &PyProject,
    ) -> Result<()> {
        println!("Creating Docker bundle...");

        // Clean and create output directory
        if output_path.exists() {
            fs::remove_dir_all(output_path)?;
        }
        fs::create_dir_all(output_path)?;

        // Generate requirements.txt from lockfile
        println!("  Generating requirements.txt...");
        let requirements_path = output_path.join("requirements.txt");
        let mut requirements = String::new();
        requirements.push_str("# Generated by T-Rex (rx bundle --target docker)\n");
        for (name, pkg) in &lockfile.packages {
            requirements.push_str(&format!("{}=={}\n", name, pkg.version));
        }
        fs::write(&requirements_path, &requirements)?;

        // Copy project source
        if !self.deps_only {
            println!("  Copying project source...");
            let src_dir = project_dir.join("src");
            let app_dir = output_path.join("app");
            fs::create_dir_all(&app_dir)?;

            if src_dir.exists() {
                copy_dir_contents(&src_dir, &app_dir)?;
            } else {
                let pkg_dir = project_dir.join(project_name);
                if pkg_dir.exists() {
                    let dest = app_dir.join(project_name);
                    copy_dir_contents(&pkg_dir, &dest)?;
                }
            }
        }

        // Generate Dockerfile
        println!("  Generating Dockerfile...");
        let dockerfile_path = output_path.join("Dockerfile");
        let dockerfile = self.generate_dockerfile(project_name, pyproject);
        fs::write(&dockerfile_path, &dockerfile)?;

        // Generate .dockerignore
        let dockerignore_path = output_path.join(".dockerignore");
        let dockerignore = r#"__pycache__
*.pyc
*.pyo
.git
.venv
*.egg-info
.pytest_cache
.mypy_cache
"#;
        fs::write(&dockerignore_path, dockerignore)?;

        println!();
        println!("✓ Created Docker bundle at {:?}", output_path);
        println!();
        println!("Contents:");
        println!("  - Dockerfile");
        println!("  - requirements.txt ({} packages)", lockfile.len());
        if !self.deps_only {
            println!("  - app/");
        }
        println!();
        println!("Build with:");
        println!("  cd {:?}", output_path);
        println!("  docker build -t {} .", project_name);

        Ok(())
    }

    fn generate_dockerfile(&self, project_name: &str, _pyproject: &PyProject) -> String {
        let python_version = &self.python_version;

        format!(
            r#"# Dockerfile generated by T-Rex

FROM python:{python_version}-slim

# Set environment variables
ENV PYTHONDONTWRITEBYTECODE=1
ENV PYTHONUNBUFFERED=1
ENV PYTHONPATH=/app

# Set working directory
WORKDIR /app

# Install system dependencies (add as needed)
# RUN apt-get update && apt-get install -y --no-install-recommends \
#     gcc \
#     && rm -rf /var/lib/apt/lists/*

# Install Python dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application code
COPY app/ .

# Create non-root user for security
RUN useradd --create-home --shell /bin/bash app && chown -R app:app /app
USER app

# Default command (customize as needed)
CMD ["python", "-m", "{project_name}"]
"#
        )
    }
}

/// Copy directory contents recursively
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in WalkDir::new(src).min_depth(1) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(src)?;
        let dest_path = dst.join(relative);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &dest_path)?;
        }
    }

    Ok(())
}

/// Add directory contents to a zip archive
fn add_dir_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    src: &Path,
    prefix: &str,
    options: &SimpleFileOptions,
    exec_options: &SimpleFileOptions,
) -> Result<()> {
    for entry in WalkDir::new(src).min_depth(1) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(src)?;

        let archive_path = if prefix.is_empty() {
            relative.to_string_lossy().to_string()
        } else {
            format!("{}/{}", prefix, relative.to_string_lossy())
        };

        // Skip __pycache__ and .pyc files
        if archive_path.contains("__pycache__") || archive_path.ends_with(".pyc") {
            continue;
        }

        // Skip dist-info directories for cleaner bundles
        if archive_path.contains(".dist-info") {
            continue;
        }

        if entry.file_type().is_dir() {
            zip.add_directory(&archive_path, *options)?;
        } else if entry.file_type().is_file() {
            let opts = if archive_path.ends_with(".sh") || archive_path.contains("/bin/") {
                exec_options
            } else {
                options
            };

            zip.start_file(&archive_path, *opts)?;
            let mut file = File::open(entry.path())?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
        }
    }

    Ok(())
}

/// Count top-level packages in a directory
fn count_packages(dir: &Path) -> usize {
    if !dir.exists() {
        return 0;
    }

    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name();
                    let name_str = name.to_string_lossy();
                    // Count directories that look like packages
                    e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                        && !name_str.starts_with('.')
                        && !name_str.ends_with(".dist-info")
                        && !name_str.ends_with(".egg-info")
                        && name_str != "__pycache__"
                })
                .count()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_count_packages() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("requests")).unwrap();
        fs::create_dir(dir.path().join("urllib3")).unwrap();
        fs::create_dir(dir.path().join("requests-2.28.0.dist-info")).unwrap();
        fs::create_dir(dir.path().join("__pycache__")).unwrap();

        let count = count_packages(dir.path());
        assert_eq!(count, 2);
    }

    #[test]
    fn test_copy_dir_contents() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();

        // Create test structure
        fs::create_dir(src.path().join("pkg")).unwrap();
        fs::write(src.path().join("pkg").join("__init__.py"), "").unwrap();
        fs::write(src.path().join("pkg").join("main.py"), "print('hello')").unwrap();

        copy_dir_contents(src.path(), dst.path()).unwrap();

        assert!(dst.path().join("pkg").join("__init__.py").exists());
        assert!(dst.path().join("pkg").join("main.py").exists());
    }
}
