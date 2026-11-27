//! Deterministic Strobe harness for the window-owned `ReviewSessions`.
//!
//! It drives the production session manager through the public `EngineFactory`,
//! `EnginePort` and `Repository` seams. A controllable in-process fake engine
//! gates every `go`, so cancellation and latest-wins are synchronized by
//! channels and a generation gate rather than sleeps. No Stockfish or other OS
//! process is started: the "ports" and "permits" counted below are Rust values,
//! not processes. Watchdog timeouts only detect hangs; they never pace a
//! scenario. Each scenario prints `strobe:metric` lines for the Strobe `cli()`
//! target, alongside CPU and wall time.
use engineroom_backend::review::{
    core,
    engine::{Cancellation, EngineFactory, EnginePort, Task},
    repository::Repository,
    session::{Resources, ReviewSessions},
    types::{
        AnalysisKind, EngineTier, Envelope, Event, LiveRequest, LiveSettings, Result, ReviewConfig,
        ReviewError, ReviewErrorCode,
    },
};
use engineroom_domain::db::mode::Mode;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};

const SCENARIOS: &[&str] = &[
    "cancel-analysis",
    "navigation-latest-wins",
    "session-lifecycle",
];
/// Hang detector only. A healthy run never reaches it.
const WATCHDOG: Duration = Duration::from_secs(5);
/// Short inline PGN keeps this harness independent of shared fixtures.
const SHORT_PGN: &str = "1. e4 e5 2. Nf3 Nc6";
/// Longer inline PGN supplies distinct FENs for rapid navigation.
const NAV_PGN: &str = "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6";

/// Stable pseudo-evaluation derived from the FEN, so scores are machine-independent.
fn deterministic_cp(fen: &str) -> i32 {
    let digest = fen.bytes().fold(0u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    (digest % 81) as i32 - 40
}

// ---------------------------------------------------------------------------
// Controllable fake engine (in-process, no OS process)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct EngineCounts {
    acquires: AtomicUsize,
    searches: AtomicUsize,
    shutdowns: AtomicUsize,
    active: AtomicUsize,
    peak_active: AtomicUsize,
}

#[derive(Debug, Clone, Copy)]
struct Snapshot {
    searches: usize,
    acquires: usize,
    shutdowns: usize,
    active: usize,
    peak_active: usize,
}

struct Inner {
    counts: Arc<EngineCounts>,
    /// One permit models the exclusive engine lease shared with other consumers.
    lease: Arc<Semaphore>,
    /// Signals `(generation, fen)` when a search enters `go`.
    started: mpsc::UnboundedSender<(u64, String)>,
    /// Searches with generation <= this value are allowed to produce output.
    gate: watch::Sender<u64>,
}

/// Production `EngineFactory` seam backed by an in-process, gated fake port.
struct FakeFactory(Arc<Inner>);

impl FakeFactory {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<(u64, String)>) {
        let (started, receiver) = mpsc::unbounded_channel();
        let factory = Arc::new(Self(Arc::new(Inner {
            counts: Arc::new(EngineCounts::default()),
            lease: Arc::new(Semaphore::new(1)),
            started,
            gate: watch::channel(0).0,
        })));
        (factory, receiver)
    }

    /// Releases all searches up to and including `generation`.
    fn release(&self, generation: u64) {
        self.0.gate.send_replace(generation);
    }

    fn snapshot(&self) -> Snapshot {
        let counts = self.0.counts.as_ref();
        Snapshot {
            searches: counts.searches.load(Ordering::SeqCst),
            acquires: counts.acquires.load(Ordering::SeqCst),
            shutdowns: counts.shutdowns.load(Ordering::SeqCst),
            active: counts.active.load(Ordering::SeqCst),
            peak_active: counts.peak_active.load(Ordering::SeqCst),
        }
    }

    fn available_permits(&self) -> usize {
        self.0.lease.available_permits()
    }
}

impl EngineFactory for FakeFactory {
    fn acquire<'a>(&'a self, cancel: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        let inner = self.0.clone();
        Box::pin(async move {
            let permit = tokio::select! {
                _ = cancel.cancelled() => return Err(ReviewError::cancelled()),
                acquired = inner.lease.clone().acquire_owned() => acquired.expect("lease not closed"),
            };
            inner.counts.acquires.fetch_add(1, Ordering::SeqCst);
            let active = inner.counts.active.fetch_add(1, Ordering::SeqCst) + 1;
            inner.counts.peak_active.fetch_max(active, Ordering::SeqCst);
            Ok(Box::new(GatePort {
                inner: inner.clone(),
                permit: Some(permit),
                queue: VecDeque::new(),
                fen: String::new(),
                multipv: 1,
                blocking: None,
            }) as Box<dyn EnginePort>)
        })
    }
}

