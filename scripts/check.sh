#!/bin/bash
set -e

echo "Running clippy with CI settings..."
cargo clippy --all-targets --all-features -- -D warnings

echo "Running format check..."
cargo fmt --all -- --check

echo "All checks passed!" 