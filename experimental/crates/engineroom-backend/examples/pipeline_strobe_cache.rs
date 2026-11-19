//! Deterministic Strobe harness for partial cache and resume after interruption.
//!
//! This example drives the production `Pipeline` through its public
//! `EnginePort`/`EngineFactory`/`Repository` seams. It **never spawns Stockfish**:
//! the engine is an in-process fake whose evaluations are a pure function of the
//! FEN, so counters do not depend on the machine or on a sidecar binary. Real
//! Stockfish measurement belongs to `strobe_review.rs` / `real-game.bench.ts`
//! (opt-in via `ENGINE_ROOM_STOCKFISH`). The `fake_engine` metric marks the
//! distinction explicitly.
//!
//! Interruption is coordinated by a barrier, not by sleeping: the fake blocks
//! the search at `block_at` until a watcher flips cancellation. The pipeline's
//! failure flush persists the completed positions, a second pipeline on the
//! same isolated SQLite database resumes from them, and the harness asserts
//! result, counter and teardown invariants before printing `strobe:metric`
//! lines for Strobe's `cli()` target.

use engineroom_application::db::mode::Mode;
use engineroom_backend::review::{
    core,
    engine::{Cancellation, EngineFactory, EnginePort, Task},
    pipeline::Pipeline,
    repository::{Repository, SqliteRepository},
    types::{
        AnalysisKind, EngineTier, Event, PgnMeta, Result, ReviewConfig, ReviewError,
        ReviewErrorCode, ReviewResult,
    },
};
use shakmaty::{CastlingMode, Position};
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::watch;

const SCENARIOS: &[&str] = &["short", "long", "fen"];
const DEPTH: u32 = 6;
const LINES: u32 = 2;
const SIZE: (u32, u32) = (1, 16);