struct GatePort {
    inner: Arc<Inner>,
    permit: Option<OwnedSemaphorePermit>,
    queue: VecDeque<String>,
    fen: String,
    multipv: u32,
    /// Generation whose gate must open before the first queued line is delivered.
    blocking: Option<u64>,
}

impl EnginePort for GatePort {
    fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, Result<()>> {
        Box::pin(async move {
            if command == "uci" {
                self.queue.push_back("id name Strobe Gate".into());
                self.queue.push_back("uciok".into());
            } else if command == "isready" {
                self.queue.push_back("readyok".into());
            } else if let Some(fen) = command.strip_prefix("position fen ") {
                self.fen = fen.into();
            } else if let Some(value) = command.strip_prefix("setoption name MultiPV value ") {
                self.multipv = value.parse().map_err(|_| {
                    ReviewError::new(ReviewErrorCode::EngineProtocol, "fake", "MultiPV inválido")
                })?;
            } else if command.starts_with("go ") {
                let generation =
                    (self.inner.counts.searches.fetch_add(1, Ordering::SeqCst) + 1) as u64;
                self.blocking = Some(generation);
                let _ = self.inner.started.send((generation, self.fen.clone()));
                let cp = deterministic_cp(&self.fen);
                // Fixed sentinel: this harness asserts counters, never PV values.
                // The cache harness generates a chess-legal PV from the FEN.
                for index in 1..=self.multipv {
                    let offset = i32::try_from(index).expect("multipv index fits i32") - 1;
                    let score = cp - offset * 20;
                    self.queue.push_back(format!(
                        "info depth 12 multipv {index} score cp {score} pv e2e4"
                    ));
                }
                self.queue.push_back("bestmove e2e4".into());
            }
            Ok(())
        })
    }

    fn next(&mut self) -> Task<'_, Result<String>> {
        Box::pin(async move {
            if let Some(generation) = self.blocking.take() {
                let mut gate = self.inner.gate.subscribe();
                gate.wait_for(|allowed| *allowed >= generation)
                    .await
                    .expect("gate sender must stay alive");
            }
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
            self.inner.counts.shutdowns.fetch_add(1, Ordering::SeqCst);
            self.inner.counts.active.fetch_sub(1, Ordering::SeqCst);
            if let Some(permit) = self.permit.take() {
                drop(permit);
            }
        })
    }
}

/// No-hit cache: every new FEN reaches the fake engine, so search counts are exact.
#[derive(Default)]
struct NoCacheRepo {
    drained: AtomicBool,
}

