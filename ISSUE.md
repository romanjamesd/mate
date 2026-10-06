Message replay currently depends on whole-second wall-clock timestamps. Give
generic message history an explicit insertion order, then make chess chronology
an enforced protocol and storage invariant using a required half-move sequence
number (`ply`). Move acknowledgement must mean that the authenticated opponent
committed the specific move being acknowledged.

## Problem

`src/storage/messages.rs` orders game messages, paginated messages, messages by
type, and messages by sender with `ORDER BY created_at ASC`. Recent messages use
`ORDER BY created_at DESC`. `Database::current_timestamp()` records whole seconds.
`src/game/rebuild.rs::rebuild_board_from_stored_messages` applies Move rows in
the supplied order without checking their sequence.

Several moves can share a timestamp during rapid CLI operations or integration
tests. SQL leaves the order of rows with equal sort keys unspecified. The current
`(game_id, created_at)` index often returns those rows in row-ID order, which
masks the defect, but that order is not part of the query's contract. A different
index or query plan can change it. Clock rollback is another failure: a later
move can receive an earlier timestamp and be replayed before its predecessors.

Incorrect replay can fail board reconstruction or produce an incorrect position,
affecting `mate board`, `mate history`, move preparation, and `SyncRequest`.

Insertion order alone also cannot establish agreement between peers. The inbound
handler currently stores moves without reconstructing the position or checking
whose turn it is. Its duplicate detection compares serialized content outside
the insert transaction. Local move preparation and commit are separated by a
network round trip, and commit does not revalidate the prepared position.
Concurrent operations, retries, and stale input therefore need explicit handling.

## Scope and delivery

Implement two independently reviewable stages:

1. Make existing message queries deterministic using local insertion IDs. This
   fixes timestamp collisions and clock rollback without a wire-format change.
2. Add explicit move sequencing and enforce it through preparation, transmission,
   acknowledgement, storage, and replay. Implement this together with an
   authoritative inbound move operation.

Stage 1 is useful on its own, but it does not provide the protocol guarantees of
stage 2. Keep `created_at` as display and audit metadata in both stages.

## Stage 1: deterministic message history

- Use `ORDER BY id ASC` in `get_messages_for_game`,
  `get_messages_for_game_paginated`, `get_messages_by_type`, and
  `get_messages_from_sender`.
- Use `ORDER BY id DESC` in `get_recent_messages`.
- Replace the game-message timestamp index with `(game_id, id)` so per-game
  insertion-order reads can use an appropriate index.
- Until stage 2 lands, board replay consumes the insertion-ordered messages.
- Update query documentation and tests to describe insertion order rather than
  asserting monotonic timestamps.

Do not keep the timestamp as the primary sort key or rely on increasing its
precision: either approach still permits clock rollback, and greater precision
does not guarantee uniqueness.

Offset pagination must return complete, non-overlapping pages for an unchanged
history. Ascending insertion IDs also keep newly appended rows after existing
rows. Do not claim snapshot consistency across pages if rows can be deleted
between reads. Cursor pagination or a pinned read snapshot is separate work if
that stronger contract is needed.

## Stage 2: explicit chess chronology

### Wire payload and storage invariants

Add a required `ply: u32` to the wire `Move` payload. Define it as a one-based
half-move sequence number: White's first move is 1, Black's first move is 2, and
White's next move is 3. Use the name `ply` consistently to distinguish it from a
chess full-move number or the fifty-move-rule halfmove clock.

The sender derives the next ply from validated committed history, not raw message
count or a timestamp. Include it in the signed Move payload. Do not supply a
deserialization default for missing ply values.

Add a dedicated nullable `ply` column to `messages`:

- Every Move row must have a positive integer ply within the wire type's range.
- Every non-Move row must have a null ply.
- Derive the column from the typed Move payload at insertion, rather than taking
  independent payload and sequence inputs from callers.
