## Summary

The engine can apply moves to a board, but it does not enforce the rules of chess. `Board::make_move` rejects only a few structural problems: no piece on the source square, moving the wrong color, capturing your own piece, and some promotion misuse. It does not check how pieces move, whether the path is blocked, whether the mover's king is left in check, or whether castling and en passant are allowed. `Board::is_legal_move` is a stub that always returns `true`, `MoveProcessor::get_legal_moves` returns an empty `Vec`, move analysis always reports "no capture / no check / no checkmate", and a game never ends.

So a player can enter `mate move a1h8` on the first move and it is accepted, sent to the opponent, stored by the opponent's server, and replayed into every later board reconstruction. Both peers compute the same (wrong) board hash, so the hash check doesn't catch it. The protocol, persistence, and CLI plumbing all work. The missing piece is the rules of chess.

This issue covers:

1. A legal-move generator in the `chess` module (piece movement, blocking, pins, check, castling, en passant, promotion).
2. Making `Board::make_move` reject illegal moves, with an explicit unchecked path where one is really needed.
3. Detecting check, checkmate, stalemate, and automatic draws.
4. Wiring that into `MoveProcessor` (move analysis, legal-move listing, game completion) and into the inbound server `Move` handler, so illegal moves are rejected on both the sending and receiving side.
5. Making `board`, `move`, and `history` show the enforced game state, including check and game-over status.

Move *notation* (SAN such as `Nf3` vs. coordinate notation such as `g1f3`) is **out of scope**. This issue keeps the current coordinate-plus-castling input format (`e2e4`, `e7e8q`, `O-O`, `O-O-O`). The legal-move generator added here is the prerequisite for a SAN parser later, because SAN disambiguation needs the list of legal moves.

## Current state (as of this branch)

### Board and move application: `src/chess/board.rs`

- `Board` stores an 8×8 mailbox (`squares[rank][file]`), the active color, halfmove clock, fullmove number, `CastlingRights`, and `en_passant_target`. FEN round-trips all of these (`from_fen` / `to_fen`). The two stale `TODO: Store castling rights / en passant` comments in `from_fen` (around lines 436 and 472) are already done and should be removed.
- `make_move` (line 678) checks:
  - the source square has a piece of the active color,
  - the destination is not your own piece,
  - promotion is used only by a pawn landing on the last rank, and is required there.
- `make_move` does **not** check:
  - movement geometry (a rook moving diagonally, a knight moving like a bishop, a pawn moving backwards or three squares),
  - blocking pieces for sliders, or a pawn's double push through a piece,
  - pawn captures versus pushes (a pawn can "push" onto an enemy piece or move diagonally onto an empty square),
  - whether the mover's own king is in check afterwards (pins, walking into check, ignoring check),
  - castling legality: `detect_castling_move` only checks that the king starts on the e-file of its home rank and moves two files. It ignores `castling_rights`, pieces between king and rook, and whether the king starts in, passes through, or lands on an attacked square,
  - king moves of more than one square that aren't castling.
- `is_legal_move` (line 787) always returns `true`.
- `Board` has no king-location helper, attack detection, or move generation.

### Game layer: `src/game/moves.rs`, `src/game/rebuild.rs`, `src/game/ops.rs`

- The CLI already uses one runtime. `handle_board`, `handle_history`, and `handle_move` in `src/cli/app.rs` all go through `GameOps::reconstruct_game_state`, which calls `rebuild_board_from_stored_messages` and replays every stored `"Move"` row through `Board::make_move`. Board reconstruction works. It is just as permissive as `make_move`.
- `MoveProcessor::prepare_move` parses the notation, applies it with `make_move`, and hashes the resulting board (`hash_board_state`, SHA-256 of the FEN). This is the only validation the sender's moves get.
- `MoveProcessor::analyze_move` (line ~392) always returns `is_capture: false, is_check: false, is_checkmate: false`.
- `MoveProcessor::update_game_status_if_needed` (line ~382) is a no-op, so games never become `Completed`.
- `MoveProcessor::get_legal_moves` (line ~274) returns `Vec::new()`.
- `MoveProcessor::apply_opponent_move` exists. It rebuilds the board, checks turn order, applies the move, and verifies the board hash. **Nothing calls it.**

