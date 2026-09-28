# Unify the CLI and server chess runtime paths

## Summary

Before this branch there were two parallel implementations of chess game
state: the live CLI path (`src/cli/app.rs`), which persisted lowercase
`"game_invite"` / `"game_accept"` / `"move"` rows and stubbed board
reconstruction (move count only, hashing the *initial* board), and an
orphaned `GameOps` / `MoveProcessor` stack (`src/cli/game_ops.rs`) that
reconstructed real board state via `Board::make_move` but persisted
PascalCase (`"GameInvite"`, `"Move"`) and had zero callers from `app.rs`.
The server's handlers (`src/network/handlers.rs`) had a third, separate
rebuild implementation. Because the CLI wrote snake_case while the
orphaned ops layer and the server's sync rebuild only read PascalCase, a
real CLI invite/move was invisible to reconstruct and to `SyncRequest`.

This branch deletes the CLI's parallel stack, lifts the stronger
reconstruct/list/history/move logic into a new shared `src/game/` domain
module owned by neither `cli` nor `network`, and wires both the CLI and
the server through it. It also fixes the invite/accept dial-back
addressing bug that would have kept end-to-end play broken even after
the message-type mismatch was fixed.

See `UNIFY_CHESS_RUNTIMES.md` for the full investigation/design doc this
branch was implemented against.

## What changed

### New shared domain module — `src/game/`

- **`message_type.rs`** — `StoredMessageType` enum (`GameInvite`,
  `GameAccept`, `GameDecline`, `Move`) is the single source of truth for
  the wire-aligned PascalCase strings stored in the `messages` table.
  `FromStr`/`TryFrom<&str>` reject snake_case and any other casing, so the
  old string-literal mismatch can't recur silently.
- **`rebuild.rs`** — one `rebuild_board_from_stored_messages` used by both
  `GameOps::reconstruct_game_state` (CLI) and the server's `SyncRequest`
  handler, replacing two divergent implementations. Moves are parsed with
  `ChessMove::from_str_with_color(..., board.active_color())` everywhere
  (the old `GameOps` path used plain `from_str`, which defaulted castling
  to White — a latent bug, now fixed).
- **`ops.rs`** — `GameOps` (list/resolve games, reconstruct board +
  history, turn heuristic) plus domain helpers
  (`store_game_invite_message`, `store_game_accept_message`,
  `store_game_decline_message`, `store_game_move_message`) used by both
  the CLI and server handlers so persistence conventions can't drift
  apart again.
- **`moves.rs`** — `MoveProcessor` split into `prepare_move` (reconstruct,
  turn check, parse, `make_move` on a copy, compute post-move hash, build
  the wire `Move` — no DB write) and `commit_move` (persist after a
  successful send). This preserves the "don't persist a move the opponent
  never received" behavior now that the CLI goes through App:
  `prepare_move → NetworkManager::send_chess_move → commit_move` on ack.

### CLI (`src/cli/`)

- **`app.rs`** shrinks from ~940 to ~710 lines by deleting all of Path
  A's hand-rolled table printing, move-count turn heuristic, and
  initial-board hashing. `handle_games` / `handle_board` / `handle_history`
  / `handle_invite` / `handle_accept` / `handle_move` now go through
  `GameOps`, `MoveProcessor`, and `cli::display::*`.
- CLI chess writes are now PascalCase via `StoredMessageType`; the
  lowercase writers are gone.
- **Invite/accept addressing fix**: invites now carry an optional
  `reply_to` dial-back address (`messages/chess.rs`) and store it in the
  game's `metadata` (`{"dial_address": "..."}`); after a successful
  handshake, `opponent_peer_id` is overwritten with the real authenticated
  peer id (`Database::update_opponent_peer_id`, new in
  `storage/games.rs`) instead of holding a raw address string. `accept`
  and `move` dial the address from metadata rather than treating
  `opponent_peer_id` as a dialable string.
- **`network_manager.rs`** — `send_game_invite` / `send_game_accept` /
  `send_chess_move` now return a `SendOutcome { response, peer_id }`
  instead of a bare `Message`, surfacing the connection's authenticated
  peer identity so `App` can update `opponent_peer_id` post-handshake.
- **`error_handler.rs`** — chess command errors (`GameOpsError`, move
  processing errors) now map through `CliError` for consistent
  `NoCurrentGame` / `GameNotFound` / etc. messaging instead of ad-hoc
  `anyhow::bail!`.
- **`src/cli/game_ops.rs` deleted** (795 lines) — fully replaced by
  `src/game/`.
- Move notation is intentionally tightened to coordinate/castling forms
  (`e2e4`, `e1g1`, ...) via `ChessMove::from_str_with_color`, closing the
  old CLI's "accept any non-empty string" hole. SAN (`Nf3`) support
  remains a separate, unstarted priority item.

### Server (`src/network/handlers.rs`)

- Store helpers (`store_invite_message`, `store_accept_message`, etc.)
  and the private rebuild implementation are deleted in favor of the
  shared `src/game::*` helpers — same behavior, one implementation.
  Dead `HandlerError::NotImplemented` removed.

### Other

- `messages/chess.rs`: `GameInvite.reply_to: Option<String>` (serialized
  always, including `None`, to keep bincode wire framing stable) plus
  `host:port` validation for it.
- `storage/games.rs`: `Database::update_opponent_peer_id`.
- `docs/old_plans/SERVER_CHESS_HANDLERS.md` relocated (unchanged) out of
  the repo root into `docs/old_plans/`.
- `PRIORITIES.md` item 2 annotated as fixed on the `server-chess-handlers`
  branch.

## Explicitly out of scope

- Real move legality / check / checkmate detection (still stubbed) —
  separate priority item.
- SAN move input (`mate move Nf3`) — coordinate/castling notation only.
- Snake_case compatibility reads — there are no production users of the
  old rows yet, so this is a clean cutover, not a dual-read migration.

## Test plan

- `cargo test` — 771 unit/integration tests plus CLI/storage suites, all
  green (`cargo fmt` clean).
- Updated `tests/integration/cli_commands.rs`, `cli_network.rs`,
  `cli_error_handling.rs`, `cli_database.rs`, and
  `tests/unit/network/handlers.rs` for the new PascalCase storage,
  coordinate-only move notation, and dial-address/peer-id split.
- Manual two-peer check described in `UNIFY_CHESS_RUNTIMES.md` §8: invite
  from peer A to peer B, accept dials A's advertised address (not a raw
  peer id), moves round-trip and `mate board` / `mate history` reflect
  real applied moves on both sides.
