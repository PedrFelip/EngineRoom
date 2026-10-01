//! Pure chess and review transformations. No Tauri, process or database access.
use super::types::ReviewError;

mod build;
mod opening;
mod pgn;
mod phase;
mod position;

pub use build::{build, position_analysis};
pub use opening::opening;
pub use pgn::{extract, Game};
pub use phase::{phase, phases};
pub use position::{add_san, fen, position, terminal, terminal_raw};

fn invalid(message: impl ToString) -> ReviewError {
    ReviewError::new("invalidPgn", "pgn", message)
}
