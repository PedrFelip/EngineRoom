//! Explicit benchmark only: deterministic inputs for the Rust pipeline.
use super::*;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Deserialize)]
struct Fixture {
    warmups: usize,
    samples: usize,
    games: HashMap<String, BenchGame>,
    scenarios: Vec<Scenario>,
}
#[derive(Deserialize)]
struct BenchGame {
    pgn: String,
    raw: Vec<RawPosition>,
}
#[derive(Deserialize, Serialize)]
struct Scenario {
    id: String,
    task: String,
    game: String,
    mode: String,
    multipv: u32,
    cache: String,
    kind: String,
    batch: usize,
    engine: Option<String>,
    samples: Option<usize>,
    warmups: Option<usize>,
}
#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Stats {
    searches: usize,
    info_lines: usize,
    progress: usize,
    cache_lookups: usize,
    cache_writes: usize,
    cache_entries: usize,
}
struct BenchFactory {
    raw: Arc<HashMap<String, RawPosition>>,
    stats: Arc<Mutex<Stats>>,
    multipv: u32,
    stall: bool,
}
struct BenchPort {
    raw: Arc<HashMap<String, RawPosition>>,
    stats: Arc<Mutex<Stats>>,
    multipv: u32,
    stall: bool,
    fen: String,
    queue: VecDeque<String>,
}
impl EngineFactory for BenchFactory {
    fn acquire<'a>(&'a self, _: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        Box::pin(async move {
            Ok(Box::new(BenchPort {
                raw: self.raw.clone(),
                stats: self.stats.clone(),
                multipv: self.multipv,
                stall: self.stall,
                fen: String::new(),
                queue: VecDeque::new(),
            }) as Box<dyn EnginePort>)
        })
    }
}
impl EnginePort for BenchPort {
    fn send(&mut self, command: &str) -> Result<()> {
        if command == "uci" {
            self.queue.push_back("uciok".into());
        }
        if command == "isready" {
            self.queue.push_back("readyok".into());
        }
        if let Some(fen) = command.strip_prefix("position fen ") {
            self.fen = fen.into();
        }
        if let Some(value) = command.strip_prefix("setoption name MultiPV value ") {
            self.multipv = value.parse().unwrap();
        }
        if command.starts_with("go ") {
            let mut stats = self.stats.lock().unwrap();
            stats.searches += 1;
            if !self.stall {
                let raw = &self.raw[&self.fen];
                for line in raw.lines.iter().take(self.multipv as usize) {
                    for depth in 1..=32 {
                        stats.info_lines += 1;
                        self.queue.push_back(format!(
                            "info depth {depth} multipv {} score cp {} pv {}",
                            line.multipv,
                            line.cp,
                            line.pv.join(" ")
                        ));
                    }
                }
                self.queue.push_back(format!(
                    "bestmove {}",
                    raw.pv.first().map(String::as_str).unwrap_or("(none)")
                ));
            }
        }
        Ok(())
    }
    fn next(&mut self) -> Task<'_, Result<String>> {
        Box::pin(async move {
            match self.queue.pop_front() {
                Some(line) => Ok(line),
                None => std::future::pending().await,
            }
        })
    }
    fn shutdown(&mut self) -> Task<'_, ()> {
        Box::pin(async {})
    }
}
struct BenchRepo {
    hits: HashMap<String, RawPosition>,
    stats: Arc<Mutex<Stats>>,
}

