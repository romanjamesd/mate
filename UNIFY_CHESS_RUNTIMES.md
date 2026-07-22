# Unify the two competing chess runtime paths

Status: **planning only** — no code has been changed for this item yet.
Branch target: `unify-chess-runtime-paths` (PRIORITIES.md item 3).

This plan is the result of investigating the live CLI path (`src/cli/app.rs`),
the unused ops layer (`src/cli/game_ops.rs`), and the server handlers
(`src/network/handlers.rs`). It standardizes on the more capable Path B
behavior, lifts it into a shared domain module, and deletes Path A’s parallel
stack — without leaving invite/accept/move networking or board rebuild without
a replacement.

---

## 1. Decision

**Canonical runtime:** the reconstruct / list / move-apply logic today in
`GameOps` + `MoveProcessor`, relocated out of `cli/` so both the CLI and the
server can share it.

**Chosen shape:** Approach 2 (shared game-domain module), incorporating:

- Thin CLI adapters over that module (Approach 1)
- PascalCase wire-aligned `message_type` strings only — no legacy dual-read
  (Approach 3, minus compatibility shims; there are no users yet)
- A typed stored-message-type helper so the string split cannot recur
  (Approach 4)

**Rejected:** keep `App` as the sole runtime and delete `GameOps` (Approach 5).
That discards the stronger reconstruct/history path and the already-tested
`cli_database` / display / validation surface.

---

## 2. Current state (confirmed)

| Path | Location | Role today |
|------|----------|------------|
| **A — live CLI** | `src/cli/app.rs` `handle_*` | What `mate games/board/invite/accept/move/history` actually run. Writes lowercase `"game_invite"` / `"game_accept"` / `"move"`. Board/move paths stub reconstruct (count only; hash initial board). Owns all networking via `NetworkManager`. |
| **B — orphan ops** | `src/cli/game_ops.rs` | `GameOps` + `MoveProcessor`. Reconstructs via `Board::make_move`, stores PascalCase `"Move"` / reads `"GameInvite"`. Used by `tests/integration/cli_database.rs`, `InputValidator`, `display.rs` — **zero** references from `app.rs`. No networking. |
| **C — server** | `src/network/handlers.rs` | Persists PascalCase; has its own `rebuild_board_from_stored_moves` for `SyncRequest`. Does not import `cli` (and must not). |

### Fatal mismatch

Wire names (`Message::message_type()`) and Path B/C use PascalCase
(`"GameInvite"`, `"Move"`, …). Path A writes snake_case. After a real
invite/move from the CLI, Path B reconstruct and Path C sync rebuild silently
skip those rows.

### Dependency fact that makes the lift feasible

`src/network/` already depends on `chess`, `messages`, `storage`, `crypto` —
**not** on `cli`. Introducing `src/game/` that depends only on
`chess` + `messages` + `storage` lets both `cli` and `network` consume it
with no cycle:

```text
chess / messages / storage
        ↑
       game          ← new shared domain
      ↗   ↖
    cli   network
```

---

## 3. Target architecture

```text
src/game/
  mod.rs              // module root + re-exports
  message_type.rs     // typed StoredMessageType ↔ wire PascalCase strings
  ops.rs              // GameOps, GameRecord, GameState, InvitationRecord, …
  moves.rs            // MoveProcessor (validate / prepare / commit / apply inbound)
  rebuild.rs          // single board rebuild from stored messages

src/cli/app.rs        // thin: validate input → domain → NetworkManager → display
src/cli/display.rs    // stays CLI-only; imports GameRecord from game
src/cli/validation.rs // stays CLI-only; wraps GameOps from game
src/network/handlers.rs
                      // store helpers + rebuild call into game::*; no cli imports

src/cli/game_ops.rs   // DELETE after move (optional short re-export shim during migration)
```

### Responsibilities after unify

