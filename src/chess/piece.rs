use super::error::ChessError;
use crate::storage::models::PlayerColor;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    White,
    Black,
}

impl Color {
    /// Opposite color
    pub fn opposite(&self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

// Implement Display trait for human-readable output
impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::White => write!(f, "White"),
            Color::Black => write!(f, "Black"),
        }
    }
}

// Implement conversion from/to storage PlayerColor
impl From<PlayerColor> for Color {
    fn from(color: PlayerColor) -> Self {
        match color {
            PlayerColor::White => Color::White,
            PlayerColor::Black => Color::Black,
        }
    }
}

impl From<Color> for PlayerColor {
    fn from(color: Color) -> Self {
        match color {
            Color::White => PlayerColor::White,
            Color::Black => PlayerColor::Black,
        }
    }
}

// Implement FromStr for parsing with consistent error handling
impl FromStr for Color {
    type Err = ChessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "white" | "w" => Ok(Color::White),
            "black" | "b" => Ok(Color::Black),
            _ => Err(ChessError::InvalidColor(format!(
                "Expected 'white' or 'black', got '{}'",
                s
            ))),
        }
    }
}
