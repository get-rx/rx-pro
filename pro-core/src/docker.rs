//! Docker integration for Python projects
//!
//! Generate Dockerfiles and build images from `[tool.rx.docker]` config:
//!
//! ```toml
//! [tool.rx.docker]
//! base-image = "python:3.11-slim"
//! python-version = "3.11"
//! entrypoint = ["python", "-m", "myapp"]
//! cmd = ["--help"]
//! expose = [8000]
//! env = { APP_ENV = "production" }
//! copy = ["src/", "config/"]
//! workdir = "/app"
//! user = "appuser"
//! labels = { maintainer = "dev@example.com" }
//! apt-packages = ["curl", "gcc"]
//! build-args = { PIP_NO_CACHE_DIR = "1" }
//! multi-stage = true
//! ```

use std::collections::HashMap;
use std::path::Path;

use crate::pep::PyProject;
use crate::{Error, Result};

/// Docker configuration from pyproject.toml
#[derive(Debug, Clone)]
pub struct DockerConfig {
    /// Base image (default: python:3.11-slim)
    pub base_image: String,
    /// Python version for base image
    pub python_version: String,
    /// Working directory in container (default: /app)
    pub workdir: String,
    /// Entrypoint command
    pub entrypoint: Option<Vec<String>>,
    /// Default command
    pub cmd: Option<Vec<String>>,
    /// Ports to expose
    pub expose: Vec<u16>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Additional files/directories to copy
    pub copy: Vec<String>,
    /// User to run as
    pub user: Option<String>,
    /// Image labels
    pub labels: HashMap<String, String>,
    /// APT packages to install
    pub apt_packages: Vec<String>,
    /// Build arguments
    pub build_args: HashMap<String, String>,
    /// Use multi-stage build
    pub multi_stage: bool,
    /// Install dev dependencies
    pub dev_deps: bool,
    /// Custom Dockerfile commands (inserted before COPY)
    pub pre_copy: Vec<String>,
    /// Custom Dockerfile commands (inserted after COPY)
    pub post_copy: Vec<String>,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            base_image: "python:3.11-slim".to_string(),
            python_version: "3.11".to_string(),
            workdir: "/app".to_string(),
            entrypoint: None,
            cmd: None,
            expose: Vec::new(),
            env: HashMap::new(),
            copy: Vec::new(),
            user: None,
            labels: HashMap::new(),
            apt_packages: Vec::new(),
            build_args: HashMap::new(),
            multi_stage: true,
            dev_deps: false,
            pre_copy: Vec::new(),
            post_copy: Vec::new(),
        }
    }
}

impl DockerConfig {
    /// Load Docker config from pyproject.toml
    pub fn load(project_dir: &Path) -> Result<Self> {
        let pyproject = PyProject::load(project_dir)?;

        let mut config = Self::default();

        let rx_config = match pyproject.tool.get("rx") {
            Some(c) => c,
            None => return Ok(config),
        };

        let docker_config = match rx_config.get("docker") {
            Some(c) => c,
            None => return Ok(config),
        };

        if let Some(v) = docker_config.get("base-image").and_then(|v| v.as_str()) {
            config.base_image = v.to_string();
        }

        if let Some(v) = docker_config.get("python-version").and_then(|v| v.as_str()) {
            config.python_version = v.to_string();
            // Update base image if not explicitly set
            if docker_config.get("base-image").is_none() {
                config.base_image = format!("python:{}-slim", v);
            }
        }

        if let Some(v) = docker_config.get("workdir").and_then(|v| v.as_str()) {
            config.workdir = v.to_string();
        }

        if let Some(arr) = docker_config.get("entrypoint").and_then(|v| v.as_array()) {
            config.entrypoint = Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
            );
        }

        if let Some(arr) = docker_config.get("cmd").and_then(|v| v.as_array()) {
            config.cmd = Some(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
            );
        }

        if let Some(arr) = docker_config.get("expose").and_then(|v| v.as_array()) {
            config.expose = arr
                .iter()
                .filter_map(|v| v.as_integer().map(|i| i as u16))
                .collect();
        }

        if let Some(table) = docker_config.get("env").and_then(|v| v.as_table()) {
            for (k, v) in table {
                if let Some(s) = v.as_str() {
                    config.env.insert(k.clone(), s.to_string());
                }
            }
        }

        if let Some(arr) = docker_config.get("copy").and_then(|v| v.as_array()) {
            config.copy = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }

        if let Some(v) = docker_config.get("user").and_then(|v| v.as_str()) {
            config.user = Some(v.to_string());
        }

        if let Some(table) = docker_config.get("labels").and_then(|v| v.as_table()) {
            for (k, v) in table {
                if let Some(s) = v.as_str() {
                    config.labels.insert(k.clone(), s.to_string());
                }
            }
        }

