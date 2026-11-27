//! Pure chess and review transformations. No process or database access.
use super::types::{ReviewError, ReviewErrorCode};

mod build;
mod opening;
mod pgn;
mod pgn_tokens;
mod phase;
mod position;
mod position_context;

pub use build::{build, position_analysis};
pub use opening::opening;
pub use pgn::{Game, extract};
pub use phase::{phase, phases};
pub use position::{
    add_san, fen, observed_coverage, position, terminal, terminal_position, terminal_raw,
};
pub use position_context::PositionContext;

fn invalid(message: impl ToString) -> ReviewError {
    ReviewError::new(ReviewErrorCode::InvalidPgn, "pgn", message)
}