- Enforce uniqueness with a partial unique index on `(game_id, ply)` where
  `message_type = 'Move'`.
- Ensure generic storage entry points cannot insert a Move without enforcing
  these requirements.

For example, the database constraint should explicitly reject null Move plies:

```sql
CHECK (
    (message_type = 'Move'
     AND ply IS NOT NULL
     AND typeof(ply) = 'integer'
     AND ply BETWEEN 1 AND 4294967295)
    OR
    (message_type <> 'Move' AND ply IS NULL)
)
```

```sql
CREATE UNIQUE INDEX idx_messages_game_ply
ON messages(game_id, ply)
WHERE message_type = 'Move';
```

The unique index does not by itself enforce non-null values, contiguity, turn,
or position validity. Those require the constraint and application operation.

There is no deployed-data compatibility requirement. The wire contract can
require ply immediately. Choose an explicit development-database policy:
migrate and validate existing history if retaining it is required, or document
recreating disposable databases. Editing the initial migration alone does not
update a database already marked as schema version 1. Do not automatically delete
existing databases or silently reinterpret corrupt history.

### Authoritative move commit and retry behavior

Route inbound moves through one authoritative operation shared with local commit
validation. Refactor `MoveProcessor::apply_opponent_move` as needed; calling it
unchanged does not provide atomicity or complete validation.

For an inbound request:

1. Load the game and require the authenticated peer to be its stored opponent.
   Require the payload game ID to match the target game.
2. Look up an existing Move at the requested ply. If its typed payload and sender
   match exactly, return idempotent success without another insert. If that ply
   contains a different move, hash, or sender, reject the request as a conflict.
3. For a new move, require an Active game and reconstruct validated committed
   history. Require the requested ply to be exactly the next expected ply.
4. Require the opponent's turn, parse and apply the move against that position,
   and verify the supplied post-move board hash.
5. Insert the typed Move and any associated game-state updates atomically. Send
   success only after commit.

Handle identical historical retries before checking the current turn or next
expected ply. A retry can arrive after later moves have committed, or after the
game has completed; it must not change state or require replaying the old move
against the current position. Authentication still precedes retry recognition.

Reject skipped plies, stale requests without an identical stored move, conflicting
duplicates, and out-of-order new moves without insertion or a successful MoveAck.
An identical retry is acknowledged, not rejected as a duplicate.

Apply equivalent invariants to local commits. A previously prepared move is not
permission to insert against a changed history. Revalidate its ply, actor's turn,
application, and post-move hash against the current committed state. An identical
already-committed local move is idempotent; a conflicting or stale preparation
must fail without inserting or changing game state.

### Transaction and concurrency requirements

Keep the read of game state and history, retry lookup, validation, insert, and
associated updates in one database transaction. Use an immediate write
transaction (`BEGIN IMMEDIATE`) for this operation so writers serialize before
reading the state they will modify.

The existing `Database::with_transaction` uses a deferred transaction. Its mutex
only serializes callers sharing that Database instance; the server and CLI can
open independent connections to the same file. Provide an appropriate immediate
transaction helper and retain database uniqueness as a final safeguard.

Use connection-level read/write helpers inside the transaction. Calling public
Database methods that acquire the same mutex from its transaction closure can
deadlock. Handle lock contention without partial writes; any retry must rerun
the entire operation against freshly read state.

Do not hold a database transaction open while awaiting network I/O. Sender
preparation and commit necessarily have a gap, so commit must revalidate. If an
opponent's next move arrives before the acknowledged local move has committed,
it must not be inserted ahead of that missing predecessor. Report a recoverable
state mismatch rather than bypassing sequence validation.

### Transmission and acknowledgement

Preserve the complete prepared Move payload through `send_chess_move` and any
retries. That method currently reconstructs a payload from selected fields;
adding ply only to the preparation result would not be sufficient.

