//! Review core: analysis types, scoring, adaptive profiles and the analysis pipeline.
//!
//! Pure Rust: no Tauri, process spawn or database access. Engine and repository
//! ports are defined here; production adapters live in `engineroom-uci` and
//! `engineroom-sqlite`.

pub mod db;
pub mod review;
