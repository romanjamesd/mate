# Review findings — `unify-chess-runtime-paths` vs `main`

Code review of this branch's diff (see `PR.md`). All findings below were
verified against the current source, not just inferred from the diff.
No code has been changed as a result of this review.

Findings tagged **[PRIORITIES 4/5]** below are ones that can reasonably be
addressed while doing items 4–5 in `PRIORITIES.md` (legal-move enforcement /
board reconstruction, and SAN-vs-coordinate notation). Untagged findings are
out of scope for that work.

## Correctness bugs

### 1. `handle_accept` ignores decline responses

**File:** `src/cli/app.rs:472`

After `send_game_accept(...)` returns `Ok(outcome)`, the code never inspects
`outcome.response` to check whether the remote actually returned
`Message::GameAccept` vs `Message::GameDecline`. The server soft-rejects
accepts with `GameDecline` when the game isn't `Pending` or the peer doesn't
match (`src/network/handlers.rs`).

**Failure scenario:** Peer B calls `mate accept <id>` on a game the server
declines (already abandoned, or opponent_peer_id mismatch). The server
replies with `GameDecline`, but B's CLI still prints "✓ Game accepted
successfully!", flips the local game to Active, and stores a GameAccept row —
while the remote never activated the game. Permanent desync, no error shown.

A secondary ordering defect compounds this: "✓ Game accepted successfully!"
is printed *before* the local writes, so even on a genuine acceptance a
failing `update_game_status` prints success and then returns an error.

**Recommended fix — typed per-command outcome:** Change the specialized
`NetworkManager::send_game_accept` API so a successfully completed transport
is not itself represented as a successful acceptance. Keep the existing
lower-level `SendOutcome` for the generic send/receive machinery, but classify
its response before returning from `send_game_accept`. The public result should
force the caller to distinguish the two valid application-level outcomes, for
example:

```rust
enum GameAcceptOutcome {
    Accepted {
        acknowledgement: GameAccept,
        peer_id: String,
    },
    Rejected {
        decline: GameDecline,
        peer_id: String,
    },
}
```

`send_game_accept` should return `Result<GameAcceptOutcome, ...>`. Put the
classification in a pure function (e.g. `classify_accept_response(&GameAccept,
SendOutcome) -> Result<GameAcceptOutcome, ChessProtocolError>` in
`src/cli/network_manager.rs`) so it is unit-testable without a network. It
classifies the received message using the request just sent:

- A `Message::GameAccept` is `Accepted` only when its `game_id` and
  `accepted_color` exactly match the request. This is a correlation check
  only: the server echoes the request verbatim in both its Pending and
  already-Active branches (`src/network/handlers.rs`), so a matching color does
  **not** prove the remote's stored acceptance agrees. Detecting a conflicting
  earlier acceptance is the server-side idempotency fix in finding 3.
- A `Message::GameDecline` is `Rejected` only when its `game_id` matches the
  request. Preserve its optional reason so the CLI can explain why the remote
  refused the accept.
- A mismatched game ID, mismatched accepted color, or any other message variant
  is a protocol error. It must never be treated as either acceptance or an
  ordinary remote rejection. `ChessProtocolError` has not been confirmed to
  have an `UnexpectedMessage` variant; if it lacks one, add an
  `UnexpectedResponse { expected, got }` (or similar) variant, which already
  maps to `CliError::Protocol`.
- Preserve the authenticated `peer_id` in both typed outcomes. Before applying
  either outcome, `handle_accept` must require
  `outcome.peer_id == game.opponent_peer_id` and return an error otherwise
  (including when `opponent_peer_id` is empty). No transitional/legacy
  handling is needed: the invitee's row is always created by
  `handle_game_invite` with the handshake-authenticated peer ID, and there is
  no deployed-data compatibility requirement.

Transport failure and remote rejection must remain different error paths. A
received `GameDecline` already arrives as `Ok` from `send_message_with_retry`
and therefore never reaches `store_pending_message`; the refactor must keep it
that way (a received decline or protocol error must not be queued). Note that
the pending-message queue lives in memory in a per-process `NetworkManager`, so
for a one-shot `mate accept` it has no practical retransmission effect; this is
a regression guard, not a behavioral feature.

`handle_accept` should then exhaustively match `GameAcceptOutcome` before
performing local writes:

- On validated `Accepted`, update the local status and color and store the
  `GameAccept` row, and only then print the success messages. Keep the
  existing three writes for now; making them atomic is finding 3's scope.
