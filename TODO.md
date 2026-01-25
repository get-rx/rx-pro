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

## Next Up: CLI Integration

## CLI Integration

Wire up CLI commands to use the implemented resolver.

### `rx init`

- [ ] Create project directory structure
- [ ] Generate pyproject.toml with PEP 621 metadata
- [ ] Prompt for project name, version, description
- [ ] Set up virtual environment
- [ ] Generate initial rx.lock (empty)

### `rx add <package>`

- [ ] Parse package specifier (name, version constraint)
- [ ] Load existing pyproject.toml
- [ ] Call resolver with new + existing dependencies
- [ ] Update pyproject.toml `[project.dependencies]`
- [ ] Generate/update rx.lock with resolved versions
- [ ] Optionally sync venv (install packages)

### `rx sync`

- [ ] Read rx.lock
- [ ] Download packages (parallel)
- [ ] Install into venv
- [ ] Verify hashes

### `rx lock`

- [ ] Read pyproject.toml dependencies
- [ ] Run resolver
- [ ] Write rx.lock (without installing)

---

## Lockfile Format (rx.lock)

Design a lockfile format that captures:

- [ ] All resolved packages with exact versions
- [ ] Download URLs
- [ ] Hashes (sha256)
- [ ] Dependency graph (which package requires which)
- [ ] Platform markers (for universal locks)

Consider: TOML vs JSON vs custom format

---

## Future Enhancements

- [ ] `rx remove` - Remove dependencies
- [ ] `rx update` - Update dependencies within constraints
- [ ] `rx build` - Build wheel/sdist
- [ ] `rx publish` - Publish to PyPI
- [ ] `rx run` - Run commands in venv
- [ ] Workspace support
- [ ] Plugin system (Wasm)
