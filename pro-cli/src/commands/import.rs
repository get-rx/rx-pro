//! Import command - migrate from other package managers
//!
//! Supports importing from:
//! - Poetry (pyproject.toml + poetry.lock)
//! - requirements.txt
//! - uv (uv.lock)
//!
//! ```bash
//! # Import from Poetry project
//! rx import poetry
//!
//! # Import from requirements.txt
//! rx import requirements
//! rx import requirements --file requirements-dev.txt --dev
//!
//! # Import from uv project
//! rx import uv
//!
//! # Import from specific directory
//! rx import poetry --project ../other-project
//!
//! # Dry run (show what would be imported)
//! rx import poetry --dry-run
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use serde::Deserialize;

use pro_core::lockfile::LockedPackage;
use pro_core::pep::PyProject;
use pro_core::Lockfile;

#[derive(Args)]
pub struct ImportCommand {
    #[command(subcommand)]
    pub command: ImportSubcommand,
}

#[derive(Subcommand)]
pub enum ImportSubcommand {
    /// Import from Poetry project
    Poetry(PoetryImportCommand),
    /// Import from requirements.txt file
    Requirements(RequirementsImportCommand),
    /// Import from uv project (uv.lock)
    Uv(UvImportCommand),
}

impl ImportCommand {
    pub async fn run(self) -> Result<()> {
        match self.command {
            ImportSubcommand::Poetry(cmd) => cmd.run().await,
            ImportSubcommand::Requirements(cmd) => cmd.run().await,
            ImportSubcommand::Uv(cmd) => cmd.run().await,
        }
    }
}

#[derive(Args)]
pub struct PoetryImportCommand {
    /// Project directory containing Poetry files
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Show what would be imported without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Keep Poetry files after import (don't suggest removal)
    #[arg(long)]
    pub keep_poetry: bool,
}

impl PoetryImportCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let pyproject_path = project_dir.join("pyproject.toml");
        if !pyproject_path.exists() {
            bail!("No pyproject.toml found in {}", project_dir.display());
        }

        println!("Importing from Poetry project: {}", project_dir.display());
        println!();

        // Read pyproject.toml
        let content = std::fs::read_to_string(&pyproject_path)?;
        let doc: toml::Value = toml::from_str(&content)?;

        // Check if this is a Poetry project
        let poetry_section = doc.get("tool").and_then(|t| t.get("poetry"));

        if poetry_section.is_none() {
            bail!("No [tool.poetry] section found. Is this a Poetry project?");
        }

        let poetry = poetry_section.unwrap();

        // Extract project metadata
        let name = poetry
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing project name"))?;

        let version = poetry
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.1.0");

        let description = poetry
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let python_requires = poetry
            .get("python")
            .and_then(|v| v.as_str())
            .map(convert_poetry_python_constraint)
            .unwrap_or_else(|| ">=3.8".to_string());

        println!("Project: {} v{}", name, version);
        println!("Python: {}", python_requires);
        println!();

        // Extract dependencies
        let mut main_deps = Vec::new();
        let mut dev_deps = Vec::new();

        if let Some(deps) = poetry.get("dependencies").and_then(|v| v.as_table()) {
            for (dep_name, constraint) in deps {
                if dep_name == "python" {
                    continue; // Skip python version constraint
                }
                let constraint_str = convert_poetry_constraint(constraint);
                main_deps.push(format!("{}{}", dep_name, constraint_str));
            }
        }

        // Dev dependencies from [tool.poetry.group.dev.dependencies] or [tool.poetry.dev-dependencies]
        if let Some(groups) = poetry.get("group").and_then(|v| v.as_table()) {
            if let Some(dev_group) = groups.get("dev").and_then(|v| v.as_table()) {
                if let Some(deps) = dev_group.get("dependencies").and_then(|v| v.as_table()) {
                    for (dep_name, constraint) in deps {
                        let constraint_str = convert_poetry_constraint(constraint);
                        dev_deps.push(format!("{}{}", dep_name, constraint_str));
                    }
                }
            }
        }

        // Also check legacy dev-dependencies
        if let Some(deps) = poetry.get("dev-dependencies").and_then(|v| v.as_table()) {
            for (dep_name, constraint) in deps {
                let constraint_str = convert_poetry_constraint(constraint);
                dev_deps.push(format!("{}{}", dep_name, constraint_str));
            }
        }

