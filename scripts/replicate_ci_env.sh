#!/bin/bash
# Comprehensive CI Environment Replication Script
# This script attempts to replicate the exact CI environment constraints locally

set -e

echo "=== CI Environment Replication Script ==="
echo "This script will:"
echo "1. Set all CI environment variables"
echo "2. Apply resource constraints similar to CI"
echo "3. Configure network settings"
echo "4. Set up Linux-compatible file system behavior"
echo "5. Run the failing tests in CI-like conditions"
echo ""

# CI Environment Variables (exact match from GitHub Actions)
export CI=true
export GITHUB_ACTIONS=true
export RUNNER_OS=Linux
export RUNNER_ARCH=X64
export TEST_TIMEOUT_MULTIPLIER=8.0
export RUST_LOG=info
export CARGO_TERM_COLOR=always
export RUST_BACKTRACE=1

# Additional CI constraints
export CARGO_INCREMENTAL=0
export RUST_TEST_THREADS=2  # Limit parallel tests like CI
export CARGO_NET_RETRY=5
export CARGO_NET_TIMEOUT=30

echo "✅ CI environment variables set"

# Function to check if we're on Linux (closer to CI)
check_platform() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "✅ Running on Linux (matches CI)"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "⚠️  Running on macOS (different from CI's Linux)"
        echo "   Some differences may still exist due to platform differences"
    else
        echo "⚠️  Running on $OSTYPE (different from CI's Linux)"
    fi
}

# Function to apply resource constraints similar to CI
apply_resource_constraints() {
    echo "=== Applying Resource Constraints ==="
    
    # Limit open file descriptors (CI has constraints)
    ulimit -n 1024 2>/dev/null || echo "⚠️  Could not set file descriptor limit"
    
    # Limit number of processes (CI has constraints)
    ulimit -u 1024 2>/dev/null || echo "⚠️  Could not set process limit"
    
    # Set virtual memory limit (CI has memory constraints)
    ulimit -v 4194304 2>/dev/null || echo "⚠️  Could not set virtual memory limit"  # 4GB
    
    echo "   Open files limit: $(ulimit -n)"
    echo "   Process limit: $(ulimit -u)"
    echo "   Virtual memory limit: $(ulimit -v)"
}

# Function to configure network settings similar to CI
configure_network() {
    echo "=== Configuring Network Settings ==="
    
    # Test network connectivity (CI may have different DNS)
    echo "   Testing DNS resolution..."
    nslookup google.com >/dev/null 2>&1 && echo "   ✅ DNS resolution working" || echo "   ⚠️  DNS resolution issues"
    
    # Test localhost connectivity
    echo "   Testing localhost connectivity..."
    ping -c 1 localhost >/dev/null 2>&1 && echo "   ✅ Localhost reachable" || echo "   ⚠️  Localhost connectivity issues"
    
    # Show network configuration
    echo "   Network interfaces:"
    if command -v ip &>/dev/null; then
        ip addr show | grep -E "^[0-9]+:|inet " | head -10
    elif command -v ifconfig &>/dev/null; then
        ifconfig | grep -E "^[a-z0-9]+:|inet " | head -10
    fi
}

# Function to set up CI-like file system behavior
setup_filesystem() {
    echo "=== Setting Up File System ==="
    
    # Create a temporary directory that mimics CI constraints
    export TMPDIR="/tmp/mate-ci-test-$$"
    mkdir -p "$TMPDIR"
    chmod 755 "$TMPDIR"
    
    echo "   Using temporary directory: $TMPDIR"
    echo "   Directory permissions: $(ls -ld "$TMPDIR")"
    
    # Clean up function
    cleanup() {
        echo "   Cleaning up temporary directory: $TMPDIR"
        rm -rf "$TMPDIR" 2>/dev/null || true
    }
    trap cleanup EXIT
}

# Function to run tests with Docker (most accurate CI replication)
run_tests_with_docker() {
    echo "=== Running Tests with Docker (Most Accurate CI Replication) ==="
    
    if command -v docker &>/dev/null; then
        echo "   Docker available - running tests in ubuntu:latest container"
        
        # Create a temporary Dockerfile for testing
        cat > Dockerfile.ci-test << 'EOF'
FROM ubuntu:latest

# Install Rust and dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    sqlite3 \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Set CI environment
ENV CI=true
ENV GITHUB_ACTIONS=true
ENV RUNNER_OS=Linux
ENV RUNNER_ARCH=X64
ENV TEST_TIMEOUT_MULTIPLIER=8.0
ENV RUST_LOG=info
ENV CARGO_TERM_COLOR=always
ENV RUST_BACKTRACE=1
ENV CARGO_INCREMENTAL=0
ENV RUST_TEST_THREADS=2

WORKDIR /workspace
COPY . .

# Build the project
RUN cargo build --bin mate --verbose

# Run the specific failing tests
CMD ["cargo", "test", "--verbose", "test_comprehensive_cli_lifecycle", "test_error_handling_edge_cases", "test_error_handling_network_failures_user_feedback", "--", "--nocapture"]
EOF

        echo "   Building Docker image..."
        docker build -f Dockerfile.ci-test -t mate-ci-test . || {
            echo "   ⚠️  Docker build failed"
            rm -f Dockerfile.ci-test
            return 1
        }
        
        echo "   Running tests in Docker container..."
        docker run --rm mate-ci-test || {
            echo "   ⚠️  Docker tests failed - this may help identify CI-specific issues"
            rm -f Dockerfile.ci-test
            return 1
        }
        
        rm -f Dockerfile.ci-test
        echo "   ✅ Docker tests completed"
    else
        echo "   ⚠️  Docker not available - skipping Docker-based CI replication"
        return 1
    fi
}

