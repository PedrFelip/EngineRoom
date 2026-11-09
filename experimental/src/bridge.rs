//! Tokio ownership and event delivery for the native session API.
use engineroom_backend::review::{
    engine::EngineFactory,
    native_engine::NativeEngineFactory,
    repository::{Repository, SqliteRepository},
    session::ReviewSessions,
    types::{Envelope, LiveRequest, LiveSettings, ReviewConfig, ReviewError},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};

// Integration contract: drain() retains the existing analysis stream; drain_lifecycle()
// reports asynchronous open/close acknowledgements and typed failures. Consumers should
// associate these with session IDs and ignore superseded opens.
#[derive(Debug, Clone)]
pub enum BridgeLifecycle {
    Opened { id: String },
    Closed { id: String },
    Error { id: String, error: ReviewError },
}

enum ImportIntent {
    Search(engineroom_backend::imports::SearchRequest),
    Cancel(String),
}

enum Intent {
    Open {
        id: String,
        config: ReviewConfig,
        generation: u64,
    },
    Analyze {
        id: String,
        request_id: u64,
        request: LiveRequest,
        settings: LiveSettings,
    },
    Cancel {
        id: String,
        request_id: u64,
    },
    Close {
        id: String,
    },
    ReadFile(PathBuf),
    ListGames(Option<engineroom_backend::db::games::GameCursor>),
    StorageStats,
    ClearCache,
    ClearHistory,
    LoadGame(i64, bool, u64),
    DeleteGame(i64),
}

pub enum DataEvent {
    Games(
        Result<engineroom_backend::db::games::GamePage, String>,
        bool,
    ),
    Storage(Result<engineroom_backend::db::stats::StorageStats, String>),
    Cleared(Result<(), String>, bool),
    Game(Result<Option<Box<ReviewConfig>>, String>, bool, u64),
}

pub struct Bridge {
    runtime: tokio::runtime::Runtime,
    import_actor: Option<tokio::task::JoinHandle<()>>,
    import_intents: Option<tokio::sync::mpsc::UnboundedSender<ImportIntent>>,
    import_events: mpsc::Receiver<engineroom_backend::imports::Event>,
    actor: Option<tokio::task::JoinHandle<()>>,
    intents: Option<tokio::sync::mpsc::UnboundedSender<Intent>>,
    generation: Arc<AtomicU64>,
    events: mpsc::Receiver<(u64, Envelope)>,
    lifecycle: mpsc::Receiver<BridgeLifecycle>,
    files: mpsc::Receiver<Result<String, String>>,
    data: mpsc::Receiver<DataEvent>,
}
impl Bridge {
    pub fn new(wake: rune::Waker) -> Result<Self, String> {
        // A missing engine must not disable storage, history, file and Chess.com
        // imports: the acquisition error is reported only when a review starts.
        let engine = engine_path();
        let base = crate::desktop_paths::data_dir()
            .map_err(|e| format!("Diretório de dados indisponível: {e}"))?;
        std::fs::create_dir_all(&base)
            .map_err(|e| format!("Não foi possível criar {}: {e}", base.display()))?;
        let db = base.join("engineroom.db");
        let repository = Arc::new(SqliteRepository::new(db.clone()));
        let http =
            Arc::new(engineroom_backend::imports::ChessComHttp::new().map_err(|e| e.to_string())?);
        Self::with_data_ports(
            Arc::new(NativeEngineFactory::new(engine)),
            repository.clone(),
            Some(repository),
            Arc::new(move || (wake)()),
            http,
            Some(db),
        )
    }