// Headless real-engine transport. This includes pipes and process teardown,
// but deliberately does not pretend to measure the Tauri IPC/rendering path.
struct RealFactory {
    stats: Arc<Mutex<Stats>>,
}
struct RealPort {
    child: Option<std::process::Child>,
    stdin: std::process::ChildStdin,
    rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    reader: Option<std::thread::JoinHandle<()>>,
    stats: Arc<Mutex<Stats>>,
}
impl EngineFactory for RealFactory {
    fn acquire<'a>(&'a self, _: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        Box::pin(async move {
            use std::io::BufRead;
            use std::process::{Command, Stdio};
            let binary = std::env::var("BENCH_STOCKFISH").unwrap_or_else(|_| {
                format!(
                    "{}/binaries/stockfish-x86_64-unknown-linux-gnu",
                    env!("CARGO_MANIFEST_DIR")
                )
            });
            let mut child = Command::new(binary)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let stdin = child.stdin.take().unwrap();
            let stdout = child.stdout.take().unwrap();
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let reader = std::thread::spawn(move || {
                for line in std::io::BufReader::new(stdout).lines() {
                    match line {
                        Ok(line) => {
                            if tx.send(line).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
            Ok(Box::new(RealPort {
                child: Some(child),
                stdin,
                rx,
                reader: Some(reader),
                stats: self.stats.clone(),
            }) as Box<dyn EnginePort>)
        })
    }
}
impl EnginePort for RealPort {
    fn send(&mut self, command: &str) -> Result<()> {
        use std::io::Write;
        if command.starts_with("go ") {
            self.stats.lock().unwrap().searches += 1;
        }
        writeln!(self.stdin, "{command}")
            .map_err(|e| ReviewError::new("engineCommand", "bench.send", e))
    }
    fn next(&mut self) -> Task<'_, Result<String>> {
        Box::pin(async move {
            let line =
                self.rx.recv().await.ok_or_else(|| {
                    ReviewError::new("engineExited", "bench.read", "stdout closed")
                })?;
            if line.starts_with("info depth ") {
                self.stats.lock().unwrap().info_lines += 1;
            }
            Ok(line)
        })
    }
    fn shutdown(&mut self) -> Task<'_, ()> {
        Box::pin(async move {
            if let Some(mut child) = self.child.take() {
                let _ = child.kill();
                let reader = self.reader.take();
                tokio::task::spawn_blocking(move || {
                    child.wait().unwrap();
                    if let Some(reader) = reader {
                        reader.join().unwrap();
                    }
                })
                .await
                .unwrap();
            }
        })
    }
}
impl Repository for BenchRepo {
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        _: Mode,
        _: u32,
        multipv: u32,
    ) -> Task<'a, Result<Vec<Option<RawPosition>>>> {
        Box::pin(async move {
            self.stats.lock().unwrap().cache_lookups += 1;
            Ok(fens
                .iter()
                .map(|fen| {
                    self.hits.get(fen).cloned().map(|mut raw| {
                        raw.lines.truncate(multipv as usize);
                        raw
                    })
                })
                .collect())
        })
    }
    fn put<'a>(
        &'a self,
        entries: &'a [RawPosition],
        _: Mode,
        _: u32,
        _: u32,
    ) -> Task<'a, Result<()>> {
        Box::pin(async move {
            let mut stats = self.stats.lock().unwrap();
            stats.cache_writes += 1;
            stats.cache_entries += entries.len();
            Ok(())
        })
    }
    fn save<'a>(&'a self, _: &'a ReviewConfig, _: &'a ReviewResult) -> Task<'a, Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum Output {
    Raw(RawPosition),
    Review(ReviewResult),
    Cancelled { cancelled: bool },
}

async fn execute(scenario: &Scenario, game: &BenchGame) -> (Output, Stats) {
    let stats = Arc::new(Mutex::new(Stats::default()));
    let real = scenario.engine.as_deref() == Some("stockfish");
    let factory: Arc<dyn EngineFactory> = if real {
        Arc::new(RealFactory {
            stats: stats.clone(),
        })
    } else {
        Arc::new(BenchFactory {
            raw: Arc::new(
                game.raw
                    .iter()
                    .cloned()
                    .map(|raw| (raw.fen.clone(), raw))
                    .collect(),
            ),
            stats: stats.clone(),
            multipv: scenario.multipv,
            stall: scenario.task == "cancel",
        })
    };
    let (tx, cancel) = cancel();
    let mode = if scenario.mode == "depth" {
        Mode::Depth
    } else {
        Mode::Time
    };
    let value = match (scenario.mode.as_str(), real) {
        ("depth", true) => 8,
        ("depth", false) => 20,
        (_, true) => 20,
        (_, false) => 120,
    };
    let mut result = Output::Cancelled { cancelled: false };
    if scenario.task == "search" {
        let mut port = factory.acquire(&cancel).await.unwrap();
        for _ in 0..scenario.batch {
            result = Output::Raw(
                evaluate(port.as_mut(), &game.raw[0].fen, mode, value, 10000, &cancel)
                    .await
                    .unwrap(),
            );
        }
        port.shutdown().await;
    } else if scenario.task == "cancel" {
        for _ in 0..scenario.batch {
            tx.send_replace(false);
            let mut port = factory.acquire(&cancel).await.unwrap();
            // Poll the blocked search before triggering cancellation, then await cleanup.
            let error = {
                let pending =
                    evaluate(port.as_mut(), &game.raw[0].fen, mode, value, 10000, &cancel);
                tokio::pin!(pending);
                tokio::select! {
                    biased;
                    result = &mut pending => panic!("search unexpectedly completed: {result:?}"),
                    _ = tokio::task::yield_now() => {},
                }
                tx.send_replace(true);
                pending.await.unwrap_err()
            };
            assert_eq!(error.code, "cancelled");
            port.shutdown().await;
        }
        result = Output::Cancelled { cancelled: true };
    } else {
        let repo = Arc::new(BenchRepo {
            hits: game
                .raw
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    scenario.cache == "warm" || (scenario.cache == "mixed" && i % 2 == 0)
                })
                .map(|(_, raw)| (raw.fen.clone(), raw.clone()))
                .collect(),
            stats: stats.clone(),
        });
        let mut pipeline = Pipeline::new(factory, repo);
        let config = ReviewConfig {
            pgn: game.pgn.clone(),
            // The pipeline derives metadata from the PGN; do not parse it twice in the harness.
            meta: PgnMeta {
                white: String::new(),
                black: String::new(),
                white_elo: None,
                black_elo: None,
                result: "*".into(),
                event: None,
                plies: 0,
            },
            engine: EngineTier {
                id: "bench".into(),
                depth: value,
                label: String::new(),
                hint: String::new(),
            },
            mode,
            movetime_ms: Some(value),
            lines: scenario.multipv,
            initial_result: None,
            analysis_kind: match scenario.kind.as_str() {
                "fast" => AnalysisKind::Fast,
                "deep" => AnalysisKind::Deep,
                _ => AnalysisKind::Manual,
            },
        };
        for _ in 0..scenario.batch {
            let review = pipeline
                .review(
                    &config,
                    if real { Some((1, 16)) } else { None },
                    &cancel,
                    &mut |event| {
                        if matches!(event, Event::Progress { .. }) {
                            stats.lock().unwrap().progress += 1;
                        }
                    },
                )
                .await
                .unwrap();
            result = Output::Review(review);
        }
        pipeline.close().await;
    }
    let counters = stats.lock().unwrap().clone();
    (result, counters)
}

