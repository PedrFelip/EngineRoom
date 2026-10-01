//! Persistent workers under one session lease. Every worker owns its UCI stream.
use super::{
    engine::{self, Cancellation, EngineFactory, EnginePort},
    types::*,
};
use crate::db::mode::Mode;
use std::sync::{Arc, Mutex};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinSet,
    time::Instant,
};

pub struct Completed {
    pub raw: RawPosition,
    /// Actual UCI budget, including clipping to the shared deadline.
    pub value: u32,
}
pub type Evaluation = oneshot::Receiver<Result<Option<Completed>>>;
struct Job {
    fen: String,
    candidates: Vec<String>,
    result: oneshot::Sender<Result<Option<Completed>>>,
}
struct Batch {
    jobs: Vec<Job>,
    mode: Mode,
    value: u32,
    multipv: u32,
    deadline: Option<Instant>,
}
pub struct Pool {
    workers: Vec<mpsc::Sender<Batch>>,
    tasks: JoinSet<()>,
    stop: watch::Sender<bool>,
    failure: Arc<Mutex<Option<ReviewError>>>,
}
impl Pool {
    pub async fn acquire(
        factory: &dyn EngineFactory,
        sizing: Option<(u32, u32)>,
        positions: usize,
        cancel: &Cancellation,
    ) -> Result<Self> {
        let (threads, memory) = sizing.unwrap_or((1, 16));
        // Keep at least two threads per worker on larger machines; cap process
        // and NNUE overhead. Hash is a total budget, never a per-process budget.
        let count = (threads / 2).clamp(1, 3).min((memory / 16).max(1)) as usize;
        let ports = factory
            .acquire_pool(count.min(positions.max(1)), cancel)
            .await?;
        if ports.is_empty() {
            return Err(ReviewError::new(
                "engineSpawn",
                "engine.pool",
                "Pool vazio.",
            ));
        }
        let count = ports.len() as u32;
        let (stop, _) = watch::channel(false);
        let failure = Arc::new(Mutex::new(None));
        let mut pool = Self {
            workers: vec![],
            tasks: JoinSet::new(),
            stop,
            failure,
        };
        for (index, port) in ports.into_iter().enumerate() {
            let (sender, receiver) = mpsc::channel(1);
            pool.workers.push(sender);
            let worker_threads =
                threads.max(count) / count + u32::from((index as u32) < threads % count);
            let worker_memory =
                memory.max(count) / count + u32::from((index as u32) < memory % count);
            pool.tasks.spawn(worker(
                port,
                receiver,
                worker_threads,
                worker_memory,
                cancel.clone(),
                pool.stop.clone(),
                pool.failure.clone(),
            ));
        }
        Ok(pool)
    }
    pub fn len(&self) -> usize {
        self.workers.len()
    }
    pub fn failure(&self) -> Option<ReviewError> {
        self.failure.lock().unwrap().clone()
    }
    pub async fn submit(
        &self,
        worker: usize,
        fen: String,
        value: u32,
        multipv: u32,
        candidates: Vec<String>,
    ) -> Result<Evaluation> {
        let (result, receiver) = oneshot::channel();
        self.workers[worker]
            .send(Batch {
                jobs: vec![Job {
                    fen,
                    result,
                    candidates,
                }],
                mode: Mode::Time,
                value,
                multipv,
                deadline: None,
            })
            .await
            .map_err(|_| self.failure().unwrap_or_else(ReviewError::cancelled))?;
        Ok(receiver)
    }
    pub async fn close(&mut self) {
        self.stop.send_replace(true);
        self.workers.clear();
        // A worker acknowledges shutdown only after process termination. The
        // shared lease remains held until the last worker has finished.
        while let Some(result) = self.tasks.join_next().await {
            if let Err(error) = result {
                self.failure.lock().unwrap().get_or_insert_with(|| {
                    ReviewError::new("engineExited", "engine.worker", error)
                });
            }
        }
    }
}

async fn worker(
    mut port: Box<dyn EnginePort>,
    mut jobs: mpsc::Receiver<Batch>,
    threads: u32,
    memory: u32,
    root: Cancellation,
    stop: watch::Sender<bool>,
    failure: Arc<Mutex<Option<ReviewError>>>,
) {
    let local = Cancellation {
        closed: stop.subscribe(),
        revision: None,
    };
    let mut configured = None;
    loop {
        let batch = tokio::select! {
            _ = root.cancelled() => break,
            _ = local.cancelled() => break,
            batch = jobs.recv() => match batch { Some(batch) => batch, None => break },
        };
        for job in batch.jobs {
            let run = async {
                root.check()?;
                local.check()?;
                let value = match batch.deadline {
                    Some(deadline) => batch.value.min(
                        deadline
                            .saturating_duration_since(Instant::now())
                            .as_millis()
                            .min(u32::MAX as u128) as u32,
                    ),
                    None => batch.value,
                };
                if value == 0 {
                    return Ok(None);
                }
                if configured.is_none() {
                    engine::configure(
                        port.as_mut(),
                        Some(threads),
                        Some(memory),
                        batch.multipv,
                        &local,
                    )
                    .await?;
                } else if configured != Some(batch.multipv) {
                    port.send(&format!("setoption name MultiPV value {}", batch.multipv))?;
                    engine::ask(port.as_mut(), "isready", "readyok", 10000, &local).await?;
                }
                configured = Some(batch.multipv);
                // Configuration can be expensive: recompute the usable budget.
                let value = match batch.deadline {
                    Some(deadline) => value.min(
                        deadline
                            .saturating_duration_since(Instant::now())
                            .as_millis()
                            .min(u32::MAX as u128) as u32,
                    ),
                    None => value,
                };
                if value == 0 {
                    return Ok(None);
                }
                let result = engine::evaluate_candidates(
                    port.as_mut(),
                    &job.fen,
                    batch.mode,
                    value,
                    super::pipeline::timeout(batch.mode, value),
                    &local,
                    &job.candidates,
                )
                .await;
                let mut raw = match result {
                    Ok(raw) => raw,
                    // A deadline-clipped search can finish before publishing a
                    // score. bestmove was consumed, so retaining triage is safe.
                    Err(error) if batch.deadline.is_some() && error.code == "missingEvaluation" => {
                        return Ok(None)
                    }
                    Err(error) => return Err(error),
                };
                super::core::add_san(&mut raw);
                tokio::task::yield_now().await;
                Ok(Some(Completed { raw, value }))
            };
            let result = tokio::select! {
                _ = root.cancelled() => Err(ReviewError::cancelled()),
                _ = local.cancelled() => Err(ReviewError::cancelled()),
                result = run => result,
            };
            let failed = result.is_err();
            if let Err(error) = &result {
                if error.code != "cancelled" {
                    failure.lock().unwrap().get_or_insert(error.clone());
                }
                stop.send_replace(true);
            }
            let _ = job.result.send(result);
            if failed {
                port.shutdown().await;
                return;
            }
        }
    }
    port.shutdown().await;
}