- On `Rejected`, return a user-facing error containing the decline reason and
  leave the local status, color, and message history unchanged.
- On a transport, identity, correlation, or unexpected-response error, return
  an error and likewise perform no local acceptance writes or success output.

Do not automatically mark the local game `Abandoned` merely because
`send_game_accept` received `GameDecline`. The server currently overloads
`GameDecline` as a generic negative acknowledgement for permanent state
conflicts, identity mismatches, validation failures, and potentially transient
database failures. Those cases do not share reliable terminal-state semantics.
Leave the local invitation `Pending` unless the protocol later adds a typed
terminal rejection or an explicit synchronization step determines the remote
state.

**Out of scope for this finding:** transactional accept finalization (one
conditional `Pending -> Active` + `my_color` + `GameAccept` operation, with an
explicit "remote may already be Active" error on failure) and strict
server-side idempotency for conflicting Active-state retries. Both belong to
finding 3 and should be implemented and tested there, not duplicated here.

Regression coverage should include:

- a pure response-classification test for matching accepts, matching declines,
  mismatched game IDs, mismatched colors, and unexpected message variants;
- a live CLI/server test where the remote game is already Abandoned;
- a live CLI/server test where the authenticated peer does not match;
- assertions that every rejection/error leaves the local game Pending, keeps
  its original color, stores no local `GameAccept`, and prints no success
  message;
- an assertion that the remote decline reason reaches the CLI error;
- an assertion that a remote rejection leaves
  `get_network_stats().total_pending_messages == 0`; and
- the existing happy-path test proving a matching authenticated
  `GameAccept` still activates both peers with opposite colors.

The same typed-outcome pattern should subsequently be applied to
`send_chess_move` (`MoveAck` versus `GameDecline`), which currently has the
same risk of treating any received response as permission to commit local
state.

### 2. Failed peer-id backfill only warns, bricks the game

**File:** `src/cli/app.rs:353`

`handle_invite` creates the game with a placeholder empty `opponent_peer_id`,
then backfills it via `database.update_opponent_peer_id` after a successful
handshake. If that DB write fails, the code only `eprintln!`s a warning and
continues as if the invite succeeded.

**Failure scenario:** Invite send succeeds but the follow-up
`update_opponent_peer_id` hits a transient SQLite error. `opponent_peer_id`
stays `""` permanently. Every subsequent inbound `GameAccept`/`Move`/sync
from the real opponent fails the `game.opponent_peer_id != peer_id` check in
`src/network/handlers.rs` and is soft-declined — the game is silently
bricked with no further error shown.

**Recommended fix — authenticate before creating the game:** Eliminate the
placeholder/backfill sequence. Split authenticated connection establishment
from invitation sending so `handle_invite` can first obtain the remote's
cryptographically authenticated peer ID, then create the local game with that
real peer ID and the dial address in metadata, and only then send the invite
over the same authenticated connection. If sending retries on a new
connection, require it to authenticate as the same peer before transmitting.
This ordering ensures a local persistence failure occurs before the remote
receives the invite, so a normal `Pending` game can never contain an empty
opponent identity.

Enforce that invariant in both the storage API and schema by rejecting blank
`opponent_peer_id` values for ordinary game rows. The network path should also
require `Connection::peer_identity()` after a successful handshake instead of
falling back to an identity taken from the response envelope, and it should
only report success after receiving a correlated `GameInvite` echo for the
same game ID.

As a short-term bridge, make the current backfill mandatory: transactionally
update the peer ID and store the local `GameInvite`, retry recoverable storage
errors with a bounded policy, and print success only after that transaction
commits. If finalization still fails after the remote acknowledged the invite,
return an explicit error explaining that delivery may have succeeded; merely
propagating the SQLite error still leaves an unrecoverable row.

Existing rows with an empty peer ID must be surfaced as incomplete rather than
as normal pending games. A repair flow may reconnect to the stored
`metadata.dial_address`, show or require confirmation of the authenticated
peer fingerprint, resend the same game ID using the server's idempotent
same-peer invite handling, and then conditionally persist that identity. Do
not automatically let the first inbound peer claim an empty row: that would
make knowledge of the game ID sufficient to replace cryptographic opponent
authentication.

Regression coverage should include:

- a local game-insert failure after handshake proving no invite reaches the
  remote;
- an invariant that successful invites never leave a blank opponent peer ID;
- rejection when an invite retry reconnects to a different peer identity;
- fault-injected finalization failure proving the CLI returns an error and
  does not print success;