### Inbound network path: `src/network/handlers.rs`

- `handle_move` (line ~404) checks that the game exists, is `Active`, and that the sender is the recorded opponent. Then it stores the move and replies `MoveAck`. Its doc comment says it "does not reconstruct the board or verify move legality". It also doesn't check turn order or the board hash. A peer can send an illegal move, a move out of turn, or a move with a wrong hash, and it is persisted. After that, every later rebuild of the game either includes the bad move or fails.

### Storage

- `Database::update_game_result(game_id, GameResult)` in `src/storage/games.rs:207` already sets `result`, `status = Completed`, and `completed_at`. Nothing calls it from the move path. `GameResult` is `Win | Loss | Draw | Abandoned`, relative to the local player.

### Tests that assume the permissive behavior

- `tests/unit/chess/move_application.rs` has a `move_validation_placeholder_tests` module that asserts `is_legal_move` returns `true` for obviously illegal moves (`a1h8` from the start position). These tests must be replaced.
- Some unit/integration tests may set up positions by playing moves that are illegal, or legal only by accident (e.g., in `board_hashing.rs`, `conversion.rs`, `chess_engine_messages.rs`, `chess_board_integration.rs`, `tests/unit/network/handlers.rs`). Run the full suite after the enforcement change. Rewrite any failing setup as either a legal move sequence or a `Board::from_fen` position. Don't weaken the rules to make them pass.

## Proposed design

### 1. Move generation in the `chess` module

Write the generator in-house against the existing `Board` / `Move` / `Position` types rather than adding a chess crate:

- The wire-level board hash is `SHA-256(board.to_fen())`. Both peers must produce byte-identical FEN, and that already depends on our own `Board`. An external crate would mean converting back and forth on every move, with two sources of truth for castling rights and en-passant state.
- The existing mailbox representation is enough for a two-player correspondence game. Performance only matters for tests (perft), and a simple implementation handles perft depth 4–5 in well under a second in release mode.
- The strongest well-known Rust option, `shakmaty`, is GPL-3.0. This project is MIT-licensed. If an oracle is wanted in tests, published perft node counts are enough and need no dependency.

Suggested layout: a new private module `src/chess/movegen.rs` (plus `src/chess/attacks.rs` if helpful), re-exported through `Board` methods:

```rust
impl Board {
    /// Square of `color`'s king, if present.
    pub fn king_position(&self, color: Color) -> Option<Position>;

    /// True if any piece of `by` attacks `square` (ignores whose turn it is).
    pub fn is_square_attacked(&self, square: Position, by: Color) -> bool;

    /// True if the side to move is in check.
    pub fn is_in_check(&self) -> bool;

    /// All legal moves for the side to move.
    pub fn legal_moves(&self) -> Vec<Move>;

    /// Real implementation replacing the stub.
    pub fn is_legal_move(&self, mv: Move) -> bool;

    /// Checkmate / stalemate / insufficient material / 75-move rule.
    /// Repetition needs history and lives in the game layer (see §3).
    pub fn outcome(&self) -> Option<BoardOutcome>;
}
```

Generation approach (pseudo-legal, then filter):

1. **Pseudo-legal generation** per piece, from the side to move:
   - Pawn: single push to an empty square, double push from the start rank if both squares are empty, diagonal captures of enemy pieces, en passant onto `en_passant_target`. Moves reaching the last rank expand into four moves (Q, R, B, N promotion).
   - Knight / king: fixed offset tables, target empty or enemy.
   - Bishop / rook / queen: slide along rays until off-board or blocked; include the first enemy piece.
   - Castling: only if the matching `castling_rights` flag is set, the king and rook are on their home squares, the squares between them are empty, and the king is not currently in check and does not pass through or land on an attacked square (e1/f1/g1 for white kingside; e1/d1/c1 for white queenside; the b-file square only needs to be empty).
