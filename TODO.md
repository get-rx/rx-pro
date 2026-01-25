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

## Future Enhancements

### Priority: Medium

- [ ] `rx remove` - Remove dependencies
- [ ] `rx update` - Update dependencies within constraints
- [ ] `rx run` - Run commands in venv

### Priority: Low

- [ ] `rx build` - Build wheel/sdist
- [ ] `rx publish` - Publish to PyPI
- [ ] Workspace support
- [ ] Plugin system (Wasm)
- [ ] Platform markers in lockfile
- [ ] Dependency graph in lockfile
- [ ] Editable installs (PEP 660)
