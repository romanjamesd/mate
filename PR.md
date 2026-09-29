# Persist invitations atomically and recover safely after delivery failures

## Summary

Previously, sending an invitation created a game with a blank opponent peer ID,
then filled in the identity and stored the invitation after the network operation.
Those writes could fail after the remote had already persisted the invitation,
leaving an unusable local game. A lost acknowledgement could also mark the local
game abandoned even though the remote had received it.

Authenticate the opponent first, then commit the pending game and its original
`GameInvite` together before transmitting the invitation. Preserve local state
when delivery is uncertain and provide an explicit retry command that works after
a process restart.

## Changes

- Add transactional storage operations for outbound and inbound invitations.
  Persistence failures roll back both the game and invitation message; inbound
  acknowledgements are returned only after commit.
- Remove post-send peer ID backfilling and reject blank or whitespace-only
  opponent identities in game creation and identity updates.
- Require an acknowledgement matching the complete invitation payload, or a
  decline matching its game ID. Declines and protocol errors report failure
  without printing success or overwriting local game status, including a
  concurrent acceptance that has already activated the game.
- Add `mate retry-invite <game-id>` for pending outbound invitations. Reload the
  original game ID, payload, dial address, and opponent identity; authenticate
  every connection against that opponent before transmitting. Invitations no
  longer enter the in-memory pending-message queue.
- Acknowledge identical inbound retries without duplicate messages; reject
  conflicting payloads, different peers, and games that are no longer pending.
- Document recovery in the README and expand the invitation failure scenarios
  and regression requirements in `PROBLEMS.md`.

## Validation

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- `cargo test --test game_invite --test game_accept` — 14 tests passed.
- `make test-ci-safe` — 852 tests passed, including documentation tests.

Regression coverage includes rollback on injected persistence failures, no
transmission before local commit, exact response correlation, peer replacement
during reconnects, retries after restart without duplicates, lost acknowledgements,
and acceptance racing with delivery failure.

## Compatibility

No wire-format or database-schema migration is introduced. Legacy invitations
without a unique original outbound payload or a usable opponent identity are not
repaired by this change. Declines preserve local state because the current
free-text response does not reliably distinguish terminal rejection from a
transient failure.
