use super::error::ChessError;
use super::piece::{Color, PieceType};
use super::position::Position;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Move {
    pub from: Position,
    pub to: Position,
    pub promotion: Option<PieceType>,
}

impl Move {
    /// Create a new move with validation
    pub fn new(
        from: Position,
        to: Position,
        promotion: Option<PieceType>,
    ) -> Result<Self, ChessError> {
        // Validate that from != to
        if from == to {
            return Err(ChessError::InvalidMove(
                "Source and destination positions cannot be the same".to_string(),
            ));
        }

        // Validate promotion logic
        if let Some(piece_type) = promotion {
            if matches!(piece_type, PieceType::King | PieceType::Pawn) {
                return Err(ChessError::InvalidMove(
                    "Cannot promote to King or Pawn".to_string(),
                ));
            }
        }

        Ok(Self {
            from,
            to,
            promotion,
        })
    }

    /// Create a new move without validation (for internal use when validity is guaranteed)
    pub const fn new_unchecked(from: Position, to: Position, promotion: Option<PieceType>) -> Self {
        Self {
            from,
            to,
            promotion,
        }
    }

    /// Create a simple move without promotion
    pub fn simple(from: Position, to: Position) -> Result<Self, ChessError> {
        Self::new(from, to, None)
    }

    /// Create a promotion move
    pub fn promotion(
        from: Position,
        to: Position,
        promotion: PieceType,
    ) -> Result<Self, ChessError> {
        Self::new(from, to, Some(promotion))
    }

    /// Check if this is a promotion move
    pub fn is_promotion(&self) -> bool {
        self.promotion.is_some()
    }

    /// Check if this is a castling move (king moves two squares horizontally)
    pub fn is_castling(&self) -> bool {
        (self.from.rank == self.to.rank) && (self.from.file.abs_diff(self.to.file) == 2)
    }

    /// Check if this might be an en passant move (diagonal pawn move)
    /// Note: This only checks the move pattern; game context needed for full validation
    pub fn is_en_passant_candidate(&self) -> bool {
        self.from.file != self.to.file && (self.from.rank.abs_diff(self.to.rank) == 1)
    }

    /// Convert to JSON format for storage compatibility
    pub fn to_json(&self) -> serde_json::Value {
        let mut json = serde_json::json!({
            "from": self.from.to_string(),
            "to": self.to.to_string()
        });

        if let Some(promotion) = self.promotion {
            json["promotion"] = serde_json::Value::String(promotion.to_string());
        }

        json
    }

    /// Parse from storage JSON format
    pub fn from_json(value: &serde_json::Value) -> Result<Self, ChessError> {
        let from_str = value.get("from").and_then(|v| v.as_str()).ok_or_else(|| {
            ChessError::InvalidMove("Missing or invalid 'from' field in JSON".to_string())
        })?;

        let to_str = value.get("to").and_then(|v| v.as_str()).ok_or_else(|| {
            ChessError::InvalidMove("Missing or invalid 'to' field in JSON".to_string())
        })?;

        let from = from_str.parse::<Position>()?;
        let to = to_str.parse::<Position>()?;

        let promotion = if let Some(promotion_value) = value.get("promotion") {
            if let Some(promotion_str) = promotion_value.as_str() {
                Some(promotion_str.parse::<PieceType>()?)
            } else {
                return Err(ChessError::InvalidMove(
                    "Invalid 'promotion' field in JSON".to_string(),
                ));
            }
        } else {
            None
        };

        Self::new(from, to, promotion)
    }

    /// Parse move string with color context for proper castling disambiguation
    pub fn from_str_with_color(s: &str, color: Color) -> Result<Self, ChessError> {
        let s = s.trim();

        // Handle special castling moves with color context
        match s.to_uppercase().as_str() {
            "O-O" | "0-0" => {
                // Kingside castling with proper color-based rank
                let rank = match color {
                    Color::White => 0, // rank 1 (e1, g1)
                    Color::Black => 7, // rank 8 (e8, g8)
                };
                return Ok(Move::new_unchecked(
                    Position::new_unchecked(4, rank), // e1 or e8
                    Position::new_unchecked(6, rank), // g1 or g8
                    None,
                ));
            }
            "O-O-O" | "0-0-0" => {
                // Queenside castling with proper color-based rank
                let rank = match color {
                    Color::White => 0, // rank 1 (e1, c1)
                    Color::Black => 7, // rank 8 (e8, c8)
                };
                return Ok(Move::new_unchecked(
                    Position::new_unchecked(4, rank), // e1 or e8
                    Position::new_unchecked(2, rank), // c1 or c8
                    None,
                ));
            }
            _ => {} // Continue with standard parsing
        }

        // Basic move format (e2e4)
        if s.len() == 4 {
            let from_str = &s[0..2];
            let to_str = &s[2..4];

            let from = from_str.parse::<Position>()?;
            let to = to_str.parse::<Position>()?;

            return Self::new(from, to, None);
        }
        // Move with promotion (e7e8q)
        else if s.len() == 5 {
            let from_str = &s[0..2];
            let to_str = &s[2..4];
            let promotion_char = &s[4..5];

            let from = from_str.parse::<Position>()?;
            let to = to_str.parse::<Position>()?;
            let promotion = promotion_char.parse::<PieceType>()?;

            return Self::new(from, to, Some(promotion));
        }

        Err(ChessError::InvalidMove(format!(
            "Invalid move format '{s}'. Expected 'e2e4', 'e7e8q' for promotion, or 'O-O'/'O-O-O' for castling."
        )))
    }
}

// Implement Display for algebraic notation
impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.from, self.to)?;
        if let Some(promotion) = self.promotion {
            write!(f, "{}", promotion)?;
        }
        Ok(())
    }
}

// Implement FromStr for parsing algebraic move notation
impl FromStr for Move {
    type Err = ChessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Default to White for backward compatibility
        // Note: For castling moves, this assumes White. Use from_str_with_color() for proper color context.
        Self::from_str_with_color(s, Color::White)
    }
}