2. **Legality filter**: for each pseudo-legal move, apply it to a clone of the board and reject it if the mover's king is attacked afterwards. This handles pins, discovered checks, moving into check, and the en-passant edge case where capturing exposes the king along the rank. It is simple and correct, and fast enough here.
3. **Attack detection** (`is_square_attacked`): look outward from the target square. Knight offsets for enemy knights, diagonal rays for bishops/queens, orthogonal rays for rooks/queens, king offsets, and the two pawn-attack squares for the attacking color. This avoids generating all enemy moves.

Keep the code that applies a move separate from the code that validates it. Split `make_move` internally into:

- `fn apply_move_unchecked(&mut self, mv: Move)`: the current state-update logic (piece relocation, castling rook move, en-passant capture, promotion, castling-rights update, en-passant target, clocks, side to move). Used by the legality filter on cloned boards.
- `pub fn make_move(&mut self, mv: Move) -> Result<(), ChessError>`: returns `ChessError::InvalidMove` with a descriptive reason if `mv` is not in `legal_moves()`, otherwise calls `apply_move_unchecked`.

Error messages should say *why* a move was rejected where that's cheap to work out (e.g., "no piece at e3", "it is Black's turn", "king would be in check", "castling kingside not available: rights lost", "path blocked"), falling back to "illegal move `a1h8`". A good approach: run the existing structural checks first (they already produce good messages), then do membership in `legal_moves()`, and for a rejected king move or castle, check whether the king would be in check to give a better message.

`Move` equality: a promotion is only legal with a `promotion` piece, and a non-promotion move only without one. The existing "promotion required" / "promotion not allowed" errors should still fire before the generic "illegal move" error.

### 2. Game outcome detection

Add a type in `src/chess` (e.g., `src/chess/outcome.rs`):

```rust
pub enum BoardOutcome {
    Checkmate { winner: Color },
    Stalemate,
    InsufficientMaterial,
    SeventyFiveMoveRule,
    FivefoldRepetition,
}
```

- **Checkmate**: `legal_moves().is_empty() && is_in_check()`.
- **Stalemate**: `legal_moves().is_empty() && !is_in_check()`.
- **Insufficient material** (automatic draw): K vs K; K+N vs K; K+B vs K; K+B vs K+B with both bishops on the same square color.
- **75-move rule** (automatic): `halfmove_clock >= 150`. Note that `Board::from_fen` currently rejects halfmove clocks above 100. Raise that limit to at least 150 so the automatic rule is reachable and FEN positions near it can be tested.
- **Fivefold repetition** (automatic): needs position history, so the game layer computes it (below) and it is not part of `Board::outcome()`.

The *claimable* draws (threefold repetition, 50-move rule) need a "claim draw" or "offer draw" protocol message, which doesn't exist yet. **Out of scope** here. Expose them as helpers (`is_fifty_move_claimable()`, and repetition counting in the game layer) so a later draw-claim feature can use them, but don't end the game on them.

For repetition, two positions are equal when piece placement, side to move, castling rights, and en-passant target all match. That is the first four FEN fields, with the halfmove/fullmove counters dropped. Strict FIDE compares the en-passant square only when a capture is actually possible. The simpler first-four-fields key is fine here, since both peers compute it the same way. Note the simplification in a doc comment.

### 3. Game layer wiring: `src/game/`

- `rebuild_board_from_stored_messages` goes through `make_move`, so it picks up legality enforcement automatically. Also make it:
  - return (or give callers a way to compute) the list of repetition keys for each ply, so fivefold repetition can be detected,
  - keep failing on the first bad row, but include the ply number and notation in the error so the user can see where a corrupted history broke.
- `GameState` (`src/game/ops.rs`) should carry the outcome, e.g. `pub outcome: Option<GameOutcome>` plus `pub in_check: bool`. `GameOutcome` wraps `BoardOutcome` and adds `FivefoldRepetition` from history.
- `MoveProcessor::analyze_move`: fill in real values.
  - `is_capture`: a piece was on `mv.to` in the old board, or the move was en passant.
  - `is_check`: `new_board.is_in_check()`.
  - `is_checkmate`: `new_board.outcome() == Some(Checkmate { .. })`.
