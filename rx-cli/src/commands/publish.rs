//! Publish command - upload packages to PyPI

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Args;

use rx_core::builder::Builder;
use rx_core::pep::PyProject;

/// PyPI upload endpoints
const PYPI_URL: &str = "https://upload.pypi.org/legacy/";
const TEST_PYPI_URL: &str = "https://test.pypi.org/legacy/";

#[derive(Args)]
pub struct PublishCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".", global = true)]
    pub project: PathBuf,

    /// Directory containing built distributions
    #[arg(short, long, default_value = "dist")]
    pub dist: PathBuf,

    /// Upload to TestPyPI instead of PyPI
    #[arg(long)]
    pub test: bool,

    /// Custom repository URL
    #[arg(long)]
    pub repository: Option<String>,

    /// PyPI API token (or use RX_PYPI_TOKEN / TWINE_PASSWORD env var)
    #[arg(long, env = "RX_PYPI_TOKEN")]
    pub token: Option<String>,

    /// Username for PyPI (default: __token__ for API token auth)
    #[arg(long, default_value = "__token__")]
    pub username: String,

    /// Build before publishing if dist/ is empty
    #[arg(long)]
    pub build: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Don't actually upload, just show what would happen
    #[arg(long)]
    pub dry_run: bool,
}

impl PublishCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Load pyproject.toml
        let pyproject = PyProject::load(&project_dir).with_context(|| {
            format!(
                "No pyproject.toml found in {:?}. Run 'rx init' first.",
                project_dir
            )
        })?;

        let project_name = pyproject.name().unwrap_or("unknown").to_string();
        let version = pyproject.version().unwrap_or("0.0.0").to_string();

        // Resolve dist directory
        let dist_dir = if self.dist.is_absolute() {
            self.dist.clone()
        } else {
            project_dir.join(&self.dist)
        };

        // Check for distributions or build if requested
        let distributions = find_distributions(&dist_dir, &project_name, &version)?;

        if distributions.is_empty() {
            if self.build {
                println!("No distributions found, building...");
                println!();
                let builder = Builder::new(&project_dir);
                builder.build_wheel(&dist_dir)?;
                builder.build_sdist(&dist_dir)?;
                // Re-scan for distributions
                let distributions = find_distributions(&dist_dir, &project_name, &version)?;
                if distributions.is_empty() {
                    bail!("Build completed but no distributions found");
                }
            } else {
                bail!(
                    "No distributions found in {:?}. Run 'rx build' first or use --build flag.",
                    dist_dir
                );
            }
        }

        let distributions = find_distributions(&dist_dir, &project_name, &version)?;

        // Determine repository URL
        let repo_url = if let Some(ref url) = self.repository {
            url.clone()
        } else if self.test {
            TEST_PYPI_URL.to_string()
        } else {
            PYPI_URL.to_string()
        };

        let repo_name = if self.test {
            "TestPyPI"
        } else if self.repository.is_some() {
            "custom repository"
        } else {
            "PyPI"
        };

        // Get token
        let token = self.get_token()?;

        // Show publish plan
        println!("Publishing {} v{} to {}", project_name, version, repo_name);
        println!("─────────────────────────────────────");
        println!("  Repository: {}", repo_url);
        println!("  Files:");
        for dist in &distributions {
            let filename = dist.file_name().unwrap().to_string_lossy();
            let size = std::fs::metadata(dist).map(|m| m.len()).unwrap_or(0);
            println!("    - {} ({} bytes)", filename, format_size(size));
        }
        println!();

        if self.dry_run {
            println!("Dry run - no files uploaded.");
            return Ok(());
        }

        // Confirm
        if !self.yes && !confirm(&format!("Upload to {}?", repo_name))? {
            println!("Aborted.");
            return Ok(());
        }

        // Upload each distribution
        let client = reqwest::Client::new();

        for dist in &distributions {
            let filename = dist.file_name().unwrap().to_string_lossy().to_string();
            print!("  Uploading {}...", filename);

            match upload_file(&client, &repo_url, &self.username, &token, dist).await {
                Ok(_) => println!(" ✓"),
                Err(e) => {
                    println!(" ✗");
                    return Err(e);
                }
            }
        }

        println!();
        println!("✓ Published {} v{} to {}", project_name, version, repo_name);

        // Show package URL
        if !self.test && self.repository.is_none() {
            println!();
            println!("  https://pypi.org/project/{}/{}/", project_name, version);
        } else if self.test {
            println!();
            println!(
                "  https://test.pypi.org/project/{}/{}/",
                project_name, version
            );
        }

        Ok(())
    }

    fn get_token(&self) -> Result<String> {
        // Check command line argument first
        if let Some(ref token) = self.token {
            return Ok(token.clone());
        }

        // Check environment variables
        if let Ok(token) = std::env::var("RX_PYPI_TOKEN") {
            return Ok(token);
        }

        if let Ok(token) = std::env::var("TWINE_PASSWORD") {
            return Ok(token);
        }

        // Check for .pypirc file
        if let Some(home) = dirs::home_dir() {
            let pypirc = home.join(".pypirc");
            if pypirc.exists() {
                if let Ok(token) = read_pypirc_token(&pypirc) {
                    return Ok(token);
                }
            }
        }

        bail!(
            "No PyPI token found. Provide via:\n\
             - --token flag\n\
             - RX_PYPI_TOKEN environment variable\n\
             - TWINE_PASSWORD environment variable\n\
             - ~/.pypirc file"
        );
    }
}

