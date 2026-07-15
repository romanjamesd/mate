//! Server-side message dispatch for the connection loop.
//!
//! Typed match, validate-before-side-effects, and a single reply path.
//! Chess handlers are stubs that return `Ok(None)` until persistence and
//! acknowledgements are implemented.

use crate::messages::chess::{
    GameAccept, GameDecline, GameInvite, Move, MoveAck, SyncRequest, SyncResponse, ValidationError,
};
use crate::messages::Message;
use crate::storage::Database;
use thiserror::Error;
use tracing::{debug, warn};

/// Errors produced while dispatching an inbound server message.
#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("message validation failed: {0}")]
    Validation(#[from] ValidationError),

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
/// Ping continues to echo. Chess variants are stubbed (no reply yet).
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

pub(crate) fn handle_game_invite(
    _database: &Database,
    peer_id: &str,
    invite: GameInvite,
) -> Result<Option<Message>, HandlerError> {
    stub_not_implemented(peer_id, "GameInvite", &invite.game_id)
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
