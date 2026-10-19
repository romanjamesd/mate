# Tests Directory

This directory contains all tests for the mate messaging system, organized into a logical hierarchical structure for maintainability and clarity.

## 📁 Directory Structure

```
mate/tests/
├── README.md               # This file - documentation for the test structure
├── mod.rs                  # Main test organization module
├── common/                 # Shared test utilities and helpers
│   ├── mod.rs             # Common module exports
│   └── ...                # Test utilities, mock objects, helper functions
├── unit/                   # Unit tests for individual components
│   ├── mod.rs             # Unit test module organization
│   ├── messages/          # Message-related unit tests
│   │   └── wire/          # Wire protocol specific tests
│   │       ├── mod.rs
│   │       ├── partial_io.rs        # Partial I/O handling tests
│   │       ├── length_prefix.rs     # Length prefix format tests
│   │       └── message_roundtrip.rs # Basic message transmission tests
│   └── network/           # Network-related unit tests
│       ├── mod.rs
│       ├── timeouts.rs    # Timeout handling tests
│       └── interruptions.rs # Network interruption recovery tests
├── integration/            # Integration tests for full system behavior
│   ├── mod.rs
│   └── ...                # End-to-end system tests
├── security/              # Security-focused tests
│   ├── mod.rs
│   ├── dos_protection.rs  # DoS protection and rate limiting tests
│   └── error_handling.rs  # Error handling and protocol violation tests
└── performance/           # Performance and resource usage tests
    ├── mod.rs
    ├── throughput.rs      # Message throughput and performance tests
    └── ...                # Memory usage, resource limits, etc.
```

## 🧪 Test Categories

### Unit Tests (`unit/`)
Tests for individual components in isolation:
- **Wire Protocol Tests**: Length prefix handling, partial I/O recovery, message round-trips
- **Network Tests**: Timeout enforcement, connection interruption handling
- **Message Tests**: Envelope creation, serialization, validation

### Integration Tests (`integration/`)
Tests for complete system behavior:
- End-to-end message flows
- Cross-component interactions
- System-level functionality

### Security Tests (`security/`)
Security-focused test scenarios:
- **DoS Protection**: Message size limits, rate limiting, resource protection
- **Error Handling**: Protocol violation detection, corrupted data handling, graceful failures

### Performance Tests (`performance/`)
Performance and resource usage validation:
- Message throughput benchmarks
- Memory usage efficiency
- Resource limit enforcement

### Common Utilities (`common/`)
Shared test infrastructure:
- Mock stream implementations
- Test data generators
- Custom assertion helpers
- Reusable test fixtures

## 🚀 Running Tests

### Run All Tests
```bash
cargo test
```

### Run Specific Test Categories

#### Unit Tests Only
```bash
cargo test --test mod unit::
```

#### Security Tests Only
```bash
cargo test --test mod security::
```

#### Performance Tests Only
```bash
cargo test --test mod performance::
```

#### Network-Related Tests
```bash
cargo test --test mod unit::network::
```

#### Wire Protocol Tests
```bash
cargo test --test mod unit::messages::wire::
```

### Run Individual Test Files
```bash
# DoS protection tests
cargo test --test mod security::dos_protection::

# Timeout handling tests
cargo test --test mod unit::network::timeouts::

# Message round-trip tests
cargo test --test mod unit::messages::wire::message_roundtrip::
```

### Run Specific Tests
```bash
# Run a specific test function
cargo test test_message_size_limit_enforcement

# Run tests matching a pattern
cargo test timeout
```

### Verbose Output
```bash
# See detailed test output
cargo test -- --nocapture

# Show test names as they run
cargo test -- --test-threads=1
```

## 📊 Test Coverage

### Authenticated signer binding regressions

Raw-socket peers complete a real handshake as A, then send a fresh, valid
envelope signed by B. Ordinary `Connection::send_message` uses its fixed local
identity, so it cannot construct this case. Socket operations are bounded by
timeouts; a timeout is a failure, not evidence of rejection.

| Boundary | Assertions | Location |
| --- | --- | --- |
| Connection receive | Both handshake directions accept A, reject B with expected/actual identities, and retain A's binding; signature and timestamp errors take precedence | [connection_core.rs](integration/connection_core.rs) |
| Live server | New Moves and historical retries in Active/Completed games receive no application reply and leave complete game/message snapshots unchanged; legitimate A reconnections still receive correlated ACKs; stored-opponent authorization remains separate | [server_chess_handlers.rs](integration/server_chess_handlers.rs) |
| Outbound Move | A mismatched ACK preserves signer diagnostics with one transmission, no local commit, and no pending entry | [game_move.rs](game_move.rs) |
| Other receive callers | Invite/accept preserve pending state without queueing; Client echo/one-shot and interactive CLI terminate without resend or reconnect | [signer_mismatch.rs](signer_mismatch.rs) |
| Retry and CLI errors | Decline mismatches are terminal and pending declines are not requeued; typed errors survive context wrappers; user-facing text hides peer IDs and preserves delivery uncertainty | [network_manager.rs](../src/cli/network_manager.rs), [error_handler.rs](../src/cli/error_handler.rs) |

Run the focused suites from the repository root:

```sh
cargo test --test mod integration::connection_core::
cargo test --test mod integration::server_chess_handlers::
cargo test --test game_move
cargo test --test signer_mismatch
cargo test --lib cli::network_manager::tests::
cargo test --lib cli::error_handler::tests::
```

