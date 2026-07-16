//! Server-side message dispatch for the connection loop.
//!
//! Typed match, validate-before-side-effects, and a single reply path.
//! `GameInvite` persists a pending game and echoes the invite.
//! `GameAccept` / `GameDecline` transition pending games to Active or
//! Abandoned and echo so send-and-wait clients never hang.
//! `Move` persists against an Active game and replies with `MoveAck`.
//! `SyncRequest` rebuilds board/history from stored moves and replies
//! with `SyncResponse`. Inbound `SyncResponse` is ignored (no reply).

use crate::chess::{Board, Color, Move as ChessMove};
use crate::messages::chess::{
    create_sync_response, GameAccept, GameDecline, GameInvite, Move, MoveAck, SyncRequest,
    SyncResponse, ValidationError,
};
use crate::messages::Message;
use crate::storage::models::{GameStatus, PlayerColor};
use crate::storage::{Database, Message as StoredMessage, StorageError};
use thiserror::Error;
use tracing::{debug, warn};

/// Errors produced while dispatching an inbound server message.
#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("message validation failed: {0}")]
    Validation(#[from] ValidationError),

    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("handler not implemented for {0}")]
    NotImplemented(&'static str),
}

/// Route an inbound message to the appropriate handler.
///
/// Returns:
/// - `Ok(Some(response))` — caller should `send_message` the response
/// - `Ok(None)` — no reply (MoveAck / SyncResponse as request, unexpected Pong)
/// - `Err` — unexpected handler failure
///
/// Validation soft-fails as `GameDecline` when the message has a `game_id`,
/// otherwise as `Ok(None)`. Ping continues to echo. Invite/accept/decline/move
/// persist and reply. SyncRequest rebuilds from stored moves and replies with
/// SyncResponse.
pub fn dispatch(
    database: &Database,
    peer_id: &str,
    message: Message,
) -> Result<Option<Message>, HandlerError> {
    debug!(
        peer_id = %peer_id,
        summary = %message.log_summary(),
        "dispatching inbound message"
    );

    if let Err(e) = message.validate() {
        warn!(
            peer_id = %peer_id,
            summary = %message.log_summary(),
            error = %e,
            "rejecting inbound message: validation failed"
        );
        // Send-and-wait clients need a reply; decline instead of hanging.
        let game_id = match &message {
            Message::GameInvite(invite) => Some(invite.game_id.as_str()),
            Message::GameAccept(accept) => Some(accept.game_id.as_str()),
            Message::GameDecline(decline) => Some(decline.game_id.as_str()),
            Message::Move(mv) => Some(mv.game_id.as_str()),
            Message::SyncRequest(req) => Some(req.game_id.as_str()),
            _ => None,
        };
        if let Some(game_id) = game_id {
            return Ok(Some(Message::new_game_decline(
                game_id.to_string(),
                Some(format!("validation failed: {e}")),
            )));
        }
        return Ok(None);
    }

    match message {
        Message::Ping { .. } => Ok(Some(message)),
        Message::Pong { .. } => {
            debug!(
                peer_id = %peer_id,
                "ignoring unexpected Pong as request"
            );
            Ok(None)
        }
        Message::GameInvite(invite) => handle_game_invite(database, peer_id, invite),
        Message::GameAccept(accept) => handle_game_accept(database, peer_id, accept),
        Message::GameDecline(decline) => handle_game_decline(database, peer_id, decline),
        Message::Move(mv) => handle_move(database, peer_id, mv),
        Message::MoveAck(ack) => handle_move_ack(database, peer_id, ack),
        Message::SyncRequest(req) => handle_sync_request(database, peer_id, req),
        Message::SyncResponse(resp) => handle_sync_response(database, peer_id, resp),
    }
}

/// Derive the invitee's local color from the inviter's suggestion.
///
/// `suggested_color` is the color offered to the invitee. When absent, use
/// provisional White until accept finalizes the choice.
fn invitee_color(suggested_color: Option<Color>) -> PlayerColor {
    match suggested_color {
        Some(color) => PlayerColor::from(color),
        None => PlayerColor::White,
    }
}

