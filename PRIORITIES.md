# Priorities

A review of the current codebase (July 2026) shows a strong, well-tested foundation — Ed25519 identities, a signed wire protocol, TCP transport, and SQLite persistence all work and are covered by a broad test suite. However, the end-to-end multiplayer chess flow (`invite -> accept -> move -> sync`) is not yet functional. The five tasks below are ordered by priority: each one unblocks the next, and together they form the shortest path to a genuinely working app.

## 1. Fix the connection-layer panic on chess messages (fixed On branch fix-connection-layer-panic)

`Connection::send_message` / `receive_message` (`src/network/connection.rs:168-171,297-300`) and the one-shot path in `src/network/client.rs:555-559,586-590` call `get_nonce()` and `get_payload()` for logging on every message that passes through the wire. Those accessors are only implemented for the basic ping/pong `Message` variants and explicitly `panic!()` for chess message variants (`src/messages/types.rs:216,234`). This means any chess message (`GameInvite`, `GameAccept`, `Move`, etc.) sent or received over a live TCP connection crashes the process immediately. This is a hard blocker — nothing chess-related can work over the network until it's fixed. The fix is to stop assuming a universal nonce/payload shape in the logging path and instead use variant-safe helpers that already exist (e.g. `log_summary()`, `estimated_size()`, `get_game_id()`).

## 2. Implement server-side chess message handlers

`src/network/server.rs:330-341` only has handling logic for the `Ping` message type; every other incoming message (including all chess messages) is logged as "no specific handler" and silently dropped. Meanwhile, the client-side `NetworkManager` (`src/cli/network_manager.rs`) already sends invites, accepts, and moves, and queues them for retry on failure, as if a server were listening and responding. Right now that assumption is false — there is no receiving side at all. This task means adding real handlers on the server that parse incoming chess messages, apply them to server-side game state, persist them, and send back appropriate acknowledgements/responses so the client's retry and confirmation logic has something real to talk to.

## 3. Unify the two competing chess runtime paths

There are currently two parallel, disconnected implementations of chess game logic: the simplified handlers directly in `src/cli/app.rs` (which the CLI actually calls) and a more capable `GameOps`/`MoveProcessor` layer in `src/cli/game_ops.rs` (which is fully unused — zero references from `app.rs`). On top of that split, the two layers don't even agree on data format: `app.rs` stores message types as lowercase strings (`"game_invite"`, `"move"`) while `game_ops.rs` expects PascalCase (`"GameInvite"`, `"Move"`) at `src/cli/game_ops.rs:160,236,303,628`. This task is to pick one canonical runtime (most likely consolidating around `GameOps`, since it has the more complete reconstruction/history logic), wire it into the actual CLI command handlers in `app.rs`, and normalize the stored message-type strings so game state reconstruction from history doesn't silently fail.

## 4. Implement real board state and legal-move enforcement

Several core chess-engine pieces are stubbed out rather than implemented:
- `is_legal_move()` in `src/chess/board.rs:788-794` always returns `true`, so no move validation actually happens.
- `get_legal_moves()` in `src/cli/game_ops.rs:605-608` returns an empty `Vec`.
- There is no checkmate/stalemate/draw detection (`src/cli/game_ops.rs:741-755`).
- `handle_board` in `src/cli/app.rs:362-363` counts stored moves but never actually applies them to reconstruct the current board position, and `handle_move` (`app.rs:733-767`) accepts any non-empty string as a move without parsing or validating it, and hashes the unmodified initial board regardless of prior moves.

Without this, "playing chess" over the protocol is really just exchanging arbitrary strings — there's no game being enforced. This task covers real legal-move generation (including check/checkmate/stalemate detection) and wiring board reconstruction so `board`, `move`, and `history` reflect the actual game state after each move.

## 5. Resolve the move-notation mismatch between docs and parser

The CLI and README advertise SAN-style input (`mate move Nf3`, `Qh5#`, `O-O`) in `src/cli/commands.rs:94-105`, but the actual move parser in `src/chess/moves.rs:159-184` only understands coordinate notation (`e2e4`, `e7e8q`) plus castling. `handle_move` currently papers over this gap by accepting any non-empty string rather than actually parsing it (see item 4). This needs a decision and follow-through: either implement a real SAN parser/serializer so the advertised UX matches reality, or update the CLI help text, README, and command examples to reflect coordinate notation as the supported input format. Either choice is fine, but the current state — docs promising one thing, code silently accepting anything — will confuse users and make move history/replay unreliable.
