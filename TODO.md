# T-Rex Implementation TODO

## ✅ Completed: Native SemVer Tool

**Location**: `rx-core/src/semver/` module

### Implemented Features

- [x] Parse SemVer strings (1.2.3, 1.2.3-alpha.1, 1.2.3+build)
- [x] Version comparison (ordering, equality)
- [x] Version bumping (major, minor, patch, prerelease)
- [x] Range parsing and satisfaction checking
  - [x] Caret ranges: `^1.2.3` (compatible with 1.x.x)
  - [x] Tilde ranges: `~1.2.3` (compatible with 1.2.x)
  - [x] Exact: `=1.2.3`
  - [x] Comparison: `>=1.2.3`, `<2.0.0`
  - [x] Hyphen ranges: `1.2.3 - 2.0.0`
  - [x] OR combinations: `^1.2.3 || ^2.0.0`
  - [x] Wildcard: `1.*`, `1.2.*`
- [x] Prerelease handling (alpha < beta < rc < release)
- [x] Build metadata (ignored in comparisons per spec)
- [x] Serde support for serialization/deserialization

---

## ✅ Completed: CLI Integration

### `rx init` ✅

- [x] Create project directory structure (src/, tests/)
- [x] Generate pyproject.toml with PEP 621 metadata
- [x] Generate initial rx.lock (empty)
- [x] Create __init__.py with version

### `rx add <package>` ✅

- [x] Parse package specifier (name, version constraint)
- [x] Load existing pyproject.toml
- [x] Call resolver with new + existing dependencies
- [x] Update pyproject.toml `[project.dependencies]`
- [x] Generate/update rx.lock with resolved versions
- [x] Support --dev flag for dev dependencies

### `rx lock` ✅

- [x] Read pyproject.toml dependencies
- [x] Run resolver
- [x] Write rx.lock (without installing)

### `rx sync` ✅

- [x] Read rx.lock
- [x] --dry-run flag to show what would be installed
- [x] Create virtual environment natively (without venv module)
- [x] Download packages (parallel, up to 8 concurrent)
- [x] Cache downloaded wheels (~/.cache/rx/wheels)
- [x] Verify SHA256 hashes
- [x] Install into venv site-packages
- [x] --recreate flag to recreate venv from scratch
- [x] --python flag to specify Python interpreter

---

## ✅ Completed: Lockfile Format (rx.lock)

TOML format with:
- [x] All resolved packages with exact versions
- [x] Download URLs
- [x] Hashes (sha256)
- [ ] Dependency graph (future enhancement)
- [ ] Platform markers (future enhancement)

---

## ✅ Completed: Security Audit (`rx audit`)

**Location**: `rx-core/src/audit/` module, `rx-cli/src/commands/audit.rs`

### Implemented Features

- [x] OSV API client for vulnerability detection
- [x] Batch detection with full detail fetching for affected packages
- [x] CVSS severity extraction from multiple OSV response locations
- [x] Report vulnerabilities with severity (Critical/High/Medium/Low)
- [x] `--fix` flag to auto-update to patched versions
- [x] Handle transitive dependencies via re-resolution
- [x] `--force` flag to apply fixes that require transitive updates
- [x] Non-zero exit code for CI integration
- [x] `--severity` flag to set minimum severity threshold
- [x] `--ignore` flag to skip specific vulnerability IDs
- [x] Text and JSON output formats (`--format`)

### Future Enhancements (audit) ✅

- [x] PyPI yanked version detection (warns about yanked packages)
- [x] Ignore list support in `[tool.rx.audit]` section of pyproject.toml
  - [x] Simple string format: `ignore = ["CVE-2023-1234"]`
  - [x] Full format with reason and expiration: `ignore = [{ id = "CVE-2023-1234", reason = "Not applicable", expires = "2024-12-31" }]`
- [x] `--no-yanked` flag to skip yanked version checking
- Note: OSV already aggregates PyPI (PYSEC) and GitHub (GHSA) advisories

