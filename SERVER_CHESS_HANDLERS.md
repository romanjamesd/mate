# Fix Plan: Server-side chess message handlers

Status: **Step 2 complete** — storage can create games with caller-supplied
IDs; handler implementation not started. This document is the result of
investigating `PRIORITIES.md` item 2 ("Implement server-side chess message
handlers") and lays out the concrete steps to implement them, plus the
verification to run at each step.

## 1. Problem (confirmed by reading the code)

`NetworkManager` (`src/cli/network_manager.rs`) already sends `GameInvite`,
`GameAccept`, and `Move` over TCP and **always waits for a reply** before
treating the send as successful. The listening peer (`mate serve` →
`Server::handle_connection_with_shutdown` in `src/network/server.rs:313-361`)
only has real handling for `Ping` (echo). Every other message type hits the
catch-all at lines 338–341:

```text
Received {} message from {} (no specific handler)
```

…and is silently dropped. No response is sent, so the client's
send-and-wait loop times out and retries.

On top of that:

- `Server` (`src/network/server.rs:123-127`) holds only `identity`,
  `listener`, and `wire_config` — **no `Database`**.
- `mate serve` (`src/main.rs:225-235`) loads identity and binds the server;
  it never opens SQLite. Persistence for games/messages currently lives only
  in the CLI `App` path.
- Protocol types, validators, and helpers for chess messages already exist
  (`src/messages/types.rs`, `src/messages/chess.rs`). Storage APIs exist
  (`src/storage/games.rs`, `src/storage/messages.rs`). What is missing is
  the wiring: Server → validate → persist → reply.

### Prerequisite: PRIORITIES item 1

Chess messages currently panic in the generic send/receive logging path
before they can reach the server's `match` (see `CONNECTION_LAYER_PANIC.md`
/ branch `fix-connection-layer-panic`). **Do not start this work until that
fix is merged** — otherwise handlers are unreachable under normal client
send paths, and any debug-level receive path still crashes.

Smoke check after item 1 lands: send a `GameInvite` to a running `mate serve`
with `RUST_LOG=mate=debug` and confirm the log shows
`"no specific handler"` (not a panic). That is the correct starting state
for this task.

## 2. What already exists (reuse; do not rebuild)

| Layer | Location | Ready for handlers? |
|-------|----------|---------------------|
| Message enum + constructors | `src/messages/types.rs` | Yes — `GameInvite` … `SyncResponse` |
| Validation | `Message::validate()`, `validate_*` in `chess.rs` | Yes |
| Reply helpers | `Message::new_move_ack`, `create_sync_response` | Yes |
| Client send + wait | `NetworkManager::send_*` | Yes — needs a real reply |
| Invite response UX | `app.rs` `handle_invite` | Treats `GameAccept` / `GameDecline` specially; any other reply = pending |
| Move ack expectation | `tests/integration/chess_protocol_core.rs` | Protocol expects `MoveAck` after `Move` |
| SQLite games/messages | `src/storage/*` | Mostly — see gaps below |
| Opponent-move apply | `MoveProcessor::apply_opponent_move` in `game_ops.rs` | Exists but unused by CLI; string-type mismatch with `app.rs` |

### Storage gaps that block correct invite handling

1. ~~`Database::create_game` always generates a new ID~~ **Fixed (Step 2):**
   `create_game_with_id` accepts the inviter's `game_id`.
2. ~~No API to update `my_color` after accept~~ **Fixed (Step 2):**
   `update_game_color`.
3. CLI invite today stores the **TCP address** in `opponent_peer_id`
   (`app.rs:476`). Handshake gives a cryptographic peer ID. Handlers must
   store peer ID (and keep address in metadata if needed for dial-back).

### Explicitly out of scope for this task

Per `PRIORITIES.md` ordering, defer to later items:

- Unifying `app.rs` vs `GameOps` as the CLI runtime (item 3) — handlers may
  reuse pieces of `GameOps`/`MoveProcessor`, but full CLI consolidation is
  separate.
- Real `is_legal_move` / checkmate / board reconstruction UX (item 4).
- SAN vs coordinate notation (item 5).

