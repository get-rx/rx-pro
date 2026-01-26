# Introducing Pro: A Blazing-Fast Python Package Manager Written in Rust

Today, I'm excited to announce Pro, a new Python package manager that brings Rust-level performance to the Python ecosystem. After months of development, Pro is ready for public use.

## Why Another Package Manager?

The Python packaging ecosystem has improved significantly with tools like Poetry and uv, but there's still room for improvement:

- **Poetry** has great UX but slow resolution
- **uv** is fast but delegates building to Python
- **pip** is ubiquitous but lacks modern features

Pro aims to combine the best of all worlds: **the speed of Rust, the ergonomics of Poetry, and unique features like WebAssembly plugins**.

## Key Features

### 10-50x Faster Than Poetry

Pro uses a native Rust dependency resolver with parallel downloads and smart caching:

```bash
$ time rx sync
Resolving... 0.3s
Downloading 47 packages... 1.2s
Installing... 0.8s
Total: 2.3s

$ time poetry install
... 45.2s
```

### Native Build Backend

Unlike other tools that shell out to Python for building, Pro builds pure Python packages entirely in Rust:

```bash
$ rx build
Building wheel... ✓ 45ms
Building sdist... ✓ 23ms
```

### WebAssembly Plugins

Pro supports sandboxed plugins via WebAssembly, allowing safe extensibility:

```toml
[tool.rx.plugins]
license-checker = "~/.rx/plugins/license-checker.wasm"
```

### Full Monorepo Support

Workspaces, unified lockfiles, and affected detection make Pro ideal for large codebases:

```bash
$ rx affected --base main
packages/api
packages/core

$ rx run --affected pytest
Running tests for 2 affected packages...
```

### Security Audit

Built-in vulnerability scanning with auto-fix:

```bash
$ rx audit
Found 2 vulnerabilities

$ rx audit --fix
Upgraded requests 2.25.0 -> 2.31.0
```

## Getting Started

```bash
# Install
pip install trex-py

# Create a project
rx init my-project
cd my-project

# Add dependencies
rx add requests numpy pandas

# Install
rx sync

# Run
rx run python main.py
```

## What's Next

Pro is open source and ready for production use. We're actively working on:

- More package manager migrations (PDM, Hatch)
- IDE integrations
- Performance optimizations
- Plugin SDK documentation

## Try It Out

- **Website**: https://stherrien.github.io/pro/
- **GitHub**: https://github.com/stherrien/pro
- **PyPI**: https://pypi.org/project/trex-py/

We'd love your feedback! Open issues, submit PRs, or just star the repo if you find it useful.

---

*Pro is dual-licensed under MIT and Apache 2.0.*