Make MoveAck identify the acknowledged request with required game ID, ply, and
a digest of the complete Move payload. Define a deterministic encoding for the
digest and include game ID, ply, move notation, and post-move hash. This prevents
an acknowledgement for a different candidate at the same ply authorizing commit.

Classify the response before authorizing a local commit:

- Accept only a MoveAck from the authenticated stored opponent whose game ID,
  ply, and request digest match the sent Move.
- Treat a correlated GameDecline as rejection and preserve its reason.
- Treat an unexpected variant, identity mismatch, or uncorrelated response as an
  error.
- A received rejection or invalid response must not cause a local Move insert,
  success output, or automatic retransmission as though transport had failed.

Acknowledgements for identical retries must identify the same stored request.

### Replay and query separation

Add a move-history query that filters Move rows and uses `ORDER BY ply ASC`.
Use it for the shared board reconstruction path, including board/history display,
move preparation, and synchronization responses. Generic histories containing
invites, accepts, declines, and moves continue to use insertion ID order.

During reconstruction, explicitly require:

- Plies form the contiguous sequence `1..N` with no nulls or duplicates.
- Each row's dedicated ply matches the deserialized Move payload.
- The payload game ID matches the row and requested game.
- Each move successfully applies against the reconstructed position, and its
  post-move hash matches the resulting board.

Fail closed on invalid history. Sorting by ply alone must not conceal gaps or
payload/column disagreement. Do not fall back to timestamp order for invalid rows.

### Validation limits and recovery boundary

The current parser and `Board::make_move` do not enforce complete chess legality;
`Board::is_legal_move` is a stub. This issue establishes sequence, actor/turn,
application, and hash validation using the available board implementation. Full
piece-movement and king-safety enforcement must be implemented separately or
explicitly included before claiming legal-move validation.

A local transaction cannot commit both peers' databases. If the opponent commits
but acknowledgement delivery or local commit fails, report uncertain delivery or
remote success with local persistence failure explicitly. Preserve the committed
history and permit an identical request to converge through idempotent handling.
Do not print unconditional success or infer that a transport error means the
opponent did not receive the move.

Durable outbound intent, automatic restart recovery, and synchronization repair
are separate work. Ply and request-correlated acknowledgements support those
features but do not provide them by themselves.

## Acceptance criteria and regression coverage

- Several known move payloads sharing exactly one timestamp are returned in
  insertion order in stage 1 and replayed in exact ply order in stage 2.
- Decreasing timestamps do not change replay order. Generic histories and
  recent-message results follow their documented ID ordering.
- Pagination concatenates to the full deterministic history without omissions
  or duplicates for an unchanged dataset; tests compare row identities and order,
  not only page lengths or timestamps.
- Schema constraints reject Move rows with missing, invalid, or duplicate plies
  and non-Move rows with a ply.
- Preparation, wire serialization, transmission, retry, and storage preserve the
  same required ply. Missing wire ply fails validation/deserialization.
- Identical retries are acknowledged without insertion, including historical
  retries after subsequent moves and retries after game completion.
- Conflicting duplicates, gaps, out-of-order new moves, wrong peers, wrong turns,
  mismatched game IDs, and bad hashes are rejected without state changes or a
  successful MoveAck.
- Concurrent identical submissions produce one row and idempotent outcomes;
  concurrent conflicting submissions produce at most one committed candidate.
  Exercise independent Database connections to the same file as well as callers
  sharing one instance.
- A stale prepared local move cannot commit after history changes. Wrong or
  uncorrelated acknowledgements and remote rejection cannot authorize local
  insertion or success output.
- Forced persistence failures roll back the move and associated game updates,
  and no inbound success acknowledgement is produced before commit.
- Reconstruction rejects a missing ply, duplicate ply, payload/column mismatch,
  wrong game ID, failed application, or incorrect post-move hash.
- Board, history, preparation, and SyncRequest use the same validated chronology
  and produce the expected board from a known sequence.
- The selected database-upgrade policy is verified for fresh and existing
  development databases; incompatible histories are never silently accepted.