/// Local color for the peer receiving an accept (the inviter).
///
/// `accepted_color` is the color the accepter plays as, so this peer takes
/// the opposite.
fn inviter_color(accepted_color: Color) -> PlayerColor {
    PlayerColor::from(accepted_color.opposite())
}

fn decline_invite(game_id: &str, reason: &str) -> Result<Option<Message>, HandlerError> {
    Ok(Some(Message::new_game_decline(
        game_id.to_string(),
        Some(reason.to_string()),
    )))
}

fn store_invite_message(
    database: &Database,
    peer_id: &str,
    invite: &GameInvite,
) -> Result<(), StorageError> {
    let content = serde_json::to_string(invite)
        .map_err(|e| StorageError::serialization_error("GameInvite message content", e))?;
    database.store_message(
        invite.game_id.clone(),
        "GameInvite".to_string(),
        content,
        "remote".to_string(),
        peer_id.to_string(),
    )?;
    Ok(())
}

/// Idempotent retry for storing the invite message.
fn ensure_invite_message_stored(
    database: &Database,
    peer_id: &str,
    invite: &GameInvite,
) -> Result<(), StorageError> {
    let messages = database.get_messages_for_game(&invite.game_id)?;
    let already_stored = messages.iter().any(|m| m.message_type == "GameInvite");
    if !already_stored {
        store_invite_message(database, peer_id, invite)?;
    }
    Ok(())
}

fn store_accept_message(
    database: &Database,
    peer_id: &str,
    accept: &GameAccept,
) -> Result<(), StorageError> {
    let content = serde_json::to_string(accept)
        .map_err(|e| StorageError::serialization_error("GameAccept message content", e))?;
    database.store_message(
        accept.game_id.clone(),
        "GameAccept".to_string(),
        content,
        "remote".to_string(),
        peer_id.to_string(),
    )?;
    Ok(())
}

fn ensure_accept_message_stored(
    database: &Database,
    peer_id: &str,
    accept: &GameAccept,
) -> Result<(), StorageError> {
    let messages = database.get_messages_for_game(&accept.game_id)?;
    let already_stored = messages.iter().any(|m| m.message_type == "GameAccept");
    if !already_stored {
        store_accept_message(database, peer_id, accept)?;
    }
    Ok(())
}

fn store_decline_message(
    database: &Database,
    peer_id: &str,
    decline: &GameDecline,
) -> Result<(), StorageError> {
    let content = serde_json::to_string(decline)
        .map_err(|e| StorageError::serialization_error("GameDecline message content", e))?;
    database.store_message(
        decline.game_id.clone(),
        "GameDecline".to_string(),
        content,
        "remote".to_string(),
        peer_id.to_string(),
    )?;
    Ok(())
}

fn ensure_decline_message_stored(
    database: &Database,
    peer_id: &str,
    decline: &GameDecline,
) -> Result<(), StorageError> {
    let messages = database.get_messages_for_game(&decline.game_id)?;
    let already_stored = messages.iter().any(|m| m.message_type == "GameDecline");
    if !already_stored {
        store_decline_message(database, peer_id, decline)?;
    }
    Ok(())
}

fn store_move_message(database: &Database, peer_id: &str, mv: &Move) -> Result<(), StorageError> {
    let content = serde_json::to_string(mv)
        .map_err(|e| StorageError::serialization_error("Move message content", e))?;
    database.store_message(
        mv.game_id.clone(),
        "Move".to_string(),
        content,
        "remote".to_string(),
        peer_id.to_string(),
    )?;
    Ok(())
}

/// Skip insert when an identical Move payload is already stored for this game.
fn ensure_move_message_stored(
    database: &Database,
    peer_id: &str,
    mv: &Move,
) -> Result<(), StorageError> {
    let content = serde_json::to_string(mv)
        .map_err(|e| StorageError::serialization_error("Move message content", e))?;
    let messages = database.get_messages_for_game(&mv.game_id)?;
    let already_stored = messages
        .iter()
        .any(|m| m.message_type == "Move" && m.content == content);
    if !already_stored {
        store_move_message(database, peer_id, mv)?;
    }
    Ok(())
}

