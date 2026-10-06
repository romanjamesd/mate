//! Rebuild board state from stored messages in the order supplied.
//!
//! Callers pass rows in local insertion order from
//! [`Database::get_messages_for_game`](crate::storage::Database::get_messages_for_game).
//!
//! Applies `"Move"` rows only, without verifying stored board-state hashes
//! (those may be incorrect until clients send post-move hashes).

use crate::chess::{Board, ChessError, Move as ChessMove};
use crate::messages::chess::Move as MoveMessage;
use crate::storage::Message as StoredMessage;

use super::StoredMessageType;

/// Errors while rebuilding a board from stored messages.
#[derive(Debug)]
pub enum RebuildError {
    /// Failed to deserialize a Move message body.
    Serialization(String),
    /// Chess parse or apply failure.
    Chess(ChessError),
}

impl std::fmt::Display for RebuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RebuildError::Serialization(e) => write!(f, "failed to parse Move message: {e}"),
            RebuildError::Chess(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RebuildError {}

impl From<ChessError> for RebuildError {
    fn from(err: ChessError) -> Self {
        RebuildError::Chess(err)
    }
}

/// Rebuild board and move history from stored messages.
///
/// Rows are applied in slice order; this function does not sort by `id` or
/// `created_at`, or validate protocol sequencing. Explicit move sequencing
/// belongs to Stage 2 (`ply`).
///
/// Skips non-`Move` rows. Parses each move with the board's active color so
/// castling resolves correctly for both sides. Fails on the first bad row.
pub fn rebuild_board_from_stored_messages(
    messages: &[StoredMessage],
) -> Result<(Board, Vec<ChessMove>), RebuildError> {
    let mut board = Board::new();
    let mut history = Vec::new();

    for message in messages {
        if message.message_type != StoredMessageType::Move.as_str() {
            continue;
        }

        let move_msg: MoveMessage = serde_json::from_str(&message.content)
            .map_err(|e| RebuildError::Serialization(e.to_string()))?;

        let chess_move =
            ChessMove::from_str_with_color(&move_msg.chess_move, board.active_color())?;

        board.make_move(chess_move)?;
        history.push(chess_move);
    }

    Ok((board, history))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::{Color, Position};
    use crate::game::StoredMessageType;
    use std::str::FromStr;

    fn stored_message(message_type: &str, content: &str) -> StoredMessage {
        StoredMessage {
            id: None,
            game_id: "00000000-0000-0000-0000-000000000001".to_string(),
            message_type: message_type.to_string(),
            content: content.to_string(),
            signature: "sig".to_string(),
            sender_peer_id: "peer".to_string(),
            created_at: 0,
        }
    }

    fn move_content(chess_move: &str) -> String {
        serde_json::json!({
            "game_id": "00000000-0000-0000-0000-000000000001",
            "chess_move": chess_move,
            "board_state_hash": "hash",
        })
        .to_string()
    }

    #[test]
    fn empty_messages_yield_starting_board() {
        let (board, history) = rebuild_board_from_stored_messages(&[]).expect("empty ok");
        assert!(history.is_empty());
        assert_eq!(board.active_color(), Color::White);
        assert_eq!(board.to_fen(), Board::new().to_fen());
    }

    #[test]
    fn non_move_rows_are_skipped() {
        let messages = vec![
            stored_message(
                StoredMessageType::GameInvite.as_str(),
                r#"{"game_id":"00000000-0000-0000-0000-000000000001"}"#,
            ),
            stored_message(
                StoredMessageType::GameAccept.as_str(),
                r#"{"game_id":"00000000-0000-0000-0000-000000000001","accepted_color":"White"}"#,
            ),
            stored_message("move", &move_content("e2e4")), // snake_case ignored
        ];

        let (board, history) =
            rebuild_board_from_stored_messages(&messages).expect("skip non-Move");
        assert!(history.is_empty());
        assert_eq!(board.to_fen(), Board::new().to_fen());
    }

    #[test]
    fn applies_coordinate_sequence() {
        let messages: Vec<_> = ["e2e4", "e7e5", "g1f3"]
            .iter()
            .map(|mv| stored_message(StoredMessageType::Move.as_str(), &move_content(mv)))
            .collect();

        let (board, history) =
            rebuild_board_from_stored_messages(&messages).expect("apply sequence");
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].to_string(), "e2e4");
        assert_eq!(history[1].to_string(), "e7e5");
        assert_eq!(history[2].to_string(), "g1f3");
        assert_eq!(board.active_color(), Color::Black);
    }

    #[test]
    fn applies_rows_in_slice_order() {
        // Both timestamp and id order disagree with the supplied move sequence.
        let messages: Vec<_> = [("e2e4", 2, 3), ("e7e5", 3, 1), ("g1f3", 1, 2)]
            .into_iter()
            .map(|(mv, id, created_at)| {
                let mut message =
                    stored_message(StoredMessageType::Move.as_str(), &move_content(mv));
                message.id = Some(id);
                message.created_at = created_at;
                message
            })
            .collect();

        let (board, history) =
            rebuild_board_from_stored_messages(&messages).expect("apply rows in slice order");

        assert_eq!(
            history.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["e2e4", "e7e5", "g1f3"]
        );
        assert_eq!(
            board.to_fen(),
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2"
        );
    }

    #[test]
    fn black_castling_uses_active_color() {
        // Clear path for Black kingside castling, then castle with O-O.
        let moves = [
            "e2e4", "e7e5", "g1f3", "g8f6", "f1c4", "f8c5", "d2d3", "O-O",
        ];
        let messages: Vec<_> = moves
            .iter()
            .map(|mv| stored_message(StoredMessageType::Move.as_str(), &move_content(mv)))
            .collect();

        let (board, history) = rebuild_board_from_stored_messages(&messages).expect("black castle");
        assert_eq!(history.len(), 8);

        let black_castle = history.last().expect("castle ply");
        assert_eq!(black_castle.from, Position::from_str("e8").unwrap());
        assert_eq!(black_castle.to, Position::from_str("g8").unwrap());
        assert_eq!(board.active_color(), Color::White);
    }

    #[test]
    fn malformed_json_fails() {
        let messages = vec![stored_message(StoredMessageType::Move.as_str(), "not json")];
        let err = rebuild_board_from_stored_messages(&messages).expect_err("bad json");
        assert!(matches!(err, RebuildError::Serialization(_)));
    }

    #[test]
    fn bad_notation_fails() {
        let messages = vec![stored_message(
            StoredMessageType::Move.as_str(),
            &move_content("not-a-move"),
        )];
        let err = rebuild_board_from_stored_messages(&messages).expect_err("bad notation");
        assert!(matches!(err, RebuildError::Chess(_)));
    }
}
