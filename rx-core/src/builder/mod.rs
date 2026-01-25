//! Native Rust build backend for Python packages (PEP 517)
//!
//! Supports bundling local path dependencies (non-editable) into wheels
//! for monorepo deployments.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::path_dep::{load_path_dependencies, PathDependency};
use crate::pep::PyProject;
use crate::{Error, Result};

/// Build backend for creating wheels and sdists
pub struct Builder {
    /// Project root directory
    project_root: PathBuf,
    /// Whether to include local path dependencies in the wheel
    include_local_deps: bool,
}

/// Result of a build operation
#[derive(Debug)]
pub struct BuildResult {
    /// Path to the built artifact
    pub path: PathBuf,
    /// Size of the artifact in bytes
    pub size: u64,
}

impl Builder {
    /// Create a new builder for the given project
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            include_local_deps: true, // Default to including local deps
        }
    }

    /// Set whether to include local path dependencies in the wheel
    pub fn with_include_local_deps(mut self, include: bool) -> Self {
        self.include_local_deps = include;
        self
    }

    /// Build a wheel (PEP 427)
    pub fn build_wheel(&self, output_dir: &Path) -> Result<BuildResult> {
        let pyproject = PyProject::load(&self.project_root)?;
        let project = pyproject
            .project
            .as_ref()
            .ok_or(Error::MissingProjectMetadata)?;

        let name = &project.name;
        let version = project.version.as_ref().ok_or(Error::MissingVersion)?;

        // Normalize name for wheel filename (PEP 427)
        let normalized_name = normalize_name(name);

        // Create output directory
        std::fs::create_dir_all(output_dir).map_err(Error::Io)?;

        // Wheel filename: {distribution}-{version}-{python}-{abi}-{platform}.whl
        let wheel_name = format!("{}-{}-py3-none-any.whl", normalized_name, version);
        let wheel_path = output_dir.join(&wheel_name);

        // Create the wheel zip
        let file = std::fs::File::create(&wheel_path).map_err(Error::Io)?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // Track files for RECORD
        let mut records: Vec<(String, String, u64)> = Vec::new();

        // Find and add package source files
        let src_dir = self.project_root.join("src");
        let package_dir = if src_dir.exists() {
            // src layout: src/package_name/
            let pkg_dir = src_dir.join(name.replace('-', "_"));
            if pkg_dir.exists() {
                Some(pkg_dir)
            } else {
                // Try finding any package in src/
                find_package_in_dir(&src_dir)
            }
        } else {
            // Flat layout: package_name/ at project root
            let pkg_dir = self.project_root.join(name.replace('-', "_"));
            if pkg_dir.exists() {
                Some(pkg_dir)
            } else {
                None
            }
        };

        if let Some(ref pkg_dir) = package_dir {
            let pkg_name = pkg_dir.file_name().unwrap().to_string_lossy().to_string();
            add_directory_to_zip(&mut zip, pkg_dir, &pkg_name, options, &mut records)?;
        }

        // Include local path dependencies (non-editable only)
        if self.include_local_deps {
            let path_deps = load_path_dependencies(&self.project_root).unwrap_or_default();
            for (dep_name, dep) in path_deps {
                // Only include non-editable dependencies
                if !dep.editable {
                    if let Ok(local_pkg_dir) = find_local_package_dir(&dep, &self.project_root) {
                        let local_pkg_name = local_pkg_dir
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        tracing::info!("Including local dependency '{}' in wheel", dep_name);
                        add_directory_to_zip(
                            &mut zip,
                            &local_pkg_dir,
                            &local_pkg_name,
                            options,
                            &mut records,
                        )?;
                    }
                }
            }
        }

        // Create dist-info directory
        let dist_info = format!("{}-{}.dist-info", normalized_name, version);

        // METADATA (PEP 566)
        let metadata = generate_metadata(&pyproject)?;
        let metadata_path = format!("{}/METADATA", dist_info);
        add_file_to_zip(
            &mut zip,
            &metadata_path,
            metadata.as_bytes(),
            options,
            &mut records,
        )?;

        // WHEEL file
        let wheel_content = generate_wheel_file();
        let wheel_file_path = format!("{}/WHEEL", dist_info);
        add_file_to_zip(
            &mut zip,
            &wheel_file_path,
            wheel_content.as_bytes(),
            options,
            &mut records,
        )?;

        // entry_points.txt (if scripts defined)
        if !project.scripts.is_empty() || !project.gui_scripts.is_empty() {
            let entry_points = generate_entry_points(project);
            let ep_path = format!("{}/entry_points.txt", dist_info);
            add_file_to_zip(
                &mut zip,
                &ep_path,
                entry_points.as_bytes(),
                options,
                &mut records,
            )?;
        }

        // top_level.txt
        if let Some(ref pkg_dir) = package_dir {
            let top_level = pkg_dir.file_name().unwrap().to_string_lossy().to_string();
            let tl_path = format!("{}/top_level.txt", dist_info);
            add_file_to_zip(
                &mut zip,
                &tl_path,
                format!("{}\n", top_level).as_bytes(),
                options,
                &mut records,
            )?;
        }

        // RECORD (must be last, contains all file hashes)
        let record_path = format!("{}/RECORD", dist_info);
        let mut record_content = String::new();
        for (path, hash, size) in &records {
            record_content.push_str(&format!("{},sha256={},{}\n", path, hash, size));
        }
        // RECORD itself has no hash
        record_content.push_str(&format!("{},,\n", record_path));

        zip.start_file(&record_path, options)
            .map_err(|e| Error::Zip(e.to_string()))?;
        zip.write_all(record_content.as_bytes())
            .map_err(Error::Io)?;

        zip.finish().map_err(|e| Error::Zip(e.to_string()))?;

        let size = std::fs::metadata(&wheel_path).map_err(Error::Io)?.len();

        Ok(BuildResult {
            path: wheel_path,
            size,
        })
    }

    /// Build a source distribution (PEP 517)
    pub fn build_sdist(&self, output_dir: &Path) -> Result<BuildResult> {
        let pyproject = PyProject::load(&self.project_root)?;
        let project = pyproject
            .project
            .as_ref()
            .ok_or(Error::MissingProjectMetadata)?;

        let name = &project.name;
        let version = project.version.as_ref().ok_or(Error::MissingVersion)?;

        // Create output directory
        std::fs::create_dir_all(output_dir).map_err(Error::Io)?;

        // Sdist filename: {name}-{version}.tar.gz
        let sdist_name = format!("{}-{}.tar.gz", name, version);
        let sdist_path = output_dir.join(&sdist_name);

        // Create tar.gz
        let file = std::fs::File::create(&sdist_path).map_err(Error::Io)?;
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(encoder);

        // Base directory in the archive
        let base_dir = format!("{}-{}", name, version);

        // Add pyproject.toml
        let pyproject_content =
            std::fs::read_to_string(self.project_root.join("pyproject.toml")).map_err(Error::Io)?;
        add_to_tar(
            &mut tar,
            &format!("{}/pyproject.toml", base_dir),
            pyproject_content.as_bytes(),
        )?;

        // Add PKG-INFO
        let pkg_info = generate_pkg_info(&pyproject)?;
        add_to_tar(
            &mut tar,
            &format!("{}/PKG-INFO", base_dir),
            pkg_info.as_bytes(),
        )?;

        // Add README if exists
        for readme in &["README.md", "README.rst", "README.txt", "README"] {
            let readme_path = self.project_root.join(readme);
            if readme_path.exists() {
                let content = std::fs::read_to_string(&readme_path).map_err(Error::Io)?;
                add_to_tar(
                    &mut tar,
                    &format!("{}/{}", base_dir, readme),
                    content.as_bytes(),
                )?;
                break;
            }
        }

        // Add LICENSE if exists
        for license in &["LICENSE", "LICENSE.txt", "LICENSE.md", "COPYING"] {
            let license_path = self.project_root.join(license);
            if license_path.exists() {
                let content = std::fs::read_to_string(&license_path).map_err(Error::Io)?;
                add_to_tar(
                    &mut tar,
                    &format!("{}/{}", base_dir, license),
                    content.as_bytes(),
                )?;
                break;
            }
        }

        // Add source files
        let src_dir = self.project_root.join("src");
        if src_dir.exists() {
            add_directory_to_tar(&mut tar, &src_dir, &format!("{}/src", base_dir))?;
        } else {
            // Flat layout - add package directory
            let pkg_dir = self.project_root.join(name.replace('-', "_"));
            if pkg_dir.exists() {
                let pkg_name = pkg_dir.file_name().unwrap().to_string_lossy().to_string();
                add_directory_to_tar(&mut tar, &pkg_dir, &format!("{}/{}", base_dir, pkg_name))?;
            }
        }

        // Add tests if they exist
        let tests_dir = self.project_root.join("tests");
        if tests_dir.exists() {
            add_directory_to_tar(&mut tar, &tests_dir, &format!("{}/tests", base_dir))?;
        }

        tar.finish().map_err(|e| Error::Tar(e.to_string()))?;

        let size = std::fs::metadata(&sdist_path).map_err(Error::Io)?.len();

        Ok(BuildResult {
            path: sdist_path,
            size,
        })
    }

    /// Get the project root
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}

