// Re-export all public items
pub use self::error::ChessError;
pub use self::piece::{Color, Piece, PieceType};
pub use self::position::Position;
// pub use self::moves::Move;

// Define submodules
mod error;
mod piece;
mod position;
// TODO: Uncomment when implementing the remaining modules
// mod moves;
