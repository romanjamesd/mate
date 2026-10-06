//! Validate protocol chronology and rebuild the board from ply-ordered move rows.
//!
//! Moves must form a contiguous one-based half-move sequence. Row and payload
//! game IDs and plies must agree, and each hash must match the post-move board.
//! Insertion IDs and timestamps never determine chess order.

use crate::chess::{Board, ChessError, Move as ChessMove};
use crate::messages::chess::{hash_board_state, Move as MoveMessage};
use crate::storage::StoredMove;

/// Board, parsed history and payloads validated together, in protocol ply order.
#[derive(Debug, Clone)]
pub struct Chronology {
    pub board: Board,
    pub history: Vec<ChessMove>,
    pub moves: Vec<MoveMessage>,
}

impl Chronology {
    /// The next one-based half-move, or an error if the protocol range is exhausted.
    pub fn next_ply(&self) -> Result<u32, ChronologyError> {
        next_ply_after(self.moves.len())
    }
}

fn next_ply_after(count: usize) -> Result<u32, ChronologyError> {
    u32::try_from(count)
        .ok()
        .and_then(|count| count.checked_add(1))
        .ok_or(ChronologyError::SequenceExhausted)
}

/// A stored chronology is rejected at the first invalid row.
#[derive(Debug, thiserror::Error)]
pub enum ChronologyError {
    #[error("move sequence exhausted")]
    SequenceExhausted,
    #[error("missing ply in row {row_id}")]
    MissingPly { row_id: i64 },
    #[error("ply {ply} out of range in row {row_id}")]
    PlyOutOfRange { row_id: i64, ply: i64 },
    #[error("expected ply {expected}, found {found}")]
    Gap { expected: u32, found: u32 },
    #[error("failed to parse Move at ply {ply}: {reason}")]
    Serialization { ply: u32, reason: String },
    #[error("column ply {column} differs from payload ply {payload}")]
    PlyMismatch { column: u32, payload: u32 },
    #[error("game mismatch at ply {ply}: expected {expected}, found {found}")]
    GameMismatch {
        ply: u32,
        expected: String,
        found: String,
    },
    #[error("failed to apply ply {ply}: {error}")]
    Apply {
        ply: u32,
        #[source]
        error: ChessError,
    },
    #[error("board hash mismatch at ply {ply}")]
    HashMismatch { ply: u32 },
}

