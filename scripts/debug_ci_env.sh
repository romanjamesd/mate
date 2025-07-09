#!/bin/bash
# CI Environment Debug Script
# Provides information about the CI environment to help debug test failures

echo "=== CI Environment Debug Information ==="
echo "Date: $(date)"
echo "Hostname: $(hostname)"
echo "User: $(whoami)"
echo "Working Directory: $(pwd)"
echo ""

echo "=== Environment Variables ==="
echo "CI: ${CI:-not set}"
echo "GITHUB_ACTIONS: ${GITHUB_ACTIONS:-not set}"
echo "RUNNER_OS: ${RUNNER_OS:-not set}"
echo "RUNNER_ARCH: ${RUNNER_ARCH:-not set}"
echo "TEST_TIMEOUT_MULTIPLIER: ${TEST_TIMEOUT_MULTIPLIER:-not set}"
echo "RUST_LOG: ${RUST_LOG:-not set}"
echo ""

echo "=== System Information ==="
echo "OS: $(uname -a)"
echo "Architecture: $(uname -m)"
echo ""

echo "=== CPU Information ==="
if command -v nproc &> /dev/null; then
    echo "CPU Cores: $(nproc)"
fi
if [ -f /proc/cpuinfo ]; then
    echo "CPU Model: $(grep 'model name' /proc/cpuinfo | head -1 | cut -d: -f2 | xargs)"
fi
echo ""

echo "=== Memory Information ==="
if command -v free &> /dev/null; then
    echo "Memory:"
    free -h
fi
echo ""

echo "=== Disk Space ==="
echo "Available disk space:"
df -h . 2>/dev/null || echo "df command failed"
echo ""

echo "=== Network Configuration ==="
echo "Network interfaces:"
if command -v ip &> /dev/null; then
    ip addr show | grep -E '^[0-9]+:|inet ' || echo "ip command failed"
elif command -v ifconfig &> /dev/null; then
    ifconfig | grep -E '^[a-z0-9]+:|inet ' || echo "ifconfig command failed"
else
    echo "No network info commands available"
fi
echo ""

echo "=== DNS Configuration ==="
if [ -f /etc/resolv.conf ]; then
    echo "DNS servers:"
    grep nameserver /etc/resolv.conf || echo "No nameservers found"
fi
echo ""

echo "=== Rust Environment ==="
echo "Rust version: $(rustc --version 2>/dev/null || echo 'not available')"
echo "Cargo version: $(cargo --version 2>/dev/null || echo 'not available')"
echo "Cargo target directory: ${CARGO_TARGET_DIR:-default}"
echo ""

echo "=== Process Limits ==="
if command -v ulimit &> /dev/null; then
    echo "Open files limit: $(ulimit -n)"
    echo "Process limit: $(ulimit -u)"
    echo "Virtual memory limit: $(ulimit -v)"
fi
echo ""

echo "=== Current Processes (top 10 by CPU) ==="
if command -v ps &> /dev/null; then
    ps aux --sort=-%cpu | head -11 2>/dev/null || echo "ps command failed"
fi
echo ""

echo "=== Mate Binary Information ==="
if [ -f "target/debug/mate" ]; then
    echo "Binary exists: target/debug/mate"
    ls -la target/debug/mate
    echo "File type: $(file target/debug/mate)"
    echo "Binary size: $(du -h target/debug/mate | cut -f1)"
    
    echo "Testing binary execution:"
    ./target/debug/mate --help >/dev/null 2>&1 && echo "Binary executes successfully" || echo "Binary execution failed"
else
    echo "Binary not found: target/debug/mate"
    echo "Target directory contents:"
    ls -la target/debug/ 2>/dev/null || echo "Target directory not found"
fi
echo ""

echo "=== Temporary Directory ==="
echo "TMPDIR: ${TMPDIR:-/tmp}"
echo "Temp directory permissions:"
ls -ld "${TMPDIR:-/tmp}" 2>/dev/null || echo "Cannot access temp directory"
echo ""

echo "=== Port Availability Test ==="
echo "Testing port availability for common test ports..."
for port in 8080 18000 18001 50000 50001; do
    if command -v nc &> /dev/null; then
        nc -z localhost $port 2>/dev/null && echo "Port $port: OCCUPIED" || echo "Port $port: available"
    elif command -v telnet &> /dev/null; then
        timeout 1 telnet localhost $port >/dev/null 2>&1 && echo "Port $port: OCCUPIED" || echo "Port $port: available"
    else
        echo "No port testing tools available"
        break
    fi
done
echo ""

echo "=== CI Environment Debug Complete ===" 