---

## ✅ Completed: Run Command (`rx run`)

**Location**: `rx-cli/src/commands/run.rs`

### Implemented Features

- [x] Execute commands in the project's virtual environment
- [x] Prepend venv bin directory to PATH
- [x] Set VIRTUAL_ENV environment variable
- [x] Forward exit codes from child process
- [x] Support for venv-installed commands (python, pip, pytest, etc.)
- [x] Fallback to system PATH for commands not in venv
- [x] `--project` flag to specify project directory

---

## ✅ Completed: Update Command (`rx update`)

**Location**: `rx-cli/src/commands/update.rs`

### Implemented Features

- [x] Re-resolve dependencies to get latest versions within constraints
- [x] Compare old vs new lockfile and show changes
- [x] Update specific packages only (`rx update requests urllib3`)
- [x] `--dry-run` flag to preview changes without applying
- [x] `--dev` flag to include dev dependencies
- [x] `--project` flag to specify project directory
- [x] Detect when all packages are already up to date

---

## Future Enhancements

### Priority: High - Competitive Features

#### Automation & Versioning
- [x] **Dynamic Versioning** - Derive version from git tags automatically ✅
  - [x] Read version from `git describe --tags`
  - [x] Support tag patterns (`v{version}`, `{version}`, custom)
  - [x] Generate dev versions for commits after tag (e.g., `1.2.3.dev4+gabc123`)
  - [x] Config in `[tool.rx.versioning]`
- [x] **Version Commands** ✅
  - [x] `rx version` - Show current version (from git or pyproject.toml)
  - [x] `rx version bump major/minor/patch/pre`
  - [x] `rx version set <version>`
- [x] **Release Workflow** (`rx release`) ✅
  - [x] Interactive release: bump version, tag, changelog, publish
  - [x] `rx release --bump minor` for non-interactive
  - [x] Conventional commits → changelog generation

#### Deployment & Exporting
- [x] **Export** (`rx export`) ✅
  - [x] Generate `requirements.txt` from lockfile
  - [x] `--format constraints` for constraints.txt
  - [x] `--with-hashes` for hash-pinned requirements
  - [x] `--only` and `--exclude` filters for specific packages
  - [x] `-o/--output` to write to file (defaults to stdout)
- [x] **Bundle** (`rx bundle`) ✅
  - [x] Bundle project into standalone venv
  - [x] `--target lambda` for AWS Lambda zip
  - [x] `--target docker` for Docker-ready bundle
  - [x] `--deps-only` for dependencies without source
  - [x] `--handler` for Lambda entry point
  - [x] `--python-version` for Docker base image
- [x] **Docker Integration** (`rx docker`) ✅
  - [x] `rx docker generate` - Generate Dockerfile from config
  - [x] `rx docker build` - Build image directly
  - [x] `rx docker config` - Show current Docker configuration
  - [x] Multi-stage builds for smaller images
  - [x] Auto-generate .dockerignore
  - [x] Support for custom entrypoint, cmd, env, expose, labels
  - [x] APT package installation
  - [x] Custom pre/post-copy commands

#### Developer Experience
- [x] **Shell Command** (`rx shell`) ✅
  - [x] Spawn subshell with venv activated
  - [x] Support bash, zsh, fish, powershell
  - [x] Auto-detect user's shell from SHELL env var
  - [x] Modified prompt to show venv name
  - [x] `-s/--shell` to override shell
- [x] **Dotenv Support** ✅
  - [x] Auto-load `.env` on `rx run` and `rx shell`
  - [x] Config in `[tool.rx.dotenv]`
  - [x] Support for quoted values, escape sequences, multiline
  - [x] Variable interpolation (`${VAR}` and `$VAR`)
  - [x] Extra files via `extra_files` config
  - [x] `--no-dotenv` flag to skip loading
  - [x] `override` config to override existing env vars
