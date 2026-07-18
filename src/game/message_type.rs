//! Typed chess-persisted `message_type` values aligned with the wire protocol.
//!
//! Only variants that are written to the messages table are represented.
//! Strings match `Message::message_type()` PascalCase exactly; snake_case is rejected.

use std::fmt;
use std::str::FromStr;

/// Chess message types persisted as DB row `message_type` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoredMessageType {
    GameInvite,
    GameAccept,
    GameDecline,
    Move,
}

impl StoredMessageType {
    /// Wire / DB string for this variant (PascalCase).
    pub fn as_str(&self) -> &'static str {
        match self {
            StoredMessageType::GameInvite => "GameInvite",
            StoredMessageType::GameAccept => "GameAccept",
            StoredMessageType::GameDecline => "GameDecline",
            StoredMessageType::Move => "Move",
        }
    }
}

/// Failed to parse a stored message type string (must be exact PascalCase).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidStoredMessageType;

impl fmt::Display for StoredMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for StoredMessageType {
    type Err = InvalidStoredMessageType;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GameInvite" => Ok(StoredMessageType::GameInvite),
            "GameAccept" => Ok(StoredMessageType::GameAccept),
            "GameDecline" => Ok(StoredMessageType::GameDecline),
            "Move" => Ok(StoredMessageType::Move),
            _ => Err(InvalidStoredMessageType),
        }
    }
}

impl TryFrom<&str> for StoredMessageType {
    type Error = InvalidStoredMessageType;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::Color;
    use crate::messages::types::Message;

    const ALL_VARIANTS: [StoredMessageType; 4] = [
        StoredMessageType::GameInvite,
        StoredMessageType::GameAccept,
        StoredMessageType::GameDecline,
        StoredMessageType::Move,
    ];

    #[test]
    fn round_trip_as_str_and_from_str() {
        for variant in ALL_VARIANTS {
            let s = variant.as_str();
            assert_eq!(StoredMessageType::from_str(s), Ok(variant));
            assert_eq!(StoredMessageType::try_from(s), Ok(variant));
            assert_eq!(variant.to_string(), s);
        }
    }

    #[test]
    fn as_str_matches_wire_message_type() {
        let game_id = "00000000-0000-0000-0000-000000000001".to_string();

        assert_eq!(
            StoredMessageType::GameInvite.as_str(),
            Message::new_game_invite(game_id.clone(), None).message_type()
        );
        assert_eq!(
            StoredMessageType::GameAccept.as_str(),
            Message::new_game_accept(game_id.clone(), Color::White).message_type()
        );
        assert_eq!(
            StoredMessageType::GameDecline.as_str(),
            Message::new_game_decline(game_id.clone(), None).message_type()
        );
        assert_eq!(
            StoredMessageType::Move.as_str(),
            Message::new_move(game_id, "e2e4".to_string(), "hash".to_string()).message_type()
        );
    }

    #[test]
    fn snake_case_rejected() {
        for s in ["move", "game_invite", "game_accept", "game_decline"] {
            assert_eq!(
                StoredMessageType::from_str(s),
                Err(InvalidStoredMessageType),
                "expected snake_case {s:?} to be rejected"
            );
        }
    }

    #[test]
    fn wrong_case_and_unknown_rejected() {
        for s in [
            "MOVE",
            "gameInvite",
            "MoveAck",
            "SyncRequest",
            "SyncResponse",
            "Ping",
            "Pong",
            "",
            "nope",
        ] {
            assert_eq!(
                StoredMessageType::from_str(s),
                Err(InvalidStoredMessageType),
                "expected {s:?} to be rejected"
            );
            assert_eq!(StoredMessageType::try_from(s), Err(InvalidStoredMessageType));
        }
    }
}
