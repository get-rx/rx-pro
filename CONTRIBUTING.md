# Contributing to Pro

Thank you for your interest in contributing to Pro! This document provides guidelines and instructions for contributing.

## Getting Started

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Python 3.8+ (for testing)
- Git

### Building from Source

```bash
# Clone the repository
git clone https://github.com/stherrien/pro.git
cd pro

# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run tests
cargo test

# Run with debug output
RUST_LOG=debug cargo run -- sync
```

### Project Structure

```
pro/
├── rx-core/          # Core library (resolver, installer, builder)
├── rx-cli/           # Command-line interface
├── rx-plugin/        # WebAssembly plugin system
├── rx-python/        # Python bindings (PyO3)
├── docs/             # Documentation and website
└── .github/          # CI/CD workflows
```

## Development Workflow

### Branch Naming

Use descriptive branch names with prefixes:

- `feat/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation changes
- `refactor/` - Code refactoring
- `test/` - Test additions/changes

Example: `feat/add-pip-compile-export`

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add pip-compile export format
fix: handle unicode in package names
docs: update installation instructions
refactor: simplify resolver logic
test: add integration tests for workspace
```

### Pull Requests

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Run lints: `cargo clippy`
6. Format code: `cargo fmt`
7. Submit a pull request

## Code Guidelines

### Rust Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting
- Use `clippy` for linting
- Document public APIs with rustdoc comments

### Error Handling

- Use `thiserror` for error types
- Use `anyhow` for application errors
- Provide helpful error messages

### Testing

- Write unit tests for new functionality
- Use `#[tokio::test]` for async tests
- Use `proptest` for property-based testing
- Integration tests go in `tests/` directory

## Areas for Contribution

### Good First Issues

Look for issues labeled `good first issue` on GitHub.

### Feature Ideas

- Additional export formats
- More package manager migrations
- Performance optimizations
- Plugin SDK examples

### Documentation

- Improve existing docs
- Add examples
- Write tutorials
- Translate to other languages

## Code of Conduct

Be respectful and inclusive. We follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

## Questions?

- Open an issue for bugs or feature requests
- Start a discussion for questions
- Join our Discord (coming soon)

Thank you for contributing!
