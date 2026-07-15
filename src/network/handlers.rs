//! Server-side message dispatch for the connection loop.
//!
//! Typed match, validate-before-side-effects, and a single reply path.
//! `GameInvite` persists a pending game and echoes the invite; other chess
//! handlers remain stubs that return `Ok(None)` until acknowledgements land.

use crate::messages::chess::{
    GameAccept, GameDecline, GameInvite, Move, MoveAck, SyncRequest, SyncResponse, ValidationError,
};
use crate::messages::Message;
use crate::storage::models::PlayerColor;
use crate::storage::{Database, StorageError};
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
/// - `Ok(None)` — no reply (stubbed chess handlers, unexpected Pong, soft reject)
/// - `Err` — unexpected handler failure (validation is soft-failed as `Ok(None)`)
///
/// Ping continues to echo. `GameInvite` persists and echoes; other chess
/// variants are stubbed (no reply yet).
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
        // GameInvite clients always wait for a reply; decline instead of hanging.
        if let Message::GameInvite(invite) = &message {
            return Ok(Some(Message::new_game_decline(
                invite.game_id.clone(),
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

fn stub_not_implemented(
    peer_id: &str,
    message_type: &'static str,
    game_id: &str,
) -> Result<Option<Message>, HandlerError> {
    debug!(
        peer_id = %peer_id,
        message_type,
        game_id,
        "handler not yet implemented (no reply)"
    );
    Ok(None)
}

/// Derive the invitee's local color from the inviter's suggestion.
///
/// `suggested_color` is the color offered to the invitee. When absent, use
/// provisional White until accept finalizes the choice.
fn invitee_color(suggested_color: Option<crate::chess::Color>) -> PlayerColor {
    match suggested_color {
        Some(color) => PlayerColor::from(color),
        None => PlayerColor::White,
    }
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
    let content = serde_json::to_string(invite).map_err(|e| {
        StorageError::serialization_error("GameInvite message content", e)
    })?;
    database.store_message(
        invite.game_id.clone(),
        "GameInvite".to_string(),
        content,
        "remote".to_string(),
        peer_id.to_string(),
    )?;
    Ok(())
}

/// idempotent retry for storing the invite message
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

/// Persist a pending game for an inbound invite and echo the invite as ack.
///
/// Does not auto-accept. On duplicate game ID with the same opponent, treats
/// the request as idempotent success. Conflicts and persistence failures reply
/// with `GameDecline` so the client's send-and-wait never hangs.
pub(crate) fn handle_game_invite(
    database: &Database,
    peer_id: &str,
    invite: GameInvite,
) -> Result<Option<Message>, HandlerError> {
    let my_color = invitee_color(invite.suggested_color);

    match database.create_game_with_id(
        invite.game_id.clone(),
        peer_id.to_string(),
        my_color,
        None,
    ) {
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
        Err(StorageError::ConstraintViolation { .. }) => {
            match database.get_game(&invite.game_id) {
                Ok(existing) if existing.opponent_peer_id == peer_id => {
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
            }
        }
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

pub(crate) fn handle_game_accept(
    _database: &Database,
    peer_id: &str,
    accept: GameAccept,
) -> Result<Option<Message>, HandlerError> {
    stub_not_implemented(peer_id, "GameAccept", &accept.game_id)
}

pub(crate) fn handle_game_decline(
    _database: &Database,
    peer_id: &str,
    decline: GameDecline,
) -> Result<Option<Message>, HandlerError> {
    stub_not_implemented(peer_id, "GameDecline", &decline.game_id)
}

pub(crate) fn handle_move(
    _database: &Database,
    peer_id: &str,
    mv: Move,
) -> Result<Option<Message>, HandlerError> {
    stub_not_implemented(peer_id, "Move", &mv.game_id)
}

pub(crate) fn handle_move_ack(
    _database: &Database,
    peer_id: &str,
    ack: MoveAck,
) -> Result<Option<Message>, HandlerError> {
    stub_not_implemented(peer_id, "MoveAck", &ack.game_id)
}

pub(crate) fn handle_sync_request(
    _database: &Database,
    peer_id: &str,
    req: SyncRequest,
) -> Result<Option<Message>, HandlerError> {
    stub_not_implemented(peer_id, "SyncRequest", &req.game_id)
}

pub(crate) fn handle_sync_response(
    _database: &Database,
    peer_id: &str,
    resp: SyncResponse,
) -> Result<Option<Message>, HandlerError> {
    stub_not_implemented(peer_id, "SyncResponse", &resp.game_id)
}
