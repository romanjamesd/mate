//! Shared chess game domain: list/reconstruct games and process moves.
//! Depends only on `chess`, `messages`, and `storage` — not on `cli` or `network`.

mod moves;
mod ops;

pub use moves::*;
pub use ops::*;
