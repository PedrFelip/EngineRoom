//! Window-owned sessions. IPC records intent; the task serializes all engine work.
use super::{
    engine::{self, Cancellation, EngineFactory, SidecarFactory},
    pipeline::{hash_mb, Pipeline},
    repository::{normalize, SqliteRepository},
    types::*,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{ipc::Channel, WebviewWindow};
use tokio::sync::{watch, Semaphore};

#[derive(Clone)]
struct Job {
    id: u64,
    request: LiveRequest,
    settings: LiveSettings,
}
struct Session {
    owner: String,
    close: watch::Sender<bool>,
    revision: watch::Sender<u64>,
    latest: Mutex<Option<Job>>,
    finished: watch::Receiver<u64>,
    done: watch::Receiver<bool>,
}
pub struct ReviewSessions {
    sessions: Arc<Mutex<HashMap<String, Arc<Session>>>>,
    permit: Arc<Semaphore>,
    exiting: AtomicBool,
}
impl Default for ReviewSessions {
    fn default() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            permit: Arc::new(Semaphore::new(1)),
            exiting: AtomicBool::new(false),
        }
    }
}
impl ReviewSessions {
    fn get(&self, id: &str, owner: &str) -> Result<Arc<Session>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|e| ReviewError::new("session", "session.lookup", e))?;
        let s = sessions
            .get(id)
            .filter(|s| s.owner == owner)
            .ok_or_else(|| {
                ReviewError::new("sessionClosed", "session.lookup", "Sessão encerrada.")
            })?;
        Ok(s.clone())
    }
    fn register(
        &self,
        id: &str,
        owner: &str,
    ) -> Result<(Arc<Session>, watch::Sender<u64>, watch::Sender<bool>)> {
        if self.exiting.load(Ordering::SeqCst) {
            return Err(ReviewError::new(
                "sessionClosed",
                "session.open",
                "O aplicativo está encerrando.",
            ));
        }
        if id.is_empty() || id.len() > 128 {
            return Err(ReviewError::new(
                "invalidPayload",
                "session.open",
                "Identificador inválido.",
            ));
        }
        let (close, _) = watch::channel(false);
        let (revision, _) = watch::channel(0);
        let (finished_tx, finished) = watch::channel(0);
        let (done_tx, done) = watch::channel(false);
        let s = Arc::new(Session {
            owner: owner.into(),
            close,
            revision,
            latest: Mutex::new(None),
            finished,
            done,
        });
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| ReviewError::new("session", "session.open", e))?;
        if self.exiting.load(Ordering::SeqCst) {
            return Err(ReviewError::new(
                "sessionClosed",
                "session.open",
                "O aplicativo está encerrando.",
            ));
        }
        if sessions.contains_key(id) {
            return Err(ReviewError::new(
                "sessionExists",
                "session.open",
                "Sessão já aberta.",
            ));
        }
        sessions.insert(id.into(), s.clone());
        Ok((s, finished_tx, done_tx))
    }
    pub fn close_window(&self, owner: &str) {
        if let Ok(sessions) = self.sessions.lock() {
            for s in sessions.values().filter(|s| s.owner == owner) {
                s.close.send_replace(true);
            }
        }
    }
    pub fn begin_shutdown(&self) -> Option<engine::Task<'static, ()>> {
        if self.exiting.swap(true, Ordering::SeqCst) {
            return None;
        }
        let sessions: Vec<_> = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        for session in &sessions {
            session.close.send_replace(true);
        }
        Some(Box::pin(async move {
            for session in sessions {
                let mut done = session.done.clone();
                let _ = done.wait_for(|v| *v).await;
            }
        }))
    }
}
fn validate(config: &ReviewConfig) -> Result<()> {
    if !(1..=128).contains(&config.engine.depth)
        || !(1..=5).contains(&config.lines)
        || (config.mode == crate::db::mode::Mode::Time
            && config.movetime_ms.is_some_and(|v| v == 0 || v > 30000))
    {
        return Err(ReviewError::new(
            "invalidPayload",
            "session.open",
            "Parâmetros de análise inválidos.",
        ));
    }
    if let Some(result) = &config.initial_result {
        super::repository::validate_review(result)?;
    }
    Ok(())
}
fn validate_live(settings: &LiveSettings) -> Result<()> {
    if !(1..=30).contains(&settings.search_seconds)
        || !(1..=5).contains(&settings.lines)
        || !(1..=256).contains(&settings.threads)
        || !(16..=4096).contains(&settings.memory_mb)
    {
        Err(ReviewError::new(
            "invalidPayload",
            "session.live",
            "Parâmetros de análise ao vivo inválidos.",
        ))
    } else {
        Ok(())
    }
}
fn remove(sessions: &Arc<Mutex<HashMap<String, Arc<Session>>>>, id: &str, s: &Arc<Session>) {
    if let Ok(mut sessions) = sessions.lock() {
        if sessions
            .get(id)
            .is_some_and(|current| Arc::ptr_eq(current, s))
        {
            sessions.remove(id);
        }
    }
}
#[tauri::command]
pub fn review_session_open(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, ReviewSessions>,
    session_id: String,
    config: ReviewConfig,
    on_event: Channel<Envelope>,
) -> Result<()> {
    validate(&config)?;
    let (session, finished, done) = state.register(&session_id, window.label())?;
    let sessions = state.sessions.clone();
    let factory = Arc::new(SidecarFactory {
        app: app.clone(),
        permit: state.permit.clone(),
    });
    tauri::async_runtime::spawn(async move {
        run_session(
            &session_id,
            &config,
            &session,
            finished,
            done,
            Pipeline::new(factory, Arc::new(SqliteRepository::new(app))),
            &mut |envelope| on_event.send(envelope).is_ok(),
        )
        .await;
        remove(&sessions, &session_id, &session);
    });
    Ok(())
}
async fn run_session(
    session_id: &str,
    config: &ReviewConfig,
    session: &Session,
    finished: watch::Sender<u64>,
    done: watch::Sender<bool>,
    mut pipeline: Pipeline,
    publish_envelope: &mut (dyn FnMut(Envelope) -> bool + Send),
) {
    let root = Cancellation {
        closed: session.close.subscribe(),
        revision: None,
    };
    let mut sequence = 0;
    let mut resources_detected = false;
    let mut publish = |request_id, event| {
        if let Event::Warning { error } = &event {
            eprintln!("review warning [{}]: {}", error.operation, error.message);
        }
        sequence += 1;
        if !publish_envelope(Envelope {
            session_id: session_id.to_owned(),
            sequence,
            request_id,
            event,
        }) {
            session.close.send_replace(true);
        }
    };
    let result = if let Some(initial) = &config.initial_result {
        serde_json::to_value(initial)
            .map_err(|e| ReviewError::new("invalidPayload", "session.restore", e))
            .and_then(normalize)
    } else {
        publish(
            None,
            Event::Progress {
                progress: Progress {
                    stage: "preparing".into(),
                    completed: 0,
                    total: config.meta.plies + 1,
                    current_ply: 0,
                    phase: None,
                    cached_positions: 0,
                    engine_positions: 0,
                    remaining_budget_ms: None,
                    update: None,
                },
            },
        );
        let resources = tauri::async_runtime::spawn_blocking(crate::system::system_resources)
            .await
            .ok();
        let sizing = resources.map(|r| {
            pipeline.set_detected_resources(r.threads as u32, r.memory_mb as u32);
            resources_detected = true;
            (r.threads as u32, hash_mb(r.memory_mb as u32))
        });
        pipeline
            .review(config, sizing, &root, &mut |event| publish(None, event))
            .await
    };
    let ready = match result {
        Ok(result) => {
            if root.check().is_ok() {
                publish(
                    None,
                    Event::Completed {
                        result: result.clone(),
                    },
                );
                if config.initial_result.is_none() {
                    if let Err(error) = pipeline.save(config, &result).await {
                        publish(None, Event::Warning { error });
                    }
                }
            }
            true
        }
        Err(error) => {
            if error.code != "cancelled" {
                publish(None, Event::Error { error, fen: None });
            }
            false
        }
    };
    let mut revisions = session.revision.subscribe();
    let mut observed = 0;
    while root.check().is_ok() {
        let (id, job) = {
            let latest = session.latest.lock().unwrap_or_else(|e| e.into_inner());
            (*session.revision.borrow(), latest.clone())
        };
        if id != observed {
            observed = id;
            if let Some(job) = job.filter(|_| ready) {
                let cancel = Cancellation {
                    closed: session.close.subscribe(),
                    revision: Some((session.revision.subscribe(), job.id)),
                };
                if job.settings.threads_auto && !resources_detected {
                    // Detection is best-effort and remains outside the engine lease.
                    if let Ok(r) =
                        tauri::async_runtime::spawn_blocking(crate::system::system_resources).await
                    {
                        pipeline.set_detected_resources(r.threads as u32, r.memory_mb as u32);
                        resources_detected = true;
                    }
                }
                if let Err(error) = pipeline
                    .live(&job.request, &job.settings, &cancel, &mut |event| {
                        publish(Some(job.id), event)
                    })
                    .await
                {
                    if error.code != "cancelled" {
                        publish(
                            Some(job.id),
                            Event::Error {
                                error,
                                fen: Some(job.request.fen),
                            },
                        );
                    }
                }
            } else {
                pipeline.discard().await;
            }
            finished.send_replace(id);
            continue;
        }
        tokio::select! { _ = root.cancelled() => break, change = revisions.changed() => { if change.is_err() { break; } } }
    }
    pipeline.close().await;
    done.send_replace(true);
}
fn update(s: &Session, id: u64, job: Option<Job>) -> Result<()> {
    if id == 0 || id > 9_007_199_254_740_991 {
        return Err(ReviewError::new(
            "invalidPayload",
            "session.intent",
            "Identificador do pedido inválido.",
        ));
    }
    let mut latest = s
        .latest
        .lock()
        .map_err(|e| ReviewError::new("session", "session.intent", e))?;
    if *s.close.borrow() {
        return Err(ReviewError::new(
            "sessionClosed",
            "session.intent",
            "Sessão encerrada.",
        ));
    }
    if id > *s.revision.borrow() {
        *latest = job;
        s.revision.send_replace(id);
    }
    Ok(())
}
#[tauri::command]
pub fn review_session_analyze_position(
    window: WebviewWindow,
    state: tauri::State<'_, ReviewSessions>,
    session_id: String,
    request_id: u64,
    request: LiveRequest,
    settings: LiveSettings,
) -> Result<()> {
    validate_live(&settings)?;
    let session = state.get(&session_id, window.label())?;
    update(
        &session,
        request_id,
        Some(Job {
            id: request_id,
            request,
            settings,
        }),
    )
}
#[tauri::command]
pub async fn review_session_cancel_live(
    window: WebviewWindow,
    state: tauri::State<'_, ReviewSessions>,
    session_id: String,
    request_id: u64,
) -> Result<()> {
    let s = state.get(&session_id, window.label())?;
    update(&s, request_id, None)?;
    let mut finished = s.finished.clone();
    let mut done = s.done.clone();
    tokio::select! { _ = finished.wait_for(|v| *v >= request_id) => {}, _ = done.wait_for(|v| *v) => {} }
    Ok(())
}
#[tauri::command]
pub async fn review_session_close(
    window: WebviewWindow,
    state: tauri::State<'_, ReviewSessions>,
    session_id: String,
) -> Result<()> {
    let s = match state.get(&session_id, window.label()) {
        Ok(s) => s,
        Err(e) if e.code == "sessionClosed" => return Ok(()),
        Err(e) => return Err(e),
    };
    s.close.send_replace(true);
    let mut done = s.done.clone();
    let _ = done.wait_for(|v| *v).await;
    Ok(())
}
#[derive(Clone, serde::Serialize)]
pub struct ProbeResult {
    pub ok: bool,
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
#[tauri::command]
pub fn engine_probe(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, ReviewSessions>,
    session_id: String,
    on_event: Channel<ProbeResult>,
    timeout_ms: Option<u64>,
) -> Result<()> {
    let timeout = timeout_ms.unwrap_or(8000).clamp(1, 30000);
    let (session, _, done) = state.register(&session_id, window.label())?;
    let sessions = state.sessions.clone();
    let factory = SidecarFactory {
        app,
        permit: state.permit.clone(),
    };
    tauri::async_runtime::spawn(async move {
        let cancel = Cancellation {
            closed: session.close.subscribe(),
            revision: None,
        };
        let mut owned = None;
        let result = async {
            let port = factory.acquire(&cancel).await?;
            owned = Some(port);
            let lines = engine::ask(
                owned.as_mut().unwrap().as_mut(),
                "uci",
                "uciok",
                timeout,
                &cancel,
            )
            .await?;
            Ok::<_, ReviewError>(
                lines
                    .iter()
                    .find_map(|l| l.strip_prefix("id name ").map(str::to_owned)),
            )
        }
        .await;
        if let Some(mut port) = owned {
            port.shutdown().await;
        }
        let payload = match result {
            Ok(name) => ProbeResult {
                ok: true,
                name,
                error: None,
            },
            Err(e) => ProbeResult {
                ok: false,
                name: None,
                error: Some(e.message),
            },
        };
        let _ = on_event.send(payload);
        done.send_replace(true);
        remove(&sessions, &session_id, &session);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::tests::{config, factory, request, settings, FakeState, MemoryRepo};
    use std::time::Duration;

    fn restored() -> ReviewConfig {
        let mut c = config("1. e4 e5", AnalysisKind::Manual);
        let game = super::super::core::extract(&c.pgn).unwrap();
        let raw: Vec<_> = game
            .fens
            .iter()
            .map(|f| super::super::core::terminal_raw(f, 0))
            .collect();
        c.initial_result = Some(super::super::core::build(&game, &raw).unwrap());
        c
    }
    async fn receive(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<Envelope>,
        wanted: impl Fn(&Envelope) -> bool,
    ) -> Envelope {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let e = rx.recv().await.unwrap();
                if wanted(&e) {
                    return e;
                }
            }
        })
        .await
        .expect("session event timeout")
    }
    #[tokio::test]
    async fn only_latest_navigation_runs_and_close_finishes_all_resources() {
        let manager = ReviewSessions::default();
        let (s, finished, done) = manager.register("session", "main").unwrap();
        let config = restored();
        let f = factory(FakeState::default());
        let repo = Arc::new(MemoryRepo::default());
        let fen = config.initial_result.as_ref().unwrap().positions[0]
            .fen
            .clone();
        for id in 1..=100 {
            update(
                &s,
                id,
                Some(Job {
                    id,
                    request: request(&fen),
                    settings: settings(),
                }),
            )
            .unwrap();
        }
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let owned = s.clone();
        let factory = f.clone();
        let running = tokio::spawn(async move {
            run_session(
                "session",
                &config,
                &owned,
                finished,
                done,
                Pipeline::new(factory, repo),
                &mut |e| tx.send(e).is_ok(),
            )
            .await;
        });
        let completed = receive(&mut rx, |e| matches!(e.event, Event::LiveCompleted { .. })).await;
        assert_eq!(completed.request_id, Some(100));
        assert_eq!(f.state.lock().unwrap().searches, 1);
        s.close.send_replace(true);
        running.await.unwrap();
        assert!(*s.done.borrow());
        assert_eq!(f.state.lock().unwrap().stopped, 1);
        assert_eq!(f.permit.available_permits(), 1);
    }
    #[tokio::test]
    async fn replacing_live_search_waits_for_discard_and_does_not_emit_old_result() {
        let manager = ReviewSessions::default();
        let (s, finished, done) = manager.register("session", "main").unwrap();
        let config = restored();
        let fen = config.initial_result.as_ref().unwrap().positions[0]
            .fen
            .clone();
        let f = factory(FakeState {
            stall: true,
            ..Default::default()
        });
        update(
            &s,
            1,
            Some(Job {
                id: 1,
                request: request(&fen),
                settings: settings(),
            }),
        )
        .unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let owned = s.clone();
        let factory = f.clone();
        let running = tokio::spawn(async move {
            run_session(
                "session",
                &config,
                &owned,
                finished,
                done,
                Pipeline::new(factory, Arc::new(MemoryRepo::default())),
                &mut |e| tx.send(e).is_ok(),
            )
            .await;
        });
        receive(&mut rx, |e| matches!(e.event, Event::LiveStarted { .. })).await;
        while f.state.lock().unwrap().searches == 0 {
            tokio::task::yield_now().await;
        }
        f.state.lock().unwrap().stall = false;
        update(
            &s,
            2,
            Some(Job {
                id: 2,
                request: request(&fen),
                settings: settings(),
            }),
        )
        .unwrap();
        let event = receive(&mut rx, |e| matches!(e.event, Event::LiveCompleted { .. })).await;
        assert_eq!(event.request_id, Some(2));
        assert_eq!(f.state.lock().unwrap().stopped, 1);
        assert_eq!(f.acquired.load(std::sync::atomic::Ordering::SeqCst), 2);
        s.close.send_replace(true);
        running.await.unwrap();
        assert_eq!(f.state.lock().unwrap().stopped, 2);
    }
    #[tokio::test]
    async fn persistence_failure_does_not_invalidate_completed_review() {
        let manager = ReviewSessions::default();
        let (s, finished, done) = manager.register("session", "main").unwrap();
        let f = factory(FakeState::default());
        let repo = Arc::new(MemoryRepo {
            fail_save: true,
            ..Default::default()
        });
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let owned = s.clone();
        let factory = f.clone();
        let running = tokio::spawn(async move {
            run_session(
                "session",
                &config("1. e4", AnalysisKind::Manual),
                &owned,
                finished,
                done,
                Pipeline::new(factory, repo),
                &mut |e| tx.send(e).is_ok(),
            )
            .await;
        });
        receive(&mut rx, |e| matches!(e.event, Event::Completed { .. })).await;
        let warning = receive(&mut rx, |e| matches!(e.event, Event::Warning { .. })).await;
        assert!(matches!(warning.event,Event::Warning {error:e} if e.code=="persistence"));
        s.close.send_replace(true);
        running.await.unwrap();
        assert!(*s.done.borrow());
    }
    #[test]
    fn window_ownership_duplicate_sessions_and_out_of_order_intents() {
        let manager = ReviewSessions::default();
        let (s, _, _) = manager.register("session", "main").unwrap();
        assert!(manager.register("session", "main").is_err());
        assert!(manager.get("session", "other").is_err());
        update(&s, 5, None).unwrap();
        update(&s, 2, None).unwrap();
        assert_eq!(*s.revision.borrow(), 5);
        manager.close_window("other");
        assert!(!*s.close.borrow());
        manager.close_window("main");
        assert!(*s.close.borrow());
        assert!(update(&s, 6, None).is_err());
    }
    #[tokio::test]
    async fn app_exit_waits_for_session_cleanup_and_blocks_new_acquisitions() {
        let manager = ReviewSessions::default();
        let (s, _, done) = manager.register("session", "main").unwrap();
        let cleanup = manager.begin_shutdown().unwrap();
        assert!(*s.close.borrow());
        assert!(manager.register("new", "main").is_err());
        assert!(manager.begin_shutdown().is_none());
        let waiting = tokio::spawn(cleanup);
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());
        done.send_replace(true);
        waiting.await.unwrap();
    }

    #[tokio::test]
    async fn real_stockfish_sidecar_cache_history_reopen_and_live_cancellation() {
        use crate::review::{
            core,
            engine::{EnginePort, Task},
            pipeline::Pipeline,
            repository::{Repository, SqliteRepository},
        };
        use std::sync::atomic::AtomicUsize;
        use tauri::Manager;
        // Test the production shell plugin and raw stdout transport without a GUI.
        // Unlike the old handshake test, a missing sidecar must fail here.
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_shell::init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        app.manage(crate::db::DbState(Mutex::new(
            crate::db::open_memory().unwrap(),
        )));
        let manager = ReviewSessions::default();
        struct RecordingFactory<F> {
            factory: F,
            sent: Arc<Mutex<Vec<String>>>,
            go: Arc<tokio::sync::Notify>,
            stopped: Arc<AtomicUsize>,
        }
        struct RecordingPort {
            port: Box<dyn EnginePort>,
            sent: Arc<Mutex<Vec<String>>>,
            go: Arc<tokio::sync::Notify>,
            stopped: Arc<AtomicUsize>,
        }
        impl EnginePort for RecordingPort {
            fn send(&mut self, cmd: &str) -> Result<()> {
                self.sent.lock().unwrap().push(cmd.into());
                self.port.send(cmd)?;
                if cmd.starts_with("go ") {
                    self.go.notify_one();
                }
                Ok(())
            }
            fn next(&mut self) -> Task<'_, Result<String>> {
                self.port.next()
            }
            fn shutdown(&mut self) -> Task<'_, ()> {
                Box::pin(async move {
                    self.port.shutdown().await;
                    self.stopped.fetch_add(1, Ordering::SeqCst);
                })
            }
        }
        impl<F: EngineFactory> EngineFactory for RecordingFactory<F> {
            fn acquire<'a>(
                &'a self,
                cancel: &'a Cancellation,
            ) -> Task<'a, Result<Box<dyn EnginePort>>> {
                Box::pin(async move {
                    Ok(Box::new(RecordingPort {
                        port: self.factory.acquire(cancel).await?,
                        sent: self.sent.clone(),
                        go: self.go.clone(),
                        stopped: self.stopped.clone(),
                    }) as Box<dyn EnginePort>)
                })
            }
        }
        let sent = Arc::new(Mutex::new(Vec::new()));
        let go = Arc::new(tokio::sync::Notify::new());
        let stopped = Arc::new(AtomicUsize::new(0));
        let factory = Arc::new(RecordingFactory {
            factory: SidecarFactory {
                app: app.handle().clone(),
                permit: manager.permit.clone(),
            },
            sent: sent.clone(),
            go: go.clone(),
            stopped: stopped.clone(),
        });
        let repo = Arc::new(SqliteRepository::new(app.handle().clone()));
        let (close, closed) = watch::channel(false);
        let root = Cancellation {
            closed,
            revision: None,
        };
        let mut pipeline = Pipeline::new(factory.clone(), repo.clone());
        let mut manual = config(
            "[White \"Alice\"]\n[Black \"Bob\"]\n1. e4 e5 1/2-1/2",
            AnalysisKind::Manual,
        );
        manual.engine.depth = 8;
        let mut events = Vec::new();
        let review = pipeline
            .review(&manual, None, &root, &mut |e| events.push(e))
            .await
            .unwrap();
        assert_eq!(review.moves.len(), 2);
        assert!(review.positions.iter().all(|p| p.depth >= 8));
        assert_eq!(manager.permit.available_permits(), 1);
        assert!(sent.lock().unwrap().iter().any(|c| c == "go depth 8"));
        repo.save(&manual, &review).await.unwrap();
        {
            let db = app.state::<crate::db::DbState>();
            let conn = db.0.lock().unwrap();
            let store = crate::db::games::Store::new(&conn);
            let page = store.list_page(10, None).unwrap();
            assert_eq!(page.games.len(), 1);
            assert_eq!(page.games[0].result, "1/2-1/2");
            let saved = store.get(page.games[0].id).unwrap().unwrap();
            let restored = normalize(serde_json::from_str(&saved.review_json).unwrap()).unwrap();
            assert_eq!(
                serde_json::to_value(&restored).unwrap(),
                serde_json::to_value(&review).unwrap()
            );
        }
        let searches = sent
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.starts_with("go "))
            .count();
        events.clear();
        let stopped_before_cache = stopped.load(Ordering::SeqCst);
        pipeline
            .review(&manual, None, &root, &mut |e| events.push(e))
            .await
            .unwrap();
        assert_eq!(
            searches,
            sent.lock()
                .unwrap()
                .iter()
                .filter(|c| c.starts_with("go "))
                .count()
        );
        assert!(events.iter().any(|e|matches!(e,Event::Progress {progress:p} if p.cached_positions==3 && p.engine_positions==0)));
        assert_eq!(stopped.load(Ordering::SeqCst), stopped_before_cache);
        let mut adaptive = manual.clone();
        adaptive.analysis_kind = AnalysisKind::Fast;
        pipeline
            .review(&adaptive, None, &root, &mut |_| {})
            .await
            .unwrap();
        assert!(sent.lock().unwrap().iter().any(|c| c == "go movetime 180"));
        let fen = core::extract(&manual.pgn).unwrap().fens[1].clone();
        let mut live = settings();
        live.lines = 1;
        pipeline
            .live(&request(&fen), &live, &root, &mut |_| {})
            .await
            .unwrap();
        pipeline.discard().await;
        assert!(sent.lock().unwrap().iter().any(|c| c == "go movetime 1000"));
        // Reopening must publish the stored review without acquiring a process.
        let (session, finished, done) = manager.register("real", "main").unwrap();
        manual.initial_result = Some(review);
        live.search_seconds = 30;
        update(
            &session,
            1,
            Some(Job {
                id: 1,
                request: request(&fen),
                settings: live,
            }),
        )
        .unwrap();
        // Drain notifications from the previous pipeline searches.
        while tokio::time::timeout(Duration::from_millis(1), go.notified())
            .await
            .is_ok()
        {}
        let owned = session.clone();
        let current_factory = factory.clone();
        let repository = repo.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let running = tokio::spawn(async move {
            run_session(
                "real",
                &manual,
                &owned,
                finished,
                done,
                Pipeline::new(current_factory, repository),
                &mut |e| tx.send(e).is_ok(),
            )
            .await;
        });
        receive(&mut rx, |e| matches!(e.event, Event::Completed { .. })).await;
        tokio::time::timeout(Duration::from_secs(3), go.notified())
            .await
            .expect("live search started");
        let before = std::time::Instant::now();
        session.close.send_replace(true);
        tokio::time::timeout(Duration::from_secs(3), running)
            .await
            .expect("cancelled search teardown")
            .unwrap();
        assert!(before.elapsed() < Duration::from_secs(3));
        assert!(sent
            .lock()
            .unwrap()
            .iter()
            .any(|c| c == "go movetime 30000"));
        assert_eq!(manager.permit.available_permits(), 1);
        assert!(*session.done.borrow());
        // The same lease remains usable for Settings' UCI probe after cancellation.
        let mut probe = factory.acquire(&root).await.unwrap();
        let id = engine::ask(probe.as_mut(), "uci", "uciok", 8000, &root)
            .await
            .unwrap();
        assert!(id.iter().any(|l| l.contains("Stockfish 18")));
        probe.shutdown().await;
        assert_eq!(manager.permit.available_permits(), 1);
        assert!(stopped.load(Ordering::SeqCst) >= 5);
        drop(close);
    }
}
