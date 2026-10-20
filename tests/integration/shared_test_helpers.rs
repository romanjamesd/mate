//! Verify shared fixtures against independently built boards and stored identities.

use crate::common::test_helpers::{next_move, play, play_move, twin_databases};
use mate::chess::{Board, Move as ChessMove};
use mate::game::{commit_move, rebuild_chronology, CommitOutcome, MoveActor, MoveCommitError};
use mate::messages::chess::{hash_board_state, Move};
use mate::storage::{Database, GameStatus, PlayerColor};

const LOCAL_PEER: &str = "fixture-local-peer";
const OPPONENT_PEER: &str = "fixture-opponent-peer";

fn active_game(db: &Database, color: PlayerColor) -> String {
    let game_id = uuid::Uuid::new_v4().to_string();
    db.create_game_with_id(game_id.clone(), OPPONENT_PEER.into(), color, None)
        .unwrap();
    db.update_game_status(&game_id, GameStatus::Active).unwrap();
    game_id
}

#[test]
fn shared_play_preserves_chronology_and_senders_for_both_local_colors() {
    for local_color in [PlayerColor::White, PlayerColor::Black] {
        let dir = tempfile::TempDir::new().unwrap();
        let db = Database::new_with_path(LOCAL_PEER, &dir.path().join("database.sqlite")).unwrap();
        let game_id = active_game(&db, local_color.clone());
        let notations = ["e2e4", "e7e5", "g1f3", "b8c6"];

        // Building a payload must leave the fixture's history untouched.
        let first = next_move(&db, &game_id, notations[0]);
        assert!(db.get_moves_for_game(&game_id).unwrap().is_empty());
        play(&db, &game_id, &notations, LOCAL_PEER);

        let rows = db.get_moves_for_game(&game_id).unwrap();
        assert_eq!(rows.len(), notations.len());
        let mut board = Board::new();
        let mut local_payload = None;
        for (index, (row, notation)) in rows.iter().zip(notations).enumerate() {
            let ply = u32::try_from(index + 1).unwrap();
            board
                .make_move(ChessMove::from_str_with_color(notation, board.active_color()).unwrap())
                .unwrap();
            let expected = Move::new(
                game_id.clone(),
                ply,
                notation.into(),
                hash_board_state(&board),
            );
            assert_eq!(row.game_id, game_id);
            assert_eq!(row.ply, Some(i64::from(ply)));
            assert_eq!(
                serde_json::from_str::<Move>(&row.content).unwrap(),
                expected
            );
            if index == 0 {
                assert_eq!(first, expected);
            }
            let is_local = (ply % 2 == 1) == (local_color == PlayerColor::White);
            assert_eq!(
                row.sender_peer_id,
                if is_local { LOCAL_PEER } else { OPPONENT_PEER }
            );
            if is_local {
                local_payload = Some(expected);
            }
        }
        assert_eq!(rebuild_chronology(&game_id, &rows).unwrap().board, board);

        // The local identity must also authorize an exact historical retry.
        let local_payload = local_payload.unwrap();
        assert_eq!(
            commit_move(
                &db,
                &game_id,
                &local_payload,
                MoveActor::Local {
                    self_peer_id: LOCAL_PEER
                }
            )
            .unwrap(),
            CommitOutcome::AlreadyCommitted
        );
        assert!(matches!(
            commit_move(
                &db,
                &game_id,
                &local_payload,
                MoveActor::Local {
                    self_peer_id: "self"
                }
            ),
            Err(MoveCommitError::Conflict { .. })
        ));
        assert_eq!(db.get_moves_for_game(&game_id).unwrap().len(), rows.len());
    }
}

#[test]
fn shared_twin_databases_observe_moves_committed_through_either_handle() {
    let (_dir, first, second) = twin_databases();
    let game_id = active_game(&first, PlayerColor::White);
    assert!(second.get_moves_for_game(&game_id).unwrap().is_empty());

    let white = play_move(&first, &game_id, "e2e4", LOCAL_PEER);
    let rows = second.get_moves_for_game(&game_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        serde_json::from_str::<Move>(&rows[0].content).unwrap(),
        white
    );
    assert_eq!(rows[0].sender_peer_id, LOCAL_PEER);

    let black = play_move(&second, &game_id, "e7e5", LOCAL_PEER);
    let rows = first.get_moves_for_game(&game_id).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        serde_json::from_str::<Move>(&rows[1].content).unwrap(),
        black
    );
    assert_eq!(rows[1].sender_peer_id, OPPONENT_PEER);
    assert_eq!(
        next_move(&first, &game_id, "g1f3"),
        next_move(&second, &game_id, "g1f3")
    );
    rebuild_chronology(&game_id, &rows).unwrap();
}