    #[cfg(test)]
    pub fn with_ports(
        factory: Arc<dyn EngineFactory>,
        repository: Arc<dyn Repository>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        Self::with_data_ports(
            factory,
            repository,
            None,
            wake,
            Arc::new(engineroom_backend::imports::ChessComHttp::new().map_err(|e| e.to_string())?),
            None,
        )
    }
    fn with_data_ports(
        factory: Arc<dyn EngineFactory>,
        repository: Arc<dyn Repository>,
        data_repo: Option<Arc<SqliteRepository>>,
        wake: Arc<dyn Fn() + Send + Sync>,
        http: Arc<dyn engineroom_backend::imports::Http>,
        db_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        let (import_tx, import_events) = mpsc::channel();
        let (import_intents, mut import_rx) = tokio::sync::mpsc::unbounded_channel();
        let import_wake = wake.clone();
        let import_actor = runtime.spawn(async move {
            use engineroom_backend::imports::{Event, EventBody, ImportTasks};
            let tasks = ImportTasks::new(http);
            while let Some(intent) = import_rx.recv().await {
                match intent {
                    ImportIntent::Search(request) => {
                        let id = request.request_id.clone();
                        let tx = import_tx.clone();
                        let wake = import_wake.clone();
                        if let Err(error) = tasks
                            .replace(request, move |event| {
                                let _ = tx.send(event);
                                wake();
                            })
                            .await
                        {
                            let _ = import_tx.send(Event {
                                request_id: id,
                                body: EventBody::Error { error },
                            });
                            import_wake();
                        }
                    }
                    ImportIntent::Cancel(id) => {
                        let _ = tasks.cancel("window", &id).await;
                    }
                }
            }
            if let Some(shutdown) = tasks.begin_shutdown() {
                shutdown.await;
            }
        });
        let sessions = Arc::new(ReviewSessions::new(factory, repository, None));
        let (sender, events) = mpsc::channel();
        let (lifecycle_tx, lifecycle) = mpsc::channel();
        let (files_tx, files) = mpsc::channel();
        let (data_tx, data) = mpsc::channel();
        let (intents, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let generation = Arc::new(AtomicU64::new(0));
        let current_generation = generation.clone();
        let actor = runtime.spawn(async move {
            let mut active: Option<String> = None;
            while let Some(intent) = receiver.recv().await {
                match intent {
                    Intent::Open {
                        id,
                        config,
                        generation,
                    } => {
                        if let Some(previous) = active.take() {
                            let result = sessions.close(&previous, "window").await;
                            publish_lifecycle(
                                &lifecycle_tx,
                                &wake,
                                match result {
                                    Ok(()) => BridgeLifecycle::Closed { id: previous },
                                    Err(error) => BridgeLifecycle::Error {
                                        id: previous,
                                        error,
                                    },
                                },
                            );
                        }
                        if current_generation.load(Ordering::SeqCst) != generation {
                            continue;
                        }
                        let event_sender = sender.clone();
                        let event_wake = wake.clone();
                        let event_generation = current_generation.clone();
                        match sessions.open(&id, "window", config, move |event| {
                            if event_generation.load(Ordering::SeqCst) != generation {
                                return true;
                            }
                            let sent = event_sender.send((generation, event)).is_ok();
                            event_wake();
                            sent
                        }) {
                            Ok(()) => {
                                active = Some(id.clone());
                                publish_lifecycle(
                                    &lifecycle_tx,
                                    &wake,
                                    BridgeLifecycle::Opened { id },
                                );
                            }
                            Err(error) => publish_lifecycle(
                                &lifecycle_tx,
                                &wake,
                                BridgeLifecycle::Error { id, error },
                            ),
                        }
                    }
                    Intent::Analyze {
                        id,
                        request_id,
                        request,
                        settings,
                    } => {
                        if let Err(error) =
                            sessions.analyze_position(&id, "window", request_id, request, settings)
                        {
                            publish_lifecycle(
                                &lifecycle_tx,
                                &wake,
                                BridgeLifecycle::Error { id, error },
                            );
                        }
                    }
                    Intent::Cancel { id, request_id } => {
                        if let Err(error) = sessions.cancel_live(&id, "window", request_id).await {
                            publish_lifecycle(
                                &lifecycle_tx,
                                &wake,
                                BridgeLifecycle::Error { id, error },
                            );
                        }
                    }
                    Intent::ListGames(cursor) => {
                        let append = cursor.is_some();
                        let result = match &data_repo {
                            Some(repo) => repo
                                .list_games(100, cursor)
                                .await
                                .map_err(|e| e.to_string()),
                            None => Err("Histórico indisponível".into()),
                        };
                        let _ = data_tx.send(DataEvent::Games(result, append));
                        wake();
                    }
                    Intent::LoadGame(id, reanalyze, request_id) => {
                        let result = match &data_repo {
                            Some(repo) => repo
                                .review_config(id)
                                .await
                                .map(|config| config.map(Box::new))
                                .map_err(|e| e.to_string()),
                            None => Err("Histórico indisponível".into()),
                        };
                        let _ = data_tx.send(DataEvent::Game(result, reanalyze, request_id));
                        wake();
                    }
                    Intent::DeleteGame(id) => {
                        let result = match &data_repo {
                            Some(repo) => match repo.delete_game(id).await {
                                Ok(()) => {
                                    repo.list_games(100, None).await.map_err(|e| e.to_string())
                                }
                                Err(e) => Err(e.to_string()),
                            },
                            None => Err("Histórico indisponível".into()),
                        };
                        let _ = data_tx.send(DataEvent::Games(result, false));
                        wake();
                    }
                    Intent::StorageStats => {
                        let result = match &data_repo {
                            Some(repo) => repo
                                .storage_stats()
                                .await
                                .map(|mut stats| {
                                    // The repository reports table sizes; the file
                                    // size is owned by the desktop bridge.
                                    if let Some(path) = &db_path {
                                        stats.db_bytes = std::fs::metadata(path)
                                            .map(|metadata| metadata.len())
                                            .unwrap_or(0);
                                    }
                                    stats
                                })
                                .map_err(|e| e.to_string()),
                            None => Err("Armazenamento indisponível".into()),
                        };
                        let _ = data_tx.send(DataEvent::Storage(result));
                        wake();
                    }
                    Intent::ClearCache | Intent::ClearHistory => {
                        let history = matches!(intent, Intent::ClearHistory);
                        let result = match &data_repo {
                            Some(repo) if history => {
                                repo.clear_games().await.map_err(|e| e.to_string())
                            }
                            Some(repo) => repo.clear_cache().await.map_err(|e| e.to_string()),
                            None => Err("Armazenamento indisponível".into()),
                        };
                        let _ = data_tx.send(DataEvent::Cleared(result, history));
                        wake();
                    }
                    Intent::ReadFile(path) => {
                        let result = tokio::task::spawn_blocking(move || {
                            std::fs::read_to_string(&path)
                                .map_err(|e| format!("Não foi possível ler o arquivo: {e}"))
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("Falha ao ler arquivo: {e}")));
                        let _ = files_tx.send(result);
                        wake();
                    }
                    Intent::Close { id } => {
                        let result = sessions.close(&id, "window").await;
                        if active.as_deref() == Some(&id) {
                            active = None;
                        }
                        publish_lifecycle(
                            &lifecycle_tx,
                            &wake,
                            match result {
                                Ok(()) => BridgeLifecycle::Closed { id },
                                Err(error) => BridgeLifecycle::Error { id, error },
                            },
                        );
                    }
                }
            }
            sessions.shutdown().await;
        });
        Ok(Self {
            runtime,
            import_actor: Some(import_actor),
            import_intents: Some(import_intents),
            import_events,
            actor: Some(actor),
            intents: Some(intents),
            generation,
            events,
            lifecycle,
            files,
            data,
        })
    }

    fn send(&self, intent: Intent) -> Result<(), String> {
        self.intents
            .as_ref()
            .ok_or("bridge encerrada")?
            .send(intent)
            .map_err(|_| "bridge encerrada".into())
    }
    pub fn open(&self, id: &str, config: ReviewConfig) -> Result<(), String> {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.send(Intent::Open {
            id: id.into(),
            config,
            generation,
        })
    }
    pub fn analyze(
        &self,
        id: &str,
        request_id: u64,
        request: LiveRequest,
        settings: LiveSettings,
    ) -> Result<(), String> {
        self.send(Intent::Analyze {
            id: id.into(),
            request_id,
            request,
            settings,
        })
    }
    pub fn cancel_live(&self, id: String, request_id: u64) {
        let _ = self.send(Intent::Cancel { id, request_id });
    }
    pub fn close(&self, id: String) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let _ = self.send(Intent::Close { id });
    }
    pub fn search_chesscom(
        &self,
        request: engineroom_backend::imports::SearchRequest,
    ) -> Result<(), String> {
        self.import_intents
            .as_ref()
            .ok_or("bridge encerrada")?
            .send(ImportIntent::Search(request))
            .map_err(|_| "bridge encerrada".into())
    }
    pub fn cancel_chesscom(&self, id: String) {
        if let Some(sender) = &self.import_intents {
            let _ = sender.send(ImportIntent::Cancel(id));
        }
    }
    pub fn drain_imports(&self) -> Vec<engineroom_backend::imports::Event> {
        self.import_events.try_iter().collect()
    }
    pub fn list_games(
        &self,
        cursor: Option<engineroom_backend::db::games::GameCursor>,
    ) -> Result<(), String> {
        self.send(Intent::ListGames(cursor))
    }
    pub fn storage_stats(&self) -> Result<(), String> {
        self.send(Intent::StorageStats)
    }
    pub fn clear_cache(&self) -> Result<(), String> {
        self.send(Intent::ClearCache)
    }
    pub fn clear_history(&self) -> Result<(), String> {
        self.send(Intent::ClearHistory)
    }
    pub fn load_game(&self, id: i64, reanalyze: bool, request_id: u64) -> Result<(), String> {
        self.send(Intent::LoadGame(id, reanalyze, request_id))
    }
    pub fn delete_game(&self, id: i64) -> Result<(), String> {
        self.send(Intent::DeleteGame(id))
    }
    pub fn drain_data(&self) -> Vec<DataEvent> {
        self.data.try_iter().collect()
    }
    pub fn read_file(&self, path: PathBuf) -> Result<(), String> {
        self.send(Intent::ReadFile(path))
    }
    pub fn drain_files(&self) -> Vec<Result<String, String>> {
        self.files.try_iter().collect()
    }
    pub fn drain(&self) -> Vec<Envelope> {
        self.events
            .try_iter()
            .filter_map(|(generation, event)| {
                (generation == self.generation.load(Ordering::SeqCst)).then_some(event)
            })
            .collect()
    }
    pub fn drain_lifecycle(&self) -> Vec<BridgeLifecycle> {
        self.lifecycle.try_iter().collect()
    }
}
/// Resolves Stockfish without making a missing executable fatal for the bridge.
fn engine_path() -> PathBuf {
    crate::desktop_paths::stockfish_path().unwrap_or_else(|_| {
        engine_fallback(std::env::var_os("ENGINE_ROOM_STOCKFISH").map(PathBuf::from))
    })
}

/// Unvalidated engine candidate used when [`crate::desktop_paths::stockfish_path`] fails.
fn engine_fallback(override_path: Option<PathBuf>) -> PathBuf {
    override_path
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/stockfish"))
}

fn publish_lifecycle(
    sender: &mpsc::Sender<BridgeLifecycle>,
    wake: &Arc<dyn Fn() + Send + Sync>,
    event: BridgeLifecycle,
) {
    let _ = sender.send(event);
    wake();
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.import_intents.take();
        if let Some(actor) = self.import_actor.take() {
            let _ = self.runtime.block_on(actor);
        }
        self.intents.take();
        if let Some(actor) = self.actor.take() {
            let _ = self.runtime.block_on(actor);
        }
    }
}

#[cfg(test)]
mod tests;