| Concern | Owner |
|---------|--------|
| List / resolve games, reconstruct board + history, turn heuristic | `game::GameOps` |
| Parse/validate move against reconstructed board, build wire `Move`, persist | `game::MoveProcessor` |
| Typed `message_type` strings for chess rows | `game::StoredMessageType` |
| TCP send/retry, handshake, dial address | `cli::NetworkManager` + `App` |
| Pretty printing | `cli::display` |
| User-facing input UX (partial IDs, prompts) | `cli::InputValidator` |
| Inbound protocol validate → persist → reply | `network::handlers` (calling `game` for rebuild/store conventions) |

---

## 4. Scope

### In scope

1. Lift Path B into `src/game/` and wire CLI + server to it.
2. Normalize chess `message_type` storage to wire PascalCase.
3. Delete Path A’s duplicate list/board/history/move-reconstruct logic.
4. Fix invite **address vs peer ID** enough that `mate accept` can dial a TCP
   address (required for end-to-end invite → accept; called out as item-3
   territory in `old_plans/SERVER_CHESS_HANDLERS.md`).
5. Update tests that assert CLI-stored chess types or exercise `handle_*`
   against the new behavior.
6. Unify reconstruct parsing on `ChessMove::from_str_with_color` (handlers
   already do this; `GameOps::reconstruct_game_state` still uses
   `from_str`, which defaults castling to White — a latent bug).

### Out of scope (do not pull in)

- Real `Board::is_legal_move` / checkmate / stalemate (PRIORITIES item 4).
  Keep current `make_move` / stub legality behavior; do not pretend item 4
  is done.
- SAN parser so `mate move Nf3` works (PRIORITIES item 5). Coordinate +
  castling remain the engine-supported input. Document the intentional
  tightening vs today’s “any non-empty string” CLI accept.
- Reworking storage-layer unit tests that use `"move"` as an **opaque**
  fixture string unrelated to chess wire types
  (`tests/storage/storage_tests.rs`, etc.). Those are not chess runtime
  conventions; leave them alone.
- Auto-accept invites on the server (handlers correctly keep Pending).

---

## 5. Feasibility constraints — what must not break without a replacement

These are the gaps that a naïve “delete App logic / call `process_move`”
would break. Each has a required replacement in the step plan.

### 5.1 Networking stays in App (Path B has none)

`GameOps` / `MoveProcessor` never call `NetworkManager`. Today’s working
network half is:

- `handle_invite` → create game → `send_game_invite` → store on success
- `handle_accept` → `send_game_accept(opponent_peer_id, …)` → activate + store
- `handle_move` → `send_chess_move` → store on success

**Replacement:** App remains the async orchestrator. Domain code is sync DB +
board only. Never move TCP into `src/game/`.

### 5.2 `MoveProcessor::process_move` stores before any send

If App called `process_move` then sent, a failed dial would leave a local
`"Move"` row the opponent never got. If it only sent (old App) without domain
persist helpers, reconstruct stays wrong.

**Replacement:** split move processing into:

1. `prepare_move(game_id, notation) -> PreparedMove` — reconstruct, turn check,
   parse, `make_move` on a copy, compute `board_state_hash`, build
   `messages::chess::Move` — **no DB write**
2. `commit_move(game_id, &Move, sender_peer_id, signature)` — store
   `StoredMessageType::Move`
3. `apply_opponent_move` — keep for inbound/local apply of peer moves
   (already exists; wire when CLI learns of opponent moves / sync)

App order (preserves today’s “don’t persist if send fails” behavior):

```text
prepare_move → NetworkManager::send_chess_move → commit_move on Ok(ack)
```

Optional later: persist-outbox / idempotent retry; not required for item 3.

### 5.3 Invite / accept write APIs do not exist on Path B

`GameOps` only **reads** `"GameInvite"` in `list_pending_invitations`.
Create-game + store invite/accept live only in `App` and server handlers.

**Replacement:** add small domain helpers (or `GameOps` methods) used by both
CLI and handlers:

- `store_game_invite_message(...)`
- `store_game_accept_message(...)`
- `store_game_decline_message(...)` (server already stores declines)