/// Normalize package name for wheel filename (PEP 427)
fn normalize_name(name: &str) -> String {
    name.replace(['-', '.'], "_")
}

/// Find a Python package directory in the given directory
fn find_package_in_dir(dir: &Path) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("__init__.py").exists() {
                return Some(path);
            }
        }
    }
    None
}

/// Find the package directory for a local path dependency
fn find_local_package_dir(dep: &PathDependency, base_dir: &Path) -> Result<PathBuf> {
    let resolved_path = dep.resolve_path(base_dir);
    let normalized_name = dep.name.replace('-', "_");

    // Try src layout first: src/<name>/
    let src_layout = resolved_path.join("src").join(&normalized_name);
    if src_layout.exists() && src_layout.join("__init__.py").exists() {
        return Ok(src_layout);
    }

    // Try flat layout: <name>/
    let flat_layout = resolved_path.join(&normalized_name);
    if flat_layout.exists() && flat_layout.join("__init__.py").exists() {
        return Ok(flat_layout);
    }

    // Try to find any package in src/
    let src_dir = resolved_path.join("src");
    if src_dir.exists() {
        if let Some(pkg) = find_package_in_dir(&src_dir) {
            return Ok(pkg);
        }
    }

    // Try to find any package at root
    if let Some(pkg) = find_package_in_dir(&resolved_path) {
        return Ok(pkg);
    }

    Err(Error::Config(format!(
        "Could not find Python package for local dependency '{}' in {}",
        dep.name,
        resolved_path.display()
    )))
}

