# Contributing to Ando

Thank you for your interest in contributing to Ando! This document provides guidelines
for contributing to the Community Edition.

## Getting Started

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests: `cargo test --workspace`
5. Run clippy: `cargo clippy --workspace`
6. Commit with a descriptive message
7. Push to your fork and submit a Pull Request

## Development Setup

### Prerequisites
- Rust 1.88+ (edition 2024)
- macOS or Linux (io_uring/kqueue support)

### Building
```bash
cargo build --release
```

### Running Tests
```bash
cargo test --workspace
```

### Code Style
- Follow standard Rust formatting: `cargo fmt`
- Zero warnings policy: `cargo clippy --workspace -- -D warnings`
- Document public APIs with doc comments

## Architecture

See the [README](README.md) for architecture overview.

Key design principles:
- **Zero overhead on hot path**: No allocations, no locks, no async overhead for simple operations
- **Thread-per-core**: Each worker is completely independent
- **Plugin extensibility**: All traffic processing goes through the plugin pipeline
- **APISIX compatibility**: Admin API and data model compatible with APISIX

## Adding a Plugin

1. Create a new file in `ando-plugins/src/` (under the appropriate category: `auth/`, `traffic/`, `transform/`)
2. Implement the `Plugin` trait (factory) and `PluginInstance` trait (per-route instance)
3. Register in `ando-plugins/src/lib.rs` via `register_all()`
4. Add tests

See existing plugins (e.g., `key_auth.rs`) for reference.

## What Goes in CE vs EE

**Community Edition (this repo):**
- Core proxy engine improvements
- Basic plugin implementations
- Bug fixes and performance optimizations
- Documentation improvements
- Standalone mode features

**Enterprise Edition (separate repo):**
- Clustering and distributed features
- Advanced authentication (OAuth2, HMAC)
- Distributed rate limiting
- Management dashboard
- Enterprise observability integrations

## Pull Request Process

1. Ensure your PR targets the `main` branch
2. Update documentation if needed
3. Add tests for new functionality
4. Ensure CI passes (tests, clippy, fmt)
5. Request review from maintainers

## Code of Conduct

Be respectful, inclusive, and constructive. We follow the
[Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

## License

By contributing, you agree that your contributions will be licensed under
the Apache License 2.0.