        if let Some(arr) = docker_config.get("apt-packages").and_then(|v| v.as_array()) {
            config.apt_packages = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }

        if let Some(table) = docker_config.get("build-args").and_then(|v| v.as_table()) {
            for (k, v) in table {
                if let Some(s) = v.as_str() {
                    config.build_args.insert(k.clone(), s.to_string());
                }
            }
        }

        if let Some(v) = docker_config.get("multi-stage").and_then(|v| v.as_bool()) {
            config.multi_stage = v;
        }

        if let Some(v) = docker_config.get("dev-deps").and_then(|v| v.as_bool()) {
            config.dev_deps = v;
        }

        if let Some(arr) = docker_config.get("pre-copy").and_then(|v| v.as_array()) {
            config.pre_copy = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }

        if let Some(arr) = docker_config.get("post-copy").and_then(|v| v.as_array()) {
            config.post_copy = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }

        Ok(config)
    }
}

/// Dockerfile generator
pub struct DockerfileGenerator {
    config: DockerConfig,
    project_name: String,
}

impl DockerfileGenerator {
    /// Create a new generator
    pub fn new(config: DockerConfig, project_name: String) -> Self {
        Self {
            config,
            project_name,
        }
    }

    /// Load from project directory
    pub fn from_project(project_dir: &Path) -> Result<Self> {
        let config = DockerConfig::load(project_dir)?;
        let pyproject = PyProject::load(project_dir)?;
        let project_name = pyproject
            .name()
            .unwrap_or("app")
            .to_string()
            .replace('-', "_");

        Ok(Self::new(config, project_name))
    }

    /// Generate Dockerfile content
    pub fn generate(&self) -> String {
        if self.config.multi_stage {
            self.generate_multi_stage()
        } else {
            self.generate_single_stage()
        }
    }

    /// Generate a multi-stage Dockerfile (smaller final image)
    fn generate_multi_stage(&self) -> String {
        let mut lines = Vec::new();

        // Build stage
        lines.push(format!(
            "# Build stage\nFROM {} AS builder",
            self.config.base_image
        ));
        lines.push(String::new());

        // Build args
        for (key, value) in &self.config.build_args {
            lines.push(format!("ARG {}={}", key, value));
        }
        if !self.config.build_args.is_empty() {
            lines.push(String::new());
        }

        // Install build dependencies
        lines.push("# Install build dependencies".to_string());
        let mut apt_deps = vec!["build-essential"];
        apt_deps.extend(self.config.apt_packages.iter().map(|s| s.as_str()));

        lines.push(format!(
            "RUN apt-get update && apt-get install -y --no-install-recommends {} && rm -rf /var/lib/apt/lists/*",
            apt_deps.join(" ")
        ));
        lines.push(String::new());

        // Set workdir
        lines.push(format!("WORKDIR {}", self.config.workdir));
        lines.push(String::new());

        // Copy and install dependencies first (for layer caching)
        lines.push("# Install dependencies".to_string());
        lines.push("COPY pyproject.toml ./".to_string());
        lines.push("COPY rx.lock* ./".to_string());
        lines.push(String::new());

        // Create virtual environment and install deps
        lines.push("RUN python -m venv /opt/venv".to_string());
        lines.push("ENV PATH=\"/opt/venv/bin:$PATH\"".to_string());
        lines.push(String::new());

        // Install using rx if available, otherwise pip
        lines.push("RUN pip install --no-cache-dir --upgrade pip && \\".to_string());
        lines.push(
            "    if [ -f rx.lock ]; then pip install --no-cache-dir -r <(python -c \"import tomllib; f=open('rx.lock','rb'); d=tomllib.load(f); print('\\\\n'.join(f\\\"{p}=={d['packages'][p]['version']}\\\" for p in d['packages']))\") 2>/dev/null || pip install .; else pip install .; fi"
                .to_string(),
        );
        lines.push(String::new());

        // Custom pre-copy commands
        for cmd in &self.config.pre_copy {
            lines.push(format!("RUN {}", cmd));
        }
        if !self.config.pre_copy.is_empty() {
            lines.push(String::new());
        }

        // Copy source code
        lines.push("# Copy source code".to_string());
        lines.push("COPY . .".to_string());
        lines.push(String::new());

        // Install the package itself
        lines.push("RUN pip install --no-cache-dir .".to_string());
        lines.push(String::new());

        // Runtime stage
        lines.push(format!(
            "# Runtime stage\nFROM {} AS runtime",
            self.config.base_image
        ));
        lines.push(String::new());

        // Labels
        for (key, value) in &self.config.labels {
            lines.push(format!("LABEL {}=\"{}\"", key, value));
        }
        if !self.config.labels.is_empty() {
            lines.push(String::new());
        }

        // Install runtime apt packages only
        if !self.config.apt_packages.is_empty() {
            let runtime_pkgs: Vec<_> = self
                .config
                .apt_packages
                .iter()
                .filter(|p| !["build-essential", "gcc", "g++", "make"].contains(&p.as_str()))
                .collect();

            if !runtime_pkgs.is_empty() {
                lines.push(format!(
                    "RUN apt-get update && apt-get install -y --no-install-recommends {} && rm -rf /var/lib/apt/lists/*",
                    runtime_pkgs.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
                ));
                lines.push(String::new());
            }
        }

        // Set workdir
        lines.push(format!("WORKDIR {}", self.config.workdir));
        lines.push(String::new());

        // Copy venv from builder
        lines.push("# Copy virtual environment from builder".to_string());
        lines.push("COPY --from=builder /opt/venv /opt/venv".to_string());
        lines.push("ENV PATH=\"/opt/venv/bin:$PATH\"".to_string());
        lines.push(String::new());

        // Copy additional files
        if !self.config.copy.is_empty() {
            lines.push("# Copy additional files".to_string());
            for path in &self.config.copy {
                lines.push(format!("COPY {} {}/", path, self.config.workdir));
            }
            lines.push(String::new());
        }

        // Custom post-copy commands
        for cmd in &self.config.post_copy {
            lines.push(format!("RUN {}", cmd));
        }
        if !self.config.post_copy.is_empty() {
            lines.push(String::new());
        }

        // Environment variables
        for (key, value) in &self.config.env {
            lines.push(format!("ENV {}=\"{}\"", key, value));
        }
        if !self.config.env.is_empty() {
            lines.push(String::new());
        }

        // Expose ports
        for port in &self.config.expose {
            lines.push(format!("EXPOSE {}", port));
        }
        if !self.config.expose.is_empty() {
            lines.push(String::new());
        }

        // User
        if let Some(ref user) = self.config.user {
            lines.push(format!("RUN useradd -m -s /bin/bash {}", user));
            lines.push(format!("USER {}", user));
            lines.push(String::new());
        }

        // Entrypoint and CMD
        if let Some(ref entrypoint) = self.config.entrypoint {
            let ep_str = entrypoint
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("ENTRYPOINT [{}]", ep_str));
        }