`--test mod` selects the target containing the integration submodules;
`game_move` and `signer_mismatch` are standalone targets. Network tests need
localhost socket access. Bind permission errors are infrastructure failures.
These tests cover signer binding and terminal mismatch handling, not broader
handshake replay protection, full chess legality, stored signature verification,
or automatic recovery.

The test suite covers these essential areas from our test specification:

| Test Area | Coverage | Location |
|-----------|----------|----------|
| Message Round-trip | ✅ | `unit/messages/wire/message_roundtrip.rs` |
| Message Ordering | ✅ | `unit/messages/wire/message_roundtrip.rs` |
| Empty/Minimal Messages | ✅ | `unit/messages/wire/message_roundtrip.rs` |
| Length Prefix Format | ✅ | `unit/messages/wire/length_prefix.rs` |
| Length Prefix Accuracy | ✅ | `unit/messages/wire/length_prefix.rs` |
| Partial I/O Recovery | ✅ | `unit/messages/wire/partial_io.rs` |
| Interrupted Operations | ✅ | `unit/messages/wire/partial_io.rs` |
| DoS Protection | ✅ | `security/dos_protection.rs` |
| Large Message Handling | ✅ | `security/dos_protection.rs` |
| Error Handling | ✅ | `security/error_handling.rs` |
| Protocol Violations | ✅ | `security/error_handling.rs` |
| Timeout Enforcement | ✅ | `unit/network/timeouts.rs` |
| Network Interruptions | ✅ | `unit/network/interruptions.rs` |

## ✨ Benefits of This Structure

### 🎯 **Organized & Logical**
- Tests are grouped by functionality and scope
- Easy to find relevant tests for specific features
- Clear separation between unit, integration, and specialized tests

### 📁 **Manageable File Sizes**
- No single test file exceeds ~500 lines
- Eliminated massive 2000+ line files
- Each file has a focused responsibility

### 🔄 **Reusable Components**
- Common test utilities in `common/` directory
- Shared mock objects and test helpers
- Reduced code duplication across tests

### 🧭 **Easy Navigation**
- IDE-friendly structure with clear module hierarchy
- Intuitive file and directory naming
- Self-documenting organization

### 📈 **Scalable**
- Easy to add new test categories
- Clear patterns for organizing new tests
- Modular structure supports growth

## 🔧 Adding New Tests

### For New Unit Tests
1. Choose the appropriate subdirectory under `unit/`
2. Add your test to an existing file or create a new `.rs` file
3. Update the corresponding `mod.rs` file to include your new module
4. Follow the existing naming conventions (`test_*` functions)

### For New Integration Tests
1. Add files to the `integration/` directory
2. Update `integration/mod.rs` to include new modules
3. Focus on end-to-end scenarios and cross-component interactions

### For New Security Tests
1. Add to `security/` directory based on the security concern
2. Follow the pattern of existing security tests
3. Include both positive and negative test cases

### For New Performance Tests
1. Add to `performance/` directory
2. Use appropriate benchmarking techniques
3. Include clear performance expectations and metrics

## 📋 Test Guidelines

### ✅ **Best Practices**
- Use descriptive test names that explain what is being tested
- Include comprehensive test documentation
- Test both success and failure scenarios
- Use appropriate assertion messages
- Keep tests focused and isolated

### 🏗️ **Test Structure**
```rust
#[tokio::test]
async fn test_descriptive_name() {
    println!("Testing [specific functionality] - [test description]");
    
    // Arrange: Set up test data and conditions
    
    // Act: Execute the code being tested
    
    // Assert: Verify the results
    
    println!("✓ [Test completion message]");
}
```

### 🔍 **Debugging Tests**
- Use `println!` statements for test progress tracking
- Include detailed error messages in assertions
- Use `--nocapture` flag to see output during test runs
- Consider using `cargo test -- --test-threads=1` for sequential execution

## 📈 Performance Impact

This reorganization achieved:
- **94.4% reduction** in test file sizes
- **Eliminated ~286KB** of redundant test code
- **Improved compilation times** through better module organization
- **Enhanced maintainability** through logical grouping

## 🤝 Contributing

When adding new tests:
1. Follow the established directory structure
2. Use the existing patterns and conventions
3. Update this README if adding new categories
4. Ensure all tests pass before committing
5. Include appropriate documentation for complex test scenarios

---

For questions about the test structure or adding new tests, refer to the `organize-tests.md` document in the project root for detailed refactoring context.

# Testing Guidelines

## Test Structure
- Unit tests go in `tests/unit/`
- Integration tests go in `tests/integration/`  
- Performance tests go in `tests/performance/`
- Security tests go in `tests/security/`

## Performance Testing Best Practices

### CI Environment Considerations
Performance tests should be CI-aware to avoid flaky failures:

```rust
// ✅ Good: CI-aware performance test
fn get_performance_multiplier() -> u32 {
    if is_ci_environment() { 10 } else { 1 }
}
let threshold = base_threshold * get_performance_multiplier();
```

```rust
// ❌ Bad: Hardcoded thresholds
assert!(duration < Duration::from_millis(100)); // Will fail in CI
```

### Alternative Approaches
1. **Relative Performance**: Compare against baseline rather than absolute time
2. **Statistical Analysis**: Use percentiles and variance over multiple runs
3. **Conditional Testing**: Skip performance tests in CI entirely with `#[cfg(not(ci))]`
4. **Benchmark Frameworks**: Use `criterion` crate for proper benchmarking

### Environment Detection
The tests detect CI environments via these environment variables:
- `CI` (generic)
- `GITHUB_ACTIONS` 
- `TRAVIS`
- `CIRCLECI`
- `JENKINS_URL` 