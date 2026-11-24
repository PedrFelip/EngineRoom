//! Persistence port used by analysis; adapters own database and runtime details.
use super::{
    engine::Task,
    types::{RawPosition, Result, ReviewConfig, ReviewResult},
};
use crate::db::mode::Mode;
use std::sync::Arc;

mod saved_review;

pub use saved_review::{normalize, validate_review};

/// Completed contextual evaluation and its provenance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextualEvaluation {
    pub key: String,
    pub raw: RawPosition,
    pub evidence: super::types::PositionEvidence,
}

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
    /// Looks up contextual evaluations without consulting the legacy FEN cache.
    fn lookup_context<'a>(
        &'a self,
        keys: &'a [String],
        _mode: Mode,
        _value: u32,
        _multipv: u32,
    ) -> Task<'a, Result<Vec<Option<ContextualEvaluation>>>> {
        Box::pin(async move { Ok(vec![None; keys.len()]) })
    }

    /// Stores contextual evaluations independently from legacy FEN entries.
    fn put_context<'a>(
        &'a self,
        _entries: &'a [ContextualEvaluation],
        _mode: Mode,
        _value: u32,
    ) -> Task<'a, Result<()>> {
        Box::pin(async { Ok(()) })
    }

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

/// Selects whether analysis reads and writes the persistent position cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheMode {
    #[default]
    Enabled,
    Bypass,
}

/// Skips position cache I/O while retaining history and persistence ownership.
pub struct BypassCacheRepository {
    inner: Arc<dyn Repository>,
}

impl BypassCacheRepository {
    /// Wraps the repository used for history and persistence cleanup.
    pub fn new(inner: Arc<dyn Repository>) -> Self {
        Self { inner }
    }
}

impl Repository for BypassCacheRepository {
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        _: Mode,
        _: u32,
        _: u32,
    ) -> Task<'a, Result<Vec<Option<RawPosition>>>> {
        Box::pin(async move { Ok(vec![None; fens.len()]) })
    }

    fn put<'a>(&'a self, _: &'a [RawPosition], _: Mode, _: u32, _: u32) -> Task<'a, Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn save<'a>(
        &'a self,
        config: &'a ReviewConfig,
        review: &'a ReviewResult,
    ) -> Task<'a, Result<()>> {
        self.inner.save(config, review)
    }

    fn drain(&self) -> Task<'_, ()> {
        self.inner.drain()
    }
}
