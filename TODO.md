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

### `rx sync` (Partial)

- [x] Read rx.lock
- [x] --dry-run flag to show what would be installed
- [ ] Download packages (parallel) - TODO
- [ ] Install into venv - TODO
- [ ] Verify hashes - TODO

---

## ✅ Completed: Lockfile Format (rx.lock)

TOML format with:
- [x] All resolved packages with exact versions
- [x] Download URLs
- [x] Hashes (sha256)
- [ ] Dependency graph (future enhancement)
- [ ] Platform markers (future enhancement)

---

## Future Enhancements

- [ ] `rx sync` - Full installation into venv
- [ ] `rx remove` - Remove dependencies
- [ ] `rx update` - Update dependencies within constraints
- [ ] `rx build` - Build wheel/sdist
- [ ] `rx publish` - Publish to PyPI
- [ ] `rx run` - Run commands in venv
- [ ] Workspace support
- [ ] Plugin system (Wasm)
