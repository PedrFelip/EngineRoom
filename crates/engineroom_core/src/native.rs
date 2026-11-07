//! Native shell adapters. All work uses the same pipeline, cache and session task.
use crate::{
    db::{
        self,
        cache::Cache,
        games::{GameCursor, GamePage, Store},
        stats::{Stats, StorageStats},
    },
    review::{
        engine::{self, Cancellation, EngineFactory, EnginePort, Task},
        pipeline::Pipeline,
        repository::{self, Database, SqliteRepository},
        session::{self, Job, Session},
        types::*,
    },
};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{mpsc, Arc, Mutex, Weak},
    thread,
};
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};

#[derive(Clone)]
struct NativeDatabase {
    connection: Arc<Mutex<rusqlite::Connection>>,
    active: Arc<watch::Sender<usize>>,
}
struct Operation(Arc<watch::Sender<usize>>);
impl Drop for Operation {
    fn drop(&mut self) {
        self.0.send_modify(|n| *n -= 1);
    }
}
impl Database for NativeDatabase {
    fn run<T: Send + 'static>(
        &self,
        operation: &'static str,
        code: ReviewErrorCode,
        run: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static,
    ) -> Task<'_, Result<T>> {
        let connection = self.connection.clone();
        self.active.send_modify(|n| *n += 1);
        let guard = Operation(self.active.clone());
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let _guard = guard;
                let conn = connection
                    .lock()
                    .map_err(|e| ReviewError::new(code, operation, e))?;
                run(&conn).map_err(|e| ReviewError::with_source(code, operation, e))
            })
            .await
            .map_err(|e| ReviewError::with_source(code, operation, e))?
        })
    }
    fn drain(&self) -> Task<'_, ()> {
        Box::pin(async move {
            let mut active = self.active.subscribe();
            let _ = active.wait_for(|n| *n == 0).await;
        })
    }
}
struct NativeFactory {
    path: PathBuf,
    permit: Arc<Semaphore>,
    reaper: Reaper,
}
// The supervisor owns the queue and each job across worker panics. A job retains
// its lease until both process reaping and reader termination have completed.
#[derive(Clone)]
struct Reaper(mpsc::Sender<Cleanup>);
struct Cleanup {
    child: Child,
    stdin: Option<ChildStdin>,
    reader: Option<thread::JoinHandle<()>>,
    permit: Arc<OwnedSemaphorePermit>,
    done: Option<tokio::sync::watch::Sender<bool>>,
}
impl Reaper {
    fn new() -> std::io::Result<Self> {
        Self::with_worker(cleanup_child)
    }

    fn with_worker(worker: fn(&mut Cleanup) -> std::io::Result<()>) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::channel::<Cleanup>();
        thread::Builder::new()
            .name("engine-reaper".into())
            .spawn(move || {
                let mut current = None;
                loop {
                    // The receiver and current job remain outside the unwind boundary.
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if current.is_none() {
                            current = rx.recv().ok();
                        }
                        if let Some(cleanup) = current.as_mut() {
                            Self::recover(cleanup, worker);
                        }
                    }));
                    match result {
                        Ok(()) if current.is_none() => break,
                        Ok(()) => {
                            current = None;
                        }
                        Err(payload) => {
                            // An arbitrary panic payload may itself panic on drop.
                            std::mem::forget(payload);
                            eprintln!(
                                "engine cleanup supervisor panicked; retaining lease and retrying"
                            );
                            thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                }
            })?;
        Ok(Self(tx))
    }

    fn recover(cleanup: &mut Cleanup, worker: fn(&mut Cleanup) -> std::io::Result<()>) {
        // The supervisor retains ownership while a failed worker is retried. In
        // particular, unwinding must never drop the queue or its permits.
        let mut attempt = worker;
        loop {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| attempt(cleanup)));
            match result {
                Ok(Ok(())) => break,
                Ok(Err(error)) => {
                    eprintln!("engine cleanup failed; retaining lease and retrying: {error}")
                }
                Err(payload) => {
                    std::mem::forget(payload);
                    eprintln!("engine cleanup panicked; retaining lease and retrying");
                }
            }
            attempt = cleanup_child;
            thread::sleep(std::time::Duration::from_millis(100));
        }
        if let Some(done) = cleanup.done.take() {
            done.send_replace(true);
        }
    }

    fn enqueue(&self, cleanup: Cleanup) {
        if let Err(error) = self.0.send(cleanup) {
            // A disconnected supervisor still transfers the complete job to a
            // recovery thread. If thread creation fails, retain it fail-closed.
            let job = Arc::new(Mutex::new(Some(error.0)));
            let recovery = job.clone();
            if thread::Builder::new()
                .name("engine-recovery".into())
                .spawn(move || {
                    let mut slot = recovery.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(cleanup) = slot.as_mut() {
                        Self::recover(cleanup, cleanup_child);
                    }
                    *slot = None;
                })
                .is_err()
            {
                if let Ok(mut slot) = job.lock() {
                    if let Some(cleanup) = slot.as_mut() {
                        drop(cleanup.stdin.take());
                        let _ = cleanup.child.kill();
                    }
                }
                eprintln!(
                    "engine cleanup recovery thread unavailable; lease retained until process exit"
                );
                std::mem::forget(job);
            }
        }
    }
}