- detection and explicit repair of legacy blank-ID rows; and
- rejection of attempts by an unrelated peer to claim an incomplete game.

### 3. Failed color update only warns, breaks turn parity

**File:** `src/cli/app.rs:479`

In `handle_accept`, `database.update_game_color(...)` failures are only
logged as a warning and not propagated, leaving the locally stored
`my_color` inconsistent with the color actually sent on the wire in the
`GameAccept` message.

**Failure scenario:** Invitee runs `mate accept <id> --color black` and the
wire accept (Black) is sent successfully, but the local `update_game_color`
call fails transiently. `my_color` stays at its provisional default (White).
`GameOps::is_your_turn` then compares `board.active_color()` against the
wrong `my_color` for the rest of the game, so `mate board`/`mate move`
disagree with reality indefinitely.

**Recommended fix — transactional accept finalization:** Replace the three
independent post-ack writes in `handle_accept` with one storage operation that
transactionally performs a conditional `Pending -> Active` transition, sets
the finalized `my_color`, and stores the correlated `GameAccept` row. Apply
the same operation to the inbound accept path in `network/handlers.rs`, which
currently has the symmetrical status-then-color-then-message partial-write
hazard. Success must be printed, and a server acknowledgement sent, only after
the transaction commits. On any failure, roll back all three changes so the
game never becomes Active with a provisional color or missing acceptance
history.

This fix must be paired with response validation from finding 1:
`send_game_accept` should authorize local finalization only after receiving a
`GameAccept` from the authenticated opponent whose game ID and accepted color
exactly match the request. A `GameDecline`, identity mismatch, correlation
mismatch, or unexpected message must leave the local game unchanged.

Retries also need stricter idempotency. For an already-Active game, the server
should echo success only when the authenticated peer, game ID, and accepted
color match the acceptance already stored for that game. The current
`ensure_accept_message_stored` check only tests whether any `GameAccept` row
exists, then echoes the new request; retrying with a different color can
therefore make the peers store incompatible assignments. A conflicting retry
should be rejected rather than treated as idempotent.

A local transaction cannot atomically commit the remote peer's database. If
the remote commits but the local finalization fails, return an explicit error
that the opponent may already have activated the game and keep the local row
Pending. An identical retry can then converge through the strict idempotent
server path. If recovery must survive process restarts without relying on the
user to repeat the same color, add a durable `Accepting` state or outbox that
stores the chosen color and exact `GameAccept` payload before transmission.
That provides stronger recovery but requires a schema migration, another
lifecycle state, and retry-worker/UI behavior; it is not necessary for the
near-term transactional fix.

Merely propagating the current `update_game_color` error is insufficient:
`update_game_status(Active)` has already committed and a second `accept` is
then rejected locally. Reordering the color update before the status update is
a useful emergency stopgap because a color failure leaves the row Pending,
but later status or message failures can still produce partial state. Making
pending `my_color` nullable (or separating proposed and finalized colors)
would also model the lifecycle more honestly and prevent provisional values
from appearing final, but it is complementary schema hardening rather than a
substitute for atomic finalization.

When implementing the storage operation, use `Database::with_transaction` but
do not call existing `Database` methods from inside its closure: they would
try to lock the same connection mutex again. Issue the update and insert
through the provided transaction connection or refactor connection-level SQL
helpers that both transactional and standalone operations can share.

Regression coverage should include:

- a fault-injected game-update failure proving status, color, and acceptance
  history all remain unchanged;
- an acceptance-message insert failure proving the preceding game update is
  rolled back;
- the equivalent rollback assertions for the inbound server handler;
- a successful Black acceptance where the invitee is Black and not initially
  on turn while the inviter is White and initially on turn;
- a remote commit followed by local transaction failure, then an identical
  retry that converges without duplicating the `GameAccept` row;
- rejection of an Active-state retry with a different accepted color;
- assertions that declines, mismatched responses, and wrong authenticated
  peers never run local finalization; and
- an assertion that success output occurs only after the local transaction
  commits.

### 4. `reply_to` uses stale config, not the real bind address

**File:** `src/cli/app.rs:342`

`GameInvite.reply_to` is populated from `self.config.default_bind_addr`, but
nothing updates that config field when the user runs `mate serve --bind
<addr>` with a non-default address — `Commands::Serve` binds directly
without touching `App::config`.