For this task, “apply a move” means: validate message shape, persist it
against the game, optionally check board hash with existing helpers, and
reply with `MoveAck`. Full chess-engine enforcement can remain stubbed until
item 4.

## 3. Request/response contract (decide before coding)

`NetworkManager` requires **some** successful receive after every send.
Recommended replies that match existing client/test behavior:

| Incoming | Server action | Reply |
|----------|---------------|-------|
| `Ping` | (existing) echo | same `Ping` |
| `GameInvite` | Validate; create **pending** game with invite's `game_id`; store invite; set opponent = handshake peer ID | Lightweight ack — **do not** auto-`GameAccept`. Echoing the invite or sending a small dedicated ack is fine; `handle_invite` already treats non-Accept/Decline as “pending.” Prefer echoing `GameInvite` or a documented ack so logs stay clear. |
| `GameAccept` | Validate; find pending game; set Active; store accept; align colors | Echo accept or lightweight ack (inviter's send-wait only needs *any* reply; special Accept handling is for when Accept arrives as the invite response) |
| `GameDecline` | Mark Abandoned; store | Echo / ack |
| `Move` | Validate; load game (must be Active); store move; optional hash check | **`MoveAck`** (`Message::new_move_ack(game_id, None)`) |
| `MoveAck` | Log / no-op (or clear pending if later wired) | No further reply (or ignore if unexpected as request) |
| `SyncRequest` | Rebuild FEN/history from stored messages | **`SyncResponse`** via `create_sync_response` |
| `SyncResponse` | Optional verify/store | Ack or ignore |

**Design rule:** never leave the client hanging. On validation/persistence
failure, still send an error-shaped reply if one exists, or at minimum close
cleanly after logging — but prefer a typed decline/reject path for invites
and a failed path that still unblocks send-wait (document the choice in the
implementation PR). Simplest v1: on hard failure, log + drop connection only
as last resort; for soft reject of invite, send `GameDecline`.

## 4. Step-by-step implementation

### Step 0 — Confirm prerequisite and baseline ✅ (2026-07-15)

1. Ensure item 1 (connection-layer panic) is on the branch you build on.
2. Run existing tests: `cargo test`.
3. Manual baseline: terminal A `mate serve --bind 127.0.0.1:8080`; from a
   small test or second process send `GameInvite`; confirm debug log
   `"no specific handler"` and no panic.

**Verified on `server-chess-handlers`:**

- Panic-fix commit `1a8dc06` is an ancestor; `Connection` send/receive
  logging uses `log_summary()`.
- `cargo test`: all suites green (718 lib/integration + 33 doctests, etc.).
- Manual: `mate serve` + `mate invite 127.0.0.1:18080` with
  `RUST_LOG=mate=debug` → server logged
  `Received GameInvite message from … (no specific handler)`; no panic.
  Client correctly hung waiting for a reply (expected until handlers exist).

### Step 1 — Open the database on the serve path ✅ (2026-07-15)

**Goal:** every `mate serve` process has the same SQLite identity DB the CLI
uses for that peer.

1. In `src/main.rs` `Commands::Serve`, open `Database` the same way `App`
   does (`Database::new(peer_id)` or `new_with_path` under the mate data
   dir).
2. Pass `Arc<Database>` into `Server` (new field + `bind` /
   `bind_with_config` constructors, or a `Server::bind_with_db` helper).
3. Clone `Arc<Database>` into each connection task so handlers can persist
   concurrently (`Database::with_connection` already serializes access).

**Verify:** serve starts; DB file exists/updates; existing Ping echo still
works.

### Step 2 — Storage: create game with caller-supplied ID ✅ (2026-07-15)

**Goal:** invitee can materialize the inviter's game ID locally.

1. Add `Database::create_game_with_id(id, opponent_peer_id, my_color,
   metadata)` (or optional `id: Option<String>` on `create_game`).
2. Reject duplicate IDs with a clear `StorageError`.
3. Optionally add `update_game_color` (or update metadata) for accept-time
   color finalization. (default behavior should eventually be to allow the challenged player to choose color on acceptance with an option to defer and allow the challenger to choose the color)
4. Unit tests in storage tests: create with fixed ID, get by ID, duplicate
   fails.

**Verify:** `cargo test` for storage module green.

**Implemented:**

- `Database::create_game_with_id` in `src/storage/games.rs`;
  `create_game` now delegates to it with a generated ID.
- Duplicate primary keys map to `StorageError::ConstraintViolation`
  (`games.id`); empty IDs map to `InvalidData`.
- `Database::update_game_color` for accept-time color finalization.
- Storage tests cover create-with-ID, duplicate rejection, empty ID, and
  color update (+ not-found in error tests).

### Step 3 — Handler scaffolding in the server loop

**Goal:** replace stringly `"Ping"` / catch-all with a typed match and a
single place for “validate → handle → reply.”

1. In `handle_connection_with_shutdown`, after successful receive, match on
   `&message` (enum variants), not only `message.message_type()`.
2. Call `message.validate()` (or the specific chess validator) before
   side effects.
3. Factor handlers into private methods or a small `src/network/handlers.rs`
   module to keep `server.rs` readable, e.g.
   `handle_game_invite(db, identity, peer_id, msg) -> Result<Message>`.
4. Always `connection.send_message(response).await` when a handler returns
   a reply; log and break on send failure (same as Ping echo today).
5. Keep Ping behavior unchanged.

**Verify:** unit/integration test that Ping still echoes; unknown/malformed
chess messages do not panic.

### Step 4 — `GameInvite` handler (unblocks network invite → local pending)

**Goal:** receiving peer gets a pending game row and the sender gets a reply.

1. Validate `GameInvite`.
2. Derive invitee's `my_color` from `suggested_color` (inverse of inviter's
   convention in `app.rs:463-469`, or store “unset” in metadata until
   accept — pick one and document it).