Handlers’ private `store_*` functions become thin wrappers or are deleted in
favor of these helpers so PascalCase cannot drift again.

### 5.4 Address vs peer ID (accept dial-back)

Today:

- CLI invite stores **TCP address** in `Game.opponent_peer_id`
  (`app.rs` `create_game_with_id(..., address, ...)`).
- Server invite handler stores handshake **peer ID** in `opponent_peer_id`.
- `handle_accept` dials `game.opponent_peer_id` — works for CLI-created rows
  that stuffed an address there, fails for server-created rows that stored a
  peer ID.
- `Connection::handshake` already returns peer ID;
  `Connection::peer_identity()` exists — but `NetworkManager` does not
  surface it to App after send.

**Replacement (in scope):**

1. On invite (initiator): store dial address in `Game.metadata` (e.g.
   `{"dial_address":"host:port"}`); after successful connect/handshake, set
   `opponent_peer_id` to the remote peer ID (may require a small
   `Database::update_opponent_peer_id` or create-then-update).
2. On receive invite (server path already correct): keep peer ID in
   `opponent_peer_id`; if the inviter also needs dial-back later, that is the
   initiator’s metadata problem, not the server’s.
3. `handle_accept` / `handle_move` dial via
   `metadata.dial_address` if present, else treat `opponent_peer_id` as a
   dial string only when it looks like `host:port` (transition), never dial a
   raw crypto peer ID.
4. Extend `NetworkManager` send paths to return or record remote
   `peer_identity` so App can update the game row after invite.

Without this, unifying message types alone still leaves accept/move E2E
broken for the serve-side invitee.

### 5.5 Move notation tightening (intentional, with test updates)

Path A accepts any non-empty string. Path B parses via
`ChessMove::from_str_with_color` (coordinate + castling only). Wiring Path B
will reject `"e4"` / `"Nf3"` that today’s CLI tests sometimes use.

**Replacement:** update CLI integration expectations to coordinate forms
(`e2e4`, …). Help text / SAN remains item 5 — do not implement SAN here, but
do not keep the silent “accept anything” path either once `prepare_move` is
wired. Empty-move rejection stays (both paths already agree).

### 5.6 Display / validation / errors already expect Path B

`display_games_list`, `InputValidator`, `CliError::from(GameOpsError)` are
built for `GameRecord` / `GameOps`. They are unused by App today but are the
replacement UX — wire them; do not delete them when deleting Path A println
tables.

### 5.7 Server sync rebuild

`handlers::rebuild_board_from_stored_moves` must become a call into
`game::rebuild` (or `GameOps` free function). Behavior should stay
soft-reject-on-failure for `SyncRequest`. Do not change handler reply
policy in this work beyond sharing rebuild + store type helpers.

### 5.8 No legacy message-type compatibility

There are no users yet, so **do not** dual-read snake_case rows
(`"move"` / `"game_invite"` / …). Writers and readers use wire PascalCase
only via `StoredMessageType`.

CI already uses temp data dirs. Anyone with an old local `MATE_DATA_DIR`
DB can delete it; supporting mixed-case history is out of scope.

---

## 6. Step-by-step implementation

Land as one PR if reviewable, or split into the three milestones below.
Each step should leave `cargo test` green (or only the tests updated in that
step failing until fixed in the same commit).

### Milestone A — Shared domain + typed message types (no CLI behavior change yet)

#### Step A1. Add `src/game/` module skeleton (Done 2026/07/18)

1. Create `src/game/mod.rs` and register `pub mod game;` in `src/lib.rs`.
2. Initially move (or copy-then-delete) types/APIs from `src/cli/game_ops.rs`:
   - `GameOps`, `GameOpsError`, `GameRecord`, `GameState`, `InvitationRecord`,
     `GameStatistics`
   - `MoveProcessor`, `MoveProcessingError`, `MoveProcessingResult`,
     `MoveHistoryEntry`
