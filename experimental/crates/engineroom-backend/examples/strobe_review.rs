//! Strobe-only harness for the production manual review pipeline.
use engineroom_backend::review::{
    core,
    engine::Cancellation,
    native_engine::NativeEngineFactory,
    pipeline::Pipeline,
    repository::SqliteRepository,
    types::{AnalysisKind, EngineTier, Event, ReviewConfig},
};
use engineroom_domain::db::mode::Mode;
use std::{path::PathBuf, sync::Arc};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = PathBuf::from(
        std::env::var_os("ENGINE_ROOM_STOCKFISH").ok_or("set ENGINE_ROOM_STOCKFISH")?,
    );
    let scenario = std::env::args().nth(1).unwrap_or_else(|| "fresh".into());
    if !["fresh", "warm", "seed", "reuse"].contains(&scenario.as_str()) {
        return Err("expected fresh, warm, seed or reuse".into());
    }
    let pgn = std::fs::read_to_string("perf/data/immortal.pgn")?;
    let game = core::extract(&pgn)?;
    let config = ReviewConfig {
        pgn,
        meta: game.meta,
        engine: EngineTier {
            id: "strobe".into(),
            depth: 6,
            label: String::new(),
            hint: String::new(),
        },
        mode: Mode::Depth,
        analysis_kind: AnalysisKind::Manual,
        movetime_ms: None,
        lines: 1,
        initial_result: None,
    };
    let dir = PathBuf::from(format!("target/strobe-runs/{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let db = if scenario == "seed" || scenario == "reuse" {
        PathBuf::from(std::env::var_os("STROBE_REVIEW_DB").ok_or("set STROBE_REVIEW_DB")?)
    } else {
        dir.join("review.db")
    };
    if (scenario == "reuse") != db.exists() {
        return Err("unexpected database state".into());
    }
    let repository = Arc::new(SqliteRepository::new(db));
    let factory = Arc::new(NativeEngineFactory::new(engine));
    let mut pipeline = Pipeline::new(factory, repository);
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let cancellation = Cancellation {
        closed: rx,
        revision: None,
    };
    for pass in 0..if scenario == "warm" { 2 } else { 1 } {
        let mut cached = 0;
        let mut searched = 0;
        let result = pipeline
            .review(&config, Some((1, 16)), &cancellation, &mut |event| {
                if let Event::Progress { progress } = event {
                    cached = progress.cached_positions;
                    searched = progress.engine_positions;
                }
            })
            .await?;
        println!(
            "pass={} positions={} moves={} cached={} searched={}",
            pass + 1,
            result.positions.len(),
            result.moves.len(),
            cached,
            searched
        );
        if result.positions.len() != 46 || result.moves.len() != 45 {
            return Err("unexpected game shape".into());
        }
        if scenario != "reuse" && pass == 0 && (cached != 0 || searched != 45) {
            return Err("fresh review did not invoke engine".into());
        }
        if (scenario == "reuse" || pass == 1) && (cached != 45 || searched != 0) {
            return Err("warm review missed SQLite cache".into());
        }
    }
    pipeline.close().await;
    std::fs::remove_dir_all(dir)?;
    Ok(())
}
