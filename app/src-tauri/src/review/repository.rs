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
    /// Corrupt cached payloads are misses; storage failures return an error.
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Task<'a, Result<Vec<Option<RawPosition>>>>;
    /// Stores completed evaluations with their actual depth and candidate coverage.
    ///
    /// Storage failures propagate so analysis cannot silently lose cache writes.
    fn put<'a>(
        &'a self,
        entries: &'a [RawPosition],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Task<'a, Result<()>>;
    /// Saves a completed review, deriving game metadata from its PGN.
    ///
    /// The caller decides how to report a history failure after completion.
    fn save<'a>(
        &'a self,
        config: &'a ReviewConfig,
        review: &'a ReviewResult,
    ) -> Task<'a, Result<()>>;
    /// Waits for owned work, including operations whose waiting future was dropped.
    ///
    /// Implementations without background work may use the immediate default.
    fn drain(&self) -> Task<'_, ()> {
        Box::pin(async {})
    }
}
