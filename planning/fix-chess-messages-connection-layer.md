# Fix Chess Messages In The Live Connection Layer

## Problem

`Connection::send_message` and `Connection::receive_message` currently do transport-level debug logging by calling `Message::get_nonce()` and `Message::get_payload()` on every message.

That is safe for `Ping` and `Pong`, but it is explicitly invalid for chess messages. In `src/messages/types.rs`, both accessors panic on `GameInvite`, `GameAccept`, `GameDecline`, `Move`, `MoveAck`, `SyncRequest`, and `SyncResponse`.

This means live chess traffic can fail before any server-side chess handling is reached, purely because the connection layer logs the wrong fields.

## What I Verified

### Direct panic path

- `src/network/connection.rs`
  - `send_message` logs `msg.get_nonce()` and `msg.get_payload().len()`.
  - `receive_message` logs `message.get_nonce()` and `message.get_payload().len()`.
- `src/messages/types.rs`
  - `get_nonce()` panics for chess message variants.
  - `get_payload()` panics for chess message variants.

### Existing safe metadata already exists

The message layer already exposes variant-safe helpers that are better suited for transport logging:

- `message_type()`
- `get_game_id()`
- `estimated_size()`
- `log_summary()`

Those work across both ping/pong and chess messages.

### The handshake path is not the same bug

The handshake code in `src/network/connection.rs` still calls `get_nonce()` and `get_payload()`, but only after it has established that it is handling `Ping` or `Pong` handshake messages. That usage is fine and should stay specific to handshake validation.

### There is an adjacent copy of the same issue

`src/network/client.rs` has the same transport-agnostic logging pattern in `send_message_to`:

- it logs outgoing `message.get_nonce()` and `message.get_payload().len()` before sending
- it logs `response_message.get_nonce()` after receiving

That will hit the same panic path for one-shot chess sends unless it is cleaned up too.

## Best Fix

Do not change `get_nonce()` or `get_payload()` to return defaults or `Option` just to satisfy generic logging.

Those methods encode an important invariant: nonce/payload belong to the ping/pong transport messages, not to chess protocol messages. Weakening that contract would blur message semantics and make future misuse easier.

The better fix is:

1. Keep the strict accessors as-is.
2. Replace generic logging in the live connection/client layers with variant-safe metadata.
3. Leave nonce/payload access only in code paths that have already narrowed the message type to ping/pong.

## Recommended Implementation Steps

### 1. Update `Connection::send_message`

Replace the current debug log with structured fields derived from safe helpers, for example:

```rust
debug!(
    message_summary = %msg.log_summary(),
    estimated_size_bytes = msg.estimated_size(),
    game_id = msg.get_game_id().unwrap_or("-"),
    is_chess_message = msg.is_chess_message(),
    "Sending message details"
);
```

Notes:

- `log_summary()` is the most useful field here because it already formats ping and chess variants appropriately.
- `estimated_size()` is a reasonable replacement for `payload_len` in generic transport logging.
- `get_game_id()` is optional but useful when tracing chess traffic.

### 2. Update `Connection::receive_message`

Apply the same change after the message is deserialized:

```rust
debug!(
    message_summary = %message.log_summary(),
    estimated_size_bytes = message.estimated_size(),
    game_id = message.get_game_id().unwrap_or("-"),
    is_chess_message = message.is_chess_message(),
    "Received message details"
);
```

This removes the panic while preserving useful observability.

### 3. Make the same cleanup in `Client::send_message_to`

`Client::send_message_to` is not the connection layer itself, but it repeats the same unsafe assumption. It should use the same logging style so the transport surface is consistent.

If this is not fixed in the same change, chess messages sent through the one-shot client path can still panic.

### 4. Keep handshake and ping helpers type-specific

Do not rewrite the following code to avoid `get_nonce()` or `get_payload()` entirely:

- handshake request/response validation in `src/network/connection.rs`
- ping validation in `src/network/client.rs`

Those are message-type-specific flows, and using nonce/payload there is correct.

## Regression Tests To Add

There does not appear to be a direct regression test for live `Connection` send/receive of chess messages. Add one.

Recommended coverage:

### 1. Connection send does not panic for chess messages

Add an integration test that:

- creates a client/server connection pair
- completes the normal handshake
- sends a `Message::GameInvite(...)` through `Connection::send_message`
- verifies the receiver gets `Message::GameInvite` and the expected `game_id`

This specifically proves the transport layer no longer crashes on chess traffic.

### 2. Connection receive does not panic for chess messages

In the same test or a second one:

- send a chess message from the other side
- verify `Connection::receive_message` returns it successfully

That covers both unsafe logging call sites in `connection.rs`.

### 3. One-shot client path does not panic for chess messages

If `src/network/client.rs` is updated in the same fix, add coverage for the one-shot send path as well, or at minimum a focused regression test around its generic logging.

## Suggested Change Order

1. Fix `src/network/connection.rs` logging.
2. Fix `src/network/client.rs` logging in the same commit.
3. Add a live chess-message connection regression test.
4. Run focused network and chess protocol tests.

## Validation After The Fix

At minimum, run:

```bash
cargo test connection_core
cargo test chess_protocol_core
```

If a new targeted regression test is added, run that test directly as well.

## Scope Boundary

This fix removes an immediate transport-layer blocker for chess messages, but it does not by itself make live multiplayer chess work end to end.

Separate work is still needed for server-side chess handling and the higher-level gameplay/runtime path. This change is best treated as a prerequisite transport fix.