# Test Suite Audit — Redundant and Malformed Tests

Audit of `/tests/` across all unit, integration, performance, security, and storage suites.

---

## Malformed Tests

These tests have structural problems that make them incapable of catching regressions.

---

### 1. `test_is_legal_move_placeholder_behavior` (DELETED)

**File:** `tests/unit/chess/move_application.rs:520`

**Problem:** Always passes. The test explicitly exercises invalid chess moves (e.g., a rook moving diagonally across the entire board, a1→h8) and asserts they return `true` from `is_legal_move`. The implementation is a known placeholder that returns `true` unconditionally. The test is pinning placeholder behavior — it will continue passing even if move validation is never implemented, and it will *break* when real validation is added (which is the opposite of correct test behavior).

```rust
// line 549–554: asserts illegal move returns true
assert!(
    board.is_legal_move(*test_move),
    "Placeholder is_legal_move should return true for move: {}",
    test_move
);
```

---

### 2. `test_is_legal_move_placeholder_consistency` (DELETED)

**File:** `tests/unit/chess/move_application.rs:558`

**Problem:** Hardcodes the assertion that `is_legal_move` returns `true`. Line 590 reads `assert!(first_call, "Placeholder should return true")`. This test documents that the function is a stub; it does not verify correctness and will actively resist real implementation.

```rust
// line 590
assert!(first_call, "Placeholder should return true");
```

---

### 3. `test_board_state_validation_placeholder` (DELETED)

**File:** `tests/unit/chess/fen.rs:344`

**Problem:** Accepts both `Ok(_)` and `Err(ChessError::BoardStateError(_))` as valid outcomes (lines 367–380). Since the current implementation always returns `Ok` for syntactically valid FEN, the test always passes regardless of what the parser does. The match arm for `Ok` contains only a comment, so no assertion is ever evaluated. The test documents a gap in validation rather than verifying behavior.

```rust
match result {
    Ok(_) => {
        // Expected for current implementation - no board state validation yet
    }
    Err(ChessError::BoardStateError(_)) => {
        // If board state validation is added, this would be the correct error
    }
    Err(other) => {
        panic!("Unexpected error for FEN '{}': {:?}", fen, other);
    }
}
```

---

### 4. `test_future_board_state_validation_requirements` (DELETED)

**File:** `tests/unit/chess/fen.rs:384`

**Problem:** Same structural issue as `test_board_state_validation_placeholder`. Tests impossible chess positions (15 black queens, no kings, pawns on back ranks) and accepts both `Ok` and `BoardStateError` as valid results (lines 420–424). No assertion ever fails. This is documentation masquerading as a test.

---

## Redundant Tests

These tests duplicate coverage that already exists in another test, adding noise without adding signal.

---


### 5. JSON Serialization: `test_game_invite_json_serialization` / `test_game_accept_json_serialization` / `test_game_decline_json_*` vs. `test_json_support_chess_messages` (CONSOLIDATED)

**Files:**
- `tests/unit/messages/chess/types.rs` — individual JSON roundtrip tests per type (lines 71, 82, 137, 200, ~290, ~380, ~440, ~490)
- `tests/unit/messages/chess/types_enhanced.rs:186` — `test_json_support_chess_messages`

**Problem:** `types.rs` has individual JSON roundtrip tests for each message type (`GameInvite`, `GameAccept`, `GameDecline`, `Move`, `MoveAck`, `SyncRequest`, `SyncResponse`). `test_json_support_chess_messages` in `types_enhanced.rs` performs the same JSON roundtrip for every one of those types in a single loop, asserting `message_type()` and `get_game_id()` survive the roundtrip. The enhanced test covers all the same code paths. The individual tests in `types.rs` use `assert_eq!(original, deserialized)` (field equality) while the enhanced test uses `message_type()` and `get_game_id()` — but the assertion difference does not justify seven separate duplicate tests; the per-field assertions could be folded into the enhanced test if deeper coverage is wanted.

---

### 6. `test_game_invite_new_basic` vs. `test_game_invite_new_no_color_preference`

**File:** `tests/unit/messages/chess/types.rs:15` and `tests/unit/messages/chess/types.rs:42`

**Problem:** Both tests verify that `suggested_color == None` on a freshly constructed `GameInvite`. They use different constructors (`GameInvite::new(id, None)` vs. `GameInvite::new_no_color_preference(id)`), but assert the exact same resulting state. Testing that a convenience constructor delegates correctly is worthwhile, but neither test asserts anything about the constructor itself — only the resulting struct fields. The assertion sets are identical.

---

### 7. `test_game_invite_new_with_white_color` vs. `test_game_invite_new_with_color`

**File:** `tests/unit/messages/chess/types.rs:24` and `tests/unit/messages/chess/types.rs:51`

**Problem:** Both test that `GameInvite` ends up with `suggested_color == Some(Color::White)`. One uses `GameInvite::new(id, Some(Color::White))`, the other uses `GameInvite::new_with_color(id, Color::White)`. The resulting state and assertions are identical.

---

## Dead Code Suppression

These are not test failures, but they indicate tests that were partially abandoned and left in a suppressed state.

---

### 8. `create_test_config` helper

**File:** `tests/unit/cli/app_foundation.rs:75`

The function is annotated `#[allow(dead_code)]` and is not called anywhere in the file. If no test uses it, it should be removed. If it is intended to be used by future tests, it should not exist yet.

---

### 9. `MockGameState` unused fields

**File:** `tests/integration/chess_protocol_core.rs:31–38`

`game_id`, `white_player`, and `black_player` are all annotated `#[allow(dead_code)]`. These fields are stored at construction time (lines 44–50) but never read. If the mock doesn't need them, the fields and the constructor arguments should be removed.

---

## Summary

| # | Test / Symbol | File | Type |
|---|---|---|---|
| 1 | `test_is_legal_move_placeholder_behavior` | `tests/unit/chess/move_application.rs:520` | Malformed — pins placeholder, always passes |
| 2 | `test_is_legal_move_placeholder_consistency` | `tests/unit/chess/move_application.rs:558` | Malformed — hardcodes stub assertion |
| 3 | `test_board_state_validation_placeholder` | `tests/unit/chess/fen.rs:344` | Malformed — accepts any result, always passes |
| 4 | `test_future_board_state_validation_requirements` | `tests/unit/chess/fen.rs:384` | Malformed — accepts any result, always passes |
| 5 | Per-type JSON tests vs. `test_json_support_chess_messages` | `types.rs` + `types_enhanced.rs:186` | Redundant — seven individual tests duplicated by one loop |
| 6 | `test_game_invite_new_basic` vs. `test_game_invite_new_no_color_preference` | `tests/unit/messages/chess/types.rs:15,42` | Redundant — identical assertions, different constructors |
| 7 | `test_game_invite_new_with_white_color` vs. `test_game_invite_new_with_color` | `tests/unit/messages/chess/types.rs:24,51` | Redundant — identical assertions, different constructors |
| 8 | `create_test_config` | `tests/unit/cli/app_foundation.rs:75` | Dead code suppressed with `#[allow(dead_code)]` |
| 9 | `MockGameState.game_id/white_player/black_player` | `tests/integration/chess_protocol_core.rs:31–38` | Dead code suppressed with `#[allow(dead_code)]` |
