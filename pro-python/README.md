# Pro Python Package

A blazing-fast Python package manager written in Rust.

## Installation

```bash
pip install rx-pro
```

## CLI Usage

```bash
# Initialize a new project
rx init my-project

# Add dependencies
rx add requests numpy pandas

# Install dependencies
rx sync

# Run commands
rx run python main.py

# Build wheel
rx build

# Security audit
rx audit
```

## Python API

```python
from pro import resolve, sync, build, audit

# Resolve dependencies
packages = resolve(["requests>=2.28", "numpy"])
for name, version, url in packages:
    print(f"{name}=={version}")

# Sync project to venv
count = sync("./my-project")
print(f"Installed {count} packages")

# Build wheel and sdist
result = build("./my-project", "./dist")
print(f"Wheel: {result['wheel']}")
print(f"Sdist: {result['sdist']}")

# Security audit
vulnerabilities = audit("./my-project")
for pkg, ver, cve, severity, description in vulnerabilities:
    print(f"{pkg}=={ver}: {cve} ({severity})")
    print(f"  {description}")
```

## Features

- **10-100x faster** than Poetry
- **Native Rust build backend** - no Python subprocess
- **WebAssembly plugins** for extensibility
- **Full monorepo support** with workspaces
- **Security audit** with OSV database
- **Docker integration** for deployment

## Documentation

See the [full documentation](https://rxpro.net/) for more details.

## License

Dual-licensed under MIT and Apache 2.0.