- `MoveProcessor::get_legal_moves`: return `state.board.legal_moves()` formatted with the existing `Move` `Display` (coordinate notation).
- `MoveProcessor::update_game_status_if_needed`: if the new position ends the game, call `Database::update_game_result` with a result relative to `game.my_color`: checkmate by us → `Win`, checkmate by opponent → `Loss`, any automatic draw → `Draw`. This must run on both peers, when committing our own move and when accepting the opponent's.
- `MoveProcessor::prepare_move` and `validate_move`: reject moves on a game whose reconstructed state already has an outcome, even if the stored status is still `Active` (e.g., the result was never written because of a crash). Reject with `InvalidGameState("game is over: checkmate")` or similar.
- `MoveProcessor::commit_move` currently calls `update_game_status(game_id, Active)` unconditionally. Make sure that call can't overwrite a `Completed` status written for the same move (call `update_game_status_if_needed` after it, or skip the `Active` write if the game is already completed).

### 4. Inbound validation: `src/network/handlers.rs`

`handle_move` should check the move before persisting it, using the same code as the local path:

1. Existing checks (game exists, `Active`, sender is the opponent). Keep them.
2. Idempotency: if an identical `Move` payload is already stored, reply `MoveAck` as today without re-validating. Otherwise, a retry of the last move would be checked against the board *after* that move and wrongly rejected.
3. Rebuild the board from stored messages and check that it's the opponent's turn: `board.active_color() != Color::from(game.my_color)`.
4. Parse the notation with `Move::from_str_with_color(&mv.chess_move, board.active_color())` and require `board.is_legal_move(chess_move)`.
5. Apply the move to a clone and require `hash_board_state(&next) == mv.board_state_hash`.
6. Persist, then update the game result if the move ended the game.
7. On any validation failure, reply with the existing soft-reject (`decline_invite(..., reason)`) with a specific reason ("illegal move e1e3", "not your turn", "board hash mismatch"), and **do not persist**.

The cleanest way is to have `handle_move` call `MoveProcessor::apply_opponent_move` (after the idempotency check), which already does steps 3–6 except the outcome update, and map its errors to decline reasons. `src/game` does not depend on `network`, and `network` already depends on `game`, so this adds no dependency cycle. Update the `handle_move` doc comment to match.

On the sending side, `prepare_move` already runs before the network send in `App::handle_move`, so enforcement there comes for free once `make_move` is strict. After a successful `commit_move`, `App::handle_move` should also call the game-completion update and tell the user about check / checkmate / draw.

### 5. CLI output: `src/cli/app.rs`, `src/cli/display.rs`

- `mate board`: after the board, print `Check!` when the side to move is in check. Print the game result for a finished game (`Checkmate — you won`, `Stalemate — draw`, `Draw by insufficient material`, …). Don't print "It's your turn to move!" once the game is over.
- `mate move`: on success, show capture / check / checkmate from `PreparedMove` (e.g. `✓ Move 'd8h4' sent. Checkmate — you won!`). On rejection, show the engine's reason (e.g. `Illegal move 'e1e2': path blocked by own pawn on e2`), and point to coordinate notation (`e2e4`, `e7e8q`, `O-O`).
- `mate history`: mark checks/mates on moves (e.g. `+` / `#` suffix, using `get_move_history_with_analysis`, which already exists and will report real values once `analyze_move` does). Show the final result if the game ended.
- Optional, small, useful: a `--legal` flag on `mate board` (or a `mate moves` subcommand) that lists `get_legal_moves()`. Nice for debugging and for users unsure of the notation. Fine to leave for a follow-up.

## Acceptance criteria

### Engine correctness

- [ ] **Perft** matches published node counts (from the Chess Programming Wiki "Perft Results" page) for at least:
  - Start position: depth 1 = 20, 2 = 400, 3 = 8,902, 4 = 197,281.
  - "Kiwipete" `r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1`: depth 1 = 48, 2 = 2,039, 3 = 97,862.
  - Position 3 `8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1`: depth 1 = 14, 2 = 191, 3 = 2,812, 4 = 43,238 (en passant and discovered-check edge cases).
  - Position 4 `r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1`: depth 1 = 6, 2 = 264, 3 = 9,467 (promotions, castling through check).
  - Position 5 `rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8`: depth 1 = 44, 2 = 1,486, 3 = 62,379.
  - Deeper depths can go behind `#[ignore]` or a release-only test so `cargo test` stays fast.