3. `create_game_with_id(invite.game_id, peer_id, my_color, metadata)` with
   status Pending. If game already exists for this ID+peer, treat as
   idempotent success.
4. `store_message` for the invite. Use **one** `message_type` string
   convention immediately — prefer the PascalCase wire names
   (`"GameInvite"`, `"Move"`, …) because `game_ops.rs` and most tests already
   expect those. (Full CLI string unification remains item 3; do not invent a
   third convention.)
5. Reply per contract in §3 (echo invite or lightweight ack — **not**
   auto-accept).

**Verify:**

- Integration test: client `send_game_invite` against real `Server` + temp
  DB → `Ok(response)` and DB contains pending game with invite's ID.
- Manual: `mate invite 127.0.0.1:PORT` against `mate serve` should print
  success instead of retry/timeout (once item 1 is fixed).

### Step 5 — `GameAccept` / `GameDecline` handlers

**Goal:** complete the invite lifecycle for the peer that receives accept or
decline over the wire.

1. **Accept:** validate; load game by ID; ensure Pending and opponent matches
   `peer_id`; set Active; store accept message; adjust color if needed;
   reply.
2. **Decline:** validate; set Abandoned; store; reply.
3. Reject accepts for unknown / wrong-peer / non-pending games with
   `GameDecline` or logged failure + safe reply.

**Verify:** integration test for invite → accept round-trip updating status
to Active on both sides' DBs (server side first; client-side status updates
already exist in `handle_invite` when Accept is the immediate response).

**Note:** human `mate accept` still dials out as a client. This step makes
the **receiving** side of that Accept message correct. Listing pending
network invites on the invitee depends on Step 4 having created the local
row.

### Step 6 — `Move` → `MoveAck` handler

**Goal:** moves sent by the client get acknowledged and persisted on the
receiver.

1. Validate move message (`validate_move_message`).
2. Load game; require Active; confirm sender is the opponent.
3. Persist via `store_message` (message type `"Move"`).
4. Optional v1: call into a thin wrapper around
   `MoveProcessor::apply_opponent_move` **only if** stored history types
   match; otherwise persist-first and skip reconstruction until item 3/4.
   Prefer persist + ack for a working network loop; add hash verification
   when reconstruction is reliable.
5. Reply with `Message::new_move_ack(game_id, None)`.

**Verify:**