        if let Some(ref cmd) = self.config.cmd {
            let cmd_str = cmd
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("CMD [{}]", cmd_str));
        } else if self.config.entrypoint.is_none() {
            // Default: run the package as a module
            lines.push(format!(
                "CMD [\"python\", \"-m\", \"{}\"]",
                self.project_name
            ));
        }

        lines.join("\n")
    }

    /// Generate a single-stage Dockerfile
    fn generate_single_stage(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("FROM {}", self.config.base_image));
        lines.push(String::new());

        // Build args
        for (key, value) in &self.config.build_args {
            lines.push(format!("ARG {}={}", key, value));
        }
        if !self.config.build_args.is_empty() {
            lines.push(String::new());
        }

        // Labels
        for (key, value) in &self.config.labels {
            lines.push(format!("LABEL {}=\"{}\"", key, value));
        }
        if !self.config.labels.is_empty() {
            lines.push(String::new());
        }

        // Install apt packages
        if !self.config.apt_packages.is_empty() {
            lines.push(format!(
                "RUN apt-get update && apt-get install -y --no-install-recommends {} && rm -rf /var/lib/apt/lists/*",
                self.config.apt_packages.join(" ")
            ));
            lines.push(String::new());
        }

        // Set workdir
        lines.push(format!("WORKDIR {}", self.config.workdir));
        lines.push(String::new());

        // Copy requirements first for caching
        lines.push("# Copy dependency files".to_string());
        lines.push("COPY pyproject.toml ./".to_string());
        lines.push("COPY rx.lock* ./".to_string());
        lines.push(String::new());

        // Install dependencies
        lines.push("# Install dependencies".to_string());
        lines.push("RUN pip install --no-cache-dir --upgrade pip".to_string());
        lines.push(String::new());

        // Custom pre-copy commands
        for cmd in &self.config.pre_copy {
            lines.push(format!("RUN {}", cmd));
        }
        if !self.config.pre_copy.is_empty() {
            lines.push(String::new());
        }

        // Copy source code
        lines.push("# Copy source code".to_string());
        lines.push("COPY . .".to_string());
        lines.push(String::new());

        // Install the package
        lines.push("RUN pip install --no-cache-dir .".to_string());
        lines.push(String::new());

        // Copy additional files
        if !self.config.copy.is_empty() {
            for path in &self.config.copy {
                lines.push(format!("COPY {} {}/", path, self.config.workdir));
            }
            lines.push(String::new());
        }

        // Custom post-copy commands
        for cmd in &self.config.post_copy {
            lines.push(format!("RUN {}", cmd));
        }
        if !self.config.post_copy.is_empty() {
            lines.push(String::new());
        }

        // Environment variables
        for (key, value) in &self.config.env {
            lines.push(format!("ENV {}=\"{}\"", key, value));
        }
        if !self.config.env.is_empty() {
            lines.push(String::new());
        }

        // Expose ports
        for port in &self.config.expose {
            lines.push(format!("EXPOSE {}", port));
        }
        if !self.config.expose.is_empty() {
            lines.push(String::new());
        }

        // User
        if let Some(ref user) = self.config.user {
            lines.push(format!("RUN useradd -m -s /bin/bash {}", user));
            lines.push(format!("USER {}", user));
            lines.push(String::new());
        }

        // Entrypoint and CMD
        if let Some(ref entrypoint) = self.config.entrypoint {
            let ep_str = entrypoint
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("ENTRYPOINT [{}]", ep_str));
        }

        if let Some(ref cmd) = self.config.cmd {
            let cmd_str = cmd
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("CMD [{}]", cmd_str));
        } else if self.config.entrypoint.is_none() {
            lines.push(format!(
                "CMD [\"python\", \"-m\", \"{}\"]",
                self.project_name
            ));
        }

        lines.join("\n")
    }

    /// Generate .dockerignore content
    pub fn generate_dockerignore(&self) -> String {
        r#"# Python
__pycache__/
*.py[cod]
*$py.class
*.so
.Python
build/
develop-eggs/
dist/
downloads/
eggs/
.eggs/
lib/
lib64/
parts/
sdist/
var/
wheels/
*.egg-info/
.installed.cfg
*.egg

# Virtual environments
.venv/
venv/
ENV/
env/

# IDE
.idea/
.vscode/
*.swp
*.swo

# Testing
.tox/
.nox/
.coverage
htmlcov/
.pytest_cache/
.mypy_cache/

# Git
.git/
.gitignore

# Docker
Dockerfile
.dockerignore
docker-compose*.yml

# Documentation
docs/
*.md
!README.md

# Misc
*.log
.DS_Store
Thumbs.db
"#
        .to_string()
    }
}