        println!("Dependencies ({}):", main_deps.len());
        for dep in &main_deps {
            println!("  - {}", dep);
        }

        if !dev_deps.is_empty() {
            println!();
            println!("Dev dependencies ({}):", dev_deps.len());
            for dep in &dev_deps {
                println!("  - {}", dep);
            }
        }

        // Try to import poetry.lock if it exists
        let lockfile_path = project_dir.join("poetry.lock");
        let lockfile = if lockfile_path.exists() {
            println!();
            println!("Found poetry.lock, importing locked versions...");
            Some(import_poetry_lock(&lockfile_path)?)
        } else {
            None
        };

        if self.dry_run {
            println!();
            println!("Dry run - no changes made.");
            return Ok(());
        }

        // Create new pyproject.toml for rx
        let mut pyproject = PyProject::new(name, version, &python_requires);
        if let Some(ref mut proj) = pyproject.project {
            proj.description = if description.is_empty() {
                None
            } else {
                Some(description.to_string())
            };
        }

        // Add dependencies
        for dep in &main_deps {
            pyproject.add_dependency(dep.clone());
        }
        for dep in &dev_deps {
            pyproject.add_dev_dependency(dep.clone());
        }

        // Extract scripts/entry points
        if let Some(scripts) = poetry.get("scripts").and_then(|v| v.as_table()) {
            if let Some(ref mut proj) = pyproject.project {
                for (name, value) in scripts {
                    if let Some(entry) = value.as_str() {
                        proj.scripts.insert(name.clone(), entry.to_string());
                    }
                }
            }
        }

        // Add build system
        pyproject.build_system = Some(pro_core::pep::pep621::BuildSystem {
            requires: vec!["hatchling".to_string()],
            build_backend: Some("hatchling.build".to_string()),
            backend_path: None,
        });

        // Save new pyproject.toml
        pyproject.save(&project_dir)?;
        println!();
        println!("✓ Updated pyproject.toml for Pro");

        // Save lockfile if we imported one
        if let Some(lockfile) = lockfile {
            let rx_lock_path = project_dir.join("rx.lock");
            lockfile.save(&rx_lock_path)?;
            println!("✓ Created rx.lock from poetry.lock");
        }

        println!();
        println!("Migration complete!");
        println!();
        println!("Next steps:");
        println!("  rx sync      # Install dependencies");

        if !self.keep_poetry {
            println!();
            println!("You can now remove Poetry files:");
            println!("  rm poetry.lock");
            println!("  # Edit pyproject.toml to remove [tool.poetry] section");
        }

        Ok(())
    }
}

// =============================================================================
// Requirements.txt Import
// =============================================================================

#[derive(Args)]
pub struct RequirementsImportCommand {
    /// Path to requirements.txt file (default: requirements.txt in current directory)
    #[arg(long, short)]
    pub file: Option<PathBuf>,

    /// Project directory for output (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Import as dev dependencies
    #[arg(long)]
    pub dev: bool,

    /// Show what would be imported without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Project name (auto-detected from directory if not specified)
    #[arg(long)]
    pub name: Option<String>,

    /// Python version requirement (e.g., ">=3.8")
    #[arg(long, default_value = ">=3.8")]
    pub python: String,
}