/// Persist a pending game for an inbound invite and echo the invite as ack.
///
/// Does not auto-accept. On duplicate game ID with the same opponent and
/// Pending status, treats the request as idempotent success. Same-peer
/// retries against Active / Abandoned / Completed games soft-reject.
/// Conflicts and persistence failures reply with `GameDecline` so the
/// client's send-and-wait never hangs.
pub(crate) fn handle_game_invite(
    database: &Database,
    peer_id: &str,
    invite: GameInvite,
) -> Result<Option<Message>, HandlerError> {
    let my_color = invitee_color(invite.suggested_color);

    match database.create_game_with_id(invite.game_id.clone(), peer_id.to_string(), my_color, None)
    {
        Ok(_) => {
            if let Err(e) = store_invite_message(database, peer_id, &invite) {
                warn!(
                    peer_id = %peer_id,
                    game_id = %invite.game_id,
                    error = %e,
                    "failed to store GameInvite message after creating game"
                );
                return decline_invite(&invite.game_id, "failed to persist invite");
            }
            debug!(
                peer_id = %peer_id,
                game_id = %invite.game_id,
                "persisted pending game from invite; echoing"
            );
            Ok(Some(Message::GameInvite(invite)))
        }
        Err(StorageError::ConstraintViolation { .. }) => match database.get_game(&invite.game_id) {
            Ok(existing) if existing.opponent_peer_id == peer_id => match existing.status {
                GameStatus::Pending => {
                    if let Err(e) = ensure_invite_message_stored(database, peer_id, &invite) {
                        warn!(
                            peer_id = %peer_id,
                            game_id = %invite.game_id,
                            error = %e,
                            "failed to ensure GameInvite message on idempotent retry"
                        );
                        return decline_invite(&invite.game_id, "failed to persist invite");
                    }
                    debug!(
                        peer_id = %peer_id,
                        game_id = %invite.game_id,
                        "idempotent GameInvite for existing pending game; echoing"
                    );
                    Ok(Some(Message::GameInvite(invite)))
                }
                other => {
                    warn!(
                        peer_id = %peer_id,
                        game_id = %invite.game_id,
                        status = ?other,
                        "GameInvite for existing non-pending game"
                    );
                    decline_invite(
                        &invite.game_id,
                        &format!("game already exists (status: {other:?})"),
                    )
                }
            },
            Ok(_) => {
                warn!(
                    peer_id = %peer_id,
                    game_id = %invite.game_id,
                    "GameInvite conflicts with existing game for another peer"
                );
                decline_invite(&invite.game_id, "game id already exists for another peer")
            }
            Err(e) => {
                warn!(
                    peer_id = %peer_id,
                    game_id = %invite.game_id,
                    error = %e,
                    "GameInvite duplicate but game lookup failed"
                );
                decline_invite(&invite.game_id, "failed to resolve invite conflict")
            }
        },
        Err(e) => {
            warn!(
                peer_id = %peer_id,
                game_id = %invite.game_id,
                error = %e,
                "failed to create pending game from invite"
            );
            decline_invite(&invite.game_id, "failed to persist invite")
        }
    }
}