3. Keep a temporary `src/cli/game_ops.rs` that `pub use crate::game::*;` so
   existing `cli_database` / validation / display imports keep compiling.
4. Point `cli/mod.rs`, `display.rs`, `validation.rs`, `error_handler.rs` at
   `crate::game` (directly or via the shim).

**Verify:** `cargo test --test integration` (or at least
`cli_database`, `unit/cli/*`) still passes. No `app.rs` changes yet.

#### Step A2. Add `StoredMessageType` (Done 2026/07/18)

1. In `src/game/message_type.rs`, define an enum covering chess-persisted
   variants used in DB rows:

   - `GameInvite`, `GameAccept`, `GameDecline`, `Move`
   - (Optionally `MoveAck` / sync types only if you ever persist them; today
     handlers do not need to.)

2. Implement `as_str(&self) -> &'static str` matching
   `Message::message_type()` PascalCase exactly.
3. Implement `from_str` / `TryFrom<&str>` for reads — PascalCase only;
   reject snake_case.
4. Add unit tests next to the type (round-trip; assert snake_case is rejected).

**Verify:** new unit tests pass. No production call sites switched yet.

#### Step A3. Extract `rebuild_board_from_stored_messages` (Done 2026/07/18)

1. Move logic from `handlers::rebuild_board_from_stored_moves` into
   `src/game/rebuild.rs`.
2. Use `StoredMessageType::Move` for filtering (PascalCase only).
3. Always parse with `ChessMove::from_str_with_color(..., board.active_color())`.
4. Change `GameOps::reconstruct_game_state` to call the same helper (delete
   its divergent `from_str` loop).
5. Change handlers’ sync path to call the shared helper; delete the private
   duplicate.

**Verify:**

- `tests/integration/server_chess_handlers.rs` (sync cases)
- `tests/integration/cli_database.rs` reconstruct cases
- Handlers unit tests under `tests/unit/network/handlers.rs`

#### Step A4. Split `MoveProcessor` prepare vs commit (Done 2026/07/18)

1. Refactor `process_move` into `prepare_move` + `commit_move` as in §5.2.
2. Keep `process_move` as a convenience for DB-only tests:
   `prepare_move` then `commit_move` (used by `cli_database`).
3. Ensure `commit_move` / invite store helpers use `StoredMessageType::*.as_str()`.
4. Align `apply_opponent_move` persistence with the same helpers.

**Verify:** `cli_database` move-processing tests still pass.

#### Step A5. Domain store helpers for invite / accept / decline (Done 2026/07/22)

1. Add helpers that serialize the wire payload and
   `store_message(..., StoredMessageType::….as_str(), ...)`.
2. Switch **server handlers only** to these helpers (behavior unchanged aside
   from shared code). CLI still writes lowercase until Milestone B.

**Verify:** server chess handler integration tests pass.

---

### Milestone B — Wire CLI to the domain (delete Path A chess logic)

#### Step B1. Read paths: `games`, `board`, `history`

Rewrite:

- `handle_games` → `GameOps::list_games` + `display_games_list`
- `handle_board` → resolve id via `InputValidator` / `GameOps::get_current_game`
  + `reconstruct_game_state` + `display_board` / `display_game_status`
- `handle_history` → `get_move_history_with_analysis` or reconstruct history +
  `display_move_history`

Delete Path A’s hand-rolled tables and `"move"` filters.

**Verify:** `tests/integration/cli_commands.rs` board/games/history cases;
update assertions if output format changes to `display_*` (prefer updating
tests to the better display, not preserving the old ASCII).

#### Step B2. Invite path (writes + address/peer metadata)

1. Validate address via `InputValidator` (already has peer-address checks).
2. `create_game_with_id` with metadata `dial_address`; placeholder or empty
   opponent peer id only if schema allows — otherwise store address temporarily
   but **overwrite** with handshake peer id immediately after successful send
   (see §5.4). Prefer: always end in “peer id in `opponent_peer_id`, address
   in metadata” after a successful invite.
