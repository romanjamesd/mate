# Unify chess runtime paths and improve peer communication

## Summary

Consolidates the CLI and server chess logic into a shared game module, replacing
duplicated implementations and inconsistent stored message formats. Board views,
move history, and synchronization now reconstruct positions through the same
logic, and outgoing moves use the resulting board state before being persisted
after a successful send.

## Changes

- Standardize chess message storage and route CLI game commands and server
  handlers through shared game operations, persistence, and board reconstruction.
- Fix invite and move routing by advertising a dial-back address and storing
  network addresses separately from authenticated peer identities.
- Require a matching acceptance acknowledgement from the expected peer before
  activating a game. Report declines, invalid responses, and persistence failures
  without announcing success.
- Improve CLI validation and error messages, including coordinate/castling move
  input and game selection for board and history commands.
- Keep echo results visible with quiet logging, show the connected peer address,
  and retry pending messages after send or receive failures.
- Cancel connection tasks when the server stops and strengthen reconnection tests.
- Update planning, known-issue, and test-failure documentation.

## Validation

Adds and updates regression coverage for shared game state, message storage,
invitation acceptance, CLI behavior, and connection recovery. `TEST_FAILURES.md`
records a successful `make test-ci-safe` run (844 tests, including documentation
tests), formatting, and Clippy checks after the fixes.

Full chess legality/checkmate detection, SAN input, and migration of legacy
snake_case message rows remain outside this change.