**Failure scenario:** User runs `mate serve --bind 0.0.0.0:9000` in one
shell and `mate invite <peer>` in another, with `default_bind_addr` still at
its default `127.0.0.1:8080`. The invite advertises the wrong address; the
invitee's later `mate accept` dials `127.0.0.1:8080` instead of the real
listening address and fails. This is the exact dial-back bug the branch set
out to fix (design doc §5.4) — still present for anyone using a custom bind
address.

**Recommended fix** — sync `serve`'s bind address into config, plus an
explicit override for advertise-vs-bind mismatches: `Commands::Serve`
(`src/main.rs:227`) should load `Config`, set `default_bind_addr` to the
`--bind` value, and call the existing `Config::save()` (`app.rs:95`) before
or while binding. `App::new()` already reloads config from disk on every CLI
invocation, so `invite` picks this up for free with no new plumbing. Guard
against wildcard hosts (`0.0.0.0`, `::`, `0`, `*`): a wildcard bind is valid
to listen on but is never itself a dialable address, so persisting it
verbatim as `reply_to` would trade today's stale-address bug for a
silently-undialable one. In that case, warn instead of overwriting
`default_bind_addr`, and require an explicit address.

That said, "the address I bound to" and "the address a remote peer should
dial to reach me" are genuinely different concepts once NAT, port-forwarding,
or multiple network interfaces are involved — config-sync alone only fixes
the same-host/single-NIC case the failure scenario describes. Add an explicit
`--reply-to <host:port>` flag to `mate invite` (defaulting to
`config.default_bind_addr` when omitted) so a user in one of those setups can
state the real reachable address directly, rather than relying on any
automatic inference from the bind address. Consider also having `mate serve`
print the address it just wrote as `default_bind_addr` on startup, so the
user notices if it's wrong.

Regression coverage should include:

- `mate serve --bind <non-default addr>` followed by `mate invite` in a
  fresh process asserting the sent `reply_to` matches the new bind address;
- a wildcard `--bind` value asserting `default_bind_addr` is left unchanged
  (or invite fails fast) rather than persisting an undialable address; and
- `mate invite --reply-to <addr>` asserting the flag overrides
  `default_bind_addr` regardless of what `serve` last persisted.

### 5. Message replay has no tiebreaker for same-second writes

**[PRIORITIES 4/5 — partial]** Item 4 can cover the companion requirement that
inbound Moves are validated against the reconstructed board (and rejected when
illegal / wrong-turn) so an ordering key does not merely sequence bad input.
The core fix here — `ply` on the wire/schema, unique `(game_id, ply)`, or the
narrow `ORDER BY id` interim — is **not** part of items 4–5 unless deliberately
bundled.

**File:** `src/storage/messages.rs:80`

`get_messages_for_game` (and its sibling queries) order strictly by
`created_at ASC`, a whole-second-resolution integer timestamp with no
secondary sort key (e.g. row id). `rebuild_board_from_stored_messages`
replays the returned slice with no additional ordering check.

**Failure scenario:** Two Move messages are stored within the same
wall-clock second (plausible with rapid `mate move` calls or a two-peer test
flow). SQLite has no guaranteed tie order, so the rows can come back
swapped; replaying them applies the second move first, producing an
illegal-move parse error or a silently wrong board for `mate
board`/`mate history`/`SyncRequest`.

**Recommended fix — make chess chronology explicit:** Add a `ply` (or
`move_number`) field to the wire `Move` payload and persist it in a dedicated
column on each stored Move row. Enforce a unique `(game_id, ply)` constraint
for Move messages and replay them with `ORDER BY ply ASC`. The sender should
set `ply` to the next expected half-move number; the receiver should require
that it is exactly the next expected value, validate the move against the
reconstructed board, verify the post-move board hash, and only then insert
and acknowledge it. The validation and insert must be one serialized or
transactional operation so two concurrent messages cannot both claim the
same next ply. Because the wire envelope signs the Move payload, including
`ply` there also prevents it from being altered independently in transit.

This makes ordering a chess-domain invariant instead of inferring it from a
wall clock. It detects gaps, duplicates, conflicting concurrent moves, and
out-of-order delivery rather than merely choosing a deterministic order for
them. It should be implemented together with routing inbound moves through
the existing validation path: `network/handlers.rs::handle_move` currently
stores and acknowledges a Move without checking its legality or whose turn
it is, so a sequence column alone would faithfully order invalid input.
`created_at` should remain display/audit metadata, not the replay key.