- Integration test mirroring `chess_protocol_core.rs` move/ack sequence but
  through `Server` + `NetworkManager::send_chess_move`.
- Assert DB message row exists and client receives `MoveAck`.

### Step 7 — `SyncRequest` → `SyncResponse` (optional but cheap)

**Goal:** protocol completeness for reconnect/repair.

1. Load messages for `game_id`; rebuild board/history with existing helpers
   if feasible, else return starting FEN + stored move strings.
2. Reply with `create_sync_response`.
3. CLI sync command can remain future work; handler alone is enough for
   protocol tests.

**Verify:** unit/integration test that SyncRequest yields SyncResponse with
matching `game_id`.

### Step 8 — Error handling, logging, and idempotency

1. Duplicate invites/moves: idempotent success + same reply shape (avoid
   client retry storms creating duplicate rows — consider dedupe by
   game_id + payload hash or sequence).
2. Log with `message.log_summary()` / `get_game_id()` (never
   `get_nonce()`/`get_payload()` on chess variants).
3. Update `Commands::Serve` help text if it still says “echo server”
   (`src/cli/commands.rs`).

### Step 9 — Integration tests and manual E2E checklist

Add focused tests (new file e.g. `tests/integration/server_chess_handlers.rs`):

1. Ping still echoes with DB-enabled server.
2. GameInvite → reply + pending row with supplied ID.
3. GameAccept → Active.
4. Move → MoveAck + stored message.
5. SyncRequest → SyncResponse (if Step 7 done).
6. Malformed / wrong-peer messages rejected without panic.

Manual E2E (two peers, two data dirs / identities):

1. Peer B: `mate serve`
2. Peer A: `mate invite <B-addr>`
3. Peer B: `mate games` shows pending invite; `mate accept <game_id>`
4. Peer A: sees accept (or checks `mate games` Active)
5. Alternate `mate move` and confirm acks / history growth

(Steps 3–5 of the manual flow still depend on CLI unify / board work for a
polished UX, but the **network** half should succeed after this task.)

## 5. Suggested implementation order (summary)

```text
0. Prerequisite: connection-layer panic fixed
1. Wire Arc<Database> into Server + mate serve
2. create_game_with_id (+ optional color update)
3. Typed match + handler scaffolding + always reply
4. GameInvite handler
5. GameAccept / GameDecline handlers
6. Move → MoveAck handler
7. SyncRequest → SyncResponse (optional in same PR)
8. Idempotency / logging / Serve help text
9. Integration tests + manual two-peer check
```

Ship as one PR if small, or split: (A) DB wiring + invite/accept/decline,
(B) move ack + sync. Do not mix item 3/4/5 refactors into these PRs.

## 6. Risks and open decisions

1. **Invite immediate reply vs human accept** — Do not auto-accept; keep
   pending UX. Document which message is used as the invite ack.
2. **Address vs peer ID** — Store handshake `peer_id` as
   `opponent_peer_id`; put dial address in `metadata` if dial-back is
   required. May need a small follow-up in `handle_accept` (item 3 territory)
   so accept dials an address, not a peer ID string.
3. **message_type string case** — Prefer PascalCase in new server writes;
   leave bulk CLI migration to item 3.
4. **Legal moves stubbed** — Hash/legality can disagree with a real engine
   until item 4; network loop can still work.
5. **Concurrent connections** — Rely on `Database` connection locking;
   avoid holding locks across `.await` send points.
6. **MoveAck `move_id`** — Protocol field is optional; v1 can always send
   `None` unless you define a stable local sequence id.

## 7. Done criteria

This task is done when:

1. `mate serve` opens the peer database.
2. Incoming `GameInvite` / `GameAccept` / `GameDecline` / `Move` (and
   ideally `SyncRequest`) are validated, persisted appropriately, and
   answered so `NetworkManager` send-and-wait succeeds.
3. Integration tests cover invite + move ack against a real server.
4. No panics on chess variants in the server receive loop.
5. Items 3–5 from `PRIORITIES.md` remain explicitly unfinished (no scope
   creep into full GameOps CLI unify or legal-move engine work).
