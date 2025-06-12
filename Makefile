.PHONY: check clippy fmt test ci

# Run the same checks as CI
ci: fmt clippy test

# Format check (matches CI)
fmt:
	cargo fmt --all -- --check

# Clippy check (matches CI)
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Run tests
test:
	cargo test

# Quick local check
check: fmt clippy
	@echo "All code quality checks passed!" 