fn cleanup_child(cleanup: &mut Cleanup) -> std::io::Result<()> {
    drop(cleanup.stdin.take());
    let _ = cleanup.child.kill();
    cleanup.child.wait()?;
    if let Some(reader) = cleanup.reader.take() {
        // The guard joins even if code between taking the handle and joining unwinds.
        let reader = ReaderJoin(Some(reader));
        reader.join();
    }
    Ok(())
}
struct ReaderJoin(Option<thread::JoinHandle<()>>);
impl ReaderJoin {
    fn join(mut self) {
        if let Some(reader) = self.0.take() {
            let _ = reader.join();
        }
    }
}
impl Drop for ReaderJoin {
    fn drop(&mut self) {
        if let Some(reader) = self.0.take() {
            let _ = reader.join();
        }
    }
}
struct NativePort {
    cleanup: Option<Cleanup>,
    lines: tokio::sync::mpsc::UnboundedReceiver<std::io::Result<String>>,
    reaper: Reaper,
    finished: Option<tokio::sync::watch::Receiver<bool>>,
}
impl NativePort {
    fn spawn(
        path: &std::path::Path,
        permit: Arc<OwnedSemaphorePermit>,
        reaper: Reaper,
    ) -> Result<Self> {
        Self::spawn_inner(path, permit, reaper, false)
    }

