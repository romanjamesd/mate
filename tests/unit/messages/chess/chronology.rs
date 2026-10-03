use mate::crypto::Identity;
use mate::messages::chess::{move_digest, validate_move_message, Move, MoveAck};
use mate::messages::{Message, SignedEnvelope};
use serde::Serialize;

fn sample_move() -> Move {
    Move::new(
        "00000000-0000-4000-8000-000000000001".into(),
        7,
        "e2e4".into(),
        "0123456789abcdef".repeat(4),
    )
}

#[test]
fn legacy_json_without_ply_is_rejected() {
    let json = serde_json::json!({
        "game_id": sample_move().game_id,
        "chess_move": "e2e4",
        "board_state_hash": sample_move().board_state_hash,
    });
    let error = serde_json::from_str::<Move>(&json.to_string()).unwrap_err();
    assert!(error.to_string().contains("missing field `ply`"));
}

#[test]
fn legacy_bincode_move_is_rejected() {
    #[derive(Serialize)]
    struct LegacyMove {
        game_id: String,
        chess_move: String,
        board_state_hash: String,
    }
    // Match the original Message::Move enum discriminant (5), without using
    // any of the new Move serialization to construct this legacy fixture.
    #[derive(Serialize)]
    #[allow(dead_code)]
    enum LegacyMessage {
        Ping,
        Pong,
        GameInvite,
        GameAccept,
        GameDecline,
        Move(LegacyMove),
    }
    let mv = sample_move();
    let bytes = bincode::serialize(&LegacyMessage::Move(LegacyMove {
        game_id: mv.game_id,
        chess_move: mv.chess_move,
        board_state_hash: mv.board_state_hash,
    }))
    .unwrap();
    assert!(Message::deserialize(&bytes).is_err());
}

#[test]
fn move_roundtrips_preserve_ply_and_reject_zero() {
    for ply in [1, 7, u32::MAX] {
        let mv = Move {
            ply,
            ..sample_move()
        };
        assert!(validate_move_message(&mv).is_ok());
        let json = serde_json::to_string(&mv).unwrap();
        assert_eq!(serde_json::from_str::<Move>(&json).unwrap(), mv);
        let message = Message::new_move(
            mv.game_id.clone(),
            ply,
            mv.chess_move.clone(),
            mv.board_state_hash.clone(),
        );
        let bytes = message.serialize().unwrap();
        let Message::Move(decoded) = Message::deserialize(&bytes).unwrap() else {
            panic!("expected Move");
        };
        assert_eq!(decoded, mv);
        assert!(message.log_summary().contains(&format!("ply={ply}")));
    }
    assert!(validate_move_message(&Move {
        ply: 0,
        ..sample_move()
    })
    .is_err());
}

#[test]
fn move_digest_known_answer() {
    // Independently computed with Python hashlib + struct.pack('>I', ...).
    assert_eq!(
        move_digest(&sample_move()),
        "599f28c10df71fc9b0b8944bfdd4638be9e84774d739139b5b150968597c51aa"
    );
}

#[test]
fn move_digest_commits_every_field_exactly() {
    let mv = sample_move();
    let digest = move_digest(&mv);
    for changed in [
        Move {
            game_id: "00000000-0000-4000-8000-000000000002".into(),
            ..mv.clone()
        },
        Move {
            ply: mv.ply + 1,
            ..mv.clone()
        },
        Move {
            chess_move: "d2d4".into(),
            ..mv.clone()
        },
        Move {
            board_state_hash: "0".repeat(64),
            ..mv.clone()
        },
        Move {
            board_state_hash: mv.board_state_hash.to_uppercase(),
            ..mv.clone()
        },
        Move {
            board_state_hash: format!(" {}", mv.board_state_hash),
            ..mv.clone()
        },
    ] {
        assert_ne!(move_digest(&changed), digest);
    }
}

#[test]
fn digest_string_boundaries_are_unambiguous() {
    let left = Move {
        chess_move: "ab".into(),
        board_state_hash: "c".into(),
        ..sample_move()
    };
    let right = Move {
        chess_move: "a".into(),
        board_state_hash: "bc".into(),
        ..sample_move()
    };
    assert_ne!(move_digest(&left), move_digest(&right));
}

#[test]
fn ply_is_covered_by_envelope_signature() {
    let identity = Identity::generate().unwrap();
    let mv = sample_move();
    let mut envelope = SignedEnvelope::create(&Message::Move(mv.clone()), &identity, None).unwrap();
    assert!(envelope.verify_signature());
    let Message::Move(decoded) = envelope.get_message().unwrap() else {
        panic!("expected Move")
    };
    assert_eq!(decoded, mv);
    envelope.message = Message::Move(Move {
        ply: mv.ply + 1,
        ..mv
    })
    .serialize()
    .unwrap();
    assert!(!envelope.verify_signature());
}

#[test]
fn repeated_acknowledgments_are_byte_identical() {
    let mv = sample_move();
    let first = Message::new_move_ack(&mv);
    let second = Message::MoveAck(MoveAck::for_move(&mv));
    assert_eq!(first.serialize().unwrap(), second.serialize().unwrap());
    assert!(first.validate().is_ok());
    assert!(first.log_summary().contains("ply=7"));
}