impl Repository for NoCacheRepo {
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        _: Mode,
        _: u32,
        _: u32,
    ) -> Task<'a, Result<Vec<Option<engineroom_backend::review::types::RawPosition>>>> {
        Box::pin(async move { Ok(vec![None; fens.len()]) })
    }

    fn put<'a>(
        &'a self,
        _: &'a [engineroom_backend::review::types::RawPosition],
        _: Mode,
        _: u32,
        _: u32,
    ) -> Task<'a, Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn save<'a>(
        &'a self,
        _: &'a ReviewConfig,
        _: &'a engineroom_backend::review::types::ReviewResult,
    ) -> Task<'a, Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn drain(&self) -> Task<'_, ()> {
        Box::pin(async move {
            self.drained.store(true, Ordering::SeqCst);
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn config(pgn: &str) -> Result<ReviewConfig> {
    let game = core::extract(pgn)?;
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let initial = core::build(&game, &raw)?;
    Ok(ReviewConfig {
        pgn: pgn.into(),
        meta: game.meta,
        engine: EngineTier {
            id: "strobe-fake".into(),
            depth: 6,
            label: String::new(),
            hint: String::new(),
        },
        mode: Mode::Depth,
        analysis_kind: AnalysisKind::Manual,
        movetime_ms: None,
        lines: 2,
        // Skips the initial full review: these scenarios target live navigation.
        initial_result: Some(initial),
    })
}

fn settings() -> LiveSettings {
    LiveSettings {
        search_seconds: 1,
        lines: 2,
        threads_auto: false,
        threads: 1,
        memory_mb: 16,
        move_feedback_enabled: false,
        fast_pass: false,
    }
}

fn request(fen: &str) -> LiveRequest {
    LiveRequest {
        fen: fen.into(),
        variation_node_id: None,
        source_fen: None,
        source_analysis: None,
    }
}

fn resources() -> Option<Resources> {
    Some(Resources {
        threads: 3,
        memory_mb: 256,
    })
}

fn forward(tx: mpsc::UnboundedSender<Envelope>) -> impl FnMut(Envelope) -> bool + Send + 'static {
    move |envelope| {
        let _ = tx.send(envelope);
        true
    }
}

async fn await_started(started: &mut mpsc::UnboundedReceiver<(u64, String)>) -> (u64, String) {
    tokio::time::timeout(WATCHDOG, started.recv())
        .await
        .expect("watchdog: search never started (hang)")
        .expect("fake engine start channel closed")
}

/// Records every published envelope in order while waiting for a specific one.
struct EventLog {
    rx: mpsc::UnboundedReceiver<Envelope>,
    all: Vec<Envelope>,
}

impl EventLog {
    fn new(rx: mpsc::UnboundedReceiver<Envelope>) -> Self {
        Self {
            rx,
            all: Vec::new(),
        }
    }

    async fn wait(&mut self, wanted: impl Fn(&Envelope) -> bool) -> Envelope {
        tokio::time::timeout(WATCHDOG, async {
            loop {
                let event = self.rx.recv().await.expect("event channel closed");
                let matched = wanted(&event);
                self.all.push(event.clone());
                if matched {
                    return event;
                }
            }
        })
        .await
        .expect("watchdog: expected event never arrived (hang)")
    }

    fn drain(mut self) -> Vec<Envelope> {
        while let Ok(event) = self.rx.try_recv() {
            self.all.push(event);
        }
        self.all
    }
}

/// Models a UI consumer that drops events whose request id is older than the
/// navigation it last asked for, while checking the IPC envelope invariants.
struct Reducer {
    session_id: String,
    max_request: u64,
    sequence: u64,
    accepted: usize,
    stale: usize,
    wrong_session: usize,
    sequence_violations: usize,
    completions: Vec<u64>,
}

impl Reducer {
    fn new(session_id: &str, frontend_latest: u64) -> Self {
        Self {
            session_id: session_id.into(),
            max_request: frontend_latest,
            sequence: 0,
            accepted: 0,
            stale: 0,
            wrong_session: 0,
            sequence_violations: 0,
            completions: Vec::new(),
        }
    }

    fn fold(&mut self, events: &[Envelope]) {
        for envelope in events {
            if envelope.session_id != self.session_id {
                self.wrong_session += 1;
                continue;
            }
            if envelope.sequence <= self.sequence {
                self.sequence_violations += 1;
            }
            self.sequence = envelope.sequence;
            match envelope.request_id {
                None => self.accepted += 1,
                Some(id) if id >= self.max_request => {
                    self.max_request = id;
                    self.accepted += 1;
                    if matches!(envelope.event, Event::LiveCompleted { .. }) {
                        self.completions.push(id);
                    }
                }
                Some(_) => self.stale += 1,
            }
        }
    }
}

#[derive(Default)]
struct Counters {
    sessions_opened: usize,
    jobs_enqueued: usize,
    searches: usize,
    acquires: usize,
    shutdowns: usize,
    peak_active: usize,
    active: usize,
    permits: usize,
    live_started: usize,
    live_completed: usize,
    latest_completed: usize,
    stale_completed: usize,
    stale_events: usize,
    accepted_events: usize,
    max_sequence: usize,
    cancels_acked: usize,
    closes_acked: usize,
    auth_rejected: usize,
    shutdown_rejects: usize,
    repo_drained: usize,
    wrong_session: usize,
    sequence_violations: usize,
}

impl Counters {
    fn finish(
        &mut self,
        engine: &FakeFactory,
        repo: &NoCacheRepo,
        events: Vec<Envelope>,
        session_id: &str,
        frontend_latest: u64,
        expected_completions: &[u64],
    ) {
        let snapshot = engine.snapshot();
        self.searches = snapshot.searches;
        self.acquires = snapshot.acquires;
        self.shutdowns = snapshot.shutdowns;
        self.peak_active = snapshot.peak_active;
        self.active = snapshot.active;
        self.permits = engine.available_permits();
        self.repo_drained = usize::from(repo.drained.load(Ordering::SeqCst));

        self.live_started = events
            .iter()
            .filter(|event| matches!(event.event, Event::LiveStarted { .. }))
            .count();
        self.live_completed = events
            .iter()
            .filter(|event| matches!(event.event, Event::LiveCompleted { .. }))
            .count();
        self.stale_completed = events
            .iter()
            .filter(|event| matches!(event.event, Event::LiveCompleted { .. }))
            .filter(|event| {
                event
                    .request_id
                    .is_some_and(|id| !expected_completions.contains(&id))
            })
            .count();

        let mut reducer = Reducer::new(session_id, frontend_latest);
        reducer.fold(&events);
        self.accepted_events = reducer.accepted;
        self.stale_events = reducer.stale;
        self.wrong_session = reducer.wrong_session;
        self.sequence_violations = reducer.sequence_violations;
        self.max_sequence = reducer.sequence as usize;
        self.latest_completed = reducer
            .completions
            .iter()
            .filter(|id| expected_completions.contains(id))
            .count();
    }

    /// Asserts the invariants every scenario must satisfy after teardown.
    fn assert_teardown(&self, expected_completions: &[u64]) {
        assert_eq!(self.active, 0, "no fake port may remain active");
        assert!(self.peak_active <= 1, "engine lease must stay exclusive");
        assert_eq!(self.permits, 1, "exclusive lease must be released");
        assert_eq!(
            self.acquires, self.shutdowns,
            "every port must be shut down"
        );
        assert_eq!(self.repo_drained, 1, "repository must be drained on close");
        assert_eq!(self.wrong_session, 0, "all envelopes must name the session");
        assert_eq!(
            self.sequence_violations, 0,
            "envelope sequences must increase"
        );
        assert_eq!(self.stale_completed, 0, "no stale result may be emitted");
        assert_eq!(
            self.latest_completed,
            expected_completions.len(),
            "only expected completions may be accepted"
        );
    }

    fn print(&self) {
        for (name, value, higher) in [
            // Self-describing marker: no Stockfish/OS process is involved here.
            ("fake_engine", 1, true),
            ("sessions_opened", self.sessions_opened, true),
            ("jobs_enqueued", self.jobs_enqueued, true),
            ("searches", self.searches, true),
            ("acquires", self.acquires, true),
            ("shutdowns", self.shutdowns, true),
            ("peak_active", self.peak_active, false),
            ("active", self.active, false),
            ("permits", self.permits, true),
            ("live_started", self.live_started, true),
            ("live_completed", self.live_completed, true),
            ("latest_completed", self.latest_completed, true),
            ("stale_completed", self.stale_completed, false),
            ("stale_events", self.stale_events, true),
            ("accepted_events", self.accepted_events, true),
            ("max_sequence", self.max_sequence, true),
            ("cancels_acked", self.cancels_acked, true),
            ("closes_acked", self.closes_acked, true),
            ("auth_rejected", self.auth_rejected, true),
            ("shutdown_rejects", self.shutdown_rejects, true),
            ("repo_drained", self.repo_drained, true),
            ("wrong_session", self.wrong_session, false),
            ("sequence_violations", self.sequence_violations, false),
        ] {
            if higher {
                println!("strobe:metric {name}={value} higher");
            } else {
                println!("strobe:metric {name}={value}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

/// Cancels a live search that is blocked inside the fake engine.
async fn cancel_analysis(pgn: &str) -> Result<Counters> {
    let config = config(pgn)?;
    let fens = core::extract(pgn)?.fens;
    let fen = fens
        .get(1)
        .ok_or_else(|| ReviewError::new(ReviewErrorCode::InvalidPgn, "pgn", "sem lances"))?;

    let (engine, mut started) = FakeFactory::new();
    let repo = Arc::new(NoCacheRepo::default());
    let sessions = ReviewSessions::new(engine.clone(), repo.clone(), resources());
    let (tx, rx) = mpsc::unbounded_channel();
    sessions.open("cancel", "window-a", config, forward(tx))?;

    let mut counters = Counters {
        sessions_opened: 1,
        ..Default::default()
    };
    sessions.analyze_position("cancel", "window-a", 1, request(fen), settings())?;
    counters.jobs_enqueued = 1;

    let (generation, _) = await_started(&mut started).await;
    assert_eq!(generation, 1, "first navigation must start one search");
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.active, 1, "search must hold the exclusive lease");
    assert_eq!(
        engine.available_permits(),
        0,
        "lease must be held in flight"
    );

    // A newer revision with no job invalidates the in-flight search and waits
    // for its acknowledged cleanup before returning.
    sessions.cancel_live("cancel", "window-a", 2).await?;
    counters.cancels_acked = 1;
    sessions.close("cancel", "window-a").await?;
    counters.closes_acked = 1;

    let events = EventLog::new(rx).drain();
    counters.finish(&engine, &repo, events, "cancel", 0, &[]);
    counters.assert_teardown(&[]);
    assert_eq!(counters.searches, 1, "cancelled search must not be retried");
    assert_eq!(
        counters.live_completed, 0,
        "cancelled search must not complete"
    );
    Ok(counters)
}

/// Rapid monotonic navigation: only the newest request reaches a result.
async fn navigation_latest_wins(pgn: &str) -> Result<Counters> {
    const LATEST: u64 = 4;
    let config = config(pgn)?;
    let fens = core::extract(pgn)?.fens;

    let (engine, mut started) = FakeFactory::new();
    let repo = Arc::new(NoCacheRepo::default());
    let sessions = ReviewSessions::new(engine.clone(), repo.clone(), resources());
    let (tx, rx) = mpsc::unbounded_channel();
    sessions.open("nav", "window-a", config, forward(tx))?;

    let mut counters = Counters {
        sessions_opened: 1,
        ..Default::default()
    };
    let first_fen = fens
        .get(1)
        .ok_or_else(|| ReviewError::new(ReviewErrorCode::InvalidPgn, "pgn", "sem lances"))?;
    sessions.analyze_position("nav", "window-a", 1, request(first_fen), settings())?;
    let (first_generation, _) = await_started(&mut started).await;
    assert_eq!(
        first_generation, 1,
        "first job must be gated before navigation"
    );
    assert_eq!(engine.snapshot().active, 1, "first search owns the lease");
    assert_eq!(
        engine.available_permits(),
        0,
        "lease held during first search"
    );

    for id in 2..=LATEST {
        let fen = fens.get(id as usize).ok_or_else(|| {
            ReviewError::new(
                ReviewErrorCode::InvalidPgn,
                "pgn",
                "PGN curto para navegação",
            )
        })?;
        sessions.analyze_position("nav", "window-a", id, request(fen), settings())?;
    }
    counters.jobs_enqueued = LATEST as usize;

    // The superseding job is the only one that reaches the engine after the
    // stale in-flight search has been cancelled and torn down.
    let (latest_generation, _) = await_started(&mut started).await;
    assert_eq!(
        latest_generation, 2,
        "only the latest job may start a new search"
    );
    engine.release(latest_generation);

    let mut log = EventLog::new(rx);
    let completed = log
        .wait(|event| matches!(event.event, Event::LiveCompleted { .. }))
        .await;
    assert_eq!(
        completed.request_id,
        Some(LATEST),
        "result must carry latest id"
    );
    assert_eq!(completed.session_id, "nav", "result must carry session id");
    sessions.close("nav", "window-a").await?;
    counters.closes_acked = 1;

    let events = log.drain();
    counters.finish(&engine, &repo, events, "nav", LATEST, &[LATEST]);
    counters.assert_teardown(&[LATEST]);
    assert_eq!(counters.searches, 2, "stale jobs must not reach the engine");
    assert_eq!(
        counters.live_completed, 1,
        "only one result may be published"
    );
    assert!(
        counters.stale_events > 0,
        "superseded events must be filtered"
    );
    Ok(counters)
}

/// Owner isolation, session replacement and idempotent shutdown.
async fn session_lifecycle(pgn: &str) -> Result<Counters> {
    let fen = core::extract(pgn)?
        .fens
        .get(1)
        .cloned()
        .ok_or_else(|| ReviewError::new(ReviewErrorCode::InvalidPgn, "pgn", "sem lances"))?;

    let (engine, mut started) = FakeFactory::new();
    let repo = Arc::new(NoCacheRepo::default());
    let sessions = ReviewSessions::new(engine.clone(), repo.clone(), resources());
    let (tx_a, rx_a) = mpsc::unbounded_channel();
    let (tx_b, rx_b) = mpsc::unbounded_channel();
    sessions.open("a", "win-a", config(pgn)?, forward(tx_a))?;
    sessions.open("b", "win-b", config(pgn)?, forward(tx_b))?;

    let mut counters = Counters {
        sessions_opened: 2,
        ..Default::default()
    };
    assert_eq!(
        sessions
            .open("a", "win-a", config(pgn)?, |_| true)
            .unwrap_err()
            .code,
        ReviewErrorCode::SessionExists,
        "duplicate session id must be rejected"
    );
    assert!(
        sessions
            .analyze_position("a", "intruder", 1, request(&fen), settings())
            .is_err(),
        "wrong owner must be rejected"
    );
    counters.auth_rejected = 2;

    // Session A holds a gated search while its owner closes it.
    sessions.analyze_position("a", "win-a", 1, request(&fen), settings())?;
    let (generation_a, _) = await_started(&mut started).await;
    assert_eq!(generation_a, 1, "session A must start the first search");
    sessions.close_owner("win-a");
    sessions.close("a", "win-a").await?;
    counters.closes_acked = 1;
    assert_eq!(engine.snapshot().active, 0, "closing A releases the lease");

    // Session B is unaffected and can acquire the released lease.
    sessions.analyze_position("b", "win-b", 1, request(&fen), settings())?;
    let (generation_b, _) = await_started(&mut started).await;
    assert_eq!(generation_b, 2, "session B reuses the released lease");
    engine.release(generation_b);

    let mut log_b = EventLog::new(rx_b);
    let completed = log_b
        .wait(|event| matches!(event.event, Event::LiveCompleted { .. }))
        .await;
    assert_eq!(
        completed.session_id, "b",
        "only session B publishes results"
    );

    sessions.close("b", "win-b").await?;
    sessions.close("b", "win-b").await?;
    counters.closes_acked += 2;
    sessions.shutdown().await;
    assert!(
        sessions.open("c", "win-c", config(pgn)?, |_| true).is_err(),
        "shutdown must prevent new sessions"
    );
    counters.shutdown_rejects = 1;

    // Session A must never have produced a completion.
    let events_a = EventLog::new(rx_a).drain();
    assert!(
        events_a
            .iter()
            .all(|event| !matches!(event.event, Event::LiveCompleted { .. })),
        "closed session must not publish results"
    );

    let events_b = log_b.drain();
    counters.jobs_enqueued = 2;
    counters.finish(&engine, &repo, events_b, "b", 1, &[1]);
    counters.assert_teardown(&[1]);
    assert_eq!(counters.searches, 2, "each session ran exactly one search");
    Ok(counters)
}

async fn run(scenario: &str, pgn_path: Option<&std::path::Path>) -> Result<Counters> {
    let default = match scenario {
        "cancel-analysis" | "session-lifecycle" => SHORT_PGN,
        "navigation-latest-wins" => NAV_PGN,
        _ => {
            return Err(ReviewError::new(
                ReviewErrorCode::InvalidPayload,
                "args",
                format!("cenário desconhecido: {scenario}"),
            ));
        }
    };
    let pgn = match pgn_path {
        Some(path) => std::fs::read_to_string(path).map_err(|error| {
            ReviewError::with_source(ReviewErrorCode::InvalidPgn, "pgn.read", error)
        })?,
        None => default.into(),
    };
    // `navigation-latest-wins` asks for FENs 1..=4; fail with a clear message
    // instead of an opaque index error when the caller supplies a short PGN.
    let required = if scenario == "navigation-latest-wins" {
        5
    } else {
        2
    };
    let available = core::extract(&pgn)?.fens.len();
    if available < required {
        return Err(ReviewError::new(
            ReviewErrorCode::InvalidPgn,
            "pgn",
            format!("{scenario} exige pelo menos {required} FENs; recebeu {available}"),
        ));
    }
    match scenario {
        "cancel-analysis" => cancel_analysis(&pgn).await,
        "navigation-latest-wins" => navigation_latest_wins(&pgn).await,
        "session-lifecycle" => session_lifecycle(&pgn).await,
        _ => unreachable!("scenario validado na seleção do PGN padrão"),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let scenario = args.next().ok_or_else(|| {
        ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "args",
            "use: pipeline_strobe_sessions <cancel-analysis|navigation-latest-wins|session-lifecycle> [pgn]",
        )
    })?;
    if !SCENARIOS.contains(&scenario.as_str()) {
        return Err(ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "args",
            format!("cenário desconhecido: {scenario}"),
        ));
    }
    let pgn = args.next().map(std::path::PathBuf::from);
    let counters = run(&scenario, pgn.as_deref()).await?;
    counters.print();
    Ok(())
}
