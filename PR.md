# Fix connection-layer panic on chess messages

## Summary

`Message::get_nonce()` and `Message::get_payload()` are only valid for the
`Ping`/`Pong` variants — they `panic!()` for every chess message variant
(`GameInvite`, `GameAccept`, `GameDecline`, `Move`, `MoveAck`, `SyncRequest`,
`SyncResponse`). Four call sites in the generic connection/client logging
path called these accessors unconditionally on messages of any type, so
sending or receiving a chess message over the wire could panic:

- `Connection::send_message` and `Connection::receive_message`
  (`src/network/connection.rs`) — logged `get_nonce()`/`get_payload()` on
  every message, `debug!`-gated.
- `Client::send_message_to` (`src/network/client.rs`) — same issue on the
  outgoing message, plus an **unconditional** `info!` panic on the response
  message (not gated by log level, so it fired regardless of `RUST_LOG`).

In practice this meant `mate invite`/`accept`/move-sending from the CLI
panicked before a byte reached the socket, and the server panicked in its
receive loop before its own message-type `match` ever ran, whenever a chess
message was sent or received.

## Fix

Replaced the panicking accessors at all four call sites with the existing
variant-safe helpers, `Message::log_summary()` and `Message::estimated_size()`,
which handle every message variant without panicking.

`log_summary()` itself had a latent bug fixed here too: it built game-ID
prefixes with byte-index slicing (`&game_id[..8.min(game_id.len())]`), which
panics if the game ID contains any multi-byte UTF-8 character within the
first 8 bytes. It now uses a `short_game_id` helper that slices on char
boundaries via `char_indices()`.

Also includes an unrelated one-line clippy fix in `src/cli/game_ops.rs`
(`move_count % 2 == 0` → `move_count.is_multiple_of(2)`).

## Testing

Added regression coverage that fails against the pre-fix code and passes now:

- `tests/integration/chess_message_wire.rs` (new): end-to-end tests over a
  real loopback TCP `Connection`/`Client`, covering all seven chess message
  variants through `send_message`/`receive_message`, `Client::send_message_to`,
  and the client/server handshake rejection paths (a peer responding with a
  chess message instead of `Ping`/`Pong` must return an `Err`, not panic).
  Installs a real `DEBUG`-level tracing subscriber so the `debug!`-gated call
  sites are actually exercised.
- `tests/unit/messages/chess/types_enhanced.rs`: adds a test that
  `log_summary()`/`estimated_size()` handle a game ID with a multi-byte UTF-8
  character straddling the byte-8 boundary, for all seven chess variants.

```
cargo test --test chess_message_wire
```
passes (4/4) on this branch; all four panicked pre-fix.

## Planning docs

Includes planning documents produced while investigating and scoping this
work and the next priorities:

- `PRIORITIES.md` — top 5 priority fixes/features.
- `CONNECTION_LAYER_PANIC.md` — root-cause analysis and step-by-step fix plan
  for this bug.
- `SERVER_CHESS_HANDLERS.md` — plan for a follow-up (server-side chess
  message handlers), not implemented in this branch.

## Non-goals

This branch fixes the panic and its root cause in `log_summary()`; it does
not add server-side handling logic for chess messages (tracked separately in
`SERVER_CHESS_HANDLERS.md`).