/// Activate a pending game for an inbound accept and echo the accept as ack.
///
/// Requires Pending status and `opponent_peer_id == peer_id`. Finalizes local
/// color as the opposite of `accepted_color`. Same-peer Active retries are
/// idempotent. Soft-rejects with `GameDecline` so send-and-wait never hangs.
pub(crate) fn handle_game_accept(
    database: &Database,
    peer_id: &str,
    accept: GameAccept,
) -> Result<Option<Message>, HandlerError> {
    let game = match database.get_game(&accept.game_id) {
        Ok(game) => game,
        Err(StorageError::GameNotFound { .. }) => {
            warn!(
                peer_id = %peer_id,
                game_id = %accept.game_id,
                "GameAccept for unknown game"
            );
            return decline_invite(&accept.game_id, "game not found");
        }
        Err(e) => {
            warn!(
                peer_id = %peer_id,
                game_id = %accept.game_id,
                error = %e,
                "GameAccept failed to load game"
            );
            return decline_invite(&accept.game_id, "failed to load game");
        }
    };

    if game.opponent_peer_id != peer_id {
        warn!(
            peer_id = %peer_id,
            game_id = %accept.game_id,
            "GameAccept from peer that is not the game opponent"
        );
        return decline_invite(&accept.game_id, "peer is not the game opponent");
    }

    match game.status {
        GameStatus::Active => {
            if let Err(e) = ensure_accept_message_stored(database, peer_id, &accept) {
                warn!(
                    peer_id = %peer_id,
                    game_id = %accept.game_id,
                    error = %e,
                    "failed to ensure GameAccept message on idempotent retry"
                );
                return decline_invite(&accept.game_id, "failed to persist accept");
            }
            debug!(
                peer_id = %peer_id,
                game_id = %accept.game_id,
                "idempotent GameAccept for already-active game; echoing"
            );
            Ok(Some(Message::GameAccept(accept)))
        }
        GameStatus::Pending => {
            let my_color = inviter_color(accept.accepted_color);

            if let Err(e) = database.update_game_status(&accept.game_id, GameStatus::Active) {
                warn!(
                    peer_id = %peer_id,
                    game_id = %accept.game_id,
                    error = %e,
                    "failed to set game Active on accept"
                );
                return decline_invite(&accept.game_id, "failed to activate game");
            }

            if let Err(e) = database.update_game_color(&accept.game_id, my_color) {
                warn!(
                    peer_id = %peer_id,
                    game_id = %accept.game_id,
                    error = %e,
                    "failed to update color on accept"
                );
                return decline_invite(&accept.game_id, "failed to update game color");
            }

            if let Err(e) = store_accept_message(database, peer_id, &accept) {
                warn!(
                    peer_id = %peer_id,
                    game_id = %accept.game_id,
                    error = %e,
                    "failed to store GameAccept message after activating game"
                );
                return decline_invite(&accept.game_id, "failed to persist accept");
            }

            debug!(
                peer_id = %peer_id,
                game_id = %accept.game_id,
                "activated game from accept; echoing"
            );
            Ok(Some(Message::GameAccept(accept)))
        }
        other => {
            warn!(
                peer_id = %peer_id,
                game_id = %accept.game_id,
                status = ?other,
                "GameAccept for non-pending game"
            );
            decline_invite(
                &accept.game_id,
                &format!("game is not pending (status: {other:?})"),
            )
        }
    }
}

