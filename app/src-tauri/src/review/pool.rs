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
    cancel: Cancellation,
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
            cancel: cancel.clone(),
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
    pub fn failure(&self) -> Result<Option<ReviewError>> {
        self.failure
            .lock()
            .map(|failure| failure.clone())
            .map_err(|_| poisoned_failure())
    }
    pub async fn submit(
        &self,
        worker: usize,
        fen: String,
        value: u32,
        multipv: u32,
    ) -> Result<Evaluation> {
        let (result, receiver) = oneshot::channel();
        if let Some(error) = self.failure()? {
            return Err(error);
        }
        self.workers
            .get(worker)
            .ok_or_else(|| {
                ReviewError::new(
                    ReviewErrorCode::EngineExited,
                    "engine.pool.submit",
                    "Worker inválido.",
                )
            })?
            .send(Job {
                fen,
                result,
                value,
                multipv,
            })
            .await
            .map_err(|_| match self.failure() {
                Ok(Some(error)) | Err(error) => error,
                Ok(None) => self.cancel.check().err().unwrap_or_else(|| {
                    ReviewError::new(
                        ReviewErrorCode::EngineExited,
                        "engine.pool.submit",
                        "Worker encerrou sem resultado.",
                    )
                }),
            })?;
        Ok(receiver)
    }
    pub async fn close(&mut self) -> Result<()> {
        let mut cleanup_error = None;
        self.stop.send_replace(true);
        self.workers.clear();
        // A worker acknowledges shutdown only after process termination. The
        // shared lease remains held until the last worker has finished.
        while let Some(result) = self.tasks.join_next().await {
            if let Err(error) = result {
                let error = ReviewError::new(ReviewErrorCode::EngineExited, "engine.worker", error);
                let error = record_failure(&self.failure, error.clone())
                    .err()
                    .unwrap_or(error);
                cleanup_error.get_or_insert(error);
            }
        }
        match self.failure() {
            Ok(Some(error)) | Err(error) => Err(error),
            Ok(None) => cleanup_error.map_or(Ok(()), Err),
        }
    }
}

fn poisoned_failure() -> ReviewError {
    ReviewError::new(
        ReviewErrorCode::EngineExited,
        "engine.pool.failure",
        "Estado de falha do pool indisponível.",
    )
}
fn record_failure(failure: &Mutex<Option<ReviewError>>, error: ReviewError) -> Result<()> {
    failure
        .lock()
        .map_err(|_| poisoned_failure())?
        .get_or_insert(error);
    Ok(())
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
        let mut result = tokio::select! {
            _ = root.cancelled() => Err(ReviewError::cancelled()),
            _ = local.cancelled() => Err(ReviewError::cancelled()),
            result = run => result,
        };
        let failed = result.is_err();
        if let Err(error) = &result {
            if error.code != ReviewErrorCode::Cancelled {
                if let Err(record_error) = record_failure(&failure, error.clone()) {
                    result = Err(record_error);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn poison(failure: &Arc<Mutex<Option<ReviewError>>>) {
        let failure = failure.clone();
        assert!(std::thread::spawn(move || {
            let _guard = failure.lock().unwrap();
            panic!("test poison");
        })
        .join()
        .is_err());
    }

    struct FailingPort(Arc<AtomicUsize>);
    impl EnginePort for FailingPort {
        fn send(&mut self, _: &str) -> Result<()> {
            Err(ReviewError::new(
                ReviewErrorCode::EngineCommand,
                "test.send",
                "original failure",
            ))
        }
        fn next(&mut self) -> super::super::engine::Task<'_, Result<String>> {
            Box::pin(async { unreachable!("send fails first") })
        }
        fn shutdown(&mut self) -> super::super::engine::Task<'_, ()> {
            Box::pin(async {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                self.0.fetch_add(1, Ordering::SeqCst);
            })
        }
    }

    fn empty_pool(cancel: Cancellation) -> Pool {
        let (stop, _) = watch::channel(false);
        Pool {
            workers: vec![],
            tasks: JoinSet::new(),
            stop,
            failure: Arc::new(Mutex::new(None)),
            cancel,
        }
    }

    #[tokio::test]
    async fn submission_distinguishes_invalid_closed_and_cancelled_workers() {
        let (closed, receiver) = watch::channel(false);
        let mut pool = empty_pool(Cancellation {
            closed: receiver,
            revision: None,
        });
        assert_eq!(
            pool.submit(0, String::new(), 1, 1)
                .await
                .err()
                .unwrap()
                .code,
            ReviewErrorCode::EngineExited
        );
        let (sender, receiver) = mpsc::channel(1);
        drop(receiver);
        pool.workers.push(sender);
        assert_eq!(
            pool.submit(0, String::new(), 1, 1)
                .await
                .err()
                .unwrap()
                .code,
            ReviewErrorCode::EngineExited
        );
        closed.send_replace(true);
        assert_eq!(
            pool.submit(0, String::new(), 1, 1)
                .await
                .err()
                .unwrap()
                .code,
            ReviewErrorCode::Cancelled
        );
        pool.close().await.unwrap();
    }

    #[tokio::test]
    async fn poisoned_failure_returns_errors_and_drains_every_worker() {
        let (_closed, receiver) = watch::channel(false);
        let mut pool = empty_pool(Cancellation {
            closed: receiver,
            revision: None,
        });
        poison(&pool.failure);
        assert_eq!(pool.failure().unwrap_err().operation, "engine.pool.failure");
        assert_eq!(
            pool.submit(0, String::new(), 1, 1)
                .await
                .err()
                .unwrap()
                .operation,
            "engine.pool.failure"
        );
        let shutdowns = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let (sender, jobs) = mpsc::channel(1);
            let (result, reply) = oneshot::channel();
            sender
                .send(Job {
                    fen: String::new(),
                    result,
                    value: 1,
                    multipv: 1,
                })
                .await
                .unwrap();
            pool.tasks.spawn(worker(
                Box::new(FailingPort(shutdowns.clone())),
                jobs,
                1,
                16,
                pool.cancel.clone(),
                pool.stop.clone(),
                pool.failure.clone(),
            ));
            pool.workers.push(sender);
            let error = reply.await.unwrap().err().unwrap();
            assert_eq!(error.operation, "engine.pool.failure");
            // Allow the next worker to exercise failed recording as well.
            pool.stop.send_replace(false);
        }
        assert!(pool.close().await.is_err());
        assert_eq!(shutdowns.load(Ordering::SeqCst), 3);
        assert!(pool.tasks.is_empty());
    }
}
