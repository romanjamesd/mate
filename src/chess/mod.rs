// Re-export all public items
pub use self::error::ChessError;
pub use self::piece::Color;
// TODO: Uncomment when implementing the remaining modules
// pub use self::piece::{Piece, PieceType};
// pub use self::position::Position;
// pub use self::moves::Move;

// Define submodules
mod error;
mod piece;
// TODO: Uncomment when implementing the remaining modules
// mod position;
// mod moves;