3. `NetworkManager::send_game_invite` — extend to expose remote peer id
   (return `(Message, String)` or a small struct). Use
   `Connection::peer_identity()` from the connection used for the send.
4. On success: domain `store_game_invite_message` with PascalCase; update
   opponent peer id; keep dial address in metadata.
5. Preserve existing response UX (`GameAccept` / `GameDecline` / pending).

**Verify:** `cli_network` invite still creates a game; server invite tests
unchanged; add/adjust a test that metadata contains `dial_address` and
`opponent_peer_id` is not the raw address after a successful handshake
(can use the existing server harness).

#### Step B3. Accept path

1. Load game; require `Pending`.
2. Resolve dial target from metadata `dial_address` (fallback rules in §5.4).
3. `send_game_accept` → on success: `update_game_status(Active)`,
   `update_game_color` if needed, `store_game_accept_message` (PascalCase).
4. Delete lowercase `"game_accept"` write.

**Verify:** accept tests in `cli_commands` / `cli_network`; add a two-peer
case if not already present that invitee accept dials metadata address while
`opponent_peer_id` is a peer id (this is the regression lock for §5.4).

#### Step B4. Move path

1. Resolve game id (validator / current active game).
2. `prepare_move` — rejects empty and unparsable notation (coordinate/castling).
3. Dial via same address resolution as accept.
4. `send_chess_move` with prepared wire move (includes real post-move hash).
5. On ack: `commit_move`.
6. Display via existing helpers / concise success line.

Delete Path A’s move-count turn heuristic, initial-board hash, and
`"move"` store.

**Verify:** update `cli_commands` / `cli_network` / error-handling tests to
use `e2e4`-style moves; expect parse errors for `"e4"` / `"Nf3"` if those
fixtures exist. `cli_database` remains the deep move-processor suite.

#### Step B5. Error mapping

Route domain errors through `CliError` / `handle_chess_command_error` instead
of ad-hoc `anyhow::bail` where those helpers already encode better UX
(`NoCurrentGame`, `GameNotFound`, …).

**Verify:** `tests/integration/cli_error_handling.rs` still makes sense;
update command names / expected hints as needed.

---

### Milestone C — Remove shims and dead Path A code

#### Step C1. Delete the CLI shim

1. Remove `src/cli/game_ops.rs` re-export (or the whole file).
2. Ensure `cli/mod.rs` re-exports from `crate::game` only what the CLI
   public surface needs (or stop re-exporting and use `mate::game` in tests).

#### Step C2. Confirm no chess snake_case writers/readers remain

Grep for `"game_invite"`, `"game_accept"`, `"move"` in `src/cli` and chess
handler paths — should be gone (storage opaque fixtures in
`tests/storage/*` may remain; those are not chess wire types).

#### Step C3. Final deletion pass

Delete from `app.rs` any remaining unused imports (`Board` reconstruct
locals, lowercase stores, duplicate formatters). Confirm handlers have no
private store/rebuild duplicates.

#### Step C4. Docs touch-up

1. Update `PRIORITIES.md` item 3 to note completion (or strike-through) when
   the PR merges — only if you normally edit that file on completion.
2. Do **not** put step numbers or this filename into source comments
   (workspace rule: no planning-doc references in comments).

---

## 7. Suggested PR / commit slicing

| Slice | Contents | Review focus |
|-------|----------|--------------|
| PR1 / Milestone A | `src/game` lift, `StoredMessageType`, shared rebuild, prepare/commit split, handler store helpers | No user-visible CLI change; layering only |
| PR2 / Milestone B | App handlers wired; PascalCase CLI writes; dial metadata + NetworkManager peer id | Behavior + E2E invite/accept/move |
| PR3 / Milestone C | Shim removal, dead code | Diff should be mostly deletions |

Single-PR is fine if the branch stays focused and tests are updated in lockstep.

---

## 8. Test plan

### Automated (must stay green or be updated deliberately)

