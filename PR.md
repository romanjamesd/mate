# Implement server-side chess message handlers

## Summary

The server's connection loop previously only echoed `Ping` messages and
logged everything else ("no specific handler") without touching storage.
This branch adds a typed dispatch layer that persists chess protocol
messages (`GameInvite`, `GameAccept`, `GameDecline`, `Move`, `SyncRequest`)
against the peer's SQLite database and replies appropriately, so two `mate
serve` peers can actually play a game over the wire instead of the invite
just hanging.

## What changed

- **`src/network/handlers.rs` (new)** — `dispatch()` validates each inbound
  message, then routes to a per-message handler:
  - `GameInvite` → creates a `Pending` game using the inviter's `game_id`
    and echoes the invite back as an ack. Duplicate invites from the same
    peer against a `Pending` game are treated as idempotent; conflicts or
    non-pending state soft-reject with `GameDecline`.
  - `GameAccept` → transitions `Pending` → `Active`, finalizes the local
    player's color, and echoes the accept.
  - `GameDecline` → transitions `Pending` → `Abandoned` and echoes the
    decline.
  - `Move` → persists the move against an `Active` game and replies with
    `MoveAck` (no legality/board verification yet).
  - `SyncRequest` → rebuilds the board and move history from stored
    `Move` messages and replies with `SyncResponse`.
  - `MoveAck` / inbound `SyncResponse` are ignored (no reply expected).
  - Any message with a `game_id` that fails validation or an ownership/
    state check soft-fails as `GameDecline` (rather than dropping the
    connection) so send-and-wait clients never hang.
- **`src/network/server.rs`** — `Server::bind` / `bind_with_config` now take
  an `Arc<Database>`, threaded into each spawned connection task and passed
  to `handlers::dispatch` in the receive loop.
- **`src/main.rs`** — `serve` now opens the same peer SQLite database the
  CLI uses (via `Database::new(identity.peer_id())`) before binding the
  server.
- **`src/storage/games.rs`** — adds `Database::create_game_with_id` (used
  to materialize an incoming invite under the inviter's `game_id`;
  `create_game` now delegates to it with a generated ID) and
  `Database::update_game_color` (finalizes color at accept time). Duplicate
  primary keys surface as `StorageError::ConstraintViolation` instead of a
  raw SQLite error.
- **`src/cli/app.rs`** — CLI invite now creates the local game record with
  an explicit UUID (`generate_game_id`) via `create_game_with_id`, since
  wire `GameInvite` validation requires UUID-format IDs (legacy storage IDs
  were peer-timestamp-counter strings).
- **`src/cli/commands.rs`** — updates the `serve` command's help text to
  reflect that it's no longer just an echo server.

## Testing

- `tests/integration/server_chess_handlers.rs` (new) — end-to-end coverage
  over real TCP connections: invite → accept → move → sync round trips,
  idempotent retries, and soft-decline paths (unknown game, wrong peer,
  wrong state, malformed messages).
- `tests/unit/network/handlers.rs` (new) — unit-level coverage of
  `dispatch` and each handler against an in-memory/temp database.
- `tests/storage/storage_tests.rs` / `storage_error_tests.rs` — coverage
  for `create_game_with_id` (including duplicate-ID and empty-ID
  rejection) and `update_game_color` (including not-found).
- Existing integration suites updated only where the `Server::bind`
  signature change or shared test helpers required it.

## Known gaps / follow-ups

- `Move` handling does not reconstruct the board or verify move legality
  before persisting — it trusts the wire payload.
- `SyncRequest` rebuilds from stored move notation without verifying
  clients' post-move board-state hashes.
- Full CLI-driven accept/move end-to-end (as opposed to direct protocol
  tests) is deferred — see `SERVER_CHESS_HANDLERS.md` for the address-vs-
  peer-ID issue that blocks it.
- Color selection at invite time is currently a fixed default
  (invitee gets White unless the inviter suggests otherwise); letting the
  invitee choose at accept time, with the inviter as fallback, is future
  work (noted in `SERVER_CHESS_HANDLERS.md`).

See `SERVER_CHESS_HANDLERS.md` for the full step-by-step design and
implementation log this branch followed.
