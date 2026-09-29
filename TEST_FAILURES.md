# `make test-ci-safe` failures

Run on 2026-09-28.

```
CI=true GITHUB_ACTIONS=true TEST_TIMEOUT_MULTIPLIER=8.0 RUST_LOG=error cargo test --jobs 1
```

`make` exited 2. Cargo reported `error: test failed` (exit 101). Compile succeeded (`Finished test profile` in 21.15s).

| Target | Result |
| --- | --- |
| `src/lib.rs` unit tests | 30 passed |
| `src/main.rs` unit tests | 0 tests |
| `tests/integration/chess_message_wire.rs` | 4 passed |
| `tests/game_accept.rs` | 6 passed |
| `tests/mod.rs` | **764 passed, 7 failed** (63.20s) |

All seven failures are in `tests/mod.rs`.

## One-shot `connect --message` prints nothing

Five tests spawn `mate connect <addr> --message <text>` against a local test server. The process exits 0, and both stdout and stderr are empty, so every assertion that looks for an echo or a round-trip time fails.

| Test | Panic location | Assertion |
| --- | --- | --- |
| `integration::cli_tests::test_successful_message_send_with_timing` | `tests/integration/cli_tests.rs:88` | combined output contains `round-trip` |
| `integration::cli_tests::test_message_content_echo_correctness` | `tests/integration/cli_tests.rs:163` | combined output contains the sent text (`Simple message` is the first case; later cases never run) |
| `integration::cli_tests::test_response_timing_measurement` | `tests/integration/cli_tests.rs:240` | combined output matches `round-trip: <n>μs`, `<n>ms`, or `<n.n>s` |
| `integration::cli_tests::test_program_exits_after_single_exchange` | `tests/integration/cli_tests.rs:432` | combined output contains `Received echo` or `round-trip` |
| `integration::cli_tests::test_one_shot_mode_comprehensive` | `tests/integration/cli_tests.rs:589` | combined output contains the sent text and `round-trip` (`First one-shot message` is the first case) |

Logged symptom for the two tests that print a warning before asserting:

```
WARNING: No output received for message '...'
Exit status: exit status: 0
```

The panic message is `combined:` with nothing after it.

## Multiple reconnection failures are not reported

`integration::connection_recovery::test_multiple_reconnection_failures_handled` (`tests/integration/connection_recovery.rs:557`).

The client stays up, echoes every message, and exits cleanly. The session banner contains the word `connection` once (`Connection status: Active`). The test counts occurrences of `error`, `failed`, and `connection` and requires that sum to be at least 2. Observed count: **1**.

Session output (abridged):

```
=== MATE Chat Session ===
Connected to peer: f0z/pSE5K+ePPTsYixqOhLmnz+xMba94WmgkpslEPFU=
Connection status: Active
...
mate> ← Received echo: "Initial message" (round-trip: 3ms)
mate> ← Received echo: "Message during first failure" (round-trip: 3ms)
mate> ← Received echo: "Message during second failure" (round-trip: 5ms)
mate> ← Received echo: "Final recovery message" (round-trip: 3ms)
=== Session Summary ===
Messages sent: 4
```

No `error` or `failed` string appears, even though the test aborts and restarts the server twice while the client is sending.

## Interactive connect omits the server address

`integration::interactive_initialization::test_connection_information_display` (`tests/integration/interactive_initialization.rs:90`).

The session banner shows the peer id and `Connection status: Active`. It does not contain the dial address (`127.0.0.1:18091` in this test) or the substring `127.0.0.1`. The earlier assertion (`Connected` or `connection`) passes; the address assertion fails.


## Root causes and patches (2026-09-29)

The same seven failures reproduced with loopback networking allowed. The initial
sandboxed run failed earlier because binding local TCP listeners was denied;
that was an execution restriction, not an application defect.

- All five one-shot failures shared one cause: `connect --message` reported its
  successful echo and round-trip time with `info!`, which `RUST_LOG=error` filters
  out. The result now uses `println!`, and the one-shot regression tests explicitly
  run with error-only logging.
- The interactive address failure came from displaying only the authenticated
  peer ID. Both the session banner and `info` command now show the dial address.
- The multiple reconnection failure came from detached server tasks. Dropping
  ordinary Tokio `JoinHandle`s does not cancel their tasks. Aborting `Server::run`
  left its connection handlers and signal listener running, so the original
  connection kept echoing messages. Connections now belong to a `JoinSet`, which
  aborts them when the server future is dropped; signal handling stays in that
  future. Graceful shutdown still broadcasts to handlers and waits up to 30 seconds.
  The regression test now waits for child output and requires two disconnections,
  two reconnections, and successful echoes after each restart.

With real disconnections, `test_reconnection_behavior_when_receiving_fails`
exposed a further defect: the receive-error branch reconnected but never retried
its pending ping. Send and receive errors now share the existing reconnect and
single-retry path, preserving echo messages and session statistics.

The Makefile comment now correctly describes `--jobs 1` as limiting build jobs;
it does not serialize test execution.

Validation after the patches: `make test-ci-safe` exited 0 with 811 unit and
integration tests plus 33 documentation tests passing (844 total, no failures).
`cargo fmt --all -- --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, and
`git diff --check` also passed.