#[tokio::test]
#[ignore = "explicit complete release benchmark; includes optional real Stockfish, excludes IPC/rendering/SQLite"]
async fn benchmark_complete_pipeline() {
    assert!(!cfg!(debug_assertions), "run benchmarks with --release");
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/benchmark.json")).unwrap();
    let samples = std::env::var("BENCH_SAMPLES")
        .ok()
        .map(|n| n.parse().unwrap())
        .unwrap_or(fixture.samples);
    let mut reports = Vec::new();
    for scenario in &fixture.scenarios {
        eprintln!("Rust {}", scenario.id);
        let game = &fixture.games[&scenario.game];
        for _ in 0..scenario.warmups.unwrap_or(fixture.warmups) {
            execute(scenario, game).await;
        }
        let mut elapsed = Vec::new();
        let count = if std::env::var("BENCH_SAMPLES").is_ok() {
            samples
        } else {
            scenario.samples.unwrap_or(samples)
        };
        for _ in 0..count {
            let start = Instant::now();
            execute(scenario, game).await;
            elapsed.push(start.elapsed().as_secs_f64() * 1000.0 / scenario.batch as f64);
        }
        let (result, stats) = execute(scenario, game).await;
        let mut report = serde_json::to_value(scenario).unwrap();
        report["elapsedMs"] = serde_json::json!(elapsed);
        report["result"] = serde_json::to_value(result).unwrap();
        report["stats"] = serde_json::to_value(stats).unwrap();
        reports.push(report);
    }
    let report =
        serde_json::json!({"samples":samples,"warmups":fixture.warmups,"scenarios":reports});
    if let Ok(path) = std::env::var("BENCH_REPORT_PATH") {
        std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    } else {
        eprintln!("RUST_COMPLETE_BENCHMARK_JSON={report}");
    }
}

#[tokio::test]
#[ignore = "explicit release benchmark; excludes Stockfish and IPC"]
async fn benchmark_analysis_overhead() {
    use std::time::Instant;
    let pgn = "1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7";
    let game = core::extract(pgn).unwrap();
    let mut scores = HashMap::new();
    for fen in &game.fens {
        scores.insert(
            fen.clone(),
            RawPosition {
                fen: fen.clone(),
                cp: 15,
                depth: 32,
                pv: vec!["e2e4".into(), "e7e5".into()],
                lines: vec![RawLine {
                    multipv: 1,
                    cp: 15,
                    depth: Some(32),
                    san: None,
                    pv: vec!["e2e4".into(), "e7e5".into()],
                }],
            },
        );
    }
    let f = factory(FakeState {
        scores,
        verbose: true,
        ..Default::default()
    });
    let (_tx, cancel) = cancel();
    let mut search_samples = Vec::new();
    let mut game_samples = Vec::new();
    let fen = &game.fens[0];
    let mut port = f.acquire(&cancel).await.unwrap();
    for sample in 0..14 {
        let start = Instant::now();
        for _ in 0..300 {
            evaluate(port.as_mut(), fen, Mode::Depth, 20, 1000, &cancel)
                .await
                .unwrap();
        }
        if sample >= 3 {
            search_samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        // Bound retained test recording allocations between samples.
        f.state.lock().unwrap().sent.clear();
    }
    port.shutdown().await;
    let mut config = config(pgn, AnalysisKind::Manual);
    config.lines = 1;
    let mut pipeline = Pipeline::new(f.clone(), Arc::new(MemoryRepo::default()));
    for sample in 0..14 {
        let start = Instant::now();
        pipeline
            .review(&config, None, &cancel, &mut |_| {})
            .await
            .unwrap();
        if sample >= 3 {
            game_samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        f.state.lock().unwrap().sent.clear();
    }
    search_samples.sort_by(f64::total_cmp);
    game_samples.sort_by(f64::total_cmp);
    eprintln!(
        "RUST_ANALYSIS_BENCHMARK_JSON={}",
        serde_json::json!({"samples":11,"searches":300,"linesPerSearch":33,"searchesMedianMs":search_samples[5],"gameWithProgressMedianMs":game_samples[5],"excludes":["Stockfish","IPC","rendering"]})
    );
}
