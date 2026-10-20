//! Atomic, authoritative persistence for local and authenticated opponent moves.

use crate::chess::{ChessError, Color, Move as ChessMove};
use crate::game::{rebuild_chronology, ChronologyError};
use crate::messages::chess::{hash_board_state, validate_move_message, Move, ValidationError};
use crate::storage::{Database, GameStatus, PlayerColor, StorageError};

/// The caller supplies an authenticated remote identity or its own local identity.
#[derive(Debug, Clone, Copy)]
pub enum MoveActor<'a> {
    Opponent { peer_id: &'a str },
    Local { self_peer_id: &'a str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitOutcome {
    Committed,
    AlreadyCommitted,
}

#[derive(Debug, thiserror::Error)]
pub enum MoveCommitError {
    #[error("invalid move message: {0}")]
    InvalidMessage(#[source] ValidationError),
    #[error("game not found")]
    GameNotFound,
    #[error("target game {target} differs from payload game {payload}")]
    GameMismatch { target: String, payload: String },
    #[error("peer is not the game opponent")]
    NotOpponent,
    #[error("conflicting move at ply {ply}")]
    Conflict { ply: u32 },
    #[error("game is not active (status: {0:?})")]
    NotActive(GameStatus),
    #[error("missing predecessor: expected ply {expected}, got {got}")]
    Gap { expected: u32, got: u32 },
    #[error("wrong turn: expected {expected:?}")]
    WrongTurn { expected: Color },
    #[error("failed to apply move: {0}")]
    Apply(#[source] ChessError),
    #[error("board state hash mismatch")]
    HashMismatch,
    #[error("move sequence exhausted")]
    SequenceExhausted,
    #[error("stored move history is invalid: {0}")]
    CorruptHistory(#[source] ChronologyError),
    #[error("failed to persist move: {0}")]
    Storage(#[from] StorageError),
}

impl MoveCommitError {
    /// Stable rejection for GameDecline, excluding internal validation/storage details.
    pub fn decline_reason(&self) -> String {
        match self {
            Self::InvalidMessage(_) => "invalid move message".into(),
            Self::GameNotFound => "game not found".into(),
            Self::GameMismatch { .. } => "move game does not match target".into(),
            Self::NotOpponent => "peer is not the game opponent".into(),
            Self::Conflict { ply } => format!("move conflict at ply {ply}"),
            Self::NotActive(status) => format!("game is not active (status: {status:?})"),
            Self::Gap { expected, got } => format!("ply gap: expected {expected}, got {got}"),
            Self::WrongTurn { .. } => "not your turn".into(),
            Self::Apply(_) => "failed to apply move".into(),
            Self::HashMismatch => "board hash mismatch".into(),
            Self::SequenceExhausted => "move sequence exhausted".into(),
            Self::CorruptHistory(_) => "stored move history is invalid".into(),
            Self::Storage(StorageError::DatabaseLocked { .. }) => {
                "temporarily unavailable; retry".into()
            }
            Self::Storage(_) => "failed to persist move".into(),
        }
    }
}

/// Validate and commit under BEGIN IMMEDIATE, retrying the entire operation only
/// on lock contention (three total attempts, with 50 ms between attempts).
/// An exact retry is recognized before status checks or current-position replay.
pub fn commit_move(
    db: &Database,
    target_game_id: &str,
    mv: &Move,
    actor: MoveActor<'_>,
) -> Result<CommitOutcome, MoveCommitError> {
    for attempt in 0..3 {
        let result = db.with_immediate_transaction(|tx| {
            // Domain rejections occur before writes and retain their typed error.
            let validated = (|| {
                validate_move_message(mv).map_err(MoveCommitError::InvalidMessage)?;
                if mv.game_id != target_game_id {
                    return Err(MoveCommitError::GameMismatch {
                        target: target_game_id.into(),
                        payload: mv.game_id.clone(),
                    });
                }
                let game = Database::get_game_on(tx, target_game_id).map_err(|e| match e {
                    StorageError::GameNotFound { .. } => MoveCommitError::GameNotFound,
                    other => MoveCommitError::Storage(other),
                })?;
                let local_color = match game.my_color {
                    PlayerColor::White => Color::White,
                    PlayerColor::Black => Color::Black,
                };
                let (sender, signature, actor_color) = match actor {
                    MoveActor::Opponent { peer_id } => {
                        if game.opponent_peer_id != peer_id {
                            return Err(MoveCommitError::NotOpponent);
                        }
                        (peer_id, "remote", local_color.opposite())
                    }
                    MoveActor::Local { self_peer_id } => (self_peer_id, "local", local_color),
                };
                if let Some(row) = Database::get_move_at_ply_on(tx, target_game_id, mv.ply)? {
                    if serde_json::from_str::<Move>(&row.content).ok().as_ref() == Some(mv)
                        && row.sender_peer_id == sender
                    {
                        return Ok((CommitOutcome::AlreadyCommitted, sender, signature));
                    }
                    return Err(MoveCommitError::Conflict { ply: mv.ply });
                }
                if game.status != GameStatus::Active {
                    return Err(MoveCommitError::NotActive(game.status));
                }
                let rows = Database::get_moves_for_game_on(tx, target_game_id)?;
                let mut chronology = rebuild_chronology(target_game_id, &rows)
                    .map_err(MoveCommitError::CorruptHistory)?;
                let expected = chronology
                    .next_ply()
                    .map_err(|_| MoveCommitError::SequenceExhausted)?;
                if mv.ply > expected {
                    return Err(MoveCommitError::Gap {
                        expected,
                        got: mv.ply,
                    });
                }
                // A contiguous chronology already supplied every earlier ply.
                debug_assert_eq!(mv.ply, expected);
                let active = chronology.board.active_color();
                debug_assert_eq!(
                    active,
                    if mv.ply % 2 == 1 {
                        Color::White
                    } else {
                        Color::Black
                    }
                );
                if actor_color != active {
                    return Err(MoveCommitError::WrongTurn { expected: active });
                }
                let chess_move = ChessMove::from_str_with_color(&mv.chess_move, active)
                    .map_err(MoveCommitError::Apply)?;
                chronology
                    .board
                    .make_move(chess_move)
                    .map_err(MoveCommitError::Apply)?;
                if hash_board_state(&chronology.board) != mv.board_state_hash {
                    return Err(MoveCommitError::HashMismatch);
                }
                Ok((CommitOutcome::Committed, sender, signature))
            })();
            let (outcome, sender, signature) = match validated {
                Ok(value) => value,
                // Storage errors must reach the transaction helper for rollback
                // and lock classification, including errors during validation.
                Err(MoveCommitError::Storage(error)) => return Err(error),
                Err(domain) => return Ok(Err(domain)),
            };
            if outcome == CommitOutcome::Committed {
                Database::insert_move_on(tx, mv, signature, sender)?;
                Database::touch_game_on(tx, target_game_id)?;
                // Future game-end detection and updates belong inside this tx.
            }
            Ok(Ok(outcome))
        });
        match result {
            Err(StorageError::DatabaseLocked { .. }) if attempt < 2 => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(error) => return Err(MoveCommitError::Storage(error)),
            Ok(outcome) => return outcome,
        }
    }
    unreachable!("the final attempt always returns")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::Board;
    use crate::game::{GameOps, MoveProcessor};
    use crate::storage::models::GameResult;
    use crate::storage::Game;
    use std::sync::{Arc, Barrier};

    #[test]
    fn decline_reasons_are_stable_and_omit_internal_details() {
        let cases = [
            (
                MoveCommitError::InvalidMessage(ValidationError::InvalidMove("internal".into())),
                "invalid move message",
            ),
            (MoveCommitError::GameNotFound, "game not found"),
            (
                MoveCommitError::GameMismatch {
                    target: "internal".into(),
                    payload: "private".into(),
                },
                "move game does not match target",
            ),
            (
                MoveCommitError::NotOpponent,
                "peer is not the game opponent",
            ),
            (
                MoveCommitError::Conflict { ply: 5 },
                "move conflict at ply 5",
            ),
            (
                MoveCommitError::NotActive(GameStatus::Completed),
                "game is not active (status: Completed)",
            ),
            (
                MoveCommitError::Gap {
                    expected: 4,
                    got: 5,
                },
                "ply gap: expected 4, got 5",
            ),
            (
                MoveCommitError::WrongTurn {
                    expected: Color::White,
                },
                "not your turn",
            ),
            (
                MoveCommitError::Apply(ChessError::InvalidMove("internal".into())),
                "failed to apply move",
            ),
            (MoveCommitError::HashMismatch, "board hash mismatch"),
            (
                MoveCommitError::SequenceExhausted,
                "move sequence exhausted",
            ),
            (
                MoveCommitError::CorruptHistory(ChronologyError::MissingPly { row_id: 42 }),
                "stored move history is invalid",
            ),
            (
                MoveCommitError::Storage(StorageError::DatabaseLocked {
                    operation: "internal".into(),
                    timeout_ms: 1,
                }),
                "temporarily unavailable; retry",
            ),
            (
                MoveCommitError::Storage(StorageError::invalid_data("private", "internal")),
                "failed to persist move",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.decline_reason(), expected);
        }
    }

    fn databases() -> (tempfile::TempDir, Database, Database) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.sqlite");
        let first = Database::new_with_path("self", &path).unwrap();
        let second = Database::new_with_path("self", &path).unwrap();
        (dir, first, second)
    }

    fn game(db: &Database, color: PlayerColor) -> Game {
        let game = db
            .create_game_with_id(
                uuid::Uuid::new_v4().to_string(),
                "opponent".into(),
                color,
                None,
            )
            .unwrap();
        db.update_game_status(&game.id, GameStatus::Active).unwrap();
        // Pin activity far in the past so an accidental retry UPDATE is visible.
        db.with_connection(|conn| {
            conn.execute("UPDATE games SET updated_at = 1 WHERE id = ?1", [&game.id])?;
            Ok(())
        })
        .unwrap();
        game
    }

    fn next(db: &Database, id: &str, notation: &str) -> Move {
        let state = GameOps::new(db).reconstruct_game_state(id).unwrap();
        let mut board = state.board;
        let chess_move = ChessMove::from_str_with_color(notation, board.active_color()).unwrap();
        board.make_move(chess_move).unwrap();
        Move::new(
            id.into(),
            u32::try_from(state.move_history.len()).unwrap() + 1,
            notation.into(),
            hash_board_state(&board),
        )
    }

    fn local() -> MoveActor<'static> {
        MoveActor::Local {
            self_peer_id: "self",
        }
    }
    fn opponent() -> MoveActor<'static> {
        MoveActor::Opponent {
            peer_id: "opponent",
        }
    }

    fn snapshot(db: &Database, id: &str) -> serde_json::Value {
        serde_json::json!({
            "game": db.get_game(id).unwrap(),
            "messages": db.get_messages_for_game(id).unwrap(),
        })
    }

    fn unchanged(
        db: &Database,
        id: &str,
        f: impl FnOnce() -> Result<CommitOutcome, MoveCommitError>,
    ) -> MoveCommitError {
        let before = snapshot(db, id);
        let err = f().unwrap_err();
        assert_eq!(snapshot(db, id), before);
        err
    }

    #[test]
    fn retries_are_exact_historical_and_authenticated_even_after_completion() {
        for color in [PlayerColor::White, PlayerColor::Black] {
            let (_dir, db, _) = databases();
            let game = game(&db, color.clone());
            let actor = if color == PlayerColor::White {
                local()
            } else {
                opponent()
            };
            let first = next(&db, &game.id, "e2e4");
            assert_eq!(
                commit_move(&db, &game.id, &first, actor).unwrap(),
                CommitOutcome::Committed
            );
            let before = snapshot(&db, &game.id);
            assert_eq!(
                commit_move(&db, &game.id, &first, actor).unwrap(),
                CommitOutcome::AlreadyCommitted
            );
            assert_eq!(snapshot(&db, &game.id), before);
            for (i, notation) in ["e7e5", "g1f3", "g8f6"].iter().enumerate() {
                let mv = next(&db, &game.id, notation);
                let white = (i + 2) % 2 == 1;
                let actor = if white == (color == PlayerColor::White) {
                    local()
                } else {
                    opponent()
                };
                commit_move(&db, &game.id, &mv, actor).unwrap();
            }
            let before = snapshot(&db, &game.id);
            assert_eq!(
                commit_move(&db, &game.id, &first, actor).unwrap(),
                CommitOutcome::AlreadyCommitted
            );
            assert_eq!(snapshot(&db, &game.id), before);
            db.update_game_result(&game.id, GameResult::Draw).unwrap();
            let before = snapshot(&db, &game.id);
            assert_eq!(
                commit_move(&db, &game.id, &first, actor).unwrap(),
                CommitOutcome::AlreadyCommitted
            );
            assert_eq!(snapshot(&db, &game.id), before);
            assert!(matches!(
                unchanged(&db, &game.id, || commit_move(
                    &db,
                    &game.id,
                    &first,
                    MoveActor::Opponent {
                        peer_id: "stranger"
                    }
                )),
                MoveCommitError::NotOpponent
            ));
            assert!(matches!(
                unchanged(&db, &game.id, || commit_move(
                    &db,
                    &game.id,
                    &first,
                    MoveActor::Local {
                        self_peer_id: "other"
                    }
                )),
                MoveCommitError::Conflict { ply: 1 }
            ));
        }
    }

    #[test]
    fn rejects_invalid_payloads_and_new_moves_without_state_changes() {
        let (_dir, db, _) = databases();
        let game = game(&db, PlayerColor::White);
        let mv = next(&db, &game.id, "e2e4");
        let mut invalid = Vec::new();
        for i in 0..4 {
            let mut bad = mv.clone();
            match i {
                0 => bad.ply = 0,
                1 => bad.game_id = "not-a-uuid".into(),
                2 => bad.chess_move = "not-a-move".into(),
                _ => bad.board_state_hash = "bad".into(),
            }
            invalid.push(bad);
        }
        for bad in invalid {
            assert!(matches!(
                unchanged(&db, &game.id, || commit_move(&db, &game.id, &bad, local())),
                MoveCommitError::InvalidMessage(_)
            ));
        }
        let mut bad = mv.clone();
        bad.game_id = uuid::Uuid::new_v4().to_string();
        assert!(matches!(
            unchanged(&db, &game.id, || commit_move(&db, &game.id, &bad, local())),
            MoveCommitError::GameMismatch { .. }
        ));
        assert!(matches!(
            commit_move(&db, &bad.game_id, &bad, local()),
            Err(MoveCommitError::GameNotFound)
        ));
        bad = mv.clone();
        bad.ply = 3;
        assert!(matches!(
            unchanged(&db, &game.id, || commit_move(&db, &game.id, &bad, local())),
            MoveCommitError::Gap {
                expected: 1,
                got: 3
            }
        ));
        assert!(matches!(
            unchanged(&db, &game.id, || commit_move(
                &db,
                &game.id,
                &mv,
                opponent()
            )),
            MoveCommitError::WrongTurn {
                expected: Color::White
            }
        ));
        bad = mv.clone();
        bad.board_state_hash = "0".repeat(64);
        assert!(matches!(
            unchanged(&db, &game.id, || commit_move(&db, &game.id, &bad, local())),
            MoveCommitError::HashMismatch
        ));
        bad = mv.clone();
        bad.chess_move = "e3e4".into();
        assert!(matches!(
            unchanged(&db, &game.id, || commit_move(&db, &game.id, &bad, local())),
            MoveCommitError::Apply(_)
        ));
        for status in [
            GameStatus::Pending,
            GameStatus::Abandoned,
            GameStatus::Completed,
        ] {
            db.update_game_status(&game.id, status.clone()).unwrap();
            assert!(
                matches!(unchanged(&db, &game.id, || commit_move(&db, &game.id, &mv, local())), MoveCommitError::NotActive(found) if found == status)
            );
        }
    }

    #[test]
    fn stale_local_preparation_rechecks_ply_and_sender_before_current_position() {
        for same in [true, false] {
            let (_dir, db, other) = databases();
            let game = game(&db, PlayerColor::White);
            let processor = MoveProcessor::new(&db);
            let prepared = processor.prepare_move(&game.id, "e2e4").unwrap();
            let mv = next(&other, &game.id, if same { "e2e4" } else { "d2d4" });
            commit_move(&other, &game.id, &mv, local()).unwrap();
            let before = snapshot(&db, &game.id);
            let outcome = processor.commit_local_move(&prepared, "self");
            if same {
                assert_eq!(outcome.unwrap(), CommitOutcome::AlreadyCommitted);
            } else {
                assert!(matches!(outcome, Err(MoveCommitError::Conflict { ply: 1 })));
            }
            assert_eq!(snapshot(&db, &game.id), before);
            let mv = next(&other, &game.id, "e7e5");
            commit_move(&other, &game.id, &mv, opponent()).unwrap();
            let before = snapshot(&db, &game.id);
            let outcome = processor.commit_local_move(&prepared, "self");
            if same {
                assert_eq!(outcome.unwrap(), CommitOutcome::AlreadyCommitted);
            } else {
                assert!(matches!(outcome, Err(MoveCommitError::Conflict { ply: 1 })));
            }
            assert_eq!(snapshot(&db, &game.id), before);
            assert!(processor.process_move(&game.id, "e3e4").is_err());
        }
    }

    #[test]
    fn conflicting_notation_hash_sender_and_unparseable_stored_payload_fail_closed() {
        let (_dir, db, _) = databases();
        let game = game(&db, PlayerColor::Black);
        let mv = next(&db, &game.id, "e2e4");
        let alternate = next(&db, &game.id, "d2d4");
        commit_move(&db, &game.id, &mv, opponent()).unwrap();
        let mut hash = mv.clone();
        hash.board_state_hash = "0".repeat(64);
        for request in [&alternate, &hash] {
            assert!(matches!(
                unchanged(&db, &game.id, || commit_move(
                    &db,
                    &game.id,
                    request,
                    opponent()
                )),
                MoveCommitError::Conflict { ply: 1 }
            ));
        }
        db.with_connection(|conn| {
            conn.execute(
                "UPDATE messages SET sender_peer_id = 'self' WHERE game_id = ?1",
                [&game.id],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(matches!(
            unchanged(&db, &game.id, || commit_move(
                &db,
                &game.id,
                &mv,
                opponent()
            )),
            MoveCommitError::Conflict { ply: 1 }
        ));
        db.with_connection(|conn| {
            conn.execute(
                "UPDATE messages SET content = '{}' WHERE game_id = ?1",
                [&game.id],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(matches!(
            unchanged(&db, &game.id, || commit_move(
                &db,
                &game.id,
                &mv,
                opponent()
            )),
            MoveCommitError::Conflict { ply: 1 }
        ));
        let mut later = mv.clone();
        later.ply = 2;
        assert!(matches!(
            unchanged(&db, &game.id, || commit_move(
                &db,
                &game.id,
                &later,
                local()
            )),
            MoveCommitError::CorruptHistory(_)
        ));
    }

    #[test]
    fn corrupt_history_and_missing_earlier_ply_reject_new_commits() {
        for gap in [true, false] {
            let (_dir, db, _) = databases();
            let game = game(&db, PlayerColor::White);
            let mut stored = next(&db, &game.id, "e2e4");
            if gap {
                stored.ply = 2;
            } else {
                stored.board_state_hash = "0".repeat(64);
            }
            db.with_connection(|conn| Database::insert_move_on(conn, &stored, "local", "self"))
                .unwrap();
            let mut request = stored.clone();
            request.ply = if gap { 1 } else { 2 };
            assert!(matches!(
                unchanged(&db, &game.id, || commit_move(
                    &db,
                    &game.id,
                    &request,
                    local()
                )),
                MoveCommitError::CorruptHistory(_)
            ));
        }
    }

    #[test]
    fn failed_insert_or_game_update_rolls_back_for_both_actors_and_does_not_retry() {
        for color in [PlayerColor::White, PlayerColor::Black] {
            for event in [
                "UPDATE ON games",
                "INSERT ON messages WHEN NEW.message_type = 'Move'",
            ] {
                let (_dir, db, _) = databases();
                let game = game(&db, color.clone());
                let processor = MoveProcessor::new(&db);
                let prepared = if color == PlayerColor::White {
                    Some(processor.prepare_move(&game.id, "e2e4").unwrap())
                } else {
                    None
                };
                let mv = next(&db, &game.id, "e2e4");
                db.with_connection(|conn| {
                    conn.execute_batch(&format!("CREATE TRIGGER reject_write BEFORE {event} BEGIN SELECT RAISE(ABORT, 'forced failure'); END;"))?;
                    Ok(())
                }).unwrap();
                let (_, txns_before, errors_before, _) = db.get_connection_stats();
                let err = unchanged(&db, &game.id, || match &prepared {
                    Some(prepared) => processor.commit_local_move(prepared, "self"),
                    None => commit_move(&db, &game.id, &mv, opponent()),
                });
                assert!(matches!(
                    err,
                    MoveCommitError::Storage(StorageError::ConnectionFailed(_))
                ));
                let (_, txns_after, errors_after, _) = db.get_connection_stats();
                assert_eq!(txns_after, txns_before);
                assert_eq!(errors_after - errors_before, 1);
                db.with_connection(|conn| {
                    conn.execute_batch("DROP TRIGGER reject_write")?;
                    Ok(())
                })
                .unwrap();
                let actor = if color == PlayerColor::White {
                    local()
                } else {
                    opponent()
                };
                assert_eq!(
                    commit_move(&db, &game.id, &mv, actor).unwrap(),
                    CommitOutcome::Committed
                );
                let rows = db.get_moves_for_game(&game.id).unwrap();
                assert_eq!(rows.len(), 1);
                assert_eq!(serde_json::from_str::<Move>(&rows[0].content).unwrap(), mv);
                assert_eq!(
                    rows[0].signature,
                    if color == PlayerColor::White {
                        "local"
                    } else {
                        "remote"
                    }
                );
                assert!(db.get_game(&game.id).unwrap().updated_at > 1);
            }
        }
    }

    #[test]
    fn check_and_unique_failures_are_storage_errors_without_retry_or_partial_insert() {
        for duplicate in [false, true] {
            let (_dir, db, _) = databases();
            let game = game(&db, PlayerColor::White);
            let mv = next(&db, &game.id, "e2e4");
            let ply = if duplicate { "NEW.ply" } else { "0" };
            db.with_connection(|conn| {
                conn.execute_batch(&format!(
                    "CREATE TRIGGER inject_constraint BEFORE INSERT ON messages
                     WHEN NEW.message_type = 'Move'
                     BEGIN INSERT INTO messages (game_id, message_type, ply, content, signature, sender_peer_id, created_at)
                     VALUES (NEW.game_id, 'Move', {ply}, NEW.content, NEW.signature, NEW.sender_peer_id, NEW.created_at); END;"
                ))?;
                Ok(())
            }).unwrap();
            let errors = db.get_connection_stats().2;
            assert!(matches!(
                unchanged(&db, &game.id, || commit_move(&db, &game.id, &mv, local())),
                MoveCommitError::Storage(StorageError::ConstraintViolation { .. })
            ));
            assert_eq!(db.get_connection_stats().2 - errors, 1);
        }
    }

    #[test]
    fn constraint_bypassed_null_ply_is_corrupt_history_and_wrong_local_turn_is_rejected() {
        let (_dir, db, _) = databases();
        let game = game(&db, PlayerColor::Black);
        let mv = next(&db, &game.id, "e2e4");
        assert!(MoveProcessor::new(&db)
            .process_move(&game.id, "e2e4")
            .is_err());
        commit_move(&db, &game.id, &mv, opponent()).unwrap();
        let next = next(&db, &game.id, "e7e5");
        db.with_connection(|conn| {
            conn.execute_batch("PRAGMA ignore_check_constraints=ON")?;
            conn.execute(
                "UPDATE messages SET ply = NULL WHERE game_id = ?1",
                [&game.id],
            )?;
            conn.execute_batch("PRAGMA ignore_check_constraints=OFF")?;
            Ok(())
        })
        .unwrap();
        assert!(matches!(
            unchanged(&db, &game.id, || commit_move(&db, &game.id, &next, local())),
            MoveCommitError::CorruptHistory(ChronologyError::MissingPly { .. })
        ));
    }

    #[test]
    fn contention_has_three_attempts_then_can_converge_after_unlock() {
        let (_dir, db, other) = databases();
        let game = game(&db, PlayerColor::White);
        let mv = next(&db, &game.id, "e2e4");
        db.with_connection(|conn| {
            conn.busy_timeout(std::time::Duration::from_millis(1))?;
            Ok(())
        })
        .unwrap();
        other
            .with_immediate_transaction(|_tx| {
                let before = snapshot(&db, &game.id);
                let (_, _, errors, _) = db.get_connection_stats();
                assert!(matches!(
                    commit_move(&db, &game.id, &mv, local()),
                    Err(MoveCommitError::Storage(
                        StorageError::DatabaseLocked { .. }
                    ))
                ));
                assert_eq!(db.get_connection_stats().2 - errors, 3);
                assert_eq!(snapshot(&db, &game.id), before);
                Ok(())
            })
            .unwrap();
        assert_eq!(
            commit_move(&db, &game.id, &mv, local()).unwrap(),
            CommitOutcome::Committed
        );
    }

    #[test]
    fn concurrent_identical_and_conflicting_requests_serialize_on_shared_and_independent_handles() {
        for independent in [false, true] {
            for conflict in [false, true] {
                for _ in 0..20 {
                    let (_dir, db, other) = databases();
                    let game = game(&db, PlayerColor::Black);
                    let first = next(&db, &game.id, "e2e4");
                    let alternate = next(&db, &game.id, "d2d4");
                    let db = Arc::new(db);
                    let other = Arc::new(other);
                    let barrier = Arc::new(Barrier::new(4));
                    let results = std::thread::scope(|scope| {
                        let workers: Vec<_> = (0..4)
                            .map(|i| {
                                let handle = if independent && i % 2 == 1 {
                                    other.clone()
                                } else {
                                    db.clone()
                                };
                                let barrier = barrier.clone();
                                let mv = if conflict && i % 2 == 1 {
                                    alternate.clone()
                                } else {
                                    first.clone()
                                };
                                scope.spawn(move || {
                                    barrier.wait();
                                    commit_move(&handle, &mv.game_id, &mv, opponent())
                                })
                            })
                            .collect();
                        workers
                            .into_iter()
                            .map(|worker| worker.join().unwrap())
                            .collect::<Vec<_>>()
                    });
                    assert_eq!(
                        results
                            .iter()
                            .filter(|result| matches!(result, Ok(CommitOutcome::Committed)))
                            .count(),
                        1
                    );
                    for result in results {
                        assert!(matches!(
                            result,
                            Ok(_) | Err(MoveCommitError::Conflict { ply: 1 })
                        ));
                        if !conflict {
                            assert!(result.is_ok());
                        }
                    }
                    let rows = db.get_moves_for_game(&game.id).unwrap();
                    assert_eq!(rows.len(), 1);
                    rebuild_chronology(&game.id, &rows).unwrap();
                }
            }
        }
    }

    #[test]
    fn local_and_inbound_race_preserves_a_valid_chronology() {
        for _ in 0..20 {
            let (_dir, db, other) = databases();
            let game = game(&db, PlayerColor::White);
            let prepared = MoveProcessor::new(&db)
                .prepare_move(&game.id, "e2e4")
                .unwrap();
            let mut board = prepared.updated_board.clone();
            board
                .make_move(ChessMove::from_str_with_color("e7e5", Color::Black).unwrap())
                .unwrap();
            let inbound = Move::new(game.id.clone(), 2, "e7e5".into(), hash_board_state(&board));
            let barrier = Barrier::new(2);
            let remote_result = std::thread::scope(|scope| {
                let local = scope.spawn(|| {
                    barrier.wait();
                    MoveProcessor::new(&db)
                        .commit_local_move(&prepared, "self")
                        .unwrap()
                });
                barrier.wait();
                let result = commit_move(&other, &game.id, &inbound, opponent());
                assert_eq!(local.join().unwrap(), CommitOutcome::Committed);
                result
            });
            assert!(matches!(
                remote_result,
                Ok(CommitOutcome::Committed)
                    | Err(MoveCommitError::Gap {
                        expected: 1,
                        got: 2
                    })
            ));
            let rows = db.get_moves_for_game(&game.id).unwrap();
            rebuild_chronology(&game.id, &rows).unwrap();
            // A predecessor race resolves by retrying after local persistence.
            assert!(commit_move(&other, &game.id, &inbound, opponent()).is_ok());
            let expected = GameOps::new(&db)
                .reconstruct_game_state(&game.id)
                .unwrap()
                .board;
            assert_eq!(expected.to_fen(), board.to_fen());
            assert_ne!(expected.to_fen(), Board::new().to_fen());
        }
    }
}
