//! Strobe-only harness for the production manual and automatic review pipelines.
use engineroom_application::db::mode::Mode;
use engineroom_backend::review::{
    core,
    engine::Cancellation,
    native_engine::NativeEngineFactory,
    pipeline::Pipeline,
    repository::SqliteRepository,
    session::{Resources, ReviewSessions, SchedulerMode},
    types::{AnalysisKind, EngineTier, Event, LiveRequest, LiveSettings, ReviewConfig},
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

async fn run_session_auto(
    engine: PathBuf,
    dir: PathBuf,
    pgn: String,
    meta: engineroom_backend::review::types::PgnMeta,
    fens: Vec<String>,
    mode: SchedulerMode,
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis_kind = match std::env::var("STROBE_REVIEW_PROFILE").as_deref() {
        Ok("deep") => AnalysisKind::Deep,
        Ok("fast") | Err(std::env::VarError::NotPresent) => AnalysisKind::Fast,
        _ => return Err("expected STROBE_REVIEW_PROFILE=fast or deep".into()),
    };
    let threads = match std::env::var("STROBE_REVIEW_THREADS") {
        Ok(value) => value.parse::<u32>()?,
        Err(std::env::VarError::NotPresent) => 4,
        Err(error) => return Err(error.into()),
    };
    let config = ReviewConfig {
        pgn,
        meta,
        engine: EngineTier {
            id: "strobe".into(),
            depth: 6,
            label: String::new(),
            hint: String::new(),
        },
        mode: Mode::Time,
        analysis_kind,
        movetime_ms: None,
        lines: 3,
        initial_result: None,
    };
    let repository = Arc::new(SqliteRepository::new(dir.join("review.db")));
    let factory = Arc::new(NativeEngineFactory::new(engine));
    let sessions = ReviewSessions::new(
        factory,
        repository,
        Some(Resources {
            threads,
            memory_mb: 2048,
        }),
    )
    .with_scheduler(mode);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let started = Instant::now();
    sessions.open("strobe", "main", config, move |envelope| {
        tx.send(envelope.event).is_ok()
    })?;

    let target_fen = fens
        .get(fens.len().saturating_sub(4))
        .ok_or("expected game positions")?
        .clone();
    let mut live_sent = false;
    let mut live_sent_at: Option<Instant> = None;
    let mut live_latency_ms = 0u128;
    let mut live_before_completed: Option<usize> = None;
    let mut cached = 0usize;
    let mut searched = 0usize;
    let mut positions = 0usize;
    let mut moves = 0usize;
    let mut first_pair_ms = None;
    let mut triage_seen = vec![false; fens.len()];
    let mut review_ms = None;
    let mut completed = false;
    let mut live_done = false;

    tokio::time::timeout(Duration::from_secs(120), async {
        while !completed || !live_done {
            let Some(event) = rx.recv().await else {
                return Err("session channel closed".into());
            };
            match event {
                Event::Progress { progress } => {
                    cached = progress.cached_positions;
                    searched = progress.engine_positions;
                    if first_pair_ms.is_none()
                        && progress.stage == "triage"
                        && let Some(update) = progress.update
                    {
                        let index = update.index;
                        triage_seen[index] = true;
                        if (index > 0 && triage_seen[index - 1])
                            || triage_seen.get(index + 1) == Some(&true)
                        {
                            first_pair_ms = Some(started.elapsed().as_millis());
                        }
                    }
                    if !live_sent && progress.engine_positions >= 1 {
                        live_sent = true;
                        live_sent_at = Some(Instant::now());
                        sessions.analyze_position(
                            "strobe",
                            "main",
                            1,
                            LiveRequest {
                                fen: target_fen.clone(),
                                variation_node_id: None,
                                source_fen: None,
                                source_analysis: None,
                            },
                            LiveSettings {
                                search_seconds: 1,
                                lines: 1,
                                threads_auto: false,
                                threads: 2,
                                memory_mb: 256,
                                move_feedback_enabled: false,
                                fast_pass: true,
                            },
                        )?;
                    }
                }
                Event::Completed { result } => {
                    review_ms = Some(started.elapsed().as_millis());
                    positions = result.positions.len();
                    moves = result.moves.len();
                    if live_before_completed.is_none() {
                        live_before_completed = Some(0);
                    }
                    completed = true;
                }
                Event::LiveCompleted { .. } => {
                    live_latency_ms = live_sent_at
                        .map(|started| started.elapsed().as_millis())
                        .unwrap_or(0);
                    if live_before_completed.is_none() {
                        live_before_completed = Some(1);
                    }
                    live_done = true;
                }
                Event::Error { error, .. } => {
                    return Err(Box::new(error) as Box<dyn std::error::Error>);
                }
                _ => {}
            }
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })
    .await??;

    sessions.close("strobe", "main").await?;
    sessions.shutdown().await;
    let live_order = live_before_completed.unwrap_or(0);
    let first_pair_ms = first_pair_ms.ok_or("expected adjacent triage evaluations")?;
    let review_ms = review_ms.ok_or("expected review completion time")?;
    println!("strobe:metric first_pair_ms={first_pair_ms} lower");
    println!("strobe:metric review_ms={review_ms} lower");
    println!("strobe:metric positions={positions} higher");
    println!("strobe:metric moves={moves} higher");
    println!("strobe:metric cached={cached} higher");
    println!("strobe:metric searched={searched}");
    println!("strobe:metric live=1 higher");
    println!("strobe:metric live_before_completed={live_order} higher");
    println!("strobe:metric live_latency_ms={live_latency_ms}");
    println!(
        "mode={mode:?} positions={positions} moves={moves} cached={cached} searched={searched} live_before_completed={live_order} live_latency_ms={live_latency_ms}"
    );
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = PathBuf::from(
        std::env::var_os("ENGINE_ROOM_STOCKFISH").ok_or("set ENGINE_ROOM_STOCKFISH")?,
    );
    let scenario = std::env::args().nth(1).unwrap_or_else(|| "fresh".into());
    if ![
        "fresh",
        "warm",
        "seed",
        "reuse",
        "pipeline-auto",
        "native-auto",
    ]
    .contains(&scenario.as_str())
    {
        return Err("expected fresh, warm, seed, reuse, pipeline-auto or native-auto".into());
    }
    let pgn_path = std::env::var_os("STROBE_REVIEW_PGN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("perf/data/immortal.pgn"));
    let pgn = std::fs::read_to_string(pgn_path)?;
    let game = core::extract(&pgn)?;
    let dir = PathBuf::from(format!("target/strobe-runs/{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    if scenario == "pipeline-auto" || scenario == "native-auto" {
        let mode = if scenario == "native-auto" {
            SchedulerMode::Native
        } else {
            SchedulerMode::Pipeline
        };
        let result = run_session_auto(engine, dir.clone(), pgn, game.meta, game.fens, mode).await;
        let _ = std::fs::remove_dir_all(&dir);
        return result;
    }
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
