# Features

Pro (rx) is a unified Python package manager and build tool that combines Rust-level performance with Poetry-like UX and WebAssembly plugin extensibility.

---

## Package Management

### Dependency Resolution
- **PubGrub solver** - SAT-based dependency resolution for reliable, fast results
- **Universal locking** - Cross-platform lockfile (`rx.lock`) with platform markers
- **Private registries** - Support for custom PyPI-compatible indexes with authentication

### Installation
- **Parallel downloads** - Up to 8 concurrent package downloads
- **Content-addressable cache** - Deduplicated wheel storage
- **Hash verification** - SHA256 verification of all downloaded packages

```bash
rx add requests               # Add a dependency
rx add requests@2.31.0        # Add with specific version (@ syntax)
rx add "requests>=2.28"       # Add with version constraint
rx add pytest --dev           # Add a dev dependency
rx remove requests            # Remove a dependency
rx sync                       # Install from lockfile
rx update                     # Update all to latest versions
rx update requests            # Update specific package
rx update requests@2.32.0     # Update to specific version
```

---

## Python Version Management

Manage Python installations without relying on system Python or pyenv.

### Commands
```bash
rx python install 3.12        # Install Python 3.12 (latest patch)
rx python install 3.11.8      # Install specific version
rx python list                # List available and installed versions
rx python list --installed    # Show only installed versions
rx python pin 3.12            # Pin version for project (.python-version)
rx python use 3.12            # Set global default version
rx python uninstall 3.11      # Remove installed version
```