/// Find distribution files for the given package
fn find_distributions(dist_dir: &PathBuf, name: &str, version: &str) -> Result<Vec<PathBuf>> {
    if !dist_dir.exists() {
        return Ok(Vec::new());
    }

    let normalized_name = name.replace('-', "_");
    let mut distributions = Vec::new();

    for entry in std::fs::read_dir(dist_dir)
        .map_err(|e| anyhow::anyhow!("Failed to read dist directory: {}", e))?
    {
        let entry = entry?;
        let path = entry.path();
        let filename = path.file_name().unwrap().to_string_lossy().to_string();

        // Match wheel: {name}-{version}-*.whl
        // Match sdist: {name}-{version}.tar.gz
        if (filename.starts_with(&format!("{}-{}", normalized_name, version))
            || filename.starts_with(&format!("{}-{}", name, version)))
            && (filename.ends_with(".whl") || filename.ends_with(".tar.gz"))
        {
            distributions.push(path);
        }
    }

    // Sort: wheels first, then sdist
    distributions.sort_by(|a, b| {
        let a_is_wheel = a.extension().map(|e| e == "whl").unwrap_or(false);
        let b_is_wheel = b.extension().map(|e| e == "whl").unwrap_or(false);
        b_is_wheel.cmp(&a_is_wheel)
    });

    Ok(distributions)
}

/// Upload a file to PyPI
async fn upload_file(
    client: &reqwest::Client,
    url: &str,
    username: &str,
    token: &str,
    file_path: &PathBuf,
) -> Result<()> {
    let filename = file_path.file_name().unwrap().to_string_lossy().to_string();
    let file_content = std::fs::read(file_path)?;

    // Determine filetype
    let filetype = if filename.ends_with(".whl") {
        "bdist_wheel"
    } else {
        "sdist"
    };

    // Calculate MD5 and SHA256 hashes
    let md5_hash = format!("{:x}", md5::compute(&file_content));
    let sha256_hash = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(&file_content))
    };

    // Parse metadata from filename
    let (name, version) = parse_distribution_name(&filename)?;

    // Build multipart form
    let form = reqwest::multipart::Form::new()
        .text(":action", "file_upload")
        .text("protocol_version", "1")
        .text("name", name)
        .text("version", version)
        .text("filetype", filetype)
        .text("md5_digest", md5_hash)
        .text("sha256_digest", sha256_hash)
        .part(
            "content",
            reqwest::multipart::Part::bytes(file_content)
                .file_name(filename.clone())
                .mime_str("application/octet-stream")?,
        );

    // Send request
    let response = client
        .post(url)
        .basic_auth(username, Some(token))
        .multipart(form)
        .send()
        .await?;

    let status = response.status();

    if status.is_success() {
        Ok(())
    } else {
        let body = response.text().await.unwrap_or_default();

        // Check for common errors
        if body.contains("already exists") || status.as_u16() == 400 {
            bail!(
                "Version already exists on PyPI. Bump the version and try again.\n\
                 Hint: Use 'rx version bump patch' or 'rx release --bump patch'"
            );
        } else if status.as_u16() == 403 {
            bail!("Authentication failed. Check your PyPI token.");
        } else if status.as_u16() == 404 {
            bail!("Repository not found. Check the repository URL.");
        } else {
            bail!("Upload failed ({}): {}", status, body);
        }
    }
}

/// Parse package name and version from distribution filename
fn parse_distribution_name(filename: &str) -> Result<(String, String)> {
    if filename.ends_with(".whl") {
        // Wheel: {name}-{version}-{python}-{abi}-{platform}.whl
        let parts: Vec<&str> = filename.trim_end_matches(".whl").split('-').collect();
        if parts.len() >= 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    } else if filename.ends_with(".tar.gz") {
        // Sdist: {name}-{version}.tar.gz
        let base = filename.trim_end_matches(".tar.gz");
        if let Some(idx) = base.rfind('-') {
            return Ok((base[..idx].to_string(), base[idx + 1..].to_string()));
        }
    }

    bail!("Could not parse distribution filename: {}", filename);
}

/// Read token from .pypirc file
fn read_pypirc_token(path: &PathBuf) -> Result<String> {
    let content = std::fs::read_to_string(path)?;

    // Simple INI parsing - look for password in [pypi] section
    let mut in_pypi_section = false;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            in_pypi_section = section == "pypi";
            continue;
        }

        if in_pypi_section && line.starts_with("password") {
            if let Some(idx) = line.find('=') {
                let token = line[idx + 1..].trim();
                return Ok(token.to_string());
            }
        }
    }

    bail!("No password found in [pypi] section of .pypirc");
}

fn confirm(message: &str) -> Result<bool> {
    use std::io::{self, Write};

    print!("{} [y/N]: ", message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