impl RequirementsImportCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Determine requirements file path
        let req_file = self
            .file
            .unwrap_or_else(|| project_dir.join("requirements.txt"));

        if !req_file.exists() {
            bail!("Requirements file not found: {}", req_file.display());
        }

        println!("Importing from: {}", req_file.display());
        println!();

        // Parse requirements
        let content = std::fs::read_to_string(&req_file)?;
        let (deps, hashes) =
            parse_requirements_txt(&content, req_file.parent().unwrap_or(&project_dir))?;

        if deps.is_empty() {
            println!("No dependencies found in requirements file.");
            return Ok(());
        }

        // Display what we found
        let dep_type = if self.dev {
            "Dev dependencies"
        } else {
            "Dependencies"
        };
        println!("{} ({}):", dep_type, deps.len());
        for dep in &deps {
            println!("  - {}", dep);
        }

        if !hashes.is_empty() {
            println!();
            println!(
                "Note: {} package(s) have pinned hashes that will be used when locking.",
                hashes.len()
            );
        }

        if self.dry_run {
            println!();
            println!("Dry run - no changes made.");
            return Ok(());
        }

        // Determine project name
        let project_name = self.name.unwrap_or_else(|| {
            project_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("myproject")
                .to_string()
        });

        // Check if pyproject.toml already exists
        let pyproject_path = project_dir.join("pyproject.toml");
        let mut pyproject = if pyproject_path.exists() {
            println!();
            println!("Found existing pyproject.toml, adding dependencies...");
            PyProject::load(&project_dir)?
        } else {
            println!();
            println!("Creating new pyproject.toml...");
            let mut p = PyProject::new(&project_name, "0.1.0", &self.python);
            // Use hatchling as build backend for new projects
            p.build_system = Some(pro_core::pep::pep621::BuildSystem {
                requires: vec!["hatchling".to_string()],
                build_backend: Some("hatchling.build".to_string()),
                backend_path: None,
            });
            p
        };

        // Add dependencies
        for dep in &deps {
            if self.dev {
                pyproject.add_dev_dependency(dep.clone());
            } else {
                pyproject.add_dependency(dep.clone());
            }
        }

        // Save
        pyproject.save(&project_dir)?;
        println!("✓ Updated pyproject.toml");

        println!();
        println!("Migration complete!");
        println!();
        println!("Next steps:");
        println!("  rx lock      # Generate lockfile with resolved versions");
        println!("  rx sync      # Install dependencies");

        Ok(())
    }
}

/// Parse requirements.txt content into a list of dependency strings
fn parse_requirements_txt(
    content: &str,
    base_dir: &Path,
) -> Result<(Vec<String>, HashMap<String, Vec<String>>)> {
    let mut deps = Vec::new();
    let mut hashes: HashMap<String, Vec<String>> = HashMap::new();
    let mut current_line = String::new();

    for line in content.lines() {
        let line = line.trim();

        // Handle line continuation
        if let Some(stripped) = current_line.strip_suffix('\\') {
            current_line = format!("{}{}", stripped.trim(), line);
            continue;
        } else if !current_line.is_empty() {
            // Process the completed line
            process_requirement_line(&current_line, base_dir, &mut deps, &mut hashes)?;
            current_line = String::new();
        }

        if line.ends_with('\\') {
            current_line = line.to_string();
            continue;
        }

        process_requirement_line(line, base_dir, &mut deps, &mut hashes)?;
    }

    // Process any remaining line
    if !current_line.is_empty() {
        let line = current_line.trim_end_matches('\\').trim();
        process_requirement_line(line, base_dir, &mut deps, &mut hashes)?;
    }

    Ok((deps, hashes))
}

fn process_requirement_line(
    line: &str,
    base_dir: &Path,
    deps: &mut Vec<String>,
    hashes: &mut HashMap<String, Vec<String>>,
) -> Result<()> {
    let line = line.trim();

    // Skip empty lines and comments
    if line.is_empty() || line.starts_with('#') {
        return Ok(());
    }

    // Handle -r (recursive include)
    if let Some(path) = line
        .strip_prefix("-r ")
        .or_else(|| line.strip_prefix("-r\t"))
    {
        let include_path = base_dir.join(path.trim());
        if include_path.exists() {
            let content = std::fs::read_to_string(&include_path)?;
            let (included_deps, included_hashes) =
                parse_requirements_txt(&content, include_path.parent().unwrap_or(base_dir))?;
            deps.extend(included_deps);
            for (k, v) in included_hashes {
                hashes.entry(k).or_default().extend(v);
            }
        }
        return Ok(());
    }

    // Handle -c (constraints) - skip for now, constraints are not direct dependencies
    if line.starts_with("-c ") || line.starts_with("-c\t") {
        return Ok(());
    }

    // Handle -e (editable installs) - skip for now, these need special handling
    if line.starts_with("-e ") || line.starts_with("-e\t") {
        // TODO: Add support for editable/path dependencies
        println!("  Warning: Skipping editable install: {}", line);
        return Ok(());
    }

    // Skip pip options
    if line.starts_with('-') {
        return Ok(());
    }

    // Parse the requirement, handling --hash options
    let (req_part, hash_list) = extract_hashes(line);

    if req_part.is_empty() {
        return Ok(());
    }

    // Skip URL/path dependencies for now (they need special handling)
    if req_part.starts_with("http://")
        || req_part.starts_with("https://")
        || req_part.starts_with("git+")
        || req_part.starts_with("git://")
        || req_part.starts_with("./")
        || req_part.starts_with("../")
        || req_part.starts_with('/')
    {
        println!("  Warning: Skipping URL/path dependency: {}", req_part);
        return Ok(());
    }

    // Validate it's a proper requirement
    if pro_core::pep::Requirement::parse(req_part).is_ok() {
        deps.push(req_part.to_string());

        // Store hashes if present
        if !hash_list.is_empty() {
            let name = req_part
                .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                .next()
                .unwrap_or(req_part)
                .to_lowercase();
            hashes.insert(name, hash_list);
        }
    }

    Ok(())
}