- [ ] Unit tests, each set up from FEN, for: blocked sliders; pawn push vs. capture; double push through a piece; pinned piece cannot move off the pin line; king cannot move into check; king cannot capture a defended piece; must answer check (block, capture, or move); castling rejected when rights are lost, the path is occupied, the king is in check, the king passes through an attacked square, or the king lands on an attacked square; en passant only immediately after the double push; en passant rejected when it exposes the king along the rank; all four promotion pieces generated; promotion to K/P rejected.
- [ ] Checkmate detection (e.g., fool's mate `f2f3 e7e5 g2g4 d8h4`, back-rank mate from FEN), stalemate detection (e.g. `7k/5Q2/6K1/8/8/8/8/8 b - - 0 1`), each insufficient-material case, and the 75-move rule.
- [ ] `is_legal_move(mv) == legal_moves().contains(&mv)` for every position used in the tests above.
- [ ] Every `legal_moves()` entry is accepted by `make_move`. Random pseudo-legal-but-illegal moves are rejected with `ChessError::InvalidMove` and leave the board unchanged.
- [ ] `make_move` rejects `a1h8`, `e2e5`, `e1e2`, `g1g3`, and `e2d3` from the starting position.

### Integration

- [ ] `rebuild_board_from_stored_messages` errors (with ply number and notation) on a history containing an illegal move, and still rebuilds legal histories including castling for both colors, en passant, and promotion.
- [ ] `MoveProcessor::prepare_move` rejects illegal moves and moves in finished games. `analyze_move` reports capture / check / checkmate correctly. `get_legal_moves` returns 20 moves for a newly started game.
- [ ] After a checkmating move, both peers' databases show `status = Completed`, with `Win` for the mating side and `Loss` for the other. After a stalemating move, both show `Draw`.
- [ ] Server `handle_move` declines (and does not persist) an illegal move, a move sent out of turn, and a move with a wrong board hash. It still acks an identical retry of an already-stored move.
- [ ] An end-to-end test (extending `tests/integration/server_chess_handlers.rs` or `cli_network.rs`) plays a short game to checkmate between two peers over TCP and checks the final stored state on both sides.

### CLI

- [ ] `mate board` shows check and final result. `mate move` shows a specific error for illegal input and announces check/mate/draw. `mate history` marks checks/mates.

### Cleanup

- [ ] Remove the `move_validation_placeholder_tests` module and the stale `TODO` comments in `Board::from_fen`. Update the `handle_move` and `rebuild_board_from_stored_messages` doc comments that describe the old permissive behavior.
- [ ] `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check` pass.

## Out of scope

- SAN input/output (`Nf3`, `exd5`, `Qh5#`) and aligning the CLI help/README with the supported notation. Tracked separately. The legal-move generator here is its prerequisite.
- Draw offers, draw claims (threefold repetition, 50-move rule), and resignation. These need new protocol messages.
- Time controls.
- Engine optimizations (bitboards, make/unmake instead of clone). Worth revisiting only if perft or rebuild time becomes a real problem.

## Suggested implementation order

1. Attack detection, `king_position`, `is_in_check`, and pseudo-legal generation, with unit tests.
2. Legality filter, castling/en-passant rules, `legal_moves`, `is_legal_move`, and perft tests. **Don't change `make_move` yet.** Get perft green first.
3. Split `make_move` into `apply_move_unchecked` + validated `make_move`. Fix the test fallout by rewriting setups as legal sequences or FEN positions.
4. `BoardOutcome`, insufficient material, 75-move rule, and the relaxed FEN halfmove limit.
5. Game layer: `analyze_move`, `get_legal_moves`, outcome + repetition in `GameState`, `update_game_status_if_needed`, reject moves in finished games.
6. Server `handle_move` validation via `apply_opponent_move`, with an idempotency-first check.
7. CLI output changes and the end-to-end checkmate test.