/// Add a file to the zip archive and record its hash
fn add_file_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    path: &str,
    content: &[u8],
    options: SimpleFileOptions,
    records: &mut Vec<(String, String, u64)>,
) -> Result<()> {
    zip.start_file(path, options)
        .map_err(|e| Error::Zip(e.to_string()))?;
    zip.write_all(content).map_err(Error::Io)?;

    let hash = base64_urlsafe_nopad(&Sha256::digest(content));
    records.push((path.to_string(), hash, content.len() as u64));

    Ok(())
}

/// Add a directory recursively to the zip archive
fn add_directory_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    dir: &Path,
    prefix: &str,
    options: SimpleFileOptions,
    records: &mut Vec<(String, String, u64)>,
) -> Result<()> {
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip __pycache__ and .pyc files
        if path.to_string_lossy().contains("__pycache__") {
            continue;
        }
        if let Some(ext) = path.extension() {
            if ext == "pyc" || ext == "pyo" {
                continue;
            }
        }

        if path.is_file() {
            let relative = path.strip_prefix(dir).unwrap();
            let archive_path = format!(
                "{}/{}",
                prefix,
                relative.to_string_lossy().replace('\\', "/")
            );

            let mut content = Vec::new();
            std::fs::File::open(path)
                .map_err(Error::Io)?
                .read_to_end(&mut content)
                .map_err(Error::Io)?;

            add_file_to_zip(zip, &archive_path, &content, options, records)?;
        }
    }

    Ok(())
}

