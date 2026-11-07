//! Bounded worker ownership boundary for the experimental planner.
use super::{driver, Evidence, Request};
use crate::review::{
    engine::{Cancellation, EngineFactory},
    repository::Repository,
    types::{Result, ReviewError, ReviewErrorCode},
};
use std::time::{Duration, Instant};

/// Measurements from one run. Optional values are unavailable, never inferred zeros.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RunMetrics {
    pub wall_ms: u64,
    pub workers_acquired: usize,
    pub cache_hits: u64,
    pub searches_started: u64,
    pub searches_accepted: u64,
    pub searches_discarded: u64,
    /// Sum of wall time spent in searches, including parallel overlap.
    pub search_wall_ms: Option<u64>,
    /// Time from observing cancellation until all acquired processes are reaped.
    pub cancellation_cleanup_ms: Option<u64>,
    pub cpu_ms: Option<u64>,
    pub peak_sampled_rss_bytes: Option<u64>,
}

fn millis(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

/// The planner distinguishes completion from a request and an invalid active state.
pub enum Next {
    Dispatch(u64, u64, Request),
    Complete,
    Wait,
}

pub trait Plan {
    type Snapshot;
    fn record_metrics(&mut self, _metrics: RunMetrics) {}
    fn next(&mut self, now_ms: u64) -> Result<Next>;
    fn accept(
        &mut self,
        generation: u64,
        execution: u64,
        evidence: Evidence,
        now_ms: u64,
    ) -> Result<()>;
    fn snapshot(&self) -> Self::Snapshot;
    fn reject(&mut self, generation: u64, execution: u64, now_ms: u64, reason: &str) -> Result<()>;
}

fn contract_error(message: impl ToString) -> ReviewError {
    ReviewError::new(ReviewErrorCode::EngineProtocol, "decision.runner", message)
}

/// Owns the acquired engine until shutdown has finished, including error paths.
pub async fn run<P: Plan>(
    factory: &dyn EngineFactory,
    plan: &mut P,
    timeout: Duration,
    cancel: &Cancellation,
) -> Result<P::Snapshot> {
    run_workers(factory, plan, timeout, cancel, 1).await
}

/// Runs bounded searches, draining every acquired process before returning.
pub async fn run_workers<P: Plan>(
    factory: &dyn EngineFactory,
    plan: &mut P,
    timeout: Duration,
    cancel: &Cancellation,
    workers: usize,
) -> Result<P::Snapshot> {
    run_workers_cached(factory, plan, timeout, cancel, workers, None).await
}

/// Runs bounded workers with optional contextual cache.
pub async fn run_workers_cached<P: Plan>(
    factory: &dyn EngineFactory,
    plan: &mut P,
    timeout: Duration,
    cancel: &Cancellation,
    workers: usize,
    repository: Option<&dyn Repository>,
) -> Result<P::Snapshot> {
    run_workers_cached_progress(factory, plan, timeout, cancel, workers, repository, None).await
}

/// Publishes coalesced snapshots without waiting on the consumer.
pub async fn run_workers_cached_progress<P: Plan>(
    factory: &dyn EngineFactory,
    plan: &mut P,
    timeout: Duration,
    cancel: &Cancellation,
    workers: usize,
    repository: Option<&dyn Repository>,
    progress: Option<&tokio::sync::watch::Sender<P::Snapshot>>,
) -> Result<P::Snapshot> {
    let started = Instant::now();
    let mut metrics = RunMetrics::default();
    if workers == 0 {
        return Err(contract_error("worker count must be positive"));
    }
    let mut ports = match factory.acquire_pool(workers, cancel).await {
        Ok(ports) => ports,
        Err(error) => {
            metrics.wall_ms = millis(started.elapsed());
            plan.record_metrics(metrics);
            return Err(error);
        }
    };
    metrics.workers_acquired = ports.len();
    if ports.is_empty() {
        metrics.wall_ms = millis(started.elapsed());
        plan.record_metrics(metrics);
        return Err(contract_error("engine pool is empty"));
    }
    let sampler = super::resource_metrics::Sampler::start(
        ports.iter().map(|port| port.process_id()).collect(),
    );
    let now = || millis(started.elapsed());
    let mut failure = None;
    for port in &mut ports {
        if let Err(error) = crate::review::engine::ask(
            port.as_mut(),
            "uci",
            "uciok",
            timeout.as_millis().min(u64::MAX as u128) as u64,
            cancel,
        )
        .await
        {
            failure = Some(error);
            break;
        }
    }
    let ports: Vec<_> = ports
        .into_iter()
        .map(|port| std::sync::Arc::new(tokio::sync::Mutex::new(port)))
        .collect();
    let mut available: Vec<_> = (0..ports.len()).collect();
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let worker_cancel = Cancellation {
        closed: stop_rx,
        revision: None,
    };
    let mut tasks = tokio::task::JoinSet::new();
    let mut active = std::collections::BTreeMap::new();
    let mut complete = false;
    let mut pending = Vec::new();
    let mut search_wall_ms = 0_u64;
    let mut timed_searches = 0_u64;
    let mut cancellation_observed = failure
        .as_ref()
        .filter(|error| error.code == ReviewErrorCode::Cancelled)
        .map(|_| Instant::now());
    while failure.is_none() && !complete {
        if let Err(error) = cancel.check() {
            cancellation_observed = Some(Instant::now());
            failure = Some(error);
            break;
        }
        if pending.len() >= 8 {
            if let Some(repository) = repository {
                match repository.put_decisions(&pending).await {
                    Ok(()) => pending.clear(),
                    Err(error) => {
                        failure = Some(error);
                        break;
                    }
                }
            }
        }
        let mut batch = Vec::new();
        while let Some(index) = available.pop() {
            match plan.next(now()) {
                Ok(Next::Dispatch(generation, execution, request)) => {
                    active.insert(execution, generation);
                    batch.push((index, generation, execution, request));
                }
                Ok(Next::Wait) => {
                    available.push(index);
                    break;
                }
                Ok(Next::Complete) => {
                    available.push(index);
                    if active.is_empty() {
                        complete = true;
                    } else {
                        failure =
                            Some(contract_error("planner completed with active reservations"));
                    }
                    break;
                }
                Err(error) => {
                    available.push(index);
                    failure = Some(error);
                    break;
                }
            }
        }
        let requests: Vec<_> = batch
            .iter()
            .map(|(_, _, _, request)| request.clone())
            .collect();
        let cached = if requests.is_empty() {
            Vec::new()
        } else if let Some(repository) = repository {
            match repository.lookup_decisions(&requests).await {
                Ok(entries) if entries.len() == requests.len() => entries,
                Ok(_) => {
                    failure = Some(contract_error("cache lookup returned wrong entry count"));
                    Vec::new()
                }
                Err(error) => {
                    failure = Some(error);
                    Vec::new()
                }
            }
        } else {
            vec![None; requests.len()]
        };
        if failure.is_some() {
            break;
        }
        for ((index, generation, execution, request), cached) in batch.into_iter().zip(cached) {
            if let Some(mut evidence) = cached {
                evidence.id = execution;
                evidence.execution_id = execution;
                for line in &mut evidence.lines {
                    line.execution_id = execution;
                }
                match plan.accept(generation, execution, evidence, now()) {
                    Ok(()) => {
                        metrics.cache_hits += 1;
                        active.remove(&execution);
                        available.push(index);
                        if let Some(progress) = progress {
                            progress.send_replace(plan.snapshot());
                        }
                    }
                    Err(error) => {
                        failure = Some(error);
                        available.push(index);
                        break;
                    }
                }
            } else {
                metrics.searches_started += 1;
                let worker_cancel = worker_cancel.clone();
                let port = ports[index].clone();
                tasks.spawn(async move {
                    let mut port = port.lock().await;
                    let search_started = Instant::now();
                    let result =
                        driver::search(port.as_mut(), &request, execution, timeout, &worker_cancel)
                            .await;
                    (
                        index,
                        generation,
                        execution,
                        request,
                        result,
                        search_started.elapsed(),
                    )
                });
            }
        }
        if failure.is_none()
            && tasks.is_empty()
            && !available.is_empty()
            && !complete
            && !requests.is_empty()
        {
            continue;
        }
        if complete || failure.is_some() {
            break;
        }
        if tasks.is_empty() {
            failure = Some(contract_error("planner is waiting without active searches"));
            break;
        }
        tokio::select! {
            _ = cancel.cancelled() => { cancellation_observed = Some(Instant::now()); failure = Some(ReviewError::cancelled()); },
            joined = tasks.join_next() => match joined {
                Some(Ok((index, generation, execution, request, result, elapsed))) => {
                    timed_searches += 1;
                    search_wall_ms = search_wall_ms.saturating_add(millis(elapsed));
                    available.push(index);
                    match result {
                        Ok((evidence, _)) => match plan.accept(generation, execution, evidence.clone(), now()) {
                            Ok(()) => {
                                metrics.searches_accepted += 1;
                                active.remove(&execution);
                                if repository.is_some() { pending.push((request, evidence)); }
                                if let Some(progress) = progress { progress.send_replace(plan.snapshot()); }
                            }
                            Err(error) => failure = Some(error),
                        },
                        Err(error) => {
                            if error.code == ReviewErrorCode::Cancelled {
                                cancellation_observed = Some(Instant::now());
                            }
                            failure = Some(error);
                        },
                    }
                }
                Some(Err(error)) => failure = Some(contract_error(format!("worker task failed: {error}"))),
                None => failure = Some(contract_error("worker task disappeared")),
            }
        }
    }
    let _ = stop_tx.send(true);
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((_, _, _, _, _, elapsed)) => {
                timed_searches += 1;
                search_wall_ms = search_wall_ms.saturating_add(millis(elapsed));
            }
            Err(error) if failure.is_none() => {
                failure = Some(contract_error(format!("worker task failed: {error}")))
            }
            Err(_) => {}
        }
    }
    if let Some(sampler) = sampler {
        let sample = sampler.finish().await;
        metrics.cpu_ms = sample.cpu_ms;
        metrics.peak_sampled_rss_bytes = sample.peak_rss_bytes;
    }
    for port in ports {
        port.lock().await.shutdown().await;
    }
    metrics.cancellation_cleanup_ms = cancellation_observed.map(|start| millis(start.elapsed()));
    if !pending.is_empty() {
        if let Some(repository) = repository {
            if let Err(error) = repository.put_decisions(&pending).await {
                if failure.is_none() {
                    failure = Some(error);
                }
            }
        }
    }
    metrics.wall_ms = millis(started.elapsed());
    metrics.searches_discarded = metrics.searches_started - metrics.searches_accepted;
    metrics.search_wall_ms = (timed_searches == metrics.searches_started).then_some(search_wall_ms);
    plan.record_metrics(metrics);
    if let Some(error) = failure {
        for (execution, generation) in active {
            if let Err(rejection) = plan.reject(generation, execution, now(), &error.to_string()) {
                return Err(contract_error(format!(
                    "{error}; failed to settle reservation: {rejection}"
                )));
            }
        }
        Err(error)
    } else {
        Ok(plan.snapshot())
    }
}