/// Validate every row in the supplied order, without sorting or repairing history.
/// Supply rows in ascending protocol ply order, starting at 1, as returned by
/// `crate::storage::Database::get_moves_for_game`. Gaps, duplicates, unsorted
/// input, and row/payload mismatches are errors; there is no timestamp or ID fallback.
/// Hashes must match the post-move board exactly. Application uses the existing
/// chess engine and does not guarantee full chess legality or king safety.
pub fn rebuild_chronology(
    game_id: &str,
    rows: &[StoredMove],
) -> Result<Chronology, ChronologyError> {
    let mut chronology = Chronology {
        board: Board::new(),
        history: Vec::new(),
        moves: Vec::new(),
    };
    for (index, row) in rows.iter().enumerate() {
        let raw = row
            .ply
            .ok_or(ChronologyError::MissingPly { row_id: row.id })?;
        let ply = u32::try_from(raw).ok().filter(|ply| *ply > 0).ok_or(
            ChronologyError::PlyOutOfRange {
                row_id: row.id,
                ply: raw,
            },
        )?;
        let expected = next_ply_after(index)?;
        if ply != expected {
            return Err(ChronologyError::Gap {
                expected,
                found: ply,
            });
        }
        let payload: MoveMessage =
            serde_json::from_str(&row.content).map_err(|error| ChronologyError::Serialization {
                ply,
                reason: error.to_string(),
            })?;
        if payload.ply != ply {
            return Err(ChronologyError::PlyMismatch {
                column: ply,
                payload: payload.ply,
            });
        }
        for found in [&payload.game_id, &row.game_id] {
            if found != game_id {
                return Err(ChronologyError::GameMismatch {
                    ply,
                    expected: game_id.to_string(),
                    found: found.clone(),
                });
            }
        }
        let chess_move =
            ChessMove::from_str_with_color(&payload.chess_move, chronology.board.active_color())
                .map_err(|error| ChronologyError::Apply { ply, error })?;
        chronology
            .board
            .make_move(chess_move)
            .map_err(|error| ChronologyError::Apply { ply, error })?;
        if hash_board_state(&chronology.board) != payload.board_state_hash {
            return Err(ChronologyError::HashMismatch { ply });
        }
        chronology.history.push(chess_move);
        chronology.moves.push(payload);
    }
    Ok(chronology)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::{Color, Position};
    use std::str::FromStr;

    const GAME: &str = "00000000-0000-0000-0000-000000000001";

    fn rows(notations: &[&str]) -> Vec<StoredMove> {
        let mut board = Board::new();
        notations
            .iter()
            .enumerate()
            .map(|(index, notation)| {
                let mv = ChessMove::from_str_with_color(notation, board.active_color()).unwrap();
                board.make_move(mv).unwrap();
                let ply = index as u32 + 1;
                StoredMove {
                    id: i64::from(ply),
                    game_id: GAME.into(),
                    ply: Some(i64::from(ply)),
                    content: serde_json::to_string(&MoveMessage::new(
                        GAME.into(),
                        ply,
                        (*notation).into(),
                        hash_board_state(&board),
                    ))
                    .unwrap(),
                    signature: "sig".into(),
                    sender_peer_id: "peer".into(),
                    created_at: 0,
                }
            })
            .collect()
    }

    fn change_payload(row: &mut StoredMove, change: impl FnOnce(&mut MoveMessage)) {
        let mut payload = serde_json::from_str(&row.content).unwrap();
        change(&mut payload);
        row.content = serde_json::to_string(&payload).unwrap();
    }

    #[test]
    fn empty_chronology() {
        let c = rebuild_chronology(GAME, &[]).unwrap();
        assert_eq!(c.board.to_fen(), Board::new().to_fen());
        assert!(c.history.is_empty());
        assert!(c.moves.is_empty());
        assert_eq!(c.next_ply().unwrap(), 1);
    }

    #[test]
    fn happy_path_ignores_ids_and_timestamps_and_castles_as_black() {
        let mut rows = rows(&[
            "e2e4", "e7e5", "g1f3", "g8f6", "f1c4", "f8c5", "d2d3", "O-O",
        ]);
        for (i, row) in rows.iter_mut().enumerate() {
            row.id = 100 - i as i64;
            row.created_at = 100 - i as i64;
        }
        let c = rebuild_chronology(GAME, &rows).unwrap();
        assert_eq!(c.next_ply().unwrap(), 9);
        assert_eq!(c.moves.last().unwrap().chess_move, "O-O");
        assert_eq!(
            c.history.last().unwrap().from,
            Position::from_str("e8").unwrap()
        );
        assert_eq!(
            c.history.last().unwrap().to,
            Position::from_str("g8").unwrap()
        );
        assert_eq!(c.board.active_color(), Color::White);
    }

    #[test]
    fn rejects_missing_and_out_of_range_ply() {
        let mut rows = rows(&["e2e4"]);
        rows[0].ply = None;
        assert!(matches!(
            rebuild_chronology(GAME, &rows),
            Err(ChronologyError::MissingPly { row_id: 1 })
        ));
        for raw in [-1, 0, i64::from(u32::MAX) + 1] {
            rows[0].ply = Some(raw);
            assert!(
                matches!(rebuild_chronology(GAME, &rows), Err(ChronologyError::PlyOutOfRange { row_id: 1, ply }) if ply == raw)
            );
        }
    }

    #[test]
    fn rejects_duplicate_gap_and_unsorted_rows() {
        let rows = rows(&["e2e4", "e7e5", "g1f3"]);
        for (indices, expected, found) in [
            (vec![0, 1, 1, 2], 3, 2),
            (vec![0, 2], 2, 3),
            (vec![1, 0], 1, 2),
        ] {
            let input: Vec<_> = indices.into_iter().map(|i| rows[i].clone()).collect();
            assert!(
                matches!(rebuild_chronology(GAME, &input), Err(ChronologyError::Gap { expected: e, found: f }) if e == expected && f == found)
            );
        }
    }

    #[test]
    fn rejects_payload_mismatch_and_wrong_games() {
        let mut input = rows(&["e2e4"]);
        change_payload(&mut input[0], |mv| mv.ply = 2);
        assert!(matches!(
            rebuild_chronology(GAME, &input),
            Err(ChronologyError::PlyMismatch {
                column: 1,
                payload: 2
            })
        ));
        for payload in [true, false] {
            let mut input = rows(&["e2e4"]);
            if payload {
                change_payload(&mut input[0], |mv| mv.game_id = "other".into());
            } else {
                input[0].game_id = "other".into();
            }
            assert!(
                matches!(rebuild_chronology(GAME, &input), Err(ChronologyError::GameMismatch { ply: 1, expected, found }) if expected == GAME && found == "other")
            );
        }
    }

    #[test]
    fn rejects_parse_apply_hash_and_json_failures() {
        for notation in ["not-a-move", "e3e4"] {
            let mut input = rows(&["e2e4"]);
            change_payload(&mut input[0], |mv| mv.chess_move = notation.into());
            assert!(matches!(
                rebuild_chronology(GAME, &input),
                Err(ChronologyError::Apply { ply: 1, .. })
            ));
        }
        let mut input = rows(&["e2e4"]);
        change_payload(&mut input[0], |mv| {
            mv.board_state_hash = mv.board_state_hash.to_uppercase()
        });
        assert!(matches!(
            rebuild_chronology(GAME, &input),
            Err(ChronologyError::HashMismatch { ply: 1 })
        ));
        input[0].content = "not json".into();
        assert!(matches!(
            rebuild_chronology(GAME, &input),
            Err(ChronologyError::Serialization { ply: 1, .. })
        ));
    }

    #[test]
    fn checked_sequence_boundary() {
        assert_eq!(next_ply_after(u32::MAX as usize - 1).unwrap(), u32::MAX);
        assert!(matches!(
            next_ply_after(u32::MAX as usize),
            Err(ChronologyError::SequenceExhausted)
        ));
        assert!(matches!(
            next_ply_after(usize::MAX),
            Err(ChronologyError::SequenceExhausted)
        ));
    }
}
