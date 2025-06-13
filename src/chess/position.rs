use super::error::ChessError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub file: u8, // 0-7 corresponding to a-h
    pub rank: u8, // 0-7 corresponding to 1-8
}

impl Position {
    pub fn new(file: u8, rank: u8) -> Result<Self, ChessError> {
        // Validate position is within bounds (0-7)
        if file > 7 {
            return Err(ChessError::InvalidPosition(format!(
                "File must be 0-7, got {}",
                file
            )));
        }
        if rank > 7 {
            return Err(ChessError::InvalidPosition(format!(
                "Rank must be 0-7, got {}",
                rank
            )));
        }

        Ok(Self { file, rank })
    }

    /// Create position without validation (for internal use when bounds are guaranteed)
    pub const fn new_unchecked(file: u8, rank: u8) -> Self {
        Self { file, rank }
    }

    /// Create position from file and rank characters
    pub fn from_chars(file: char, rank: char) -> Result<Self, ChessError> {
        let file_lower = file.to_ascii_lowercase();
        if !('a'..='h').contains(&file_lower) {
            return Err(ChessError::InvalidPosition(format!(
                "Invalid file '{}'. Must be a-h.",
                file
            )));
        }

        if !('1'..='8').contains(&rank) {
            return Err(ChessError::InvalidPosition(format!(
                "Invalid rank '{}'. Must be 1-8.",
                rank
            )));
        }

        let file_num = file_lower as u8 - b'a';
        let rank_num = rank as u8 - b'1';

        Ok(Position {
            file: file_num,
            rank: rank_num,
        })
    }

    // Convert file to character (0 -> 'a', 1 -> 'b', etc.)
    pub fn file_char(&self) -> char {
        (self.file + b'a') as char
    }

    // Convert rank to chess notation (0 -> '1', 1 -> '2', etc.)
    pub fn rank_char(&self) -> char {
        (self.rank + b'1') as char
    }

    /// Calculate Manhattan distance between positions
    pub fn distance(&self, other: &Position) -> u8 {
        ((self.file as i8 - other.file as i8).abs() + (self.rank as i8 - other.rank as i8).abs())
            as u8
    }

    /// Check if positions are on the same rank
    pub fn same_rank(&self, other: &Position) -> bool {
        self.rank == other.rank
    }

    /// Check if positions are on the same file
    pub fn same_file(&self, other: &Position) -> bool {
        self.file == other.file
    }

    /// Check if positions are on the same diagonal
    pub fn same_diagonal(&self, other: &Position) -> bool {
        let file_diff = (self.file as i8 - other.file as i8).abs();
        let rank_diff = (self.rank as i8 - other.rank as i8).abs();
        file_diff == rank_diff
    }

    /// Get all positions on the board
    pub fn all_positions() -> impl Iterator<Item = Position> {
        (0..8).flat_map(|rank| (0..8).map(move |file| Position { file, rank }))
    }
}

// Implement Display trait for algebraic notation
impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.file_char(), self.rank_char())
    }
}

// Implement FromStr for parsing algebraic notation
impl FromStr for Position {
    type Err = ChessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 2 {
            return Err(ChessError::InvalidPosition(format!(
                "Position must be exactly 2 characters (e.g., 'e4'), got '{}'",
                s
            )));
        }

        let mut chars = s.chars();
        let file_char = chars.next().unwrap();
        let rank_char = chars.next().unwrap();

        Self::from_chars(file_char, rank_char)
    }
}
