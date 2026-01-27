# Pro (rx)

<div align="center">

<img src="docs/assets/logo.svg" alt="Pro Logo" width="400">

**A blazing-fast Python package manager written in Rust**

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/pro-cli.svg)](https://crates.io/crates/pro-cli)
[![PyPI](https://img.shields.io/pypi/v/rx-pro.svg)](https://pypi.org/project/rx-pro/)
[![CI](https://github.com/stherrien/pro/workflows/CI/badge.svg)](https://github.com/stherrien/pro/actions)

[Installation](#installation) • [Quick Start](#quick-start) • [Features](#features) • [Documentation](#documentation)

</div>

---

## Why Pro?

Pro combines the speed of Rust with the ergonomics of Poetry to deliver the best Python package management experience:

| Feature | Pro | uv | Poetry | pip |
|---------|-----|----|---------|-----|
| **Dependency Resolution** | ⚡ Fast (Rust) | ⚡ Fast (Rust) | 🐢 Slow | 🐢 Slow |
| **Native Build Backend** | ✅ Rust | ❌ Python | ❌ Python | ❌ Python |
| **Python Version Management** | ✅ Yes | ✅ Yes | ❌ No | ❌ No |
| **Tool Runner** | ✅ Yes (`rx tool`) | ✅ Yes (`uvx`) | ❌ No | ❌ No |
| **PEP 723 Scripts** | ✅ Yes | ✅ Yes | ❌ No | ❌ No |
| **WebAssembly Plugins** | ✅ Yes | ❌ No | ❌ No | ❌ No |
| **Monorepo Support** | ✅ Full | ⚠️ Basic | ⚠️ Basic | ❌ No |
| **Polylith Architecture** | ✅ Yes | ❌ No | ❌ No | ❌ No |
| **Security Audit** | ✅ OSV + PyPI | ⚠️ Basic | ❌ No | ❌ No |
| **Task Runner** | ✅ Yes | ❌ No | ✅ Yes | ❌ No |
| **Docker Integration** | ✅ Yes | ❌ No | ❌ No | ❌ No |

## Installation

### Quick Install (curl)

```bash
curl -sSf https://raw.githubusercontent.com/stherrien/pro/main/install.sh | bash
```

### From PyPI

```bash
pip install rx-pro
```

### From Cargo

```bash
cargo install pro-cli
```

### From Source

```bash
git clone https://github.com/stherrien/pro.git
cd pro
cargo build --release
```

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/stherrien/pro/releases):

- **Linux (x86_64)**: `rx-x86_64-unknown-linux-gnu.tar.gz`
- **Linux (ARM64)**: `rx-aarch64-unknown-linux-gnu.tar.gz`
- **macOS (Intel)**: `rx-x86_64-apple-darwin.tar.gz`
- **macOS (Apple Silicon)**: `rx-aarch64-apple-darwin.tar.gz`
- **Windows**: `rx-x86_64-pc-windows-msvc.zip`

## Quick Start

```bash
# Create a new project
rx init my-project
cd my-project

# Add dependencies
rx add requests numpy pandas

# Add dev dependencies
rx add --dev pytest black ruff

# Install everything
rx sync

# Run your code
rx run python main.py

# Run tests
rx run pytest

# Build for distribution
rx build

# Publish to PyPI
rx publish
```

## Features

### ⚡ Blazing Fast

Pro is written in Rust and uses parallel downloads, efficient caching, and a native build backend:

```bash
$ rx sync
Resolving dependencies...  ✓ 0.3s
Downloading 47 packages... ✓ 1.2s
Installing packages...     ✓ 0.8s

Total: 2.3s
```

### 📦 Full Project Lifecycle

```bash
rx init          # Initialize new project
rx add/remove    # Manage dependencies
rx lock          # Generate lockfile
rx sync          # Install dependencies
rx run           # Run commands in venv
rx shell         # Spawn activated shell
rx build         # Build wheel/sdist
rx publish       # Publish to PyPI
rx audit         # Security vulnerability scan
```

### 🔒 Security First

```bash
# Scan for vulnerabilities
$ rx audit
Found 2 vulnerabilities in 47 packages

CRITICAL: requests 2.25.0
  CVE-2023-32681: Unintended leak of Proxy-Authorization header
  Fix: Upgrade to >= 2.31.0

# Auto-fix vulnerabilities
$ rx audit --fix
```

### 🏢 Monorepo & Workspace Support

```bash
# Initialize workspace
rx workspace init

# Add projects
rx workspace add packages/core
rx workspace add packages/api
rx workspace add packages/cli

# Unified operations
rx workspace sync     # Install all dependencies
rx workspace lock     # Single lockfile
rx run --affected test  # Test only changed packages
```

### 🧱 Polylith Architecture

Build modular Python applications with clean separation:

```bash
rx polylith init myapp
rx polylith create component user
rx polylith create component database
rx polylith create base cli
rx polylith create project myapp-cli
```

### 🔌 WebAssembly Plugins

Extend Pro safely with sandboxed plugins:

```toml
# pyproject.toml
[tool.rx.plugins]
license-checker = "~/.rx/plugins/license-checker.wasm"
custom-resolver = { path = "./plugins/resolver.wasm" }
```

```bash
rx plugin add license-checker https://example.com/license-checker.wasm
rx plugin run pre-build
```

### 🐳 Docker Integration

```bash
# Generate Dockerfile
rx docker generate

# Build image
rx docker build --tag myapp:latest

# Configure in pyproject.toml
```

```toml
[tool.rx.docker]
base_image = "python:3.11-slim"
apt_packages = ["curl", "git"]
expose = [8000]
cmd = ["python", "-m", "myapp"]
```

### 📋 Task Runner

Define and run complex tasks with dependencies:

```toml
[tool.rx.tasks]
test = { cmd = "pytest -v", depends_on = ["lint"] }
lint = { cmd = "ruff check ." }
format = { cmd = "black .", parallel = true }
ci = { depends_on = ["test", "format"] }
```

```bash
rx task ci  # Runs lint -> test and format in parallel
```

### 🔄 Migration from Poetry

```bash
rx import poetry
# Converts pyproject.toml and poetry.lock to rx format
```

## Configuration

Pro uses `pyproject.toml` with the `[tool.rx]` section:

```toml
[project]
name = "my-project"
version = "1.0.0"
requires-python = ">=3.8"
dependencies = ["requests>=2.28", "numpy>=1.24"]

[project.optional-dependencies]
dev = ["pytest>=7.0", "black>=23.0"]

[project.scripts]
myapp = "my_project:main"

[tool.rx]
# Script aliases
[tool.rx.scripts]
test = "pytest -v tests/"
lint = "ruff check ."

# Private registries
[[tool.rx.registries]]
name = "private"
url = "https://pypi.mycompany.com/simple/"
username = "${PYPI_USER}"
password = "${PYPI_PASS}"

# Path dependencies (monorepo)
[tool.rx.dependencies]
shared-lib = { path = "../shared", editable = true }

# Dynamic versioning from git tags
[tool.rx.versioning]
source = "git-tag"
pattern = "v{version}"

# Docker configuration
[tool.rx.docker]
base_image = "python:3.11-slim"
multi_stage = true
```

## Comparison to Other Tools

### vs Poetry

- **10-100x faster** dependency resolution
- Native Rust build backend (no Python subprocess)
- WebAssembly plugin system
- Better monorepo support

### vs uv

- Native build backend (uv delegates to Python)
- WebAssembly plugins
- Task runner
- Docker integration
- Polylith architecture support

### vs pip-tools

- Full project management (not just requirements)
- Parallel downloads and caching
- Built-in security audit
- Workspace support

## Documentation

- [Documentation](https://rxpro.net/)
- [Migration Guide](docs/MIGRATION.md)
- [Plugin Development](docs/PLUGINS.md)
- [Contributing](CONTRIBUTING.md)

## Benchmarks

Resolving and installing a fresh Django project (Django + 50 dependencies):

| Tool | Cold Resolve | Install | Total |
|------|-------------|---------|-------|
| **Pro** | 0.4s | 1.8s | **2.2s** |
| uv | 0.5s | 2.1s | 2.6s |
| Poetry | 8.2s | 12.4s | 20.6s |
| pip | 6.1s | 15.2s | 21.3s |

*Benchmarks run on M1 MacBook Pro, warm cache disabled*

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

```bash
# Clone and build
git clone https://github.com/stherrien/pro.git
cd pro
cargo build

# Run tests
cargo test

# Run with debug output
RUST_LOG=debug cargo run -- sync
```

## License

Dual-licensed under MIT and Apache 2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

---

<div align="center">

**Built with 🦀 Rust and ❤️ by the Pro team**

[Website](https://rxpro.net/) • [GitHub](https://github.com/stherrien/pro) • [PyPI](https://pypi.org/project/rx-pro/) • [Crates.io](https://crates.io/crates/pro-cli)

</div>
