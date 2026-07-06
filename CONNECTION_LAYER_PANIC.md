# Fix Plan: Connection-layer panic on chess messages

Status: **planning only** — no code has been changed. This document is the
result of investigating `PRIORITIES.md` item 1 ("Fix the connection-layer
panic on chess messages") and lays out the concrete steps to fix it, plus the
verification to run at each step.

## 1. Root cause (confirmed by reading the code)

`Message::get_nonce()` and `Message::get_payload()`
(`src/messages/types.rs:205-219` and `:223-237`) are only meaningful for the
`Ping`/`Pong` variants. For every chess variant (`GameInvite`, `GameAccept`,
`GameDecline`, `Move`, `MoveAck`, `SyncRequest`, `SyncResponse`) they
`panic!()`.

Four call sites invoke these accessors **unconditionally** — i.e. without
first checking `is_ping()`/`is_pong()` — on a `Message` value that can
legitimately be any variant, because they sit in the generic send/receive
path used for every message type:

| # | File / lines | Function | What it does |
|---|---|---|---|
| 1 | `src/network/connection.rs:168-172` | `Connection::send_message` | `debug!` logs `msg.get_nonce()` / `msg.get_payload().len()` before sending *any* message |
| 2 | `src/network/connection.rs:297-301` | `Connection::receive_message` | `debug!` logs `message.get_nonce()` / `message.get_payload().len()` after receiving *any* message |
| 3 | `src/network/client.rs:555-560` | `Client::send_message_to` | `debug!` logs `message.get_nonce()` / `message.get_payload().len()` before sending |
| 4 | `src/network/client.rs:586-591` | `Client::send_message_to` | `info!` logs `response_message.get_nonce()` after receiving the response |

`Connection::send_message`/`receive_message` are the two methods every other
network path funnels through (`Client::send_message_to`, the server's
per-connection loop in `src/network/server.rs:323`, and the CLI's
`NetworkManager::send_message_with_strategy` in
`src/cli/network_manager.rs:225,228`). So today:

- **Client side**: `src/cli/network_manager.rs:225` calls
  `connection.send_message(message.clone())` for outgoing `GameInvite`/`Move`/etc.
  messages (this already runs today — `mate invite`, `mate accept`, and
  move-sending in the CLI build the chess `Message` and call this). That hits
  call site #1/#3 and panics **before a single byte reaches the socket**.
- **Server side**: `src/network/server.rs:323` calls
  `connection.receive_message()` in its main loop. That hits call site #2 and
  panics **before the server's own message-type `match` (line 330) ever runs**
  — so the server's existing "no specific handler" fallback for chess
  messages (relevant to PRIORITIES.md item 2) is never even reached today.

### Important nuance: this only fires when debug-level logging is enabled

`src/main.rs:85-96` configures the tracing subscriber with
`with_default_directive("mate=info".parse()?)`. The four call sites above are
all `debug!`/`info!` macro invocations — wait, call site #4 is `info!`, so it
**always** fires regardless of log level; call sites #1-#3 are `debug!` and
are gated by the subscriber's enabled level.

I verified experimentally (small standalone `tracing`/`tracing-subscriber`
0.1/0.3 program, matching this project's versions) that `tracing`'s macros
skip evaluating their argument expressions entirely when the callsite is
disabled — a panicking argument to a filtered-out `debug!()` does **not**
panic. So:

- Call site #4 (`info!` in `send_message_to`, line 586-591) panics **unconditionally**, on every chess message sent through `Client::send_message_to`, regardless of `RUST_LOG`.
- Call sites #1, #2, #3 (`debug!`) only panic when the `mate` target is logging at `debug` or lower (e.g. `RUST_LOG=debug` or `RUST_LOG=mate=debug`).

This matters for reproduction steps below and explains why this may not have
been caught by casual manual testing at default log verbosity, even though it
is a real, deterministic crash under normal debugging conditions (and an
unconditional crash via call site #4's `info!`).

### Why this hasn't been caught by tests

`tests/integration/connection_core.rs` and friends already exercise
`Connection::send_message`/`receive_message` end-to-end over real TCP
sockets, but every existing test only ever constructs `Message::new_ping(...)`
values. Grepping the whole `tests/` tree turns up no test that pushes a
`GameInvite`/`Move`/etc. through `Connection::send_message`/`receive_message`
or `Client::send_message_to`. That's the coverage gap that let this ship.

### The fix that already exists, half-used

`Message` already has variant-safe helpers built for exactly this purpose
(currently referenced only in doctests, not from any production code path):

- `log_summary() -> String` — human-readable one-liner for *any* variant (e.g. `"Move(game=abcd1234, move=e2e4)"`, `"Ping(nonce=42)"`).
- `estimated_size() -> usize` — approximate wire size for *any* variant.
- `get_game_id() -> Option<&str>` — `Some(id)` for chess variants, `None` for Ping/Pong.
- `message_type() -> &'static str` — already used in some of these same log lines.

The fix is to stop calling `get_nonce()`/`get_payload()` in code paths that
see every message type, and use these variant-safe helpers instead.

## 2. Related risk found during investigation (recommend fixing in the same pass)

While tracing every call site of `get_nonce`/`get_payload` (there are ~25
across the codebase), two more spots in `src/network/connection.rs` call
these accessors *before* validating the message variant, inside the
handshake methods:

- `Connection::handshake()` (client-initiated handshake), `connection.rs:374-379`: logs `response_message.get_nonce()` / `get_payload()` **before** the `is_pong()` check on line 382.
- `Connection::handle_handshake_request()` (server-side handshake), `connection.rs:573-578`: logs `request_message.get_nonce()` / `get_payload()` **before** the `is_ping()` check on line 581.

Today these are unreachable in practice because `receive_message()` itself
already panics first (call site #2) for any non-Ping/Pong message before
returning to these callers. **Once call site #2 is fixed, these two become
the new first point of failure**: if a peer ever sends a chess message (or
anything malformed/unexpected) during the handshake phase instead of the
expected Ping/Pong, the connection will panic here instead of returning a
clean `Err` for an invalid handshake. Since handshake is the very first thing
that happens on every inbound and outbound connection, this is worth closing
in the same change rather than leaving a second latent panic one layer
deeper. The rest of both functions (lines after the `is_ping()`/`is_pong()`
checks, e.g. `connection.rs:395,398,404,411,595,642,645,666`) are safe because
they run only after the variant has already been confirmed.

## 3. Explicitly out of scope (do not touch for this task)

Grepping for all ~25 `get_nonce()`/`get_payload()` call sites also turns up
places that are **not** part of the chess-message panic and should be left
alone:

- `src/network/client.rs:509,519` (`verify_echo_integrity`) and `:622,626` (`ping()`) — these already guard with `is_pong()` before calling the accessors, or are only ever called with a message the caller just confirmed is a Pong. Safe as-is.
- `src/network/client.rs:405` (inside `echo_session`, the ping/pong quality-test feature) — logs `response_message.get_nonce()` right after receiving, before `verify_echo_integrity()` checks `is_pong()`. This is part of the ping-only echo/quality-test feature (`mate`'s echo session, not the chess protocol), reachable only if a non-Ping/Pong message arrives during an echo test. Same *shape* of bug, but unrelated to "chess messages over the wire" — flag it, but leave fixing it as an optional follow-up unless the team wants it bundled in.
- `src/main.rs:300,408,486` — the `mate connect --message` / interactive REPL debug tool always sends `Ping` and unconditionally reads `.get_payload()` off the response, assuming the peer replies with `Pong` (matching today's server behavior, which only ever echoes `Ping` back and drops everything else). Unrelated to the chess protocol; leave as-is.

Keeping these out of scope keeps the change focused and matches
`PRIORITIES.md`'s framing of item 1 as specifically about the chess-message
send/receive panic.

## 4. Step-by-step plan

### Step 0 — Reproduce the bug manually, then write the regression tests (all failing/panicking) before changing any source

This step both confirms the diagnosis and produces the exact test harness
that Steps 1-4 will each re-run to check their fix in isolation. Nothing in
`src/` is touched in this step — only the manual repro and new test files.

**0a. Manual repro** (quick sanity check, done once):

1. In one terminal: `RUST_LOG=debug cargo run --bin mate -- serve --bind 127.0.0.1:8181`
2. In another terminal: `RUST_LOG=debug cargo run --bin mate -- key generate` (if no identity yet), then `RUST_LOG=debug cargo run --bin mate -- invite 127.0.0.1:8181`
3. Expect: the **client** process panics with
   `get_nonce() called on chess message - use get_game_id() instead`
   (from `Client::send_message_to`'s `info!` at call site #4, which panics
   even without `RUST_LOG=debug` — so this step should reproduce either way;
   the `RUST_LOG=debug` is there so you also see call site #1/#3 fire if the
   panic order ever shifts).

**0b. Write the automated regression tests, one group per call site being
fixed, so each subsequent step has its own targeted test to flip from red to
green:**

1. **Unit test** (targets the shared root cause, not tied to one call site)
   in `src/messages/types.rs` or `tests/unit/messages/` — for every chess
   variant, assert `log_summary()` and `estimated_size()` succeed and don't
   panic. This one should already pass today (it never calls
   `get_nonce`/`get_payload`); it exists to lock in the variant-safe helpers
   as the sanctioned replacement before they're wired in anywhere.
2. **Test for Steps 1 & 2** — in `tests/integration/connection_core.rs` (or a
   new `tests/integration/chess_message_wire.rs`), construct a real
   `Connection` pair over a loopback TCP socket (mirroring the existing
   `test_client_identity_usage`-style setup in that file), and for each chess
   variant (`GameInvite`, `GameAccept`, `GameDecline`, `Move`, `MoveAck`,
   `SyncRequest`, `SyncResponse`): call `send_message` on one side and
   `receive_message` on the other, assert both return `Ok(..)`, and assert
   the received message round-trips correctly (e.g. same `game_id`). Run it
   now — it should panic on `send_message` (call site #1), confirming Step 1
   is needed.
3. **Test for Step 3** — a variant of the above using
   `Client::send_message_to` (or `NetworkManager::send_message_with_strategy`
   if it's easier to set up a fixture at that layer) sending a `GameInvite`
   against a minimal echo/no-op server, asserting no panic occurs. Run it
   now — it should panic (call site #3/#4), confirming Step 3 is needed.
4. **Test for Step 4** — a handshake test where the peer sends a non-Ping/Pong
   (or malformed) message instead of the expected handshake message, and
   assert the call returns an `Err` (e.g. "Expected Ping message, got
   GameInvite") rather than panicking. This one can't be run meaningfully yet
   — `receive_message` (call site #2) will panic first before the handshake
   logic in question is even reached — but write it now so Step 4's fix has
   something to satisfy.

**Verification for Step 0**: the manual repro panics as described, and
`cargo test` shows tests 2 and 3 above panicking (test 1 passing, test 4
panicking one layer up in `receive_message`). This confirms the bug is real,
pins down the exact panicking call site per test, and gives each of Steps
1-4 below a concrete "was red, now green" check instead of a shared
end-of-task test pass.

### Step 1 — Fix `Connection::send_message` (`src/network/connection.rs:165-227`)

Replace the `debug!` block at lines 168-172 (which calls `msg.get_nonce()` /
`msg.get_payload().len()`) with something built from variant-safe helpers,
e.g.:

```rust
debug!(
    "Message summary: {}, estimated_size: {} bytes",
    msg.log_summary(),
    msg.estimated_size()
);
```

**Verification**: rerun the Step 0 test #2 (`send_message`/`receive_message`
round-trip test) — it should get past the `send_message` panic now, though it
may still panic on the `receive_message` side (Step 2 not applied yet) — that
partial progress itself confirms this fix landed correctly. Also run
`cargo test --lib connection` (or the relevant unit tests) to confirm no
regression; no behavior change for Ping/Pong (the log line's *content*
changes, but nothing downstream parses this log line).

### Step 2 — Fix `Connection::receive_message` (`src/network/connection.rs:230-317`)

Same treatment for the `debug!` block at lines 297-301
(`message.get_nonce()` / `message.get_payload().len()`):

```rust
debug!(
    "Message summary: {}, estimated_size: {} bytes",
    message.log_summary(),
    message.estimated_size()
);
```

**Verification**: rerun Step 0's test #2 — it should now pass fully (both
`send_message` and `receive_message` round-trip cleanly for all seven chess
variants). Also rerun the manual repro from 0a with the server rebuilt: the
server should no longer crash when it receives a chess message; instead it
should reach its existing `match message.message_type() { "Ping" => ..., _ => debug!("... no specific handler") }` branch at `server.rs:330-341` and log
"no specific handler" for `GameInvite`. (Full server-side handling is
PRIORITIES.md item 2, out of scope here — the acceptance bar for this task is
"doesn't panic," not "does something useful with the invite.")

### Step 3 — Fix `Client::send_message_to` (`src/network/client.rs:553-600`)

Two spots:

- Lines 555-560 (pre-send `debug!`): replace with
  `message.log_summary()` / `message.estimated_size()`.
- Lines 586-591 (post-receive `info!`, currently calls
  `response_message.get_nonce()`): replace with
  `response_message.log_summary()` (drop the raw nonce from the format
  string, or use `get_game_id()` if the log message wants to call out the
  game specifically).

**Verification**: rerun Step 0's test #3 (`Client::send_message_to` /
`NetworkManager` test) — it should now pass. Also rerun the manual repro from
0a end-to-end (client `invite` against a running server) with **both**
processes rebuilt. Neither process should panic. Confirm via `RUST_LOG=debug`
that both send and receive sides log a readable summary line for the
`GameInvite` message.

### Step 4 — Harden the handshake logging (`src/network/connection.rs:373-379` and `:573-578`)

For defense-in-depth (see section 2 above), move the `debug!` calls that log
`get_nonce()`/`get_payload()` so they only run after the variant check, or
replace them with the variant-safe helpers so they can never panic regardless
of what a peer sends during handshake:

- `handshake()` (`:374-379`): use `response_message.log_summary()` instead of
  raw `get_nonce()`/`get_payload()`, or move the existing debug log below the
  `is_pong()` check at line 382.
- `handle_handshake_request()` (`:573-578`): same treatment relative to the
  `is_ping()` check at line 581.

**Verification**: rerun Step 0's test #4 (handshake-with-unexpected-message
test) — it should now pass, returning a clean `Err` instead of panicking.
Existing handshake tests (`tests/integration/connection_core.rs`,
`connection_recovery.rs`) should continue to pass unmodified.

### Step 5 — Full verification pass

1. `cargo build` — no warnings introduced.
2. `cargo test` — full suite green, including all four tests written in
   Step 0 (now all passing) and all existing tests in
   `tests/unit/messages/`, `tests/integration/connection_core.rs`,
   `tests/integration/connection_recovery.rs`, `tests/integration/chess_protocol_core.rs`,
   `tests/integration/chess_protocol_advanced.rs`.
3. `cargo clippy --all-targets -- -D warnings` — clean.
4. Re-run the manual repro from 0a one final time end-to-end
   (`mate serve` + `mate invite`, then also try `mate accept`/a move if the
   CLI supports sending one without a listening opponent's full handler) with
   `RUST_LOG=debug` — confirm no panic on either process, and confirm the new
   log lines read sensibly (e.g. `Message summary: GameInvite(game=abcd1234, color=any), estimated_size: 45 bytes`).
5. Grep for leftover unconditional calls to confirm nothing was missed:
   `grep -rn "get_nonce()\|get_payload()" src --include="*.rs"` — every
   remaining hit should be either (a) in `src/messages/types.rs`'s own
   definitions, or (b) preceded by an `is_ping()`/`is_pong()` check in the
   same function, or (c) in one of the explicitly-out-of-scope spots listed
   in section 3.

### Step 6 — Document the decision on out-of-scope sites

Note in the PR description (not required in code) which sites from section 3
were deliberately left alone and why, so a future reader doesn't mistake the
omission for an oversight.

## 5. Acceptance criteria (definition of done)

- Sending or receiving any chess `Message` variant over a real `Connection`
  (both `send_message` and `receive_message`) no longer panics, at any log
  level.
- `Client::send_message_to` no longer panics when sending a chess message or
  receiving one as a response.
- Handshake failure due to an unexpected message variant returns a clean
  `Err`, never a panic.
- New tests from Step 0 exist and pass, and demonstrably would have caught
  the original bug (verified by running them against the pre-fix code first,
  in Step 0 itself).
- Full test suite and clippy are clean.
- No behavior change for existing `Ping`/`Pong` flows (log message *text*
  changes are acceptable; return values and control flow are not).
