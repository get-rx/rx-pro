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

### Future Enhancements (audit)

- [ ] Check packages against PyPI Advisory Database
- [ ] Check packages against GitHub Advisory Database
- [ ] Ignore list support in `[tool.rx.audit]` section of pyproject.toml

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
- [ ] **Docker Integration** (`rx docker build`)
  - [ ] Generate Dockerfile from `[tool.rx.docker]` config
  - [ ] Build image directly without manual Dockerfile

#### Developer Experience
- [ ] **Shell Command** (`rx shell`)
  - [ ] Spawn subshell with venv activated
  - [ ] Support bash, zsh, fish, powershell
- [ ] **Dotenv Support**
  - [ ] Auto-load `.env` on `rx run` and `rx shell`
  - [ ] Config in `[tool.rx.dotenv]`
- [ ] **Script Aliases** (`[tool.rx.scripts]`)
  - [ ] Define command aliases like npm scripts
  - [ ] `rx run test` → `pytest -v tests/`
- [ ] **Task Runner** (`rx task`)
  - [ ] Predefined tasks with dependencies
  - [ ] Parallel task execution

### Priority: Medium

- [ ] `rx remove` - Remove dependencies
- [x] `rx build` - Build wheel/sdist ✅
- [x] `rx publish` - Publish to PyPI ✅

### Priority: Medium - Monorepo & Workspace

- [ ] **Workspace Support**
  - [ ] `rx workspace init` - Initialize workspace
  - [ ] `rx workspace add <path>` - Add member project
  - [ ] Unified `rx.lock` at workspace root
  - [ ] Shared venv option across members
- [ ] **Polylith Architecture**
  - [ ] Component-based code sharing across projects
  - [ ] `bases/`, `components/`, `projects/` structure
- [ ] **Local Path Dependencies**
  - [ ] Handle relative path deps in monorepos
  - [ ] Include local code when building wheels
- [ ] **Affected Detection**
  - [ ] `rx run --affected` for changed packages only
  - [ ] Git-based change detection

### Priority: Low - Platform & Infrastructure

- [ ] **Plugin System (Wasm)**
  - [ ] `rx plugin list/add/remove`
  - [ ] Lifecycle hooks: pre-resolve, post-build, etc.
  - [ ] Sandboxed execution via Wasmtime/Extism
- [ ] Platform markers in lockfile
- [ ] Dependency graph in lockfile
- [ ] Editable installs (PEP 660)
- [ ] Private registry authentication
- [ ] `rx import poetry` - Migration from Poetry