There is no deployed-data compatibility requirement, so the schema and wire
format can be changed directly rather than preserving the timestamp-based
contract. Generic message listings that mix invites, accepts, declines, and
moves should still have an explicit deterministic order; `ORDER BY id ASC`
is appropriate because `id` is an `AUTOINCREMENT` key representing local
insertion order. Recent-message queries should use `ORDER BY id DESC`.

If changing the wire protocol is intentionally deferred, the best narrow
fix is to replay and paginate by `id ASC` (and use `id DESC` for recent
messages). That is stronger than `ORDER BY created_at, id`: all production
timestamps are assigned locally at insertion, so the timestamp contributes
no authoritative chronology and can move backwards when the system clock is
adjusted. Adding `id` only as a secondary key is nevertheless a valid minimal
patch if preserving wall-clock-first ordering becomes a requirement.
Increasing timestamp precision is not a complete fix because collisions and
clock rollback remain possible.

Regression coverage should force several legal Move rows to share one
`created_at`, assert that replay follows their exact ply order, and verify
that duplicate, skipped, stale, and concurrently submitted ply values are
rejected without inserting a row. It should also prove that pagination
returns a complete, non-overlapping deterministic sequence and that an
out-of-order inbound Move is not acknowledged.

### 6. Two independent, disagreeing "whose turn" checks

**[PRIORITIES 4/5]** Reasonable during item 4: once board reconstruction is the
authority for `board` / `move` / `history`, fold `create_game_record`'s turn /
move-count / last-move summary onto that same rebuild and delete the raw-row
parity path. Item 5 helps only indirectly (fewer unparsable-notation failures);
it does not replace unifying the two turn calculations.

**File:** `src/game/ops.rs:256` (vs. `is_your_turn` at `:280`)

`create_game_record` derives `your_turn` by counting stored Move rows by
`message_type == "Move"` and checking count parity against `game.my_color`,
while `is_your_turn` (used by `reconstruct_game_state`) derives the same
fact from a full board rebuild via `board.active_color()` — two
separately-implemented calculations of the same fact.

**Failure scenario:** A stored Move row is type-tagged correctly but fails
to deserialize/parse (malformed JSON or unparsable notation). `is_your_turn`'s
path aborts reconstruction with an error, while `create_game_record`'s naive
count still includes that row toward parity, so `mate games` can show a
different turn indicator than `mate board` for the same game.

**Recommended fix:** Make the existing board reconstruction path the single
source of all chess-derived summary state. Factor a shared helper that calls
`rebuild_board_from_stored_messages` and returns the rebuilt board and
validated move history. Both `reconstruct_game_state` and
`create_game_record` should use that result: derive `your_turn` from
`board.active_color()`, `move_count` from the validated history length, and
`last_move` from the final validated history entry. Delete the independent
raw-row parity calculation. Although validated move-count parity is equivalent
for a standard starting position, the board is the better authority because
it also accommodates any future nonstandard initial position.

For the games list, preserve healthy rows when one game cannot be rebuilt, but
do not turn a reconstruction error into either player's turn. Replace the
boolean presentation state with an explicit status such as `Yours`,
`Opponent`, `NotApplicable`, or `Invalid`; render an invalid game's turn and
move count as unknown. Board display and move-processing paths should remain
strict and fail closed on the same reconstruction error. `get_current_game`
should select the most recent raw `Game` record directly rather than going
through enriched game-list records, so a corrupt history cannot prevent
default game selection.

This read-side fix should be paired with validating inbound moves before
insertion and updating the move plus any cached ply/state transactionally.
Typed serialization or a `json_valid` database constraint can prevent some
malformed content, but neither replaces position-aware parsing and move
validation, and neither by itself removes the duplicate turn calculations.

### 7. `apply_opponent_move` is dead code; inbound path is unaudited

**[PRIORITIES 4/5 — partial]** Item 4 is exactly the later move-parsing /
legality strengthening this finding anticipates (`is_legal_move`, real
application against the reconstructed board). Item 5 decides what notation that
parser accepts. The interim architecture work — route
`network/handlers.rs::handle_move` through `apply_opponent_move`, turn/hash
checks, transactional insert-before-`MoveAck`, typed sender-side `MoveAck` vs
`GameDecline` — is adjacent and reasonable to do in the same effort if
end-to-end enforcement is the goal, but is not strictly required to land the
engine stubs alone.

**File:** `src/game/moves.rs:216`

