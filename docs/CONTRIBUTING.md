# Contributing to T-Rex

Thank you for your interest in contributing to T-Rex! This document provides guidelines and information for contributors.

## Getting Started

### Prerequisites

- **Rust**: Latest stable version (install via [rustup](https://rustup.rs/))
- **Python**: 3.8+ (for testing)
- **Git**: For version control

### Setting Up the Development Environment

```bash
# Clone the repository
git clone https://github.com/stherrien/t-rex.git
cd t-rex

# Build the project
cargo build

# Run tests
cargo test

# Run the CLI
cargo run -- --help
```

## Development Workflow

### Branch Naming

Use descriptive branch names with a prefix:

- `feat/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation changes
- `refactor/` - Code refactoring
- `test/` - Test additions/changes

Example: `feat/add-workspace-support`

### Commit Messages

Follow conventional commit format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `refactor`: Code refactoring
- `test`: Test changes
- `chore`: Build/tooling changes

Example:
```
feat(resolver): add universal lockfile support

Implements cross-platform dependency locking as specified in REQ-CORE-001.

Closes #42
```

### Pull Requests

1. Create a branch from `main`
2. Make your changes
3. Ensure all tests pass: `cargo test`
4. Ensure code is formatted: `cargo fmt`
5. Ensure no clippy warnings: `cargo clippy`
6. Open a PR with a clear description

## Code Style

### Rust Guidelines

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` with default settings
- Address all `clippy` warnings
- Write documentation for public APIs

### Error Handling

- Use `thiserror` for library errors
- Use `anyhow` in the CLI binary
- Provide actionable error messages

```rust
// Good
Err(ResolverError::ConflictingVersions {
    package: "requests".into(),
    required: ">=2.0".into(),
    found: "1.5.0".into(),
    hint: "Try updating your constraints in pyproject.toml".into(),
})

// Bad
Err("version conflict".into())
```

### Testing

- Unit tests go in the same file as the code
- Integration tests go in `tests/`
- Use `proptest` for property-based testing where appropriate

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }
}
```

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for an overview of the codebase structure.

### Key Modules

| Module | Purpose |
|--------|---------|
| `rx-cli` | Command-line interface |
| `rx-core` | Core library (resolver, builder, etc.) |
| `rx-plugin` | Plugin SDK and runtime |

## Adding New Commands

1. Create a new module in `rx-cli/src/commands/`
2. Implement the command struct with `clap` derive
3. Register in `rx-cli/src/commands/mod.rs`

```rust
// rx-cli/src/commands/my_command.rs
use clap::Args;

#[derive(Args)]
pub struct MyCommand {
    #[arg(short, long)]
    verbose: bool,
}

impl MyCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        // Implementation
        Ok(())
    }
}
```

## Adding New PEP Support

When implementing a new PEP:

1. Create a module under `rx-core/src/pep/`
2. Add comprehensive tests with real-world examples
3. Document any deviations or edge cases
4. Update the compatibility matrix in docs

## Release Process

1. Update `CHANGELOG.md`
2. Bump version in `Cargo.toml`
3. Create a git tag: `git tag v0.1.0`
4. Push tag: `git push origin v0.1.0`
5. CI will build and publish binaries

## Getting Help

- **Issues**: Use GitHub Issues for bugs and feature requests
- **Discussions**: Use GitHub Discussions for questions
- **Discord**: Join our community (link TBD)

## Code of Conduct

We follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Be respectful and constructive.

## License

By contributing, you agree that your contributions will be dual-licensed under MIT and Apache 2.0.
