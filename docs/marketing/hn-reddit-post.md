# Hacker News Post

**Title:** Pro: A Python package manager written in Rust with native build backend and Wasm plugins

**Text:**
Hi HN! I built Pro, a Python package manager that's 10-50x faster than Poetry.

Key differentiators:
- **Native Rust build backend** - builds wheels without spawning Python
- **WebAssembly plugins** - safely extend functionality
- **Full monorepo support** - workspaces, affected detection, Polylith architecture
- **Security audit** - OSV database with auto-fix

Quick start:
```
pip install trex-py
rx init my-project && cd my-project
rx add requests numpy
rx sync
```

Website: https://stherrien.github.io/pro/
GitHub: https://github.com/stherrien/pro

Would love feedback on the API design and feature set!

---

# Reddit r/Python Post

**Title:** I built a Python package manager in Rust that's 10-50x faster than Poetry

**Text:**
After being frustrated with slow dependency resolution in Poetry, I decided to build something faster. Pro is written entirely in Rust and includes:

**Speed:**
- Parallel downloads with smart caching
- Native Rust resolver (same algorithm as cargo)
- Resolving Django + 50 deps: 2.3s vs Poetry's 20+ seconds

**Unique Features:**
- Native build backend (builds wheels in Rust, not Python)
- WebAssembly plugin system for safe extensibility
- Full monorepo support with workspaces
- Security audit with auto-fix
- Docker integration
- Polylith architecture support

**Getting Started:**
```bash
pip install trex-py
rx init my-project
rx add requests numpy pandas
rx sync
```

Links:
- Website: https://stherrien.github.io/pro/
- GitHub: https://github.com/stherrien/pro
- Docs: https://github.com/stherrien/pro#readme

Looking for feedback! What features would you want in a package manager?

---

# Reddit r/rust Post

**Title:** Show r/rust: Pro - A Python package manager with native Rust build backend

**Text:**
I've been working on Pro, a Python package manager written in Rust. The interesting part from a Rust perspective:

**Architecture:**
- `rx-core`: Core library with resolver, installer, builder
- `rx-cli`: CLI using clap
- `rx-plugin`: WebAssembly plugin system using wasmtime
- `rx-python`: PyO3 bindings

**Technical Highlights:**
- PubGrub algorithm for dependency resolution
- Parallel async downloads with tokio
- Native wheel/sdist building (no Python subprocess)
- TOML-based lockfile with platform markers

**Why Rust?**
The main motivation was performance. Python's packaging tools are slow because they're written in Python. By using Rust, we get:
- 10-50x faster resolution
- Sub-50ms wheel builds
- Memory-safe concurrent downloads

GitHub: https://github.com/stherrien/pro

Would appreciate any feedback on the code!