/// Abandon a pending game for an inbound decline and echo the decline as ack.
///
/// Requires Pending status and `opponent_peer_id == peer_id`. Same-peer
/// Abandoned retries are idempotent. Soft-rejects with `GameDecline` so
/// send-and-wait never hangs.
pub(crate) fn handle_game_decline(
    database: &Database,
    peer_id: &str,
    decline: GameDecline,
) -> Result<Option<Message>, HandlerError> {
    let game = match database.get_game(&decline.game_id) {
        Ok(game) => game,
        Err(StorageError::GameNotFound { .. }) => {
            warn!(
                peer_id = %peer_id,
                game_id = %decline.game_id,
                "GameDecline for unknown game"
            );
            return decline_invite(&decline.game_id, "game not found");
        }
        Err(e) => {
            warn!(
                peer_id = %peer_id,
                game_id = %decline.game_id,
                error = %e,
                "GameDecline failed to load game"
            );
            return decline_invite(&decline.game_id, "failed to load game");
        }
    };

    if game.opponent_peer_id != peer_id {
        warn!(
            peer_id = %peer_id,
            game_id = %decline.game_id,
            "GameDecline from peer that is not the game opponent"
        );
        return decline_invite(&decline.game_id, "peer is not the game opponent");
    }

    match game.status {
        GameStatus::Abandoned => {
            if let Err(e) = ensure_decline_message_stored(database, peer_id, &decline) {
                warn!(
                    peer_id = %peer_id,
                    game_id = %decline.game_id,
                    error = %e,
                    "failed to ensure GameDecline message on idempotent retry"
                );
                return decline_invite(&decline.game_id, "failed to persist decline");
            }
            debug!(
                peer_id = %peer_id,
                game_id = %decline.game_id,
                "idempotent GameDecline for already-abandoned game; echoing"
            );
            Ok(Some(Message::GameDecline(decline)))
        }
        GameStatus::Pending => {
            if let Err(e) = database.update_game_status(&decline.game_id, GameStatus::Abandoned) {
                warn!(
                    peer_id = %peer_id,
                    game_id = %decline.game_id,
                    error = %e,
                    "failed to set game Abandoned on decline"
                );
                return decline_invite(&decline.game_id, "failed to abandon game");
            }

            if let Err(e) = store_decline_message(database, peer_id, &decline) {
                warn!(
                    peer_id = %peer_id,
                    game_id = %decline.game_id,
                    error = %e,
                    "failed to store GameDecline message after abandoning game"
                );
                return decline_invite(&decline.game_id, "failed to persist decline");
            }

            debug!(
                peer_id = %peer_id,
                game_id = %decline.game_id,
                "abandoned game from decline; echoing"
            );
            Ok(Some(Message::GameDecline(decline)))
        }
        other => {
            warn!(
                peer_id = %peer_id,
                game_id = %decline.game_id,
                status = ?other,
                "GameDecline for non-pending game"
            );
            decline_invite(
                &decline.game_id,
                &format!("game is not pending (status: {other:?})"),
            )
        }
    }
}

/// Persist an inbound move against an Active game and reply with `MoveAck`.
///
/// Requires Active status and `opponent_peer_id == peer_id`. Identical payload
/// retries are idempotent. Soft-rejects with `GameDecline` so send-and-wait
/// never hangs. Does not reconstruct the board or verify move legality.
pub(crate) fn handle_move(
    database: &Database,
    peer_id: &str,
    mv: Move,
) -> Result<Option<Message>, HandlerError> {
    let game = match database.get_game(&mv.game_id) {
        Ok(game) => game,
        Err(StorageError::GameNotFound { .. }) => {
            warn!(
                peer_id = %peer_id,
                game_id = %mv.game_id,
                "Move for unknown game"
            );
            return decline_invite(&mv.game_id, "game not found");
        }
        Err(e) => {
            warn!(
                peer_id = %peer_id,
                game_id = %mv.game_id,
                error = %e,
                "Move failed to load game"
            );
            return decline_invite(&mv.game_id, "failed to load game");
        }
    };

    if game.opponent_peer_id != peer_id {
        warn!(
            peer_id = %peer_id,
            game_id = %mv.game_id,
            "Move from peer that is not the game opponent"
        );
        return decline_invite(&mv.game_id, "peer is not the game opponent");
    }

    match game.status {
        GameStatus::Active => {
            if let Err(e) = ensure_move_message_stored(database, peer_id, &mv) {
                warn!(
                    peer_id = %peer_id,
                    game_id = %mv.game_id,
                    error = %e,
                    "failed to store Move message"
                );
                return decline_invite(&mv.game_id, "failed to persist move");
            }
            debug!(
                peer_id = %peer_id,
                game_id = %mv.game_id,
                "persisted Move; acknowledging"
            );
            Ok(Some(Message::new_move_ack(mv.game_id, None)))
        }
        other => {
            warn!(
                peer_id = %peer_id,
                game_id = %mv.game_id,
                status = ?other,
                "Move for non-active game"
            );
            decline_invite(
                &mv.game_id,
                &format!("game is not active (status: {other:?})"),
            )
        }
    }
}