### Features
- Downloads from [python-build-standalone](https://github.com/astral-sh/python-build-standalone)
- Project-level pinning via `.python-version` (pyenv compatible)
- Global default configuration in `~/.config/rx/config.toml`
- Automatic platform detection (Linux, macOS, Windows / x86_64, ARM64)

### Storage Locations
| Location | Purpose |
|----------|---------|
| `~/.local/share/rx/python/` | Installed Python versions |
| `~/.config/rx/config.toml` | Global configuration |
| `.python-version` | Project-level version pin |

---

## Tool Runner

Run Python tools in ephemeral environments without polluting your project.

### Commands
```bash
rx tool run black .           # Run black formatter
rx tool run ruff check .      # Run ruff linter
rx tool run mypy src/         # Run mypy type checker
rx tool run --command isort black .  # Use different command name
rx tool list                  # List cached tools
rx tool clear                 # Clear all cached tools
rx tool clear black           # Clear specific tool
```

### Features
- **Automatic installation** - Tools are installed on first use
- **Caching** - Tool environments are cached for fast re-execution
- **Version tracking** - Records installed versions for reproducibility

### Storage
- Tool cache: `~/.local/share/rx/tools/{package}/`

---

## Script Support (PEP 723)

Run Python scripts with inline dependency declarations.

### Usage
```bash
rx run script.py              # Auto-detect and install inline deps
rx run script.py arg1 arg2    # Pass arguments to script
```

### PEP 723 Format
```python
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "requests>=2.28",
#   "rich",
# ]
# ///

import requests
from rich import print

response = requests.get("https://api.example.com")
print(response.json())
```

### Features
- **Auto-detection** - Automatically detects PEP 723 metadata in `.py` files
- **Dependency caching** - Environments cached by dependency hash
- **Python version constraints** - Respects `requires-python` specification

### Storage
- Script environments: `~/.cache/rx/scripts/{hash}/`

---

## Virtual Environment Management

Native virtual environment creation without requiring the `venv` module.

```bash
rx sync                       # Create venv and install dependencies
rx sync --recreate            # Force recreate virtual environment
rx shell                      # Spawn shell with venv activated
rx run python script.py       # Run command in venv
```

### Features
- **Native creation** - No Python required to create venvs
- **Cross-platform** - Works on Linux, macOS, and Windows
- **Activation scripts** - Generates bash/zsh activation scripts

---

## Build System

PEP 517 compliant build backend written in Rust.

```bash
rx build                      # Build wheel and sdist
rx build --wheel              # Build wheel only
rx build --sdist              # Build sdist only
rx build --output dist/       # Custom output directory
```

### Features
- **Zero-Python builds** - Pure Python packages built without invoking Python
- **PEP 621 compliant** - Reads metadata from `[project]` table
- **Fast** - Sub-50ms builds for pure Python packages

---

## Workspace / Monorepo Support

Manage multiple Python packages in a single repository.

```bash
rx workspace sync             # Sync all workspace members
rx workspace list             # List workspace members
rx affected                   # Detect changed packages
rx run --affected pytest      # Run tests only on affected packages
```

### Configuration
```toml
# pyproject.toml at workspace root
[tool.rx.workspace]
members = [
    "packages/*",
    "libs/*",
]
```

### Features
- **Unified lockfile** - Single `rx.lock` for entire workspace
- **Affected detection** - Git-based change detection
- **Parallel execution** - Run commands across packages concurrently

---

## Polylith Architecture

Component-based monorepo architecture for maximum code reuse.

```bash
rx polylith init              # Initialize Polylith structure
rx polylith create base api   # Create a base
rx polylith create component auth  # Create a component
rx polylith info              # Show workspace info
```

### Structure
```
workspace/
├── bases/          # Entry points (CLI, API, etc.)
├── components/     # Reusable business logic
├── projects/       # Deployable artifacts
└── development/    # Development configuration
```

---

## Security Auditing

Check dependencies for known vulnerabilities.

```bash
rx audit                      # Check for vulnerabilities
rx audit --fix                # Auto-update vulnerable packages
rx audit --ignore CVE-2023-xxx  # Ignore specific CVE
```

### Data Sources
- OSV (Open Source Vulnerabilities) database
- PyPI vulnerability API

---

## Plugin System

Extend Pro with WebAssembly plugins for enterprise workflows.

```bash
rx plugin install ./my-plugin.wasm
rx plugin list
rx plugin remove my-plugin
```

### Lifecycle Hooks
- `pre-resolve` - Before dependency resolution
- `post-resolve` - After resolution, before installation
- `pre-build` - Before building artifacts
- `post-build` - After build completion
- `pre-publish` - Before publishing to registry

### Features
- **Sandboxed execution** - Plugins run in isolated Wasm environment
- **Permission system** - Explicit grants for file system access
- **Low overhead** - <5ms startup overhead

---

## Import / Export

Migrate from other package managers.

```bash
rx import poetry              # Import from Poetry
rx import requirements.txt    # Import from requirements.txt
rx export requirements.txt    # Export to requirements.txt
rx export constraints.txt     # Export as constraints file
```

---

## Docker Support

Generate Dockerfiles and build images.

```bash
rx docker init                # Generate Dockerfile
rx docker build               # Build Docker image
```

### Generated Dockerfile Features
- Multi-stage builds for minimal image size
- Non-root user for security
- Proper layer caching for dependencies

---

## Publishing

Publish packages to PyPI or private registries.

```bash
rx publish                    # Publish to PyPI
rx publish --registry private # Publish to private registry
rx release patch              # Bump version and publish
rx release minor --tag        # Bump, tag, and publish
```

### Features
- **Version bumping** - Semantic versioning support
- **Git tagging** - Automatic tag creation
- **Multiple registries** - Support for private indexes

---

## Configuration

### pyproject.toml
```toml
[project]
name = "my-package"
version = "0.1.0"
requires-python = ">=3.10"
dependencies = [
    "requests>=2.28",
]

[project.optional-dependencies]
dev = ["pytest", "black", "ruff"]

[tool.rx]
python = "3.12"

[tool.rx.scripts]
test = "pytest -v tests/"
lint = "ruff check ."
format = "black ."

[tool.rx.dotenv]
enabled = true
files = [".env", ".env.local"]
```

### Environment Variables
| Variable | Description |
|----------|-------------|
| `RX_CACHE_DIR` | Override cache directory |
| `RX_NO_CACHE` | Disable caching |
| `RX_OFFLINE` | Offline mode (use cache only) |
| `VIRTUAL_ENV` | Active virtual environment |

---

## Self-Update

Keep rx up to date with built-in update functionality.

```bash
rx self-update              # Update to latest version
rx self-update --check      # Check for updates without installing
rx self-update --force      # Force update even if on latest
```

### Smart Update Detection

rx automatically detects how it was installed and uses the appropriate update method:

| Install Method | Update Command |
|---------------|----------------|
| pip (`pip install rx-pro`) | `pip install --upgrade rx-pro` |
| cargo (`cargo install pro-cli`) | `cargo install pro-cli` |
| Binary (curl/GitHub release) | Downloads from GitHub releases |

```bash
$ rx self-update --check
Current version: 0.1.13
Install method:  pip
Location:        /usr/local/bin/rx

To update, run: pip install --upgrade rx-pro
```

---

## Performance

Pro is built for speed:

| Operation | Pro | Poetry | pip |
|-----------|-----|--------|-----|
| Cold install (medium project) | ~100ms | ~5s | ~3s |
| Warm install (cached) | ~20ms | ~2s | ~1s |
| Dependency resolution | ~50ms | ~3s | ~2s |
| Pure Python wheel build | ~30ms | ~500ms | ~400ms |

*Benchmarks on M1 MacBook Pro, representative medium-sized project*