/// Generate METADATA file content (PEP 566)
fn generate_metadata(pyproject: &PyProject) -> Result<String> {
    let project = pyproject
        .project
        .as_ref()
        .ok_or(Error::MissingProjectMetadata)?;

    let mut metadata = String::new();
    metadata.push_str("Metadata-Version: 2.1\n");
    metadata.push_str(&format!("Name: {}\n", project.name));

    if let Some(ref version) = project.version {
        metadata.push_str(&format!("Version: {}\n", version));
    }

    if let Some(ref description) = project.description {
        metadata.push_str(&format!("Summary: {}\n", description));
    }

    if let Some(ref requires_python) = project.requires_python {
        metadata.push_str(&format!("Requires-Python: {}\n", requires_python));
    }

    // Authors
    for author in &project.authors {
        if let Some(ref name) = author.name {
            if let Some(ref email) = author.email {
                metadata.push_str(&format!("Author-email: {} <{}>\n", name, email));
            } else {
                metadata.push_str(&format!("Author: {}\n", name));
            }
        }
    }

    // License
    if let Some(ref license) = project.license {
        match license {
            crate::pep::License::Text { text } => {
                metadata.push_str(&format!("License: {}\n", text));
            }
            crate::pep::License::File { .. } => {
                // License file is included separately
            }
        }
    }

    // Classifiers
    for classifier in &project.classifiers {
        metadata.push_str(&format!("Classifier: {}\n", classifier));
    }

    // Keywords
    if !project.keywords.is_empty() {
        metadata.push_str(&format!("Keywords: {}\n", project.keywords.join(",")));
    }

    // URLs
    for (label, url) in &project.urls {
        metadata.push_str(&format!("Project-URL: {}, {}\n", label, url));
    }

    // Dependencies
    for dep in &project.dependencies {
        metadata.push_str(&format!("Requires-Dist: {}\n", dep));
    }

    // Optional dependencies (extras)
    for (extra, deps) in &project.optional_dependencies {
        for dep in deps {
            metadata.push_str(&format!(
                "Requires-Dist: {} ; extra == \"{}\"\n",
                dep, extra
            ));
        }
        metadata.push_str(&format!("Provides-Extra: {}\n", extra));
    }

    // Long description (from README)
    if let Some(ref readme) = project.readme {
        match readme {
            crate::pep::Readme::Path(path) => {
                let readme_path = pyproject
                    .project
                    .as_ref()
                    .map(|_| Path::new(path))
                    .unwrap_or(Path::new(path));

                if let Ok(content) = std::fs::read_to_string(readme_path) {
                    let content_type = if path.ends_with(".md") {
                        "text/markdown"
                    } else if path.ends_with(".rst") {
                        "text/x-rst"
                    } else {
                        "text/plain"
                    };
                    metadata.push_str(&format!("Description-Content-Type: {}\n", content_type));
                    metadata.push('\n');
                    metadata.push_str(&content);
                }
            }
            crate::pep::Readme::Inline {
                text, content_type, ..
            } => {
                if let Some(ref ct) = content_type {
                    metadata.push_str(&format!("Description-Content-Type: {}\n", ct));
                }
                if let Some(ref t) = text {
                    metadata.push('\n');
                    metadata.push_str(t);
                }
            }
        }
    }

    Ok(metadata)
}

/// Generate WHEEL file content
fn generate_wheel_file() -> String {
    let mut wheel = String::new();
    wheel.push_str("Wheel-Version: 1.0\n");
    wheel.push_str("Generator: rx (T-Rex)\n");
    wheel.push_str("Root-Is-Purelib: true\n");
    wheel.push_str("Tag: py3-none-any\n");
    wheel
}

/// Generate entry_points.txt content
fn generate_entry_points(project: &crate::pep::ProjectMetadata) -> String {
    let mut content = String::new();

    if !project.scripts.is_empty() {
        content.push_str("[console_scripts]\n");
        for (name, entry) in &project.scripts {
            content.push_str(&format!("{} = {}\n", name, entry));
        }
    }

    if !project.gui_scripts.is_empty() {
        if !content.is_empty() {
            content.push('\n');
        }
        content.push_str("[gui_scripts]\n");
        for (name, entry) in &project.gui_scripts {
            content.push_str(&format!("{} = {}\n", name, entry));
        }
    }

    for (group, entries) in &project.entry_points {
        if !content.is_empty() {
            content.push('\n');
        }
        content.push_str(&format!("[{}]\n", group));
        for (name, entry) in entries {
            content.push_str(&format!("{} = {}\n", name, entry));
        }
    }

    content
}

/// Generate PKG-INFO for sdist
fn generate_pkg_info(pyproject: &PyProject) -> Result<String> {
    // PKG-INFO is essentially the same as METADATA
    generate_metadata(pyproject)
}

/// Add content to tar archive
fn add_to_tar<W: Write>(tar: &mut tar::Builder<W>, path: &str, content: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header
        .set_path(path)
        .map_err(|e| Error::Tar(e.to_string()))?;
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_cksum();

    tar.append(&header, content)
        .map_err(|e| Error::Tar(e.to_string()))?;

    Ok(())
}

/// Add directory recursively to tar archive
fn add_directory_to_tar<W: Write>(
    tar: &mut tar::Builder<W>,
    dir: &Path,
    prefix: &str,
) -> Result<()> {
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip __pycache__ and .pyc files
        if path.to_string_lossy().contains("__pycache__") {
            continue;
        }
        if let Some(ext) = path.extension() {
            if ext == "pyc" || ext == "pyo" {
                continue;
            }
        }

        if path.is_file() {
            let relative = path.strip_prefix(dir).unwrap();
            let archive_path = format!(
                "{}/{}",
                prefix,
                relative.to_string_lossy().replace('\\', "/")
            );

            let content = std::fs::read(path).map_err(Error::Io)?;
            add_to_tar(tar, &archive_path, &content)?;
        }
    }

    Ok(())
}

/// Base64 URL-safe encoding without padding (for RECORD hashes)
fn base64_urlsafe_nopad(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_name() {
        assert_eq!(normalize_name("my-package"), "my_package");
        assert_eq!(normalize_name("my.package"), "my_package");
        assert_eq!(normalize_name("mypackage"), "mypackage");
    }
}
