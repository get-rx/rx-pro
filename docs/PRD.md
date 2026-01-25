# Product Requirements Document (PRD): T-Rex

| Field | Value |
|-------|-------|
| **Project Name** | T-Rex (rx) |
| **Status** | Draft / Planning |
| **Target Release** | v0.1.0 (Alpha) |
| **Primary Language** | Rust |
| **License** | MIT / Apache 2.0 |

---

## 1. Executive Summary

T-Rex is a unified Python package manager and build tool written in Rust. It aims to obsolete both Poetry and uv by combining the speed and standards-compliance of uv with the developer UX and extensibility of Poetry.

Its primary innovation is **rx-core**, a native Rust build backend that eliminates the need to spawn Python processes during the build step, and a **WebAssembly (Wasm) plugin system** that offers safe, high-performance extensibility.

---

## 2. Problem Statement

The current Python ecosystem is fragmented between two extremes:

### Poetry
- Excellent UX and "all-in-one" feel
- Suffers from slow performance (Python runtime)
- Non-standard configuration (legacy `[tool.poetry]`)
- Brittle plugin system

### uv
- Incredible installation speed
- Lacks a native build backend (delegates to slower Python tools like Hatch/Flit)
- Lacks a plugin system (monolithic)
- Forces users to script complex workflows externally

### Opportunity

There is no single tool that offers Rust-speed for both installation AND building, while retaining the ability to safely extend the tool for enterprise workflows.

---

## 3. Goals & Success Metrics

### 3.1 Key Objectives

| Objective | Description |
|-----------|-------------|
| **Performance** | Match or exceed uv in dependency resolution and installation time |
| **Build Innovation** | Reduce wheel build times by 90% compared to standard Python backends (Hatchling/Setuptools) for pure Python packages |
| **Extensibility** | Enable a plugin ecosystem that incurs <5ms overhead startup time |
| **Standards** | Full compliance with PEP 621 (`[project]` table) and PEP 517 (Build backend) |

### 3.2 Success Metrics (KPIs)

| Metric | Target |
|--------|--------|
| **Cold Install** | <100ms for a medium-sized project (cached) |
| **Build Time** | <50ms to produce a generic Wheel/Sdist for a pure Python project |
| **Adoption** | Support 100% of the top 500 PyPI packages' metadata formats |

---

## 4. Functional Requirements

### 4.1 Feature Set: The Core (Lifecycle Management)

| ID | Requirement |
|----|-------------|
| **REQ-CORE-001** | **Resolver**: Must use `pubgrub-rs` for dependency resolution. Must support universal resolution (cross-platform locking) by default. |
| **REQ-CORE-002** | **Installer**: Must use parallel downloading and zero-copy caching (`rkyv` or similar) to unpack wheels. Support for hardlinking in monorepos. |
| **REQ-CORE-003** | **Virtual Environments**: Must natively manage venvs without requiring `virtualenv` or the `venv` module. |
| **REQ-CORE-004** | **Script Running**: `rx run <script>` must execute scripts in the isolated environment with minimal bootstrap overhead. |

### 4.2 Feature Set: The Builder (rx-core)

| ID | Requirement |
|----|-------------|
| **REQ-BLD-001** | **Native Backend**: A PEP 517 compliant build backend written in Rust. |
| **REQ-BLD-002** | **Zero-Python Build**: For pure Python packages, the tool must generate `.whl` and `.tar.gz` (sdist) artifacts by reading `pyproject.toml` and file systems directly, without invoking a Python interpreter. |
| **REQ-BLD-003** | **C-Ext Support**: For mixed packages (Rust/C), the backend must support hooks to invoke compilers (e.g., calling `maturin` or `setuptools` only when necessary). |

### 4.3 Feature Set: The Plugin System

| ID | Requirement |
|----|-------------|
| **REQ-PLG-001** | **Wasm Runtime**: Integrate a Wasm runtime (e.g., Wasmtime or Extism) to load plugins. |
| **REQ-PLG-002** | **Hooks**: Expose lifecycle hooks: `pre-resolve`, `post-resolve`, `pre-build`, `post-build`, `pre-publish`. |
| **REQ-PLG-003** | **Sandboxing**: Plugins must not have arbitrary file system access unless explicitly granted permissions in `pyproject.toml`. |

### 4.4 Feature Set: The Workspace (Monorepo)

| ID | Requirement |
|----|-------------|
| **REQ-WORK-001** | **Unified Lockfile**: A single `rx.lock` at the workspace root resolving dependencies for all member projects. |
| **REQ-WORK-002** | **Dependency Hoisting**: Shared dependencies across the workspace must be physically stored once on disk (content-addressable store). |
| **REQ-WORK-003** | **Graph Execution**: `rx run --affected` should identify which packages have changed and run tasks only for them and their dependents. |

---

## 5. Technical Architecture & Constraints

### 5.1 Tech Stack

| Component | Technology |
|-----------|------------|
| **Language** | Rust (Stable) |
| **CLI Framework** | `clap` |
| **Async Runtime** | `tokio` |
| **Wasm Runtime** | `extism` (for multi-language plugin support) |
| **Compression** | `zstd` (preferred for cache), `deflate` (for wheels) |

