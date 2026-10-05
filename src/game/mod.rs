//! Shared chess game domain: list/reconstruct games and process moves.
//! Depends only on `chess`, `messages`, and `storage` — not on `cli` or `network`.

pub mod chronology;
mod message_type;
mod moves;
mod ops;
mod rebuild;
mod store;

pub use chronology::*;
pub use message_type::*;
pub use moves::*;
pub use ops::*;
pub use rebuild::*;
pub use store::*;
