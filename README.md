# T-Rex (rx)

> A unified Python package manager and build tool written in Rust

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-planning-yellow.svg)]()

## Overview

T-Rex is a next-generation Python package manager that combines:

- **Rust-level performance** for both dependency resolution AND building
- **Poetry-like UX** with intuitive commands and workflows
- **WebAssembly plugins** for safe, high-performance extensibility
- **Full PEP compliance** (621, 517, 508, 440, 660)

## Why T-Rex?

| Tool | Speed | Build Backend | Plugins | Standards |
|------|-------|---------------|---------|-----------|
| Poetry | Slow | Delegates to Python | Brittle | Non-standard |
| uv | Fast | Delegates to Python | None | PEP compliant |
| **T-Rex** | Fast | Native Rust | Wasm-based | PEP compliant |

## Quick Start

```bash
# Initialize a new project
rx init

# Add dependencies
rx add requests numpy

# Install dependencies
rx sync

# Run your project
rx run python main.py

# Build for distribution
rx build
```

## Key Features

### Native Rust Build Backend (rx-core)

Build pure Python packages without spawning a Python interpreter:

```bash
$ rx build
Built wheel in 45ms: dist/mypackage-1.0.0-py3-none-any.whl
```

### WebAssembly Plugin System

Extend T-Rex safely with sandboxed plugins:

```toml
# pyproject.toml
[tool.rx.plugins]
license-checker = { path = "./plugins/license-checker.wasm" }
```

### Workspace Support

Manage monorepos with a unified lockfile:

```bash
$ rx run --affected test
Running tests for 3 affected packages...
```

## Documentation

- [Product Requirements Document](docs/PRD.md)
- [Architecture Overview](docs/ARCHITECTURE.md)
- [Contributing Guide](docs/CONTRIBUTING.md)

## Project Status

T-Rex is currently in the **planning phase**. See the [roadmap](docs/PRD.md#7-roadmap--phasing) for details.

| Phase | Status | Target |
|-------|--------|--------|
| Phase 1: The Fast Consumer | Planning | Months 1-3 |
| Phase 2: The Native Producer | Not Started | Months 3-5 |
| Phase 3: The Platform | Not Started | Months 5-8 |

## License

Dual-licensed under MIT and Apache 2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
