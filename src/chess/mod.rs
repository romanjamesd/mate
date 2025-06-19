// Re-export all public items
pub use self::board::Board;
pub use self::error::ChessError;
pub use self::moves::Move;
pub use self::piece::{Color, Piece, PieceType};
pub use self::position::Position;

// Define submodules
mod board;
mod error;
mod moves;
mod piece;
mod position;