| Suite | Why |
|-------|-----|
| `tests/integration/cli_database.rs` | Primary Path B consumer; prepare/commit + reconstruct |
| `tests/integration/cli_commands.rs` | Live `App::handle_*` |
| `tests/integration/cli_network.rs` | Invite/accept/move still attempt network + DB side effects |
| `tests/integration/server_chess_handlers.rs` | PascalCase + sync rebuild |
| `tests/unit/network/handlers.rs` | Handler store/idempotency |
| `tests/unit/cli/display.rs` / `validation.rs` | Still compile against moved types |
| `tests/integration/cli_error_handling.rs` | Error UX after wiring `CliError` |

### Manual two-peer check (done criteria for item 3)

Use two `MATE_DATA_DIR`s / identities:

1. Peer B: `mate serve`
2. Peer A: `mate invite <B-addr>` → success; A’s DB has PascalCase `GameInvite`,
   `opponent_peer_id` = B’s peer id, metadata dial address set
3. Peer B: `mate games` shows Pending; `mate accept <game_id>` dials A and
   succeeds
4. Alternate `mate move e2e4` / `e7e5` (coordinate); both sides
   `mate board` / `mate history` show applied moves (not an empty initial
   board with a fake move count)
5. Confirm no process panics; send-and-wait does not time out on ack

### Explicit non-goals to re-check (should still be unfinished)

- Illegal moves may still apply if the engine stub allows them (item 4).
- `mate move Nf3` should fail closed (parse error), not silently store — until
  item 5.

---

## 9. Done criteria

Item 3 is done when:

1. There is a single chess domain module (`src/game/`) owned by neither
   “CLI-only” nor “server-only” code.
2. `App::handle_*` chess commands go through that module for state; Path A’s
   parallel reconstruct/list/history/move-hash logic is gone.
3. CLI and server both persist chess rows with wire PascalCase types via
   `StoredMessageType` (no remaining chess writers using snake_case).
4. Server sync and CLI board/history share one rebuild implementation.
5. Invite stores dial address in metadata and peer id in `opponent_peer_id`
   after handshake; accept/move dial the address, not the peer id string.
6. `MoveProcessor` support prepare → send → commit so failed sends do not
   orphan local moves.
7. Tests above pass; manual two-peer flow in §8 works.
8. `src/cli/game_ops.rs` is removed or is a deprecated one-liner re-export
   scheduled for immediate removal — not a second runtime.

---

## 10. Risks and open choices

| Risk | Mitigation |
|------|------------|
| Large diff / review fatigue | Prefer Milestone A → B → C PR split |
| `NetworkManager` API change for peer id | Small return-type change; update call sites in App only |
| Display output changes break brittle tests | Assert on domain state / key substrings, or golden the new `display_*` output once |
| Item 4/5 scope creep | Refuse SAN and real legality in this branch; coordinate-only is the contract |
| `prepare_move` vs server `handle_move` both persisting | Server remains source of truth for inbound; CLI commits only after ack of **outbound** moves; inbound opponent moves use `apply_opponent_move` or sync rebuild — do not double-store the same move from CLI and server on one DB |

---

## 11. Implementation order (summary)

```text
A1  src/game skeleton + cli shim
A2  StoredMessageType (PascalCase only)
A3  Shared rebuild; fix from_str → from_str_with_color
A4  prepare_move / commit_move split
A5  Domain invite/accept/decline store helpers; handlers switch
B1  CLI read paths → GameOps + display
B2  CLI invite + metadata dial_address + peer id from handshake
B3  CLI accept dials metadata address; PascalCase store
B4  CLI move: prepare → send → commit; coordinate notation in tests
B5  CliError mapping cleanup
C1  Delete cli/game_ops shim
C2  Grep: no chess snake_case left in cli/handlers
C3  Dead-code deletion in app.rs / handlers
C4  PRIORITIES note on completion (optional)
```

Prerequisite: items 1–2 already fixed (connection-layer panic; server chess
handlers). This plan assumes those are on the branch ancestry.
