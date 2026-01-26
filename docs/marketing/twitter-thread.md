# Twitter/X Launch Thread

## Tweet 1 (Main)
Introducing Pro: A Python package manager written in Rust.

10-50x faster than Poetry. Native build backend. WebAssembly plugins.

pip install trex-py

https://github.com/stherrien/pro

Thread with features:

## Tweet 2 (Speed)
Speed comparison:

Pro: 2.3s
uv: 2.6s
Poetry: 20.6s
pip: 21.3s

(Installing Django + 50 dependencies, cold cache)

Rust resolver + parallel downloads + smart caching = fast.

## Tweet 3 (Native Build)
Unlike other tools, Pro builds wheels WITHOUT spawning Python.

Pure Rust build backend = 45ms wheel builds.

No setuptools. No subprocess. Just fast.

## Tweet 4 (Plugins)
Extend Pro safely with WebAssembly plugins:

[tool.rx.plugins]
license-checker = "path/to/plugin.wasm"

Sandboxed execution. Custom hooks. Infinite possibilities.

## Tweet 5 (Monorepo)
Full monorepo support:

rx workspace init
rx workspace add packages/*
rx run --affected test

Unified lockfile. Affected detection. Polylith architecture.

## Tweet 6 (Security)
Built-in security scanning:

rx audit
Found 2 vulnerabilities

rx audit --fix
Upgraded requests 2.25.0 -> 2.31.0

Auto-fix with one command.

## Tweet 7 (Docker)
Docker integration:

rx docker generate
rx docker build --tag myapp:latest

Multi-stage builds. Optimized images. One command deploy.

## Tweet 8 (CTA)
Get started in 30 seconds:

pip install trex-py
rx init my-project
cd my-project
rx add requests
rx sync

Star the repo if you find it useful!

https://github.com/stherrien/pro

---

# LinkedIn Post

Excited to announce Pro, a new Python package manager written in Rust!

After experiencing slow dependency resolution with existing tools, I built something faster. Pro is:

- 10-50x faster than Poetry
- Has a native Rust build backend (no Python subprocess)
- Supports WebAssembly plugins for extensibility
- Includes full monorepo/workspace support
- Built-in security vulnerability scanning

The Python ecosystem deserves fast, modern tooling. Pro brings Rust-level performance to Python development.

Try it out: pip install trex-py
GitHub: https://github.com/stherrien/pro

#Python #Rust #OpenSource #DeveloperTools #PackageManager
