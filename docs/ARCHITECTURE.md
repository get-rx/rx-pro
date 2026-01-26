# Architecture Overview

This document describes the high-level architecture of Pro (rx).

## System Components

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI (clap)                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Resolver   │  │   Builder    │  │   Runner     │          │
│  │  (pubgrub)   │  │  (rx-core)   │  │              │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                   │
│  ┌──────┴─────────────────┴─────────────────┴───────┐          │
│  │                  Core Engine                      │          │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐          │          │
│  │  │ VEnv    │  │ Cache   │  │ Index   │          │          │
│  │  │ Manager │  │ Manager │  │ Client  │          │          │
│  │  └─────────┘  └─────────┘  └─────────┘          │          │
│  └──────────────────────────────────────────────────┘          │
│                                                                 │
│  ┌──────────────────────────────────────────────────┐          │
│  │              Plugin Host (Extism/Wasm)           │          │
│  └──────────────────────────────────────────────────┘          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Component Details

### CLI Layer

The command-line interface built with `clap`. Responsible for:

- Parsing user commands and arguments
- Dispatching to appropriate subsystems
- Formatting output for terminal display

### Resolver

Dependency resolution engine using `pubgrub-rs`:

- **Universal Resolution**: Generates cross-platform compatible lockfiles
- **Conflict Detection**: Provides clear error messages for unsatisfiable constraints
- **Caching**: Remembers resolution decisions for incremental updates

### Builder (rx-core)

Native Rust build backend (PEP 517 compliant):

- **Wheel Generation**: Creates `.whl` files without Python
- **Sdist Generation**: Creates `.tar.gz` source distributions
- **Metadata Parsing**: Reads `pyproject.toml` directly in Rust

### Runner

Script and command execution:

- **Environment Activation**: Configures PATH and PYTHONPATH
- **Process Management**: Spawns and monitors child processes
- **Signal Handling**: Proper cleanup on interrupts

### Core Engine

Shared infrastructure components:

#### VEnv Manager
- Creates virtual environments natively
- Manages Python interpreter discovery
- Handles activation scripts

#### Cache Manager
- Content-addressable storage for wheels
- Zero-copy unpacking with `rkyv`
- Hardlink support for monorepos

#### Index Client
- PyPI API communication
- Simple/JSON index parsing
- Authentication handling

### Plugin Host

WebAssembly runtime for extensions:

- **Sandboxing**: Restricted file system access
- **Lifecycle Hooks**: pre-resolve, post-resolve, pre-build, post-build, pre-publish
- **Multi-language**: Supports plugins written in any Wasm-compilable language

## Data Flow

### Install Flow

```
User Command
    │
    ▼
┌─────────┐     ┌─────────┐     ┌─────────┐
│  Parse  │────▶│ Resolve │────▶│  Fetch  │
│pyproject│     │  Deps   │     │ Wheels  │
└─────────┘     └─────────┘     └─────────┘
                                     │
                                     ▼
                              ┌─────────┐     ┌─────────┐
                              │  Cache  │────▶│ Install │
                              │ Wheels  │     │ to VEnv │
                              └─────────┘     └─────────┘
```

### Build Flow

```
User Command
    │
    ▼
┌─────────┐     ┌─────────┐     ┌─────────┐
│  Parse  │────▶│ Collect │────▶│ Generate│
│Metadata │     │  Files  │     │  Wheel  │
└─────────┘     └─────────┘     └─────────┘
                                     │
                                     ▼
                              ┌─────────┐
                              │  Write  │
                              │ to Dist │
                              └─────────┘
```

## Directory Structure

```
pro/
├── Cargo.toml              # Workspace manifest
├── rx-cli/                 # CLI binary crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       └── commands/
├── rx-core/                # Core library crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── resolver/
│       ├── installer/
│       ├── builder/
│       └── venv/
├── rx-plugin/              # Plugin SDK crate
│   ├── Cargo.toml
│   └── src/
└── tests/                  # Integration tests
    └── fixtures/
```

## Key Design Decisions

### Why Rust?

1. **Performance**: Zero-cost abstractions, no GC pauses
2. **Safety**: Memory safety without runtime overhead
3. **Ecosystem**: Excellent crates for parsing, networking, compression
4. **Distribution**: Single binary, no runtime dependencies

### Why pubgrub for Resolution?

1. **Proven Algorithm**: Used by Dart/Pub, well-understood semantics
2. **Quality Errors**: Explains why resolution failed
3. **Rust Implementation**: `pubgrub-rs` is mature and maintained

### Why Wasm for Plugins?

1. **Sandboxing**: Capability-based security model
2. **Performance**: Near-native execution speed
3. **Language Agnostic**: Python, Rust, Go, etc. can all compile to Wasm
4. **Portability**: Same plugin works on all platforms

### Why Native VEnv Creation?

1. **Speed**: No Python interpreter spawn overhead
2. **Reliability**: No dependency on `venv` module availability
3. **Control**: Can optimize structure for our use case

## Performance Targets

| Operation | Target | Approach |
|-----------|--------|----------|
| Cold install (cached) | <100ms | Parallel downloads, zero-copy unpacking |
| Dependency resolution | <500ms | pubgrub with caching |
| Wheel build (pure Python) | <50ms | Native Rust, no Python spawn |
| Plugin hook execution | <5ms overhead | Wasm with AOT compilation |

## Security Model

### Plugin Sandboxing

Plugins run in a Wasm sandbox with explicit permissions:

```toml
[tool.rx.plugins.my-plugin]
path = "plugin.wasm"
permissions = ["read:pyproject.toml", "network:pypi.org"]
```

### Credential Management

- Keyring integration for secure storage
- Environment variable fallback
- No plaintext credentials in config files