/// Inbound `MoveAck` as a request needs no further reply.
pub(crate) fn handle_move_ack(
    _database: &Database,
    peer_id: &str,
    ack: MoveAck,
) -> Result<Option<Message>, HandlerError> {
    debug!(
        peer_id = %peer_id,
        game_id = %ack.game_id,
        "ignoring inbound MoveAck as request (no reply)"
    );
    Ok(None)
}

/// Rebuild board and move history from stored `"Move"` rows.
///
/// Applies moves in chronological order without verifying stored board-state
/// hashes (those may be incorrect until clients send post-move hashes).
fn rebuild_board_from_stored_moves(
    messages: &[StoredMessage],
) -> Result<(Board, Vec<ChessMove>), String> {
    let mut board = Board::new();
    let mut history = Vec::new();

    for message in messages {
        if message.message_type != "Move" {
            continue;
        }

        let move_msg: Move = serde_json::from_str(&message.content)
            .map_err(|e| format!("failed to parse Move message: {e}"))?;

        let chess_move = ChessMove::from_str_with_color(&move_msg.chess_move, board.active_color())
            .map_err(|e| format!("failed to parse move '{}': {e}", move_msg.chess_move))?;

        board
            .make_move(chess_move)
            .map_err(|e| format!("failed to apply move '{}': {e}", move_msg.chess_move))?;
        history.push(chess_move);
    }

    Ok((board, history))
}

/// Rebuild FEN/history from stored moves and reply with `SyncResponse`.
///
/// Requires a known game with `opponent_peer_id == peer_id` (any status).
/// Soft-rejects with `GameDecline` so send-and-wait never hangs.
pub(crate) fn handle_sync_request(
    database: &Database,
    peer_id: &str,
    req: SyncRequest,
) -> Result<Option<Message>, HandlerError> {
    let game = match database.get_game(&req.game_id) {
        Ok(game) => game,
        Err(StorageError::GameNotFound { .. }) => {
            warn!(
                peer_id = %peer_id,
                game_id = %req.game_id,
                "SyncRequest for unknown game"
            );
            return decline_invite(&req.game_id, "game not found");
        }
        Err(e) => {
            warn!(
                peer_id = %peer_id,
                game_id = %req.game_id,
                error = %e,
                "SyncRequest failed to load game"
            );
            return decline_invite(&req.game_id, "failed to load game");
        }
    };

    if game.opponent_peer_id != peer_id {
        warn!(
            peer_id = %peer_id,
            game_id = %req.game_id,
            "SyncRequest from peer that is not the game opponent"
        );
        return decline_invite(&req.game_id, "peer is not the game opponent");
    }

    let messages = match database.get_messages_for_game(&req.game_id) {
        Ok(messages) => messages,
        Err(e) => {
            warn!(
                peer_id = %peer_id,
                game_id = %req.game_id,
                error = %e,
                "SyncRequest failed to load messages"
            );
            return decline_invite(&req.game_id, "failed to load messages");
        }
    };

    let (board, history) = match rebuild_board_from_stored_moves(&messages) {
        Ok(rebuilt) => rebuilt,
        Err(reason) => {
            warn!(
                peer_id = %peer_id,
                game_id = %req.game_id,
                error = %reason,
                "SyncRequest failed to rebuild board from stored moves"
            );
            return decline_invite(&req.game_id, &format!("failed to rebuild board: {reason}"));
        }
    };

    debug!(
        peer_id = %peer_id,
        game_id = %req.game_id,
        moves = history.len(),
        "rebuilt board for SyncRequest; sending SyncResponse"
    );
    Ok(Some(create_sync_response(&req.game_id, &board, &history)))
}

/// Inbound `SyncResponse` as a request needs no further reply.
pub(crate) fn handle_sync_response(
    _database: &Database,
    peer_id: &str,
    resp: SyncResponse,
) -> Result<Option<Message>, HandlerError> {
    debug!(
        peer_id = %peer_id,
        game_id = %resp.game_id,
        "ignoring inbound SyncResponse as request (no reply)"
    );
    Ok(None)
}
