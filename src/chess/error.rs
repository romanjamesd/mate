use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChessError {
    InvalidColor(String),
    InvalidPieceType(String),
    InvalidPosition(String),
    InvalidMove(String),
}

impl fmt::Display for ChessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChessError::InvalidColor(msg) => write!(f, "Invalid color: {}", msg),
            ChessError::InvalidPieceType(msg) => write!(f, "Invalid piece type: {}", msg),
            ChessError::InvalidPosition(msg) => write!(f, "Invalid position: {}", msg),
            ChessError::InvalidMove(msg) => write!(f, "Invalid move: {}", msg),
        }
    }
}

impl std::error::Error for ChessError {}