/// Standalone process adapter. It does not acquire the application's engine lease.
pub mod process {
    use crate::review::{
        engine::{Cancellation, EngineFactory, EnginePort, Task},
        types::{Result, ReviewError, ReviewErrorCode},
    };
    use std::{path::PathBuf, process::Stdio, time::Duration};
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
        process::{Child, ChildStdout, Command},
    };

    pub struct Factory {
        pub executable: PathBuf,
        pub timeout: Duration,
    }
    struct Port {
        child: Child,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
        writer: tokio::task::JoinHandle<()>,
        lines: Lines<BufReader<ChildStdout>>,
    }
    fn err(code: ReviewErrorCode, e: impl ToString) -> ReviewError {
        ReviewError::new(code, "decision.process", e)
    }
    impl EngineFactory for Factory {
        fn acquire<'a>(
            &'a self,
            cancel: &'a Cancellation,
        ) -> Task<'a, Result<Box<dyn EnginePort>>> {
            Box::pin(async move {
                cancel.check()?;
                let mut child = Command::new(&self.executable)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|e| err(ReviewErrorCode::EngineSpawn, e))?;
                let mut stdin = child.stdin.take().expect("piped stdin");
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
                let writer = tokio::spawn(async move {
                    while let Some(line) = rx.recv().await {
                        if stdin.write_all(line.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                });
                let lines = BufReader::new(child.stdout.take().expect("piped stdout")).lines();
                let mut port = Port {
                    child,
                    tx,
                    writer,
                    lines,
                };
                let handshake = async {
                    port.send("uci")?;
                    loop {
                        if port.next().await?.trim() == "uciok" {
                            return Ok(());
                        }
                    }
                };
                let result = tokio::select! {
                    _ = cancel.cancelled() => Err(ReviewError::cancelled()),
                    r = tokio::time::timeout(self.timeout, handshake) => r.unwrap_or_else(|_| Err(err(ReviewErrorCode::EngineTimeout, "UCI handshake timed out"))),
                };
                if let Err(e) = result {
                    port.shutdown().await;
                    return Err(e);
                }
                Ok(Box::new(port) as Box<dyn EnginePort>)
            })
        }
    }
    impl EnginePort for Port {
        fn process_id(&self) -> Option<u32> {
            self.child.id()
        }
        fn send(&mut self, command: &str) -> Result<()> {
            self.tx
                .send(format!("{command}\n"))
                .map_err(|e| err(ReviewErrorCode::EngineCommand, e))
        }
        fn next(&mut self) -> Task<'_, Result<String>> {
            Box::pin(async move {
                self.lines
                    .next_line()
                    .await
                    .map_err(|e| err(ReviewErrorCode::EngineExited, e))?
                    .ok_or_else(|| err(ReviewErrorCode::EngineExited, "stdout closed"))
            })
        }
        fn shutdown(&mut self) -> Task<'_, ()> {
            Box::pin(async move {
                let _ = self.child.start_kill();
                let _ = self.child.wait().await;
                self.writer.abort();
                let _ = (&mut self.writer).await;
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::{
        decision::{self, planner},
        engine::{EnginePort, Task},
    };
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    struct Factory {
        shutdown: Arc<AtomicBool>,
        mode: u8,
        cancel: tokio::sync::watch::Sender<bool>,
    }
    struct Port {
        shutdown: Arc<AtomicBool>,
        mode: u8,
        step: usize,
        cancel: tokio::sync::watch::Sender<bool>,
    }
    impl EngineFactory for Factory {
        fn acquire<'a>(&'a self, _: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
            Box::pin(async move {
                Ok(Box::new(Port {
                    shutdown: self.shutdown.clone(),
                    mode: self.mode,
                    step: 0,
                    cancel: self.cancel.clone(),
                }) as Box<dyn EnginePort>)
            })
        }
    }
    impl EnginePort for Port {
        fn send(&mut self, _: &str) -> Result<()> {
            Ok(())
        }
        fn next(&mut self) -> Task<'_, Result<String>> {
            Box::pin(async move {
                self.step += 1;
                if self.step == 1 {
                    return Ok("uciok".into());
                }
                if self.step == 2 {
                    return Ok("readyok".into());
                }
                match self.mode {
                    0 => Err(contract_error("engine failed")),
                    1 => {
                        let _ = self.cancel.send(true);
                        std::future::pending().await
                    }
                    3 => panic!("fake worker panic"),
                    _ => Ok(if self.step == 3 {
                        "info depth 8 multipv 1 score cp 0 pv e2e4"
                    } else {
                        "bestmove e2e4"
                    }
                    .into()),
                }
            })
        }
        fn shutdown(&mut self) -> Task<'_, ()> {
            Box::pin(async move {
                tokio::task::yield_now().await;
                self.shutdown.store(true, Ordering::SeqCst);
            })
        }
    }
    struct Rejecting(planner::Planner);
    impl Plan for Rejecting {
        type Snapshot = planner::Snapshot;
        fn next(&mut self, now: u64) -> Result<Next> {
            Plan::next(&mut self.0, now)
        }
        fn accept(&mut self, _: u64, _: u64, _: Evidence, _: u64) -> Result<()> {
            Err(contract_error("rejected"))
        }
        fn reject(&mut self, g: u64, e: u64, now: u64, reason: &str) -> Result<()> {
            Plan::reject(&mut self.0, g, e, now, reason)
        }
        fn snapshot(&self) -> Self::Snapshot {
            self.0.snapshot()
        }
    }
    #[tokio::test]
    async fn shutdown_and_reservation_settlement_on_search_failure_cancel_and_rejection() {
        for mode in 0..3 {
            let (tx, rx) = tokio::sync::watch::channel(false);
            let shutdown = Arc::new(AtomicBool::new(false));
            let factory = Factory {
                shutdown: shutdown.clone(),
                mode,
                cancel: tx,
            };
            let cancel = Cancellation {
                closed: rx,
                revision: None,
            };
            let config = planner::Config {
                generation: 1,
                engine: decision::EngineSignature {
                    executable_digest: "a".repeat(64),
                    options: vec![("MultiPV".into(), "1".into())],
                },
                screening_depth: 8,
                multipv: 1,
                verification_depth: 12,
                critical_cp: 100,
                max_attempts: 1,
            };
            let mut plan = Rejecting(planner::Planner::new("1. e4 *", config).unwrap());
            assert!(run(&factory, &mut plan, Duration::from_secs(1), &cancel)
                .await
                .is_err());
            assert!(shutdown.load(Ordering::SeqCst));
            let ledger = plan.snapshot().ledger;
            assert_eq!(ledger.len(), 1);
            assert!(!ledger[0].reserved);
            assert!(ledger[0].outcome.is_some());
        }
    }
    #[tokio::test]
    async fn cancellation_drains_every_worker_and_settles_reservations() {
        struct Pool {
            shutdowns: Vec<Arc<AtomicBool>>,
            cancel: tokio::sync::watch::Sender<bool>,
            mode: u8,
        }
        impl EngineFactory for Pool {
            fn acquire<'a>(&'a self, _: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
                Box::pin(async { Err(contract_error("unexpected single acquire")) })
            }
            fn acquire_pool<'a>(
                &'a self,
                _: usize,
                _: &'a Cancellation,
            ) -> Task<'a, Result<Vec<Box<dyn EnginePort>>>> {
                Box::pin(async move {
                    Ok(self
                        .shutdowns
                        .iter()
                        .map(|shutdown| {
                            Box::new(Port {
                                shutdown: shutdown.clone(),
                                mode: self.mode,
                                step: 0,
                                cancel: self.cancel.clone(),
                            }) as Box<dyn EnginePort>
                        })
                        .collect())
                })
            }
        }
        for mode in [1, 3] {
            let (tx, rx) = tokio::sync::watch::channel(false);
            let shutdowns = vec![
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicBool::new(false)),
            ];
            let factory = Pool {
                shutdowns: shutdowns.clone(),
                cancel: tx,
                mode,
            };
            let cancel = Cancellation {
                closed: rx,
                revision: None,
            };
            let config = planner::Config {
                generation: 1,
                engine: decision::EngineSignature {
                    executable_digest: "a".repeat(64),
                    options: vec![("MultiPV".into(), "1".into())],
                },
                screening_depth: 8,
                multipv: 1,
                verification_depth: 12,
                critical_cp: 100,
                max_attempts: 1,
            };
            let mut plan = planner::Planner::new("1. e4 e5 *", config).unwrap();
            assert!(
                run_workers(&factory, &mut plan, Duration::from_secs(1), &cancel, 2)
                    .await
                    .is_err()
            );
            assert!(shutdowns.iter().all(|s| s.load(Ordering::SeqCst)));
            let ledger = plan.snapshot().ledger;
            assert_eq!(ledger.len(), 2);
            assert!(ledger
                .iter()
                .all(|entry| !entry.reserved && entry.outcome.is_some()));
            let metrics = plan.snapshot().metrics.expect("run metrics recorded");
            assert_eq!(metrics.workers_acquired, 2);
            assert_eq!(metrics.searches_started, 2);
            assert_eq!(metrics.searches_accepted, 0);
            assert_eq!(metrics.searches_discarded, 2);
            if mode == 1 {
                assert!(metrics.search_wall_ms.is_some());
                assert!(metrics.cancellation_cleanup_ms.is_some());
            } else {
                assert!(metrics.search_wall_ms.is_none());
                assert!(metrics.cancellation_cleanup_ms.is_none());
            }
            assert!(metrics.cpu_ms.is_none(), "fake ports do not expose PIDs");
        }
    }
    #[tokio::test]
    async fn contextual_cache_hit_and_storage_errors_are_observed() {
        use crate::db::mode::Mode;
        use crate::review::{
            decision::{Bound, Line, Score},
            types::{RawPosition, ReviewConfig, ReviewResult},
        };
        struct Cache {
            mode: u8,
        }
        impl Repository for Cache {
            fn lookup<'a>(
                &'a self,
                _: &'a [String],
                _: Mode,
                _: u32,
                _: u32,
            ) -> Task<'a, Result<Vec<Option<RawPosition>>>> {
                unreachable!()
            }
            fn put<'a>(
                &'a self,
                _: &'a [RawPosition],
                _: Mode,
                _: u32,
                _: u32,
            ) -> Task<'a, Result<()>> {
                unreachable!()
            }
            fn save<'a>(
                &'a self,
                _: &'a ReviewConfig,
                _: &'a ReviewResult,
            ) -> Task<'a, Result<()>> {
                unreachable!()
            }
            fn lookup_decisions<'a>(
                &'a self,
                requests: &'a [Request],
            ) -> Task<'a, Result<Vec<Option<Evidence>>>> {
                Box::pin(async move {
                    if self.mode == 2 {
                        return Err(ReviewError::new(
                            ReviewErrorCode::Cache,
                            "test.lookup",
                            "lookup failed",
                        ));
                    }
                    Ok(requests
                        .iter()
                        .map(|r| {
                            if self.mode == 0 {
                                Some(Evidence {
                                    id: 99,
                                    execution_id: 99,
                                    context: r.context.clone(),
                                    engine: r.engine.clone(),
                                    unrestricted: true,
                                    depth: r.depth,
                                    lines: vec![Line {
                                        depth: r.depth,
                                        execution_id: 99,
                                        score: Score::Cp(0),
                                        bound: Bound::Exact,
                                        pv: vec!["e2e4".into()],
                                    }],
                                })
                            } else {
                                None
                            }
                        })
                        .collect())
                })
            }
            fn put_decisions<'a>(&'a self, _: &'a [(Request, Evidence)]) -> Task<'a, Result<()>> {
                Box::pin(async {
                    Err(ReviewError::new(
                        ReviewErrorCode::Cache,
                        "test.put",
                        "write failed",
                    ))
                })
            }
        }
        struct One {
            request: Option<Request>,
            accepted: bool,
        }
        impl Plan for One {
            type Snapshot = bool;
            fn next(&mut self, _: u64) -> Result<Next> {
                Ok(match self.request.take() {
                    Some(r) => Next::Dispatch(1, 1, r),
                    None if self.accepted => Next::Complete,
                    None => Next::Wait,
                })
            }
            fn accept(&mut self, _: u64, execution: u64, e: Evidence, _: u64) -> Result<()> {
                assert_eq!(execution, e.id);
                assert_eq!(execution, e.execution_id);
                assert_eq!(execution, e.lines[0].execution_id);
                self.accepted = true;
                Ok(())
            }
            fn snapshot(&self) -> bool {
                self.accepted
            }
            fn reject(&mut self, _: u64, _: u64, _: u64, _: &str) -> Result<()> {
                Ok(())
            }
        }
        for mode in 0..3 {
            let (tx, rx) = tokio::sync::watch::channel(false);
            let shutdown = Arc::new(AtomicBool::new(false));
            let factory = Factory {
                shutdown: shutdown.clone(),
                mode: if mode == 0 { 0 } else { 2 },
                cancel: tx,
            };
            let cancel = Cancellation {
                closed: rx,
                revision: None,
            };
            let config = planner::Config {
                generation: 1,
                engine: decision::EngineSignature {
                    executable_digest: "a".repeat(64),
                    options: vec![("MultiPV".into(), "1".into())],
                },
                screening_depth: 8,
                multipv: 1,
                verification_depth: 12,
                critical_cp: 100,
                max_attempts: 1,
            };
            let request = planner::Planner::new("1. e4 *", config)
                .unwrap()
                .next_request(0)
                .unwrap()
                .request;
            let mut plan = One {
                request: Some(request),
                accepted: false,
            };
            let (progress_tx, progress_rx) = tokio::sync::watch::channel(false);
            let result = run_workers_cached_progress(
                &factory,
                &mut plan,
                Duration::from_secs(1),
                &cancel,
                1,
                Some(&Cache { mode }),
                Some(&progress_tx),
            )
            .await;
            if mode == 0 {
                assert!(result.unwrap());
                assert!(progress_rx.has_changed().unwrap());
                assert!(*progress_rx.borrow());
            } else {
                assert_eq!(result.unwrap_err().code, ReviewErrorCode::Cache);
            }
            assert!(shutdown.load(Ordering::SeqCst));
        }
    }
}