`MoveProcessor::apply_opponent_move` — the function meant to validate and
apply an inbound peer move against the reconstructed board — has zero
callers anywhere in `src/` or `tests/`. Inbound moves are instead persisted
directly by `src/network/handlers.rs::handle_move`, whose own doc comment
states it does not reconstruct the board or verify move legality.

**Failure scenario:** A remote sends a `Move` message for an Active game
with a matching opponent_peer_id; the server stores it and acks with
`MoveAck` without ever checking that the move is legal, that it's the
sender's turn, or that it parses against the current board — the one code
path built to do that validation is never invoked.

**Reasonable interim fix while full move parsing remains a separate PR:**
Enforce position consistency without claiming complete chess legality.
Refactor `apply_opponent_move` into the authoritative inbound move operation
and route `handle_move` through it. Require matching game IDs, Active status,
the authenticated opponent, the opponent's turn, successful application by
the current parser/board implementation, and a matching post-move board hash.
Handle an exact retry before the turn check so it is acknowledged without a
duplicate insert.

Reconstruction, validation, duplicate detection, and insertion should be one
serialized database transaction, and `MoveAck` should only be sent after it
commits. On the sending side, `send_chess_move` must distinguish a correlated
`MoveAck` from `GameDecline` or an unexpected response; only the former may
trigger the local move commit. Update handler fixtures to use the post-move
hash and add regression coverage for wrong-turn moves, bad hashes, idempotent
retries, rejection without insertion, and rejection without a sender-side
commit.

Describe this interim guarantee as turn/state/hash validation rather than
legal-move validation. The later move-parsing/legality PR can strengthen the
canonical board operation without changing the inbound architecture again.
If the explicit `ply` and schema uniqueness work from problem 5 is not being
implemented imminently, include it here so concurrent and retried moves have
an authoritative ordering key.

### 8. `handle_move` hard-fails after the move was already delivered

**File:** `src/cli/app.rs:546`

`handle_move` now propagates any error from `processor.commit_move(...)`
with `?` (non-zero exit, no success message), whereas the prior
implementation only warned on a failed local store and still reported
success — after the network send to the opponent had already succeeded.

**Failure scenario:** `mate move e2e4` sends successfully and the opponent
receives/acks it, but the subsequent local `commit_move` DB write fails
transiently. The CLI now exits with an error and prints no confirmation,
even though the move was actually delivered — a script or user checking the
exit code will believe the move failed when it didn't, and may resend or
diverge from the opponent's view of the game.

### 9. Default board/history resolution drops the completed-game fallback

**File:** `src/game/ops.rs:160`

`GameOps::get_current_game` (used by `resolve_read_game_id` when `mate
board`/`mate history` is run with no `--game-id`) only considers
Pending/Active games. The prior `app.rs` implementation fell back to
`games.first()` — any status, including Completed/Abandoned — when no
active game existed.

**Failure scenario:** A user whose only games are all Completed runs `mate
board` or `mate history` with no game id. Previously this displayed the
finished game; now it returns `NoCurrentGame` and shows nothing, requiring
the user to know and pass the game id explicitly.

## Architecture / type-safety gap

### 10. `StoredMessageType`'s safety stops at the storage boundary

**File:** `src/game/store.rs:24`

`StoredMessageType` is converted to a raw `String` via
`.as_str().to_string()` immediately before calling `Database::store_message`,
whose parameter is still an untyped `String`. Dozens of other call sites (in
tests and elsewhere) call `store_message` directly with hand-written string
literals, bypassing the enum entirely.

**Failure scenario:** A future call site (test or production code) writes a
chess message type as a raw string literal with a typo or wrong case (e.g.
`"move"`), and nothing at the type level catches it — the exact class of bug
this branch was created to fix (message_type casing drift) can recur outside
`src/game/`, since `Database::store_message` never requires
`StoredMessageType`.

## Noted but not prioritized

- `SendOutcome.peer_id` (`src/cli/network_manager.rs`) falls back to an
  unauthenticated `sender` string if `connection.peer_identity()` is `None`.
  Currently unreachable dead code — handshake is mandatory before a
  `Connection` can send — but a fragile pattern if that invariant ever
  changes, since the fallback would silently weaken an identity guarantee
  that `update_opponent_peer_id` depends on.
- `StoredMessageType::as_str()` (`src/game/message_type.rs`) and
  `Message::message_type()` (`src/messages/types.rs`) are two independently
  hardcoded string tables kept in sync only by a unit test. A new persisted
  message type added to one without the other compiles fine and silently
  drops that type from `StoredMessageType`-based filtering.
