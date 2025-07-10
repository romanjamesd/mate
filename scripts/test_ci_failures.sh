#!/bin/bash
# Quick Test Script for CI-Failing Tests
# This script runs the specific failing tests with CI environment simulation

set -e

echo "=== Testing CI-Failing Tests ==="
echo "Running the 3 tests that fail in CI but pass locally"
echo ""

# Set CI environment variables
export CI=true
export GITHUB_ACTIONS=true
export TEST_TIMEOUT_MULTIPLIER=8.0
export RUST_LOG=debug
export RUST_TEST_THREADS=1  # Single-threaded to avoid race conditions

# Build the binary
echo "Building mate binary..."
cargo build --bin mate --verbose

# Verify binary exists
if [[ ! -f "target/debug/mate" ]]; then
    echo "❌ Binary not found! Build failed."
    exit 1
fi

echo "✅ Binary built successfully"
echo ""

# Array of failing tests
failing_tests=(
    "test_comprehensive_cli_lifecycle"
    "test_error_handling_edge_cases"
    "test_error_handling_network_failures_user_feedback"
)

# Run each test individually first
echo "=== Running Tests Individually ==="
for test in "${failing_tests[@]}"; do
    echo "Testing: $test"
    
    # Run with timeout and capture output
    if timeout 300 cargo test "$test" --verbose -- --nocapture --test-threads=1; then
        echo "✅ $test PASSED"
    else
        echo "❌ $test FAILED"
        echo "   This test fails in CI but passed locally - this is the issue!"
    fi
    
    echo ""
done

echo "=== Running All Tests Together (Parallel) ==="
if timeout 600 cargo test --verbose -- --nocapture \
    test_comprehensive_cli_lifecycle \
    test_error_handling_edge_cases \
    test_error_handling_network_failures_user_feedback; then
    echo "✅ All tests PASSED in parallel"
else
    echo "❌ Tests FAILED in parallel"
    echo "   This indicates race conditions or resource conflicts"
fi

echo ""
echo "=== Summary ==="
echo "If all tests passed above, but they fail in CI, the issue is likely:"
echo "1. Linux vs macOS platform differences"
echo "2. Stricter resource limits in CI"
echo "3. Different network configuration"
echo "4. Container-based file system differences"
echo ""
echo "Next steps:"
echo "1. Run: ./scripts/replicate_ci_env.sh (for Docker-based CI replication)"
echo "2. Review: CI_DEBUGGING_GUIDE.md"
echo "3. Add debug logging to the failing tests"
echo "4. Consider increasing CI timeouts" 