use super::{
    adaptive, core,
    engine::*,
    pipeline::Pipeline,
    repository::{self, Repository},
    types::*,
};
use crate::db::{
    self,
    cache::{Cache, CachedPositionPut},
    mode::Mode,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};

mod benchmark;
#[path = "core.rs"]
mod core_tests;
mod pipeline;
mod pool;
#[path = "repository.rs"]
mod repository_tests;

#[derive(Default)]
pub(super) struct FakeState {
    pub(super) sent: Vec<String>,
    pub(super) searches: usize,
    pub(super) stopped: usize,
    pub(super) fail_at: Option<usize>,
    pub(super) stall: bool,
    pub(super) scores: HashMap<String, RawPosition>,
    pub(super) critical_fen: Option<String>,
    pub(super) empty_once: bool,
    pub(super) verbose: bool,
}
struct FakePort {
    state: Arc<Mutex<FakeState>>,
    queue: VecDeque<String>,
    fen: String,
    permit: Option<OwnedSemaphorePermit>,
}
impl EnginePort for FakePort {
    fn send(&mut self, command: &str) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        s.sent.push(command.into());
        if command == "uci" {
            self.queue.push_back("id name Fake Stockfish".into());
            self.queue.push_back("uciok".into());
        } else if command == "isready" {
            self.queue.push_back("readyok".into());
        } else if let Some(f) = command.strip_prefix("position fen ") {
            self.fen = f.into();
        } else if command.starts_with("go ") {
            s.searches += 1;
            if s.fail_at == Some(s.searches) {
                return Err(ReviewError::new("engineExited", "fake", "engine failed"));
            }
            if s.stall {
                return Ok(());
            }
            if s.empty_once {
                s.empty_once = false;
            } else {
                let raw = s.scores.get(&self.fen).cloned();
                if let Some(raw) = raw {
                    for l in raw.lines {
                        let depths: Vec<_> = if s.verbose {
                            (1..=32).collect()
                        } else {
                            vec![l.depth.unwrap_or(raw.depth)]
                        };
                        for depth in depths {
                            self.queue.push_back(format!(
                                "info depth {depth} multipv {} score cp {} pv {}",
                                l.multipv,
                                l.cp,
                                l.pv.join(" ")
                            ));
                        }
                    }
                } else {
                    let cp = if s.searches == 11 {
                        s.critical_fen = Some(self.fen.clone());
                        500
                    } else if s.critical_fen.as_ref() == Some(&self.fen) {
                        500
                    } else {
                        0
                    };
                    self.queue
                        .push_back(format!("info depth 12 multipv 1 score cp {cp} pv e2e4"));
                    self.queue
                        .push_back("info depth 12 multipv 2 score cp -20 pv d2d4".into());
                }
            }
            self.queue.push_back("bestmove e2e4".into());
        }
        Ok(())
    }
    fn next(&mut self) -> Task<'_, Result<String>> {
        Box::pin(async move {
            if let Some(line) = self.queue.pop_front() {
                Ok(line)
            } else {
                std::future::pending().await
            }
        })
    }
    fn shutdown(&mut self) -> Task<'_, ()> {
        Box::pin(async move {
            self.state.lock().unwrap().stopped += 1;
            self.permit.take();
        })
    }
}
pub(super) struct FakeFactory {
    pub(super) state: Arc<Mutex<FakeState>>,
    pub(super) permit: Arc<Semaphore>,
    pub(super) acquired: AtomicUsize,
}
impl EngineFactory for FakeFactory {
    fn acquire<'a>(&'a self, cancel: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        Box::pin(async move {
            let permit = tokio::select! { _ = cancel.cancelled() => return Err(ReviewError::cancelled()), p = self.permit.clone().acquire_owned() => p.unwrap() };
            self.acquired.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(FakePort {
                state: self.state.clone(),
                queue: VecDeque::new(),
                fen: String::new(),
                permit: Some(permit),
            }) as Box<dyn EnginePort>)
        })
    }
}
#[derive(Default)]
pub(super) struct MemoryRepo {
    pub(super) hits: Mutex<HashMap<String, RawPosition>>,
    pub(super) writes: Mutex<Vec<(u32, usize)>>,
    pub(super) fail_put: bool,
    pub(super) fail_read: bool,
    pub(super) fail_save: bool,
}
impl Repository for MemoryRepo {
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        _: Mode,
        _: u32,
        multipv: u32,
    ) -> Task<'a, Result<Vec<Option<RawPosition>>>> {
        Box::pin(async move {
            if self.fail_read {
                return Err(ReviewError::new("cache", "lookup", "cache read failed"));
            }
            let hits = self.hits.lock().unwrap();
            Ok(fens
                .iter()
                .map(|f| {
                    hits.get(f).cloned().map(|mut r| {
                        r.lines.truncate(multipv as usize);
                        r
                    })
                })
                .collect())
        })
    }
    fn put<'a>(
        &'a self,
        entries: &'a [RawPosition],
        _: Mode,
        value: u32,
        _: u32,
    ) -> Task<'a, Result<()>> {
        Box::pin(async move {
            self.writes.lock().unwrap().push((value, entries.len()));
            if self.fail_put {
                return Err(ReviewError::new("cache", "put", "cache write failed"));
            }
            Ok(())
        })
    }
    fn save<'a>(&'a self, _: &'a ReviewConfig, _: &'a ReviewResult) -> Task<'a, Result<()>> {
        Box::pin(async move {
            if self.fail_save {
                Err(ReviewError::new("persistence", "save", "save failed"))
            } else {
                Ok(())
            }
        })
    }
}
pub(super) fn config(pgn: &str, kind: AnalysisKind) -> ReviewConfig {
    ReviewConfig {
        pgn: pgn.into(),
        meta: core::extract(pgn).unwrap().meta,
        engine: EngineTier {
            id: "balanced".into(),
            depth: 20,
            label: String::new(),
            hint: String::new(),
        },
        mode: Mode::Depth,
        analysis_kind: kind,
        movetime_ms: None,
        lines: 3,
        initial_result: None,
    }
}
fn cancel() -> (watch::Sender<bool>, Cancellation) {
    let (tx, rx) = watch::channel(false);
    (
        tx,
        Cancellation {
            closed: rx,
            revision: None,
        },
    )
}
pub(super) fn factory(state: FakeState) -> Arc<FakeFactory> {
    Arc::new(FakeFactory {
        state: Arc::new(Mutex::new(state)),
        permit: Arc::new(Semaphore::new(1)),
        acquired: AtomicUsize::new(0),
    })
}
pub(super) fn settings() -> LiveSettings {
    LiveSettings {
        search_seconds: 1,
        lines: 3,
        threads_auto: false,
        threads: 1,
        memory_mb: 16,
        move_feedback_enabled: false,
        fast_pass: false,
    }
}
pub(super) fn request(fen: &str) -> LiveRequest {
    LiveRequest {
        fen: fen.into(),
        variation_node_id: None,
        source_fen: None,
        source_analysis: None,
    }
}