/// Extract requirement and hashes from a line like "requests==2.28.0 --hash=sha256:abc"
fn extract_hashes(line: &str) -> (&str, Vec<String>) {
    let mut hashes = Vec::new();
    let mut req_end = line.len();

    // Find all --hash= occurrences
    let parts: Vec<&str> = line.split("--hash=").collect();
    if parts.len() > 1 {
        req_end = line.find("--hash=").unwrap_or(line.len());
        for hash_part in &parts[1..] {
            // Hash ends at whitespace or end of string
            let hash = hash_part.split_whitespace().next().unwrap_or(hash_part);
            hashes.push(hash.to_string());
        }
    }

    (line[..req_end].trim(), hashes)
}

// =============================================================================
// UV Import
// =============================================================================

#[derive(Args)]
pub struct UvImportCommand {
    /// Project directory containing uv.lock
    #[arg(long, default_value = ".")]
    pub project: PathBuf,

    /// Show what would be imported without making changes
    #[arg(long)]
    pub dry_run: bool,
}

impl UvImportCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        let pyproject_path = project_dir.join("pyproject.toml");
        let uv_lock_path = project_dir.join("uv.lock");

        // Check for pyproject.toml first (to get project metadata)
        if !pyproject_path.exists() {
            bail!(
                "No pyproject.toml found in {}. UV projects require pyproject.toml.",
                project_dir.display()
            );
        }

        println!("Importing from UV project: {}", project_dir.display());
        println!();

        // Load existing pyproject.toml
        let content = std::fs::read_to_string(&pyproject_path)?;
        let doc: toml::Value = toml::from_str(&content)?;

        // Extract project metadata
        let project = doc.get("project");
        let name = project
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing project name in pyproject.toml"))?;

        let version = project
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.1.0");

        let python_requires = project
            .and_then(|p| p.get("requires-python"))
            .and_then(|v| v.as_str())
            .unwrap_or(">=3.8");

        println!("Project: {} v{}", name, version);
        println!("Python: {}", python_requires);
        println!();

        // Get dependencies from pyproject.toml (UV uses standard PEP 621)
        let main_deps: Vec<String> = project
            .and_then(|p| p.get("dependencies"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Get optional/dev dependencies
        let mut dev_deps: Vec<String> = Vec::new();
        if let Some(optional) = project
            .and_then(|p| p.get("optional-dependencies"))
            .and_then(|v| v.as_table())
        {
            if let Some(dev) = optional.get("dev").and_then(|v| v.as_array()) {
                dev_deps = dev
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
            }
        }

        println!("Dependencies ({}):", main_deps.len());
        for dep in &main_deps {
            println!("  - {}", dep);
        }

        if !dev_deps.is_empty() {
            println!();
            println!("Dev dependencies ({}):", dev_deps.len());
            for dep in &dev_deps {
                println!("  - {}", dep);
            }
        }

        // Try to import uv.lock if it exists
        let lockfile = if uv_lock_path.exists() {
            println!();
            println!("Found uv.lock, importing locked versions...");
            Some(import_uv_lock(&uv_lock_path)?)
        } else {
            None
        };

        if self.dry_run {
            println!();
            println!("Dry run - no changes made.");
            return Ok(());
        }

        // Create new pyproject.toml for rx
        let mut pyproject = PyProject::new(name, version, python_requires);

        // Copy description if present
        if let Some(desc) = project
            .and_then(|p| p.get("description"))
            .and_then(|v| v.as_str())
        {
            if let Some(ref mut proj) = pyproject.project {
                proj.description = Some(desc.to_string());
            }
        }

        // Add dependencies
        for dep in &main_deps {
            pyproject.add_dependency(dep.clone());
        }
        for dep in &dev_deps {
            pyproject.add_dev_dependency(dep.clone());
        }

        // Copy scripts if present
        if let Some(scripts) = project
            .and_then(|p| p.get("scripts"))
            .and_then(|v| v.as_table())
        {
            if let Some(ref mut proj) = pyproject.project {
                for (name, value) in scripts {
                    if let Some(entry) = value.as_str() {
                        proj.scripts.insert(name.clone(), entry.to_string());
                    }
                }
            }
        }

        // Set build system
        pyproject.build_system = Some(pro_core::pep::pep621::BuildSystem {
            requires: vec!["hatchling".to_string()],
            build_backend: Some("hatchling.build".to_string()),
            backend_path: None,
        });

        // Save pyproject.toml
        pyproject.save(&project_dir)?;
        println!();
        println!("✓ Updated pyproject.toml for rx");

        // Save lockfile if we imported one
        if let Some(lockfile) = lockfile {
            let rx_lock_path = project_dir.join("rx.lock");
            lockfile.save(&rx_lock_path)?;
            println!("✓ Created rx.lock from uv.lock");
        }

        println!();
        println!("Migration complete!");
        println!();
        println!("Next steps:");
        println!("  rx sync      # Install dependencies");
        println!();
        println!("You can now remove UV files:");
        println!("  rm uv.lock");

        Ok(())
    }
}

