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
};

pub struct Completed {
    pub raw: RawPosition,
    /// UCI time budget used for this search.
    pub value: u32,
}
pub type Evaluation = oneshot::Receiver<Result<Completed>>;
struct Job {
    fen: String,
    result: oneshot::Sender<Result<Completed>>,
    value: u32,
    multipv: u32,
}
pub struct Pool {
    workers: Vec<mpsc::Sender<Job>>,
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
                ReviewErrorCode::EngineSpawn,
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
    ) -> Result<Evaluation> {
        let (result, receiver) = oneshot::channel();
        self.workers[worker]
            .send(Job {
                fen,
                result,
                value,
                multipv,
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
                    ReviewError::new(ReviewErrorCode::EngineExited, "engine.worker", error)
                });
            }
        }
    }
}

async fn worker(
    mut port: Box<dyn EnginePort>,
    mut jobs: mpsc::Receiver<Job>,
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
        let job = tokio::select! {
            _ = root.cancelled() => break,
            _ = local.cancelled() => break,
            job = jobs.recv() => match job { Some(job) => job, None => break },
        };
        let run = async {
            root.check()?;
            local.check()?;
            if configured.is_none() {
                engine::configure(
                    port.as_mut(),
                    Some(threads),
                    Some(memory),
                    job.multipv,
                    &local,
                )
                .await?;
            } else if configured != Some(job.multipv) {
                port.send(&format!("setoption name MultiPV value {}", job.multipv))?;
                engine::ask(port.as_mut(), "isready", "readyok", 10000, &local).await?;
            }
            configured = Some(job.multipv);
            let mut raw = engine::evaluate(
                port.as_mut(),
                &job.fen,
                Mode::Time,
                job.value,
                super::pipeline::timeout(Mode::Time, job.value),
                &local,
            )
            .await?;
            super::core::add_san(&mut raw);
            Ok(Completed {
                raw,
                value: job.value,
            })
        };
        let result = tokio::select! {
            _ = root.cancelled() => Err(ReviewError::cancelled()),
            _ = local.cancelled() => Err(ReviewError::cancelled()),
            result = run => result,
        };
        let failed = result.is_err();
        if let Err(error) = &result {
            if error.code != ReviewErrorCode::Cancelled {
                failure.lock().unwrap().get_or_insert(error.clone());
            }
            stop.send_replace(true);
        }
        let _ = job.result.send(result);
        if failed {
            break;
        }
    }
    port.shutdown().await;
}
