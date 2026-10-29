//! Persistence port used by analysis; adapters own database and runtime details.
use super::{
    engine::Task,
    types::{RawPosition, Result, ReviewConfig, ReviewResult},
};
use crate::db::mode::Mode;

mod cache_entry;
mod saved_review;
mod sqlite;

pub use cache_entry::shape;
pub use saved_review::{normalize, validate_review};
pub use sqlite::{
    __cmd__games_get_review_config, __tauri_command_name_games_get_review_config,
    games_get_review_config, SqliteRepository,
};

/// Stores analysis results and owns outstanding persistence work until drained.
pub trait Repository: Send + Sync {
    /// Returns one entry per FEN, in input order, satisfying the requested budget.
    ///
    /// Corrupt cached payloads are misses.
    ///
    /// # Errors
    /// Returns actual storage failures; callers treat these as fatal.
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Task<'a, Result<Vec<Option<RawPosition>>>>;
    /// Stores completed evaluations with their actual depth and candidate coverage.
    ///
    /// # Errors
    /// Encoding and storage failures propagate so analysis cannot silently lose cache writes.
    fn put<'a>(
        &'a self,
        entries: &'a [RawPosition],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Task<'a, Result<()>>;
    /// Saves a completed review, deriving game metadata from its PGN.
    ///
    /// The caller publishes completion first and reports history failure best-effort.
    ///
    /// # Errors
    /// Returns PGN, encoding or storage failures.
    fn save<'a>(
        &'a self,
        config: &'a ReviewConfig,
        review: &'a ReviewResult,
    ) -> Task<'a, Result<()>>;
    /// Waits for owned work, including operations whose waiting future was dropped.
    ///
    /// Cancellation signals intent; only completed drain acknowledges finished work.
    /// Dropping a waiter must not detach work from this ownership contract.
    /// Implementations without background work may use the immediate default.
    fn drain(&self) -> Task<'_, ()> {
        Box::pin(async {})
    }
}
