.PHONY: check clippy fmt test test-ci ci

# Run the same checks as CI (with CI environment simulation)
ci: fmt clippy test-ci

# Format check (matches CI)
fmt:
	cargo fmt --all -- --check

# Clippy check (matches CI)
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Run tests with CI environment variables (simulates CI exactly)
test-ci:
	CI=true GITHUB_ACTIONS=true TEST_TIMEOUT_MULTIPLIER=8.0 RUST_LOG=debug cargo test

# Run tests (normal local development)
test:
	cargo test

# Quick local check
check: fmt clippy
	@echo "All code quality checks passed!" 