/// Build a Docker image
pub fn build_image(
    project_dir: &Path,
    tag: &str,
    dockerfile_path: Option<&Path>,
    build_args: &HashMap<String, String>,
    no_cache: bool,
) -> Result<()> {
    use std::process::Command;

    let mut cmd = Command::new("docker");
    cmd.arg("build");
    cmd.arg("-t").arg(tag);

    if let Some(df) = dockerfile_path {
        cmd.arg("-f").arg(df);
    }

    for (key, value) in build_args {
        cmd.arg("--build-arg").arg(format!("{}={}", key, value));
    }

    if no_cache {
        cmd.arg("--no-cache");
    }

    cmd.arg(project_dir);
    cmd.current_dir(project_dir);

    let status = cmd
        .status()
        .map_err(|e| Error::Config(format!("Failed to run docker build: {}", e)))?;

    if !status.success() {
        return Err(Error::Config("Docker build failed".to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DockerConfig::default();
        assert_eq!(config.base_image, "python:3.11-slim");
        assert_eq!(config.workdir, "/app");
        assert!(config.multi_stage);
    }

    #[test]
    fn test_generate_dockerfile() {
        let config = DockerConfig {
            base_image: "python:3.11-slim".to_string(),
            python_version: "3.11".to_string(),
            workdir: "/app".to_string(),
            expose: vec![8000],
            env: [("APP_ENV".to_string(), "production".to_string())]
                .into_iter()
                .collect(),
            multi_stage: false,
            ..Default::default()
        };

        let generator = DockerfileGenerator::new(config, "myapp".to_string());
        let dockerfile = generator.generate();

        assert!(dockerfile.contains("FROM python:3.11-slim"));
        assert!(dockerfile.contains("WORKDIR /app"));
        assert!(dockerfile.contains("EXPOSE 8000"));
        assert!(dockerfile.contains("ENV APP_ENV=\"production\""));
    }

    #[test]
    fn test_multi_stage_dockerfile() {
        let config = DockerConfig {
            multi_stage: true,
            ..Default::default()
        };

        let generator = DockerfileGenerator::new(config, "myapp".to_string());
        let dockerfile = generator.generate();

        assert!(dockerfile.contains("AS builder"));
        assert!(dockerfile.contains("AS runtime"));
        assert!(dockerfile.contains("COPY --from=builder"));
    }
}
