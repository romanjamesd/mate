# GitHub Actions CI Workflows

This directory contains GitHub Actions workflows for continuous integration and testing.

## Workflows

### 1. `ci.yml` - Comprehensive CI Pipeline

A full-featured CI pipeline that runs different test categories separately:

- **Quality Checks**: Code formatting, clippy lints, documentation
- **Unit Tests**: Separated by component (crypto, messages, network, display) 
- **Integration Tests**: End-to-end system tests
- **Security Tests**: DoS protection and error handling
- **Performance Tests**: Benchmark and throughput tests
- **Cross-platform Tests**: Linux, Windows, and macOS
- **Code Coverage**: Generates coverage reports

### 2. `test.yml` - Simple Test Runner

A lightweight workflow for quick validation:

- Runs all tests together
- Code formatting check
- Clippy linting
- Fast feedback for pull requests

## Test Categories

Based on your project structure in `tests/`, the workflows run:

```bash
# Unit tests by component
cargo test --test mod unit::crypto::         # Crypto operations
cargo test --test mod unit::messages::      # Message handling  
cargo test --test mod unit::network::       # Network operations
cargo test --test mod unit::display::       # Display utilities

# Integration tests
cargo test --test mod integration::         # End-to-end tests

# Security tests  
cargo test --test mod security::dos_protection::    # DoS protection
cargo test --test mod security::error_handling::    # Error handling

# Performance tests
cargo test --test mod performance::         # Performance benchmarks
```

## Triggering Workflows

Workflows automatically run on:
- Pushes to `main` and `develop` branches
- Pull requests to `main` and `develop` branches

## Local Testing

Before pushing, ensure your code passes locally:

```bash
# Run all tests
cargo test

# Check formatting
cargo fmt --all -- --check

# Run clippy
cargo clippy -- -D warnings

# Check documentation builds
cargo doc --no-deps --document-private-items --all-features
```

## Customization

### Changing Test Categories

To modify which tests run, edit the `cargo test` commands in the workflow files.

### Adjusting Timeout Values

Some tests have timeout limits:
- Integration tests: 10 minutes
- Performance tests: 15 minutes

### Adding New Test Types

1. Add test commands to the appropriate workflow file
2. Consider whether they should run in parallel or sequentially
3. Set appropriate timeouts for long-running tests

### Coverage Reports

The `ci.yml` workflow generates code coverage using `cargo-tarpaulin` and uploads to Codecov. To use this:

1. Sign up for [Codecov](https://codecov.io) 
2. Add your repository
3. The workflow will automatically upload coverage reports

## Workflow Features

### Caching
- Rust dependencies are cached using `Swatinem/rust-cache@v2`
- Speeds up subsequent builds significantly

### Matrix Testing
- Tests against multiple Rust versions (stable, beta)
- Cross-platform testing (Linux, Windows, macOS)

### Dependency Management
- Workflows use specific action versions for reproducibility
- Rust toolchain is pinned to stable versions

### Security
- Security audit runs with `cargo audit`
- DoS protection tests verify security measures

## Troubleshooting

### Test Failures
- Check the specific job that failed in GitHub Actions
- Run the same command locally to reproduce
- Look at the test output for specific error messages

### Timeout Issues
- Performance tests may timeout on slow runners
- Consider increasing timeout values or optimizing tests

### Clippy Failures
- Fix clippy warnings locally first
- Use `cargo clippy -- -D warnings` to catch all issues
- Some clippy rules can be disabled if needed with `#[allow()]`

### Formatting Issues
- Run `cargo fmt --all` to fix formatting
- Check `.rustfmt.toml` for custom formatting rules 