# Function to run tests with resource constraints
run_tests_with_constraints() {
    echo "=== Running Tests with Resource Constraints ==="
    
    # Build the binary first
    echo "   Building mate binary..."
    cargo build --bin mate --verbose || {
        echo "   ❌ Build failed"
        return 1
    }
    
    # Verify binary exists
    if [[ -f "target/debug/mate" ]]; then
        echo "   ✅ Binary exists: target/debug/mate"
        ls -la target/debug/mate
    else
        echo "   ❌ Binary not found"
        return 1
    fi
    
    # Run the debug CI environment script
    echo "   Running CI environment debug script..."
    ./scripts/debug_ci_env.sh || echo "   ⚠️  Debug script failed"
    
    # Run the failing tests individually with constraints
    local failing_tests=(
        "test_comprehensive_cli_lifecycle"
        "test_error_handling_edge_cases"
        "test_error_handling_network_failures_user_feedback"
    )
    
    echo "   Running failing tests individually..."
    for test in "${failing_tests[@]}"; do
        echo "   Testing: $test"
        
        # Run with single thread to avoid race conditions
        timeout 300 cargo test "$test" --verbose -- --nocapture --test-threads=1 || {
            echo "   ❌ $test failed"
            echo "   This failure may help identify the CI-specific issue"
        }
        
        echo "   Waiting 5 seconds between tests..."
        sleep 5
    done
    
    echo "   Running all failing tests together (parallel)..."
    timeout 600 cargo test \
        test_comprehensive_cli_lifecycle \
        test_error_handling_edge_cases \
        test_error_handling_network_failures_user_feedback \
        --verbose -- --nocapture || {
        echo "   ❌ Parallel test execution failed"
        echo "   This may indicate race conditions or resource conflicts"
    }
}

# Function to gather CI debugging information
gather_ci_debug_info() {
    echo "=== Gathering CI Debug Information ==="
    
    # System information
    echo "   System: $(uname -a)"
    echo "   CPU cores: $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 'unknown')"
    echo "   Memory: $(free -h 2>/dev/null || echo 'not available')"
    echo "   Disk space: $(df -h . | tail -1)"
    
    # Process information
    echo "   Current processes:"
    ps aux | head -10
    
    # Network information
    echo "   Network interfaces:"
    ip addr show 2>/dev/null || ifconfig 2>/dev/null || echo "Network info not available"
    
    # Port availability
    echo "   Testing common ports:"
    for port in 8080 18000 18001 50000 50001; do
        if command -v nc &>/dev/null; then
            nc -z localhost $port 2>/dev/null && echo "   Port $port: OCCUPIED" || echo "   Port $port: available"
        fi
    done
    
    # Environment variables
    echo "   Key environment variables:"
    env | grep -E "(CI|GITHUB|RUST|CARGO|TEST)" | sort
}

# Main execution
main() {
    echo "Starting CI environment replication..."
    
    check_platform
    apply_resource_constraints
    configure_network
    setup_filesystem
    gather_ci_debug_info
    
    echo ""
    echo "=== Attempting CI Environment Replication ==="
    
    # Try Docker first (most accurate)
    if run_tests_with_docker; then
        echo "✅ Docker-based CI replication completed successfully"
    else
        echo "⚠️  Docker-based replication failed, trying local constraints..."
        run_tests_with_constraints
    fi
    
    echo ""
    echo "=== CI Environment Replication Complete ==="
    echo "If tests still pass locally but fail in CI, the issue may be:"
    echo "1. Stricter resource limits in CI"
    echo "2. Different Linux kernel behavior"
    echo "3. Network configuration differences"
    echo "4. Timing-sensitive race conditions"
    echo "5. CI-specific environment variables or constraints"
    echo ""
    echo "Consider:"
    echo "- Increasing timeouts for CI"
    echo "- Adding more robust retry logic"
    echo "- Using single-threaded test execution"
    echo "- Adding CI-specific environment detection"
}

# Run the main function
main "$@" 