/// Stable pseudo-evaluation so a fixture's scores never depend on a process id.
fn deterministic_cp(fen: &str) -> i32 {
    let digest = fen.bytes().fold(0u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    (digest % 81) as i32 - 40
}

/// First legal UCI move for a FEN, so the fake PV is a real chess move and the
/// harness can assert legality instead of echoing a constant such as `e2e4`
/// (which is illegal from most positions in the fixtures).
fn legal_move(fen: &str) -> Result<String> {
    let chess = core::position(fen)?;
    let moves = chess.legal_moves();
    let mv = *moves.first().ok_or_else(|| {
        ReviewError::new(ReviewErrorCode::InvalidPgn, "legal_move", "sem lance legal")
    })?;
    Ok(mv.to_uci(CastlingMode::Standard).to_string())
}

/// Instrumentation shared by the fake engine, its factory and the harness.
struct FakeState {
    /// `go` commands issued, including the one parked at the barrier.
    go_count: AtomicUsize,
    /// Searches that produced a `bestmove`.
    completed: AtomicUsize,
    /// Publishes `go_count` so the driver can cancel deterministically.
    go_tx: watch::Sender<usize>,
    /// Search index at which `send("go …")` parks until cancellation.
    block_at: Option<usize>,
    /// Engine leases acquired (fake "processes").
    acquired: AtomicUsize,
    /// Leases not yet acknowledged by `shutdown()`.
    active: AtomicUsize,
    /// Leases acknowledged by `shutdown()`.
    terminated: AtomicUsize,
    /// High-water mark of concurrently active leases.
    peak_active: AtomicUsize,
}

impl FakeState {
    fn new(block_at: Option<usize>) -> Self {
        Self {
            go_count: AtomicUsize::new(0),
            completed: AtomicUsize::new(0),
            go_tx: watch::channel(0).0,
            block_at,
            acquired: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            terminated: AtomicUsize::new(0),
            peak_active: AtomicUsize::new(0),
        }
    }

    fn value(counter: &AtomicUsize) -> usize {
        counter.load(Ordering::SeqCst)
    }

    fn go(&self) -> usize {
        Self::value(&self.go_count)
    }

    fn completed(&self) -> usize {
        Self::value(&self.completed)
    }

    fn acquired(&self) -> usize {
        Self::value(&self.acquired)
    }

    fn active(&self) -> usize {
        Self::value(&self.active)
    }

    fn terminated(&self) -> usize {
        Self::value(&self.terminated)
    }

    fn peak_active(&self) -> usize {
        Self::value(&self.peak_active)
    }
}

/// In-process fake engine; no OS process, socket or sidecar is involved.
struct FakePort {
    state: Arc<FakeState>,
    queue: VecDeque<String>,
    fen: String,
    multipv: u32,
    shutdown_done: bool,
}

impl EnginePort for FakePort {
    fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, Result<()>> {
        Box::pin(async move {
            if command == "uci" {
                self.queue.push_back("id name Strobe Cache Fake".into());
                self.queue.push_back("uciok".into());
            } else if command == "isready" {
                self.queue.push_back("readyok".into());
            } else if let Some(fen) = command.strip_prefix("position fen ") {
                self.fen = fen.into();
            } else if let Some(value) = command.strip_prefix("setoption name MultiPV value ") {
                self.multipv = value.parse().map_err(|_| {
                    ReviewError::new(
                        ReviewErrorCode::EngineProtocol,
                        "fake.multipv",
                        "MultiPV inválido",
                    )
                })?;
            } else if command.starts_with("go ") {
                let index = self.state.go_count.fetch_add(1, Ordering::SeqCst) + 1;
                self.state.go_tx.send_replace(index);
                if self.state.block_at == Some(index) {
                    // Deterministic barrier. `engine::evaluate` selects on
                    // cancellation and drops this future; no timer is involved.
                    std::future::pending::<()>().await;
                }
                let cp = deterministic_cp(&self.fen);
                let pv = legal_move(&self.fen)?;
                for multipv in 1..=self.multipv {
                    self.queue.push_back(format!(
                        "info depth 12 multipv {multipv} score cp {cp} pv {pv}"
                    ));
                }
                self.queue.push_back(format!("bestmove {pv}"));
                self.state.completed.fetch_add(1, Ordering::SeqCst);
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
        Box::pin(async move {
            if self.shutdown_done {
                return;
            }
            self.shutdown_done = true;
            self.state.terminated.fetch_add(1, Ordering::SeqCst);
            self.state.active.fetch_sub(1, Ordering::SeqCst);
        })
    }
}

impl Drop for FakePort {
    fn drop(&mut self) {
        // Best-effort bookkeeping. Skipping `terminated` on this path keeps a
        // dropped-without-shutdown lease visible to the teardown assertion.
        if !self.shutdown_done {
            self.state.active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

struct FakeFactory {
    state: Arc<FakeState>,
}

impl EngineFactory for FakeFactory {
    fn acquire<'a>(&'a self, cancel: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        Box::pin(async move {
            cancel.check()?;
            self.state.acquired.fetch_add(1, Ordering::SeqCst);
            let active = self.state.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.state.peak_active.fetch_max(active, Ordering::SeqCst);
            let port = FakePort {
                state: Arc::clone(&self.state),
                queue: VecDeque::new(),
                fen: String::new(),
                multipv: 1,
                shutdown_done: false,
            };
            let port: Box<dyn EnginePort> = Box::new(port);
            Ok(port)
        })
    }
}

/// Isolated SQLite directory, recreated for every CLI invocation.
struct TempDb {
    directory: PathBuf,
    path: PathBuf,
}

impl TempDb {
    fn new(scenario: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let serial = NEXT.fetch_add(1, Ordering::SeqCst);
        let directory = std::env::temp_dir().join(format!(
            "engineroom-strobe-cache-{scenario}-{}-{serial}",
            std::process::id()
        ));
        // A stale directory from an interrupted run would change the counters.
        let _ = std::fs::remove_dir_all(&directory);
        if let Err(error) = std::fs::create_dir_all(&directory) {
            panic!("cannot create {}: {error}", directory.display());
        }
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

fn config(pgn: String, meta: PgnMeta) -> ReviewConfig {
    ReviewConfig {
        pgn,
        meta,
        engine: EngineTier {
            id: "strobe-cache-fake".into(),
            depth: DEPTH,
            label: String::new(),
            hint: String::new(),
        },
        mode: Mode::Depth,
        analysis_kind: AnalysisKind::Manual,
        movetime_ms: None,
        lines: LINES,
        initial_result: None,
    }
}

/// Outcome of the cold review that is stopped at the barrier.
struct Interrupted {
    state: Arc<FakeState>,
    go: usize,
    completed: usize,
    searched: usize,
    cached: usize,
}

async fn interrupt(config: &ReviewConfig, db: &TempDb, block_at: usize) -> Result<Interrupted> {
    // The blocked search is the (block_at + 1)-th: the first `block_at` searches
    // completed, were pushed to `pending` and are persisted by the failure flush.
    let block_index = block_at + 1;
    let state = Arc::new(FakeState::new(Some(block_index)));
    let factory: Arc<dyn EngineFactory> = Arc::new(FakeFactory {
        state: Arc::clone(&state),
    });
    let repository: Arc<dyn Repository> = Arc::new(SqliteRepository::new(db.path.clone()));
    let mut pipeline = Pipeline::new(factory, repository);
    let (cancel_tx, cancel) = cancellation();

    // `done` prevents the watcher from outliving the review if it errors early.
    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
    let watcher = tokio::spawn({
        let cancel_tx = cancel_tx.clone();
        let mut go_rx = state.go_tx.subscribe();
        async move {
            tokio::select! {
                reached = go_rx.wait_for(|index| *index >= block_index) => {
                    if reached.is_ok() {
                        let _ = cancel_tx.send(true);
                    }
                }
                _ = done_rx => {}
            }
        }
    });

    let mut searched = 0;
    let mut cached = 0;
    let outcome = pipeline
        .review(config, Some(SIZE), &cancel, &mut |event| {
            if let Event::Progress { progress } = event {
                searched = progress.engine_positions;
                cached = progress.cached_positions;
            }
        })
        .await;

    let _ = done_tx.send(());
    let _ = watcher.await;

    match outcome {
        Err(error) if error.code == ReviewErrorCode::Cancelled => {}
        Err(error) => return Err(error),
        Ok(_) => {
            return Err(ReviewError::new(
                ReviewErrorCode::Session,
                "interrupt",
                "a revisão deveria terminar cancelada no barreira",
            ));
        }
    }
    pipeline.close().await;

    Ok(Interrupted {
        go: state.go(),
        completed: state.completed(),
        state,
        searched,
        cached,
    })
}

/// Outcomes of the warm review that resumes from the partial cache.
struct Resumed {
    state: Arc<FakeState>,
    go: usize,
    cached: usize,
    searched: usize,
    result: ReviewResult,
}

async fn resume(config: &ReviewConfig, db: &TempDb) -> Result<Resumed> {
    let state = Arc::new(FakeState::new(None));
    let factory: Arc<dyn EngineFactory> = Arc::new(FakeFactory {
        state: Arc::clone(&state),
    });
    let repository: Arc<dyn Repository> = Arc::new(SqliteRepository::new(db.path.clone()));
    let mut pipeline = Pipeline::new(factory, repository);
    let (_cancel_tx, cancel) = cancellation();

    let mut cached = 0;
    let mut searched = 0;
    let result = pipeline
        .review(config, Some(SIZE), &cancel, &mut |event| {
            if let Event::Progress { progress } = event {
                cached = progress.cached_positions;
                searched = progress.engine_positions;
            }
        })
        .await?;
    pipeline.close().await;

    Ok(Resumed {
        go: state.go(),
        state,
        cached,
        searched,
        result,
    })
}

/// Cache keys persisted by the interrupted pass, read through a fresh connection.
async fn persisted_fens(db: &TempDb, game: &core::Game) -> Result<HashSet<String>> {
    let repository = SqliteRepository::new(db.path.clone());
    let hits = repository
        .lookup(&game.fens, Mode::Depth, DEPTH, LINES)
        .await?;
    Ok(game
        .fens
        .iter()
        .zip(hits)
        .filter_map(|(fen, hit)| hit.map(|_| fen.clone()))
        .collect())
}

/// Values derived from the fixture that the two passes must reproduce.
struct Expected {
    positions: usize,
    moves: usize,
    terminals: usize,
    search_positions: usize,
    block_at: usize,
    partial_cache_entries: usize,
    resume_cache_hits: usize,
    resume_searched: usize,
}

impl Expected {
    fn new(game: &core::Game, override_block_at: Option<usize>) -> Result<Self> {
        let terminal: Vec<bool> = game
            .positions
            .iter()
            .map(|position| core::terminal_position(position).is_some())
            .collect();
        let search_fens: Vec<&str> = game
            .fens
            .iter()
            .zip(&terminal)
            .filter(|(_, is_terminal)| !**is_terminal)
            .map(|(fen, _)| fen.as_str())
            .collect();
        let search_positions = search_fens.len();
        if search_positions < 2 {
            return Err(ReviewError::new(
                ReviewErrorCode::InvalidPgn,
                "expected",
                "o fixture precisa de pelo menos duas posições não terminais",
            ));
        }
        let block_at = override_block_at
            .unwrap_or(search_positions / 3)
            .clamp(1, search_positions - 1);
        let cached: HashSet<&str> = search_fens[..block_at].iter().copied().collect();
        let resume_cache_hits = search_fens
            .iter()
            .filter(|fen| cached.contains(**fen))
            .count();

        Ok(Self {
            positions: game.fens.len(),
            moves: game.moves.len(),
            terminals: terminal.iter().filter(|is_terminal| **is_terminal).count(),
            search_positions,
            block_at,
            partial_cache_entries: cached.len(),
            resume_cache_hits,
            resume_searched: search_positions - resume_cache_hits,
        })
    }
}

fn ensure(
    code: ReviewErrorCode,
    condition: bool,
    operation: &str,
    message: impl ToString,
) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(ReviewError::new(code, operation, message))
    }
}

/// Exact `strobe:metric` rows; `higher` marks counters where growth is useful.
#[derive(Default)]
struct Metrics {
    rows: Vec<(&'static str, usize, bool)>,
}

impl Metrics {
    fn push(&mut self, name: &'static str, value: usize, higher: bool) {
        self.rows.push((name, value, higher));
    }

    fn print(&self) {
        for (name, value, higher) in &self.rows {
            if *higher {
                println!("strobe:metric {name}={value} higher");
            } else {
                println!("strobe:metric {name}={value}");
            }
        }
    }
}

fn fixture_path(name: &str) -> Result<PathBuf> {
    let relative = format!("perf/fixtures/{name}.pgn");
    let mut candidates = Vec::new();
    if let Ok(directory) = std::env::var("STROBE_CACHE_FIXTURES") {
        candidates.push(PathBuf::from(directory).join(format!("{name}.pgn")));
    }
    candidates.push(PathBuf::from(&relative));
    candidates.push(PathBuf::from("experimental").join(&relative));
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            ReviewError::new(
                ReviewErrorCode::InvalidPayload,
                "fixture",
                format!("fixture não encontrada: {relative} (use STROBE_CACHE_FIXTURES)"),
            )
        })
}

/// Reads a fixture, runs both passes and asserts every counter and teardown rule.
async fn run(scenario: &str, pgn_path: &Path, override_block_at: Option<usize>) -> Result<Metrics> {
    let pgn = std::fs::read_to_string(pgn_path).map_err(|error| {
        ReviewError::with_source(ReviewErrorCode::InvalidPgn, "pgn.read", error)
    })?;
    let game = core::extract(&pgn)?;
    let expected = Expected::new(&game, override_block_at)?;
    let config = config(pgn, game.meta.clone());
    ensure(
        ReviewErrorCode::EngineProtocol,
        config.engine.id == "strobe-cache-fake",
        "verify.engine_kind",
        "este harness usa apenas a engine falsa, nunca Stockfish",
    )?;
    let db = TempDb::new(scenario);

    let interrupted = interrupt(&config, &db, expected.block_at).await?;
    let persisted = persisted_fens(&db, &game).await?;
    let resumed = resume(&config, &db).await?;

    ensure(
        ReviewErrorCode::Session,
        interrupted.searched == expected.block_at,
        "verify.interrupt_searched",
        format!(
            "searched={} esperado={}",
            interrupted.searched, expected.block_at
        ),
    )?;
    ensure(
        ReviewErrorCode::EngineProtocol,
        interrupted.completed == expected.block_at,
        "verify.interrupt_completed",
        format!(
            "completed={} esperado={}",
            interrupted.completed, expected.block_at
        ),
    )?;
    ensure(
        ReviewErrorCode::EngineProtocol,
        interrupted.go == expected.block_at + 1,
        "verify.interrupt_go",
        format!("go={} esperado={}", interrupted.go, expected.block_at + 1),
    )?;
    ensure(
        ReviewErrorCode::Session,
        interrupted.cached == 0,
        "verify.interrupt_cold",
        format!("cached={} esperado=0", interrupted.cached),
    )?;
    ensure(
        ReviewErrorCode::Cache,
        persisted.len() == expected.partial_cache_entries,
        "verify.partial_cache",
        format!(
            "entradas={} esperado={}",
            persisted.len(),
            expected.partial_cache_entries
        ),
    )?;
    ensure(
        ReviewErrorCode::Cache,
        resumed.cached == expected.resume_cache_hits,
        "verify.resume_hits",
        format!(
            "cached={} esperado={}",
            resumed.cached, expected.resume_cache_hits
        ),
    )?;
    ensure(
        ReviewErrorCode::Session,
        resumed.searched == expected.resume_searched && resumed.go == expected.resume_searched,
        "verify.resume_searches",
        format!(
            "searched={} go={} esperado={}",
            resumed.searched, resumed.go, expected.resume_searched
        ),
    )?;
    ensure(
        ReviewErrorCode::InvalidPgn,
        resumed.result.positions.len() == expected.positions
            && resumed.result.moves.len() == expected.moves,
        "verify.result_shape",
        format!(
            "positions={} moves={} esperado={}/{}",
            resumed.result.positions.len(),
            resumed.result.moves.len(),
            expected.positions,
            expected.moves
        ),
    )?;

    let mut deterministic = true;
    for position in &resumed.result.positions {
        let matches = match core::terminal(&position.fen)? {
            Some(cp) => position.cp == cp && position.pv.is_empty() && position.depth == 0,
            None => {
                let want = deterministic_cp(&position.fen);
                let white = position.fen.split_whitespace().nth(1) != Some("b");
                let line_cp = if white { want } else { want.saturating_neg() };
                let pv = legal_move(&position.fen)?;
                position.cp == want
                    && position.depth == 12
                    && position.pv.len() == 1
                    && position.pv[0] == pv
                    && position.lines.iter().all(|line| line.cp == line_cp)
            }
        };
        if !matches {
            deterministic = false;
            break;
        }
    }
    ensure(
        ReviewErrorCode::EngineProtocol,
        deterministic,
        "verify.deterministic",
        "alguma posição não corresponde à avaliação determinística",
    )?;

    let acquired = interrupted.state.acquired() + resumed.state.acquired();
    let terminated = interrupted.state.terminated() + resumed.state.terminated();
    let active = interrupted.state.active() + resumed.state.active();
    let peak = interrupted
        .state
        .peak_active()
        .max(resumed.state.peak_active());
    let teardown_ok = terminated > 0 && acquired == terminated && active == 0 && peak <= acquired;
    ensure(
        ReviewErrorCode::Session,
        teardown_ok,
        "verify.teardown",
        format!("acquired={acquired} terminated={terminated} active={active} peak={peak}"),
    )?;

    let mut metrics = Metrics::default();
    metrics.push("fixture_positions", expected.positions, true);
    metrics.push("fixture_moves", expected.moves, true);
    metrics.push("fixture_terminals", expected.terminals, false);
    metrics.push("fixture_search_positions", expected.search_positions, false);
    metrics.push("interrupt_after", expected.block_at, false);
    metrics.push("interrupt_go", interrupted.go, false);
    metrics.push("interrupt_completed", interrupted.completed, false);
    metrics.push("interrupt_progress_searched", interrupted.searched, false);
    metrics.push("interrupt_progress_cached", interrupted.cached, false);
    metrics.push("partial_cache_entries", persisted.len(), true);
    metrics.push("resume_cache_hits", resumed.cached, true);
    metrics.push("resume_searched", resumed.searched, false);
    metrics.push("resume_go", resumed.go, false);
    metrics.push("result_positions", resumed.result.positions.len(), true);
    metrics.push("result_moves", resumed.result.moves.len(), true);
    metrics.push("deterministic_ok", usize::from(deterministic), true);
    metrics.push("ports_acquired", acquired, false);
    metrics.push("ports_terminated", terminated, false);
    metrics.push("ports_active", active, false);
    metrics.push("ports_peak_active", peak, false);
    metrics.push("teardown_ok", usize::from(teardown_ok), true);
    // Marks this harness as fake-engine only; Stockfish lives in strobe_review.
    metrics.push("fake_engine", 1, true);

    eprintln!(
        "scenario={scenario} fixture={} positions={} moves={} terminals={} block_at={} partial={} resume_hits={} resume_searched={} ports={acquired}/{terminated} active={active}",
        pgn_path.display(),
        expected.positions,
        expected.moves,
        expected.terminals,
        expected.block_at,
        persisted.len(),
        resumed.cached,
        resumed.searched,
    );
    Ok(metrics)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let scenario = args.next().ok_or_else(|| {
        ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "args",
            "use: pipeline_strobe_cache <short|long|fen> [pgn] [interrupt_after]",
        )
    })?;
    if !SCENARIOS.contains(&scenario.as_str()) {
        return Err(ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "args",
            format!("cenário desconhecido: {scenario}"),
        ));
    }
    let pgn_path = match args.next() {
        Some(path) => PathBuf::from(path),
        None => fixture_path(&scenario)?,
    };
    let override_block_at = args
        .next()
        .map(|value| {
            value.parse::<usize>().map_err(|_| {
                ReviewError::new(
                    ReviewErrorCode::InvalidPayload,
                    "args",
                    format!("interrupt_after inválido: {value}"),
                )
            })
        })
        .transpose()?;

    let metrics = run(&scenario, &pgn_path, override_block_at).await?;
    metrics.print();
    Ok(())
}