- [x] **Script Aliases** (`[tool.rx.scripts]`) ✅
  - [x] Define command aliases like npm scripts
  - [x] `rx run test` → `pytest -v tests/`
  - [x] `rx run --list` to show available scripts
  - [x] Append extra args: `rx run test -k foo` → `pytest -v tests/ -k foo`
  - [x] Proper quote handling in script commands
- [x] **Task Runner** (`rx task`) ✅
  - [x] Predefined tasks with dependencies
  - [x] Parallel task execution
  - [x] Topological sort with cycle detection
  - [x] `--list` flag to show available tasks
  - [x] `--sequential` flag to disable parallel execution

### Priority: Medium

- [x] `rx remove` - Remove dependencies ✅
  - [x] Remove from main or dev dependencies
  - [x] Re-resolve and update lockfile
  - [x] `--dry-run` to preview changes
  - [x] `--no-lock` to skip lockfile update
- [x] `rx build` - Build wheel/sdist ✅
- [x] `rx publish` - Publish to PyPI ✅

### Priority: Medium - Monorepo & Workspace

- [x] **Workspace Support** ✅
  - [x] `rx workspace init` - Initialize workspace
  - [x] `rx workspace add <path>` - Add member project
  - [x] `rx workspace remove <path>` - Remove member
  - [x] `rx workspace list` - List members with info
  - [x] `rx workspace lock` - Generate unified lockfile
  - [x] `rx workspace sync` - Install all dependencies
  - [x] Unified `rx.lock` at workspace root
  - [x] Shared venv option (`--shared-venv`)
- [x] **Polylith Architecture** ✅
  - [x] `rx polylith init <namespace>` - Initialize Polylith workspace
  - [x] `rx polylith create base <name>` - Create entry point
  - [x] `rx polylith create component <name>` - Create reusable component
  - [x] `rx polylith create project <name>` - Create deployable project
  - [x] `rx polylith list` - List all bricks
  - [x] `rx polylith check` - Check for cycles and architecture violations
  - [x] `rx polylith info <name>` - Show brick details
  - [x] Component interface pattern (interface.py for public API)
  - [x] Workspace integration with glob patterns
- [x] **Local Path Dependencies** ✅
  - [x] Handle relative path deps in monorepos
  - [x] Support editable and copy installation modes
  - [x] Auto-install during `rx sync` and `rx workspace sync`
  - [ ] Include local code when building wheels (future enhancement)
- [x] **Affected Detection** ✅
  - [x] `rx affected` command to list affected workspace members
  - [x] `rx run --affected` to run commands on changed packages only
  - [x] Git-based change detection (committed, uncommitted, untracked)
  - [x] Transitive dependency detection (`--include-dependents`)
  - [x] Configurable base branch (`--base`)

### Priority: Low - Platform & Infrastructure

- [x] **Plugin System (WebAssembly)** ✅
  - [x] `rx plugin list` - List installed plugins
  - [x] `rx plugin add <name> <source>` - Add plugin from file or URL
  - [x] `rx plugin remove <name>` - Remove plugin
  - [x] `rx plugin info <name>` - Show plugin details
  - [x] `rx plugin enable/disable <name>` - Toggle plugin
  - [x] `rx plugin run <hook>` - Manually run a hook
  - [x] `rx plugin init <name>` - Create plugin development template
  - [x] Lifecycle hooks: pre-resolve, post-resolve, pre-build, post-build, pre-publish
  - [x] Sandboxed execution via Extism/Wasmtime
  - [x] Capability-based permissions model
  - [x] Plugin manifest support (.toml file)
  - [x] Global (~/.rx/plugins) and project-local plugins
  - [x] Configuration in `[tool.rx.plugins]`
- [ ] Platform markers in lockfile
- [ ] Dependency graph in lockfile
- [ ] Editable installs (PEP 660)
- [ ] Private registry authentication
- [ ] `rx import poetry` - Migration from Poetry