    fn spawn_inner(
        path: &std::path::Path,
        permit: Arc<OwnedSemaphorePermit>,
        reaper: Reaper,
        fail_setup: bool,
    ) -> Result<Self> {
        let child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                ReviewError::with_source(ReviewErrorCode::EngineSpawn, "engine.spawn", e)
            })?;
        let (tx, lines) = tokio::sync::mpsc::unbounded_channel();
        // Install the cleanup guard before any fallible post-spawn setup.
        let mut port = Self {
            cleanup: Some(Cleanup {
                child,
                stdin: None,
                reader: None,
                permit,
                done: None,
            }),
            lines,
            reaper,
            finished: None,
        };
        if fail_setup {
            return Err(ReviewError::new(
                ReviewErrorCode::EngineSpawn,
                "engine.spawn",
                "setup failed",
            ));
        }
        let cleanup = port.cleanup.as_mut().expect("new port owns its child");
        cleanup.stdin = cleanup.child.stdin.take();
        let stdout = cleanup.child.stdout.take().ok_or_else(|| {
            ReviewError::new(
                ReviewErrorCode::EngineSpawn,
                "engine.spawn",
                "stdout indisponível",
            )
        })?;
        cleanup.reader = Some(
            thread::Builder::new()
                .name("engine-stdout".into())
                .spawn(move || {
                    for line in BufReader::new(stdout).lines() {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                })
                .map_err(|e| {
                    ReviewError::with_source(ReviewErrorCode::EngineSpawn, "engine.reader", e)
                })?,
        );
        Ok(port)
    }
}
impl Drop for NativePort {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            self.reaper.enqueue(cleanup);
        }
    }
}
impl EnginePort for NativePort {
    fn send(&mut self, command: &str) -> Result<()> {
        let stdin = self
            .cleanup
            .as_mut()
            .and_then(|cleanup| cleanup.stdin.as_mut())
            .ok_or_else(|| {
                ReviewError::new(
                    ReviewErrorCode::EngineExited,
                    "engine.send",
                    "Engine encerrada",
                )
            })?;
        writeln!(stdin, "{command}")
            .and_then(|_| stdin.flush())
            .map_err(|e| ReviewError::with_source(ReviewErrorCode::EngineCommand, "engine.send", e))
    }
    fn next(&mut self) -> Task<'_, Result<String>> {
        Box::pin(async move {
            self.lines
                .recv()
                .await
                .ok_or_else(|| {
                    ReviewError::new(
                        ReviewErrorCode::EngineExited,
                        "engine.read",
                        "stdout encerrado",
                    )
                })?
                .map_err(|e| {
                    ReviewError::with_source(ReviewErrorCode::EngineExited, "engine.read", e)
                })
        })
    }
    fn shutdown(&mut self) -> Task<'_, ()> {
        Box::pin(async move {
            if let Some(mut cleanup) = self.cleanup.take() {
                let (done, finished) = tokio::sync::watch::channel(false);
                cleanup.done = Some(done);
                self.finished = Some(finished);
                self.reaper.enqueue(cleanup);
            }
            if let Some(finished) = &mut self.finished {
                wait_for_cleanup(finished).await;
            }
        })
    }
}
async fn wait_for_cleanup(finished: &mut tokio::sync::watch::Receiver<bool>) {
    if finished.wait_for(|done| *done).await.is_err() {
        eprintln!("engine cleanup acknowledgement lost; cleanup is unconfirmed");
        std::future::pending::<()>().await;
    }
}
impl EngineFactory for NativeFactory {
    fn acquire<'a>(&'a self, cancel: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        Box::pin(async move {
            self.acquire_pool(1, cancel).await?.pop().ok_or_else(|| {
                ReviewError::new(ReviewErrorCode::EngineSpawn, "engine.acquire", "Pool vazio")
            })
        })
    }
    fn acquire_pool<'a>(
        &'a self,
        count: usize,
        cancel: &'a Cancellation,
    ) -> Task<'a, Result<Vec<Box<dyn EnginePort>>>> {
        Box::pin(async move {
            let permit = tokio::select! {_=cancel.cancelled()=>return Err(ReviewError::cancelled()),p=self.permit.clone().acquire_owned()=>p.map_err(|e|ReviewError::new(ReviewErrorCode::EngineSpawn,"engine.acquire",e))?};
            let permit = Arc::new(permit);
            let mut ports: Vec<Box<dyn EnginePort>> = Vec::new();
            for _ in 0..count.max(1) {
                match cancel.check().and_then(|_| {
                    NativePort::spawn(&self.path, permit.clone(), self.reaper.clone())
                }) {
                    Ok(port) => ports.push(Box::new(port)),
                    Err(error) => {
                        for port in &mut ports {
                            port.shutdown().await;
                        }
                        return Err(error);
                    }
                }
            }
            Ok(ports)
        })
    }
}
struct Inner {
    runtime: tokio::runtime::Runtime,
    factory: Arc<NativeFactory>,
    repository: Arc<SqliteRepository<NativeDatabase>>,
    sessions: Mutex<Vec<Weak<Session>>>,
    db_path: PathBuf,
}
/// Shared native backend. Clones share the runtime, database and engine lease.
/// Call `shutdown` on a worker before releasing the last owner.
#[derive(Clone)]
pub struct Backend(Arc<Inner>);
impl Backend {
    /// Opens the database and runtime; performs blocking filesystem work.
    ///
    /// # Errors
    /// Returns filesystem, database, runtime or reaper startup errors.
    pub fn open(stockfish: PathBuf, db_path: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ReviewError::with_source(ReviewErrorCode::Persistence, "db.mkdir", e)
            })?;
        }
        let conn = db::open_file(&db_path)
            .map_err(|e| ReviewError::with_source(ReviewErrorCode::Persistence, "db.open", e))?;
        let (active, _) = watch::channel(0);
        let database = NativeDatabase {
            connection: Arc::new(Mutex::new(conn)),
            active: Arc::new(active),
        };
        Ok(Self(Arc::new(Inner {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|e| {
                    ReviewError::with_source(ReviewErrorCode::Session, "runtime.open", e)
                })?,
            factory: Arc::new(NativeFactory {
                path: stockfish,
                permit: Arc::new(Semaphore::new(1)),
                reaper: Reaper::new().map_err(|e| {
                    ReviewError::with_source(ReviewErrorCode::EngineSpawn, "engine.reaper", e)
                })?,
            }),
            repository: Arc::new(SqliteRepository::new(database)),
            sessions: Mutex::new(Vec::new()),
            db_path,
        })))
    }
    /// Probe the configured engine under the same exclusive lease as reviews.
    /// Call from a worker thread; this waits for any current engine owner.
    ///
    /// # Errors
    /// Returns an error if the engine cannot start or complete its UCI handshake.
    pub fn probe(&self) -> Result<Option<String>> {
        let (_close, closed) = watch::channel(false);
        self.probe_cancellable(closed)
    }
    /// Probe on a worker outside a Tokio runtime, with cancellation covering lease waiting and the UCI handshake.
    /// Cancellation is acknowledged only after owned process teardown.
    ///
    /// # Errors
    /// Returns cancellation, spawn, transport or handshake errors.
    pub fn probe_cancellable(&self, closed: watch::Receiver<bool>) -> Result<Option<String>> {
        self.0.runtime.block_on(async {
            let cancel = Cancellation {
                closed,
                revision: None,
            };
            let mut port = self.0.factory.acquire(&cancel).await?;
            let result = engine::ask(port.as_mut(), "uci", "uciok", 8000, &cancel)
                .await
                .map(|lines| {
                    lines
                        .iter()
                        .find_map(|line| line.strip_prefix("id name ").map(str::to_owned))
                });
            port.shutdown().await;
            result
        })
    }
    /// Starts a session after validating its configuration.
    ///
    /// # Errors
    /// Returns an error for invalid input or an unavailable session registry.
    pub fn open_session(&self, id: String, config: ReviewConfig) -> Result<NativeSession> {
        session::validate(&config)?;
        if id.is_empty() || id.len() > 128 {
            return Err(ReviewError::new(
                ReviewErrorCode::InvalidPayload,
                "session.open",
                "Identificador inválido",
            ));
        }
        let (close, _) = watch::channel(false);
        let (revision, _) = watch::channel(0);
        let (finished_tx, finished) = watch::channel(0);
        let (done_tx, done) = watch::channel(false);
        let state = Arc::new(Session {
            owner: id.clone(),
            close,
            revision,
            latest: Mutex::new(None),
            finished,
            done,
        });
        self.0
            .sessions
            .lock()
            .map_err(|e| ReviewError::new(ReviewErrorCode::Session, "session.open", e))?
            .push(Arc::downgrade(&state));
        let (tx, events) = mpsc::channel();
        let task_state = state.clone();
        let pipeline = Pipeline::new(self.0.factory.clone(), self.0.repository.clone());
        self.0.runtime.spawn(async move {
            session::run_session(
                &id,
                &config,
                &task_state,
                finished_tx,
                done_tx,
                pipeline,
                &mut |event| tx.send(event).is_ok(),
            )
            .await;
        });
        Ok(NativeSession {
            events,
            control: SessionHandle {
                state,
                backend: self.clone(),
            },
        })
    }
    fn database<T: Send + 'static>(
        &self,
        run: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static,
    ) -> Result<T> {
        self.0.runtime.block_on(
            self.0
                .repository
                .db("storage", ReviewErrorCode::Persistence, run),
        )
    }
    /// Storage methods block on database work. Call them on a worker outside a Tokio runtime.
    ///
    /// # Errors
    /// Returns database or storage decoding errors.
    pub fn history(&self, limit: usize, cursor: Option<GameCursor>) -> Result<GamePage> {
        self.database(move |conn| Store::new(conn).list_page(limit, cursor.as_ref()))
    }
    /// Blocks on storage work; call on a worker outside a Tokio runtime.
    ///
    /// # Errors
    /// Returns an error if the storage operation fails.
    pub fn review_config(&self, id: i64) -> Result<Option<ReviewConfig>> {
        self.database(move |conn| Store::new(conn).get(id))?
            .map(repository::review_config)
            .transpose()
    }
    /// Blocks on storage work; call on a worker outside a Tokio runtime.
    ///
    /// # Errors
    /// Returns an error if the storage operation fails.
    pub fn delete_game(&self, id: i64) -> Result<()> {
        self.database(move |conn| Store::new(conn).remove(id))
    }
    /// Blocks on storage work; call on a worker outside a Tokio runtime.
    ///
    /// # Errors
    /// Returns an error if the storage operation fails.
    pub fn clear_history(&self) -> Result<()> {
        self.database(|conn| Store::new(conn).clear())
    }
    /// Blocks on storage work; call on a worker outside a Tokio runtime.
    ///
    /// # Errors
    /// Returns an error if the storage operation fails.
    pub fn clear_cache(&self) -> Result<()> {
        self.database(|conn| Cache::new(conn).clear())
    }
    /// Blocks on storage work; call on a worker outside a Tokio runtime.
    ///
    /// # Errors
    /// Returns an error if the storage operation fails.
    pub fn storage_stats(&self) -> Result<StorageStats> {
        let mut stats = self.database(|conn| Stats::new(conn).compute())?;
        stats.db_bytes = std::fs::metadata(&self.0.db_path)
            .map(|m| m.len())
            .unwrap_or(0);
        Ok(stats)
    }
    /// Waits for session and database cleanup. Call on a worker outside a Tokio runtime before process exit.
    pub fn shutdown(&self) {
        let sessions = self
            .0
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        for s in &sessions {
            s.close.send_replace(true);
        }
        self.0.runtime.block_on(async {
            for s in sessions {
                let mut done = s.done.clone();
                let _ = done.wait_for(|v| *v).await;
            }
            self.0.repository.database.drain().await;
        });
    }
}
pub struct NativeSession {
    pub events: mpsc::Receiver<Envelope>,
    pub control: SessionHandle,
}
impl Drop for NativeSession {
    fn drop(&mut self) {
        self.control.request_close();
    }
}
#[derive(Clone)]
pub struct SessionHandle {
    state: Arc<Session>,
    backend: Backend,
}
impl SessionHandle {
    pub fn analyze(&self, id: u64, request: LiveRequest, settings: LiveSettings) -> Result<()> {
        session::validate_live(&settings)?;
        session::update(
            &self.state,
            id,
            Some(Job {
                id,
                request,
                settings,
            }),
        )
    }
    pub fn cancel_live(&self, id: u64) -> Result<()> {
        session::update(&self.state, id, None)
    }
    pub fn request_close(&self) {
        self.state.close.send_replace(true);
    }
    pub fn is_closed(&self) -> bool {
        *self.state.done.borrow()
    }
    /// Requests closure and waits for cleanup; call on a worker outside a Tokio runtime.
    pub fn close(&self) {
        self.request_close();
        let mut done = self.state.done.clone();
        self.backend.0.runtime.block_on(async {
            let _ = done.wait_for(|v| *v).await;
        });
    }
    /// Waits for cancellation acknowledgement on a worker outside a Tokio runtime.
    pub fn wait_live_cancelled(&self, id: u64) {
        let mut finished = self.state.finished.clone();
        let mut done = self.state.done.clone();
        self.backend.0.runtime.block_on(async {
            tokio::select! {_=finished.wait_for(|v|*v>=id)=>{},_=done.wait_for(|v|*v)=>{}}
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[cfg(unix)]
    #[test]
    fn dropped_port_and_failed_setup_hold_lease_until_cleanup_finishes() {
        let reaper = Reaper::new().unwrap();
        let semaphore = Arc::new(Semaphore::new(1));
        let lease = Arc::new(semaphore.clone().try_acquire_owned().unwrap());
        let (release, blocked) = mpsc::channel::<()>();
        let reader = thread::spawn(move || {
            let _ = blocked.recv();
        });
        let child = Command::new("/bin/sleep").arg("60").spawn().unwrap();
        let (_, lines) = tokio::sync::mpsc::unbounded_channel();
        let port = NativePort {
            cleanup: Some(Cleanup {
                child,
                stdin: None,
                reader: Some(reader),
                permit: lease,
                done: None,
            }),
            lines,
            reaper: reaper.clone(),
            finished: None,
        };
        let start = std::time::Instant::now();
        drop(port);
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(semaphore.available_permits(), 0);
        release.send(()).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let lease = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), semaphore.clone().acquire_owned())
                .await
                .unwrap()
                .unwrap()
        });
        // Queue a blocker first to make the failed setup's queued child observable.
        let (release, blocked) = mpsc::channel::<()>();
        let blocker = Cleanup {
            child: Command::new("/bin/sleep").arg("60").spawn().unwrap(),
            stdin: None,
            reader: Some(thread::spawn(move || {
                let _ = blocked.recv();
            })),
            permit: Arc::new(lease),
            done: None,
        };
        let shared = blocker.permit.clone();
        reaper.enqueue(blocker);
        assert!(NativePort::spawn_inner(
            std::path::Path::new("/bin/sleep"),
            shared,
            reaper.clone(),
            true
        )
        .is_err());
        assert_eq!(semaphore.available_permits(), 0);
        release.send(()).unwrap();
        let lease = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), semaphore.clone().acquire_owned())
                .await
                .unwrap()
                .unwrap()
        });
        drop(lease);
    }

    #[cfg(unix)]
    #[test]
    fn worker_panic_recovers_in_flight_and_queued_children_before_acknowledging() {
        fn panic_worker(_: &mut Cleanup) -> std::io::Result<()> {
            panic!("injected cleanup worker failure");
        }
        let reaper = Reaper::with_worker(panic_worker).unwrap();
        let semaphore = Arc::new(Semaphore::new(1));
        let lease = Arc::new(semaphore.clone().try_acquire_owned().unwrap());
        let (release, blocked) = mpsc::channel::<()>();
        let mut acknowledgements = Vec::new();
        let mut pids = Vec::new();
        let child = Command::new("/bin/sleep").arg("60").spawn().unwrap();
        pids.push(child.id());
        let reader = thread::spawn(move || {
            let _ = blocked.recv();
        });
        let (done, finished) = tokio::sync::watch::channel(false);
        acknowledgements.push(finished);
        reaper.enqueue(Cleanup {
            child,
            stdin: None,
            reader: Some(reader),
            permit: lease.clone(),
            done: Some(done),
        });
        for _ in 0..2 {
            let child = Command::new("/bin/sleep").arg("60").spawn().unwrap();
            pids.push(child.id());
            let (done, finished) = tokio::sync::watch::channel(false);
            acknowledgements.push(finished);
            reaper.enqueue(Cleanup {
                child,
                stdin: None,
                reader: None,
                permit: lease.clone(),
                done: Some(done),
            });
        }
        drop(lease);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            assert!(tokio::time::timeout(
                Duration::from_millis(300),
                acknowledgements[0].wait_for(|done| *done)
            )
            .await
            .is_err());
            assert_eq!(semaphore.available_permits(), 0);
            release.send(()).unwrap();
            for mut acknowledgement in acknowledgements {
                tokio::time::timeout(
                    Duration::from_secs(5),
                    acknowledgement.wait_for(|done| *done),
                )
                .await
                .unwrap()
                .unwrap();
            }
        });
        assert_eq!(semaphore.available_permits(), 1);
        for pid in pids {
            assert!(
                !std::path::Path::new(&format!("/proc/{pid}")).exists(),
                "child {pid} was not reaped"
            );
        }
    }

    #[test]
    fn lost_cleanup_acknowledgement_never_completes_shutdown() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (done, mut finished) = tokio::sync::watch::channel(false);
            drop(done);
            assert!(tokio::time::timeout(
                Duration::from_millis(100),
                wait_for_cleanup(&mut finished)
            )
            .await
            .is_err());
        });
    }

    #[test]
    fn cancelled_probe_does_not_wait_for_another_owners_lease() {
        let backend =
            Backend::open(PathBuf::from("/missing/engine"), PathBuf::from(":memory:")).unwrap();
        let lease = backend
            .0
            .factory
            .permit
            .clone()
            .try_acquire_owned()
            .unwrap();
        let (cancel, closed) = watch::channel(false);
        let (tx, rx) = mpsc::channel();
        let probe = backend.clone();
        let worker = thread::spawn(move || tx.send(probe.probe_cancellable(closed)).unwrap());
        cancel.send_replace(true);
        let error = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("cancelled probe must finish while the other owner still holds its lease")
            .unwrap_err();
        assert_eq!(error.code, ReviewErrorCode::Cancelled);
        assert_eq!(backend.0.factory.permit.available_permits(), 0);
        worker.join().unwrap();
        drop(lease);
        assert_eq!(backend.0.factory.permit.available_permits(), 1);
        backend.shutdown();
    }
}
