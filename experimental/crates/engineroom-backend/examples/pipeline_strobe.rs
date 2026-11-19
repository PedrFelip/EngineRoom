//! Deterministic Strobe harness for selected stages of the review pipeline.
//!
//! It drives the production `Pipeline` through the public `EnginePort`,
//! `EngineFactory` and `Repository` seams. The fake engine makes the scenarios
//! reproducible without Stockfish; the real-engine scenarios live in the
//! existing `strobe_review.rs` example. Each scenario prints `strobe:metric`
//! lines so Strobe's `cli()` target can read counters alongside CPU and time.
use engineroom_application::db::mode::Mode;
use engineroom_backend::review::{
    core,
    engine::{Cancellation, EngineFactory, EnginePort, Task},
    pipeline::Pipeline,
    repository::{Repository, SqliteRepository},
    types::{
        AnalysisKind, EngineTier, Event, LiveRequest, LiveSettings, Result, ReviewConfig,
        ReviewError, ReviewErrorCode,
    },
};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::watch;

const DEFAULT_PGN: &str = "perf/data/immortal.pgn";
const SCENARIOS: &[&str] = &["pgn", "manual", "manual-warm", "live", "automatic"];

#[derive(Default)]
struct Counters {
    positions: usize,
    moves: usize,
    cached: usize,
    searched: usize,
    searches: usize,
    live: usize,
    classifications: usize,
}

impl Counters {
    fn print(&self) {
        // Strobe reads these lines from a cli() process. `higher` marks counters
        // where growth is the useful direction; the rest default to `lower`.
        for (name, value, higher) in [
            ("positions", self.positions, true),
            ("moves", self.moves, true),
            ("cached", self.cached, true),
            ("searched", self.searched, false),
            ("searches", self.searches, false),
            ("live", self.live, true),
            ("classifications", self.classifications, true),
        ] {
            if higher {
                println!("strobe:metric {name}={value} higher");
            } else {
                println!("strobe:metric {name}={value}");
            }
        }
    }
}

/// Stable pseudo-evaluation so a scenario's scores do not depend on a process id.
fn deterministic_cp(fen: &str) -> i32 {
    let digest = fen.bytes().fold(0u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    (digest % 81) as i32 - 40
}

#[derive(Default)]
struct FakeState {
    searches: AtomicUsize,
}

struct FakePort {
    state: Arc<FakeState>,
    queue: VecDeque<String>,
    fen: String,
    multipv: u32,
}

impl EnginePort for FakePort {
    fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, Result<()>> {
        Box::pin(async move {
            if command == "uci" {
                self.queue.push_back("id name Strobe Fake".into());
                self.queue.push_back("uciok".into());
            } else if command == "isready" {
                self.queue.push_back("readyok".into());
            } else if let Some(fen) = command.strip_prefix("position fen ") {
                self.fen = fen.into();
            } else if let Some(value) = command.strip_prefix("setoption name MultiPV value ") {
                let parsed = value.parse().map_err(|_| {
                    ReviewError::new(ReviewErrorCode::EngineProtocol, "fake", "MultiPV inválido")
                })?;
                self.multipv = parsed;
            } else if command.starts_with("go ") {
                self.state.searches.fetch_add(1, Ordering::SeqCst);
                let cp = deterministic_cp(&self.fen);
                for index in 1..=self.multipv {
                    self.queue.push_back(format!(
                        "info depth 12 multipv {index} score cp {cp} pv e2e4"
                    ));
                }
                self.queue.push_back("bestmove e2e4".into());
            }
            Ok(())
        })
    }

    fn next(&mut self) -> Task<'_, Result<String>> {
        Box::pin(async move {
            self.queue.pop_front().ok_or_else(|| {
                ReviewError::new(
                    ReviewErrorCode::EngineProtocol,
                    "fake.next",
                    "fila UCI vazia",
                )
            })
        })
    }

    fn shutdown(&mut self) -> Task<'_, ()> {
        Box::pin(async {})
    }
}

#[derive(Default)]
struct FakeFactory {
    state: Arc<FakeState>,
}

impl EngineFactory for FakeFactory {
    fn acquire<'a>(&'a self, _cancel: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        Box::pin(async move {
            let port: Box<dyn EnginePort> = Box::new(FakePort {
                state: self.state.clone(),
                queue: VecDeque::new(),
                fen: String::new(),
                multipv: 1,
            });
            Ok(port)
        })
    }
}

/// Isolated cache directory so repeated Strobe runs start from the same state.
struct TempDb {
    directory: PathBuf,
    path: PathBuf,
}