/// Import uv.lock into rx.lock format
fn import_uv_lock(path: &PathBuf) -> Result<Lockfile> {
    let content = std::fs::read_to_string(path)?;

    // UV lock is TOML with [[distribution]] entries
    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct UvLock {
        #[serde(default)]
        version: Option<u32>,
        #[serde(default, rename = "requires-python")]
        requires_python: Option<String>,
        #[serde(default, rename = "distribution")]
        distributions: Vec<UvDistribution>,
        // Also support the "package" key used in some uv versions
        #[serde(default)]
        package: Vec<UvDistribution>,
    }

    #[derive(Deserialize)]
    struct UvDistribution {
        name: String,
        version: String,
        #[serde(default)]
        source: Option<UvSource>,
        #[serde(default)]
        dependencies: Vec<UvDependency>,
        #[serde(default)]
        wheels: Vec<UvWheel>,
        #[serde(default)]
        sdist: Option<UvSdist>,
    }

    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct UvSource {
        #[serde(rename = "type")]
        source_type: Option<String>,
        url: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum UvDependency {
        Simple(String),
        Complex { name: String },
    }

    #[derive(Deserialize)]
    struct UvWheel {
        url: Option<String>,
        hash: Option<String>,
        #[serde(default)]
        requires_python: Option<String>,
    }

    #[derive(Deserialize)]
    struct UvSdist {
        url: Option<String>,
        hash: Option<String>,
    }

    let uv_lock: UvLock = toml::from_str(&content).context("Failed to parse uv.lock")?;

    let mut lockfile = Lockfile::new();

    // Use distributions or package (depending on uv version)
    let distributions = if !uv_lock.distributions.is_empty() {
        uv_lock.distributions
    } else {
        uv_lock.package
    };

    for dist in distributions {
        // Extract dependencies
        let dependencies: Vec<String> = dist
            .dependencies
            .iter()
            .map(|d| match d {
                UvDependency::Simple(s) => {
                    // Parse out just the package name from strings like "urllib3>=1.0"
                    s.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                        .next()
                        .unwrap_or(s)
                        .to_lowercase()
                        .replace('_', "-")
                }
                UvDependency::Complex { name } => name.to_lowercase().replace('_', "-"),
            })
            .collect();

        // Get URL and hash from wheels or sdist
        let (url, hash) = if let Some(wheel) = dist.wheels.first() {
            (wheel.url.clone(), wheel.hash.clone())
        } else if let Some(ref sdist) = dist.sdist {
            (sdist.url.clone(), sdist.hash.clone())
        } else if let Some(ref source) = dist.source {
            (source.url.clone(), None)
        } else {
            (None, None)
        };

        // Convert platform-specific wheels to files
        let files: Vec<_> = dist
            .wheels
            .iter()
            .skip(1) // First one is already the default
            .filter_map(|w| {
                Some(pro_core::lockfile::PlatformFile {
                    url: w.url.clone()?,
                    hash: w.hash.clone().unwrap_or_default(),
                    markers: None,
                    python: w.requires_python.clone(),
                    tags: None,
                })
            })
            .collect();

        lockfile.packages.insert(
            dist.name.to_lowercase().replace('_', "-"),
            LockedPackage {
                version: dist.version,
                url,
                hash,
                dependencies,
                markers: None,
                files,
            },
        );
    }

    Ok(lockfile)
}

// =============================================================================
// Poetry Import Helpers
// =============================================================================

/// Convert Poetry version constraint to PEP 440
fn convert_poetry_constraint(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => convert_poetry_version_string(s),
        toml::Value::Table(t) => {
            // Handle complex constraints like { version = "^1.0", extras = ["dev"] }
            if let Some(version) = t.get("version").and_then(|v| v.as_str()) {
                let base = convert_poetry_version_string(version);
                if let Some(extras) = t.get("extras").and_then(|v| v.as_array()) {
                    let extras_str: Vec<_> = extras.iter().filter_map(|e| e.as_str()).collect();
                    if !extras_str.is_empty() {
                        return format!("[{}]{}", extras_str.join(","), base);
                    }
                }
                base
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

/// Convert Poetry version string to PEP 440
fn convert_poetry_version_string(version: &str) -> String {
    let v = version.trim();

    if v == "*" {
        return String::new(); // Any version
    }

    if let Some(ver) = v.strip_prefix('^') {
        // Caret requirement: ^1.2.3 means >=1.2.3 <2.0.0
        let parts: Vec<&str> = ver.split('.').collect();
        match parts.as_slice() {
            [major, ..] if *major != "0" => {
                let next_major: u32 = major.parse().unwrap_or(1) + 1;
                format!(">={},<{}.0.0", ver, next_major)
            }
            [_, minor, ..] if *minor != "0" => {
                let next_minor: u32 = minor.parse().unwrap_or(0) + 1;
                format!(">={}.<0.{}.0", ver, next_minor)
            }
            _ => format!(">={}", ver),
        }
    } else if let Some(ver) = v.strip_prefix('~') {
        // Tilde requirement: ~1.2.3 means >=1.2.3 <1.3.0
        let parts: Vec<&str> = ver.split('.').collect();
        if parts.len() >= 2 {
            let major = parts[0];
            let minor: u32 = parts[1].parse().unwrap_or(0) + 1;
            format!(">={},<{}.{}.0", ver, major, minor)
        } else {
            format!(">={}", ver)
        }
    } else if v.contains("||") {
        // OR constraint
        v.split("||")
            .map(|p| convert_poetry_version_string(p.trim()))
            .collect::<Vec<_>>()
            .join(" || ")
    } else if !v.starts_with('>')
        && !v.starts_with('<')
        && !v.starts_with('=')
        && !v.starts_with('!')
    {
        // Plain version: 1.2.3 means ==1.2.3
        format!("=={}", v)
    } else {
        v.to_string()
    }
}

/// Convert Poetry python constraint to PEP 440
fn convert_poetry_python_constraint(constraint: &str) -> String {
    // Poetry uses ^3.8 or ~3.8 style, convert to >=3.8
    let v = constraint.trim();
    // Handle both ^ and ~ prefixes the same way - convert to >=
    if let Some(stripped) = v.strip_prefix('^').or_else(|| v.strip_prefix('~')) {
        format!(">={}", stripped)
    } else {
        v.to_string()
    }
}

/// Import poetry.lock into rx.lock format
fn import_poetry_lock(path: &PathBuf) -> Result<Lockfile> {
    let content = std::fs::read_to_string(path)?;

    // Poetry lock is TOML
    #[derive(Deserialize)]
    struct PoetryLock {
        #[serde(default)]
        package: Vec<PoetryPackage>,
    }

    #[derive(Deserialize)]
    struct PoetryPackage {
        name: String,
        version: String,
        #[serde(default)]
        dependencies: HashMap<String, toml::Value>,
        #[serde(default)]
        files: Vec<PoetryFile>,
    }

    #[derive(Deserialize)]
    struct PoetryFile {
        file: String,
        hash: String,
    }

    let poetry_lock: PoetryLock =
        toml::from_str(&content).context("Failed to parse poetry.lock")?;

    let mut lockfile = Lockfile::new();

    for pkg in poetry_lock.package {
        // Extract dependencies
        let dependencies: Vec<String> = pkg
            .dependencies
            .keys()
            .map(|k| k.to_lowercase().replace('_', "-"))
            .collect();

        // Get first file's hash if available
        let (url, hash) = if let Some(file) = pkg.files.first() {
            // Poetry doesn't store URLs, so we construct PyPI URL
            let name_normalized = pkg.name.to_lowercase().replace('_', "-");
            let url = format!(
                "https://files.pythonhosted.org/packages/{}/{}",
                name_normalized, file.file
            );
            let hash = format!("sha256:{}", file.hash.trim_start_matches("sha256:"));
            (Some(url), Some(hash))
        } else {
            (None, None)
        };

        lockfile.packages.insert(
            pkg.name.to_lowercase().replace('_', "-"),
            LockedPackage {
                version: pkg.version,
                url,
                hash,
                dependencies,
                markers: None,
                files: vec![],
            },
        );
    }

    Ok(lockfile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_convert_poetry_caret() {
        assert_eq!(convert_poetry_version_string("^1.2.3"), ">=1.2.3,<2.0.0");
        assert_eq!(convert_poetry_version_string("^2.0"), ">=2.0,<3.0.0");
    }

    #[test]
    fn test_convert_poetry_tilde() {
        assert_eq!(convert_poetry_version_string("~1.2.3"), ">=1.2.3,<1.3.0");
    }

    #[test]
    fn test_convert_poetry_exact() {
        assert_eq!(convert_poetry_version_string("1.2.3"), "==1.2.3");
    }

    #[test]
    fn test_convert_poetry_any() {
        assert_eq!(convert_poetry_version_string("*"), "");
    }

    // Requirements.txt tests

    #[test]
    fn test_parse_requirements_simple() {
        let content = "requests==2.28.0\nnumpy>=1.20\n";
        let base_dir = PathBuf::from(".");
        let (deps, hashes) = parse_requirements_txt(content, &base_dir).unwrap();

        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"requests==2.28.0".to_string()));
        assert!(deps.contains(&"numpy>=1.20".to_string()));
        assert!(hashes.is_empty());
    }

    #[test]
    fn test_parse_requirements_with_comments() {
        let content = r#"# Main dependencies
requests==2.28.0
# HTTP library

numpy>=1.20
"#;
        let base_dir = PathBuf::from(".");
        let (deps, _) = parse_requirements_txt(content, &base_dir).unwrap();

        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_parse_requirements_with_extras() {
        let content = "requests[security]>=2.28.0\n";
        let base_dir = PathBuf::from(".");
        let (deps, _) = parse_requirements_txt(content, &base_dir).unwrap();

        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], "requests[security]>=2.28.0");
    }

    #[test]
    fn test_parse_requirements_with_markers() {
        let content = "pywin32>=300 ; sys_platform == 'win32'\n";
        let base_dir = PathBuf::from(".");
        let (deps, _) = parse_requirements_txt(content, &base_dir).unwrap();

        assert_eq!(deps.len(), 1);
        assert!(deps[0].contains("pywin32"));
        assert!(deps[0].contains("sys_platform"));
    }

    #[test]
    fn test_parse_requirements_with_hashes() {
        let content = "requests==2.28.0 --hash=sha256:abc123 --hash=sha256:def456\n";
        let base_dir = PathBuf::from(".");
        let (deps, hashes) = parse_requirements_txt(content, &base_dir).unwrap();

        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], "requests==2.28.0");
        assert_eq!(hashes.get("requests").unwrap().len(), 2);
    }

    #[test]
    fn test_parse_requirements_line_continuation() {
        let content = "requests==2.28.0 \\\n    --hash=sha256:abc123\nnumpy>=1.20\n";
        let base_dir = PathBuf::from(".");
        let (deps, hashes) = parse_requirements_txt(content, &base_dir).unwrap();

        assert_eq!(deps.len(), 2);
        assert!(hashes.contains_key("requests"));
    }

    #[test]
    fn test_extract_hashes() {
        let line = "requests==2.28.0 --hash=sha256:abc --hash=sha256:def";
        let (req, hashes) = extract_hashes(line);

        assert_eq!(req, "requests==2.28.0");
        assert_eq!(hashes.len(), 2);
        assert_eq!(hashes[0], "sha256:abc");
        assert_eq!(hashes[1], "sha256:def");
    }

    #[test]
    fn test_extract_hashes_no_hash() {
        let line = "requests>=2.28.0";
        let (req, hashes) = extract_hashes(line);

        assert_eq!(req, "requests>=2.28.0");
        assert!(hashes.is_empty());
    }
}