### 5.2 Compatibility

| Aspect | Requirement |
|--------|-------------|
| **OS** | Linux, macOS, Windows |
| **Python Versions** | Must manage Python 3.8+ |
| **Standards** | PEP 508 (Dependencies), PEP 440 (Versioning), PEP 621 (Metadata), PEP 517 (Builds), PEP 660 (Editable Installs) |

### 5.3 Migration Path

| Source | Command |
|--------|---------|
| **From Poetry** | `rx import poetry` command to convert `[tool.poetry]` to `[project]` |
| **From pip/uv** | `rx init` attempts to parse `requirements.txt` |

---

## 6. User Stories

| ID | As a... | I want to... | So that... |
|----|---------|--------------|------------|
| **US-01** | Library Author | Run `rx build` | I can generate a wheel for PyPI in milliseconds without waiting for a build backend to spin up |
| **US-02** | Enterprise Dev | Write a plugin in Python that checks for valid licenses | I can enforce compliance in CI without forking the package manager |
| **US-03** | Data Scientist | Install pytorch instantly | I don't waste 5 minutes waiting for dependency resolution on large graphs |
| **US-04** | Monorepo Lead | Manage 50 microservices in one repo | I can update a shared library and ensure all services are locked to compatible versions immediately |

---

## 7. Roadmap / Phasing

### Phase 1: "The Fast Consumer" (MVP)

| Aspect | Details |
|--------|---------|
| **Goal** | Replicate uv functionality + PEP 621 support |
| **Deliverables** | Installer, Resolver, Virtual Env management |
| **Timeline** | Months 1-3 |

### Phase 2: "The Native Producer"

| Aspect | Details |
|--------|---------|
| **Goal** | Introduce rx-core and build capabilities |
| **Deliverables** | `rx build`, `rx publish`, support for `[build-system] requires = ["rx-core"]` |
| **Timeline** | Months 3-5 |

### Phase 3: "The Platform"

| Aspect | Details |
|--------|---------|
| **Goal** | Plugins and Workspaces |
| **Deliverables** | Wasm plugin host, Workspace dependency graph logic |
| **Timeline** | Months 5-8 |

---

## 8. Open Questions / Risks

### Risks

| Risk | Mitigation |
|------|------------|
| Can we perfectly replicate setuptools behavior for legacy packages? | Fallback to standard `pip install` logic for non-standard packages |
| Will the Wasm plugin system be too difficult for Python devs to adopt? | Provide a "Python-to-Wasm" compiler helper or a standard Python library that compiles to Wasm automatically |

### Open Questions

- How do we handle authentication for private feeds in a way that is better than Poetry's keyring issues?

---

## Appendix: CLI Command Reference (Proposed)

```bash
# Project initialization
rx init                    # Initialize new project
rx import poetry           # Import from Poetry project

# Dependency management
rx add <package>           # Add dependency
rx remove <package>        # Remove dependency
rx lock                    # Generate/update lockfile
rx sync                    # Sync venv with lockfile

# Building & Publishing
rx build                   # Build wheel/sdist
rx publish                 # Publish to PyPI

# Execution
rx run <script>            # Run script in venv
rx run --affected <cmd>    # Run only for affected packages

# Workspace
rx workspace init          # Initialize workspace
rx workspace add <path>    # Add member to workspace

# Versioning
rx version                 # Show current version
rx version bump <part>     # Bump major/minor/patch
rx version set <version>   # Set explicit version
```

---

## 9. Implementation Status & TODO

### Completed ✅

| Component | Status | Notes |
|-----------|--------|-------|
| PEP 440 Version Parsing | ✅ Done | Full support for pre/post/dev/epoch/local |
| PEP 508 Requirement Parsing | ✅ Done | Includes markers and extras |
| Version Specifier Parsing | ✅ Done | Converts to pubgrub ranges |
| PyPI Index Client | ✅ Done | Caching, concurrent fetching |
| pubgrub DependencyProvider | ✅ Done | Pre-crawls transitive deps |
| Dependency Resolver | ✅ Done | Returns packages with URLs/hashes |

### In Progress 🚧

| Component | Status | Notes |
|-----------|--------|-------|
| CLI Integration | 🚧 Stub | Commands exist but don't call resolver |

### TODO 📋

| Priority | Component | Description |
|----------|-----------|-------------|
| **P0** | **Native SemVer Tool** | Implement Semantic Versioning that beats Poetry's - fast, correct, with comparison/bumping/range satisfaction |
| **P1** | CLI `init` Command | Create pyproject.toml with PEP 621 metadata, venv setup |
| **P1** | CLI `add` Command | Wire up to resolver, update pyproject.toml, generate lockfile |
| **P2** | Lockfile Format | Design and implement `rx.lock` format |
| **P2** | CLI `sync` Command | Install resolved packages into venv |
| **P3** | CLI `remove` Command | Remove dependencies, re-resolve |
| **P3** | CLI `lock` Command | Regenerate lockfile without installing |