impl TempDb {
    fn new(scenario: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "engineroom-strobe-{}-{scenario}",
            std::process::id()
        ));
        // A stale directory from an interrupted run would change the counters.
        let _ = std::fs::remove_dir_all(&directory);
        let _ = std::fs::create_dir_all(&directory);
        Self {
            path: directory.join("review.db"),
            directory,
        }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn config(pgn: String, kind: AnalysisKind, mode: Mode) -> Result<ReviewConfig> {
    let meta = core::extract(&pgn)?.meta;
    Ok(ReviewConfig {
        pgn,
        meta,
        engine: EngineTier {
            id: "strobe-fake".into(),
            depth: 6,
            label: String::new(),
            hint: String::new(),
        },
        mode,
        analysis_kind: kind,
        movetime_ms: None,
        lines: 2,
        initial_result: None,
    })
}

fn cancellation() -> (watch::Sender<bool>, Cancellation) {
    let (sender, receiver) = watch::channel(false);
    (
        sender,
        Cancellation {
            closed: receiver,
            revision: None,
        },
    )
}

async fn review(
    config: &ReviewConfig,
    db: &TempDb,
    passes: usize,
    counters: &mut Counters,
) -> Result<usize> {
    let factory = Arc::new(FakeFactory::default());
    let repository: Arc<dyn Repository> = Arc::new(SqliteRepository::new(db.path.clone()));
    let mut pipeline = Pipeline::new(factory.clone(), repository);
    let (_sender, cancel) = cancellation();
    for pass in 0..passes {
        let mut cached = 0;
        let mut searched = 0;
        let result = pipeline
            .review(config, Some((1, 16)), &cancel, &mut |event| {
                if let Event::Progress { progress } = event {
                    cached = progress.cached_positions;
                    searched = progress.engine_positions;
                }
            })
            .await?;
        if pass + 1 == passes {
            counters.positions = result.positions.len();
            counters.moves = result.moves.len();
            counters.cached = cached;
            counters.searched = searched;
        }
    }
    let searches = factory.state.searches.load(Ordering::SeqCst);
    pipeline.close().await;
    Ok(searches)
}

async fn live(config: &ReviewConfig, db: &TempDb, counters: &mut Counters) -> Result<()> {
    let game = core::extract(&config.pgn)?;
    let request =
        LiveRequest {
            fen: game.fens.get(1).cloned().ok_or_else(|| {
                ReviewError::new(ReviewErrorCode::InvalidPgn, "pgn", "sem lances")
            })?,
            variation_node_id: Some("strobe-node".into()),
            source_fen: game.fens.first().cloned(),
            source_analysis: None,
        };
    let settings = LiveSettings {
        search_seconds: 1,
        lines: 2,
        threads_auto: false,
        threads: 1,
        memory_mb: 16,
        move_feedback_enabled: true,
        fast_pass: false,
    };
    let factory = Arc::new(FakeFactory::default());
    let repository: Arc<dyn Repository> = Arc::new(SqliteRepository::new(db.path.clone()));
    let mut pipeline = Pipeline::new(factory.clone(), repository);
    let (_sender, cancel) = cancellation();
    pipeline
        .live(&request, &settings, &cancel, &mut |event| match event {
            Event::LiveCompleted { .. } => counters.live += 1,
            Event::Classification { .. } => counters.classifications += 1,
            _ => {}
        })
        .await?;
    counters.searches = factory.state.searches.load(Ordering::SeqCst);
    pipeline.close().await;
    Ok(())
}

async fn run(scenario: &str, pgn_path: &Path) -> Result<Counters> {
    let pgn = std::fs::read_to_string(pgn_path).map_err(|error| {
        ReviewError::with_source(ReviewErrorCode::InvalidPgn, "pgn.read", error)
    })?;
    let mut counters = Counters::default();
    if scenario == "pgn" {
        let game = core::extract(&pgn)?;
        counters.positions = game.fens.len();
        counters.moves = game.moves.len();
        return Ok(counters);
    }
    let db = TempDb::new(scenario);
    match scenario {
        "manual" | "manual-warm" => {
            let config = config(pgn, AnalysisKind::Manual, Mode::Depth)?;
            let passes = usize::from(scenario == "manual-warm");
            counters.searches = review(&config, &db, passes + 1, &mut counters).await?;
        }
        "live" => {
            let config = config(pgn, AnalysisKind::Manual, Mode::Time)?;
            live(&config, &db, &mut counters).await?;
        }
        "automatic" => {
            let config = config(pgn, AnalysisKind::Fast, Mode::Time)?;
            counters.searches = review(&config, &db, 1, &mut counters).await?;
        }
        _ => unreachable!("scenario validado antes do despacho"),
    }
    Ok(counters)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let scenario = args.next().ok_or_else(|| {
        ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "args",
            "use: pipeline_strobe <pgn|manual|manual-warm|live|automatic> [pgn]",
        )
    })?;
    if !SCENARIOS.contains(&scenario.as_str()) {
        return Err(ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "args",
            format!("cenário desconhecido: {scenario}"),
        ));
    }
    let pgn = PathBuf::from(args.next().unwrap_or_else(|| DEFAULT_PGN.into()));
    let counters = run(&scenario, &pgn).await?;
    counters.print();
    Ok(())
}
