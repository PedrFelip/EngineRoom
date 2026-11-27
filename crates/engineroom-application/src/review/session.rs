//! Window-owned sessions. IPC records intent; the task serializes all engine work.
use super::{
    engine::EngineFactory,
    pipeline::Pipeline,
    repository::Repository,
    types::{Envelope, ReviewConfig},
    types::{LiveRequest, LiveSettings, Result, ReviewError, ReviewErrorCode},
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::watch;

mod task;
mod validation;

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
impl Session {
    async fn cancel_live(&self, request_id: u64) -> Result<()> {
        update(self, request_id, None)?;
        let mut finished = self.finished.clone();
        let mut done = self.done.clone();
        tokio::select! {
            _ = finished.wait_for(|id| *id >= request_id) => {},
            _ = done.wait_for(|closed| *closed) => {},
        }
        Ok(())
    }

    async fn close(&self) {
        self.close.send_replace(true);
        let mut done = self.done.clone();
        let _ = done.wait_for(|closed| *closed).await;
    }
}

pub struct ReviewSessions {
    sessions: Arc<Mutex<HashMap<String, Arc<Session>>>>,
    factory: Arc<dyn EngineFactory>,
    repository: Arc<dyn Repository>,
    resources: Option<Resources>,
    exiting: AtomicBool,
}

/// Optional host resource limits.
#[derive(Debug, Clone, Copy)]
pub struct Resources {
    /// Available CPU threads.
    pub threads: u32,
    /// Available memory in MiB.
    pub memory_mb: u32,
}
impl ReviewSessions {
    /// Creates a registry with shared engine and persistence ports.
    pub fn new(
        factory: Arc<dyn EngineFactory>,
        repository: Arc<dyn Repository>,
        resources: Option<Resources>,
    ) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            factory,
            repository,
            resources,
            exiting: AtomicBool::new(false),
        }
    }
    /// Starts a session and delivers ordered events through the callback.
    pub fn open(
        &self,
        session_id: &str,
        owner: &str,
        config: ReviewConfig,
        mut publish: impl FnMut(Envelope) -> bool + Send + 'static,
    ) -> Result<()> {
        validation::validate(&config)?;
        let (session, finished, done) = self.register(session_id, owner)?;
        let sessions = self.sessions.clone();
        let id = session_id.to_owned();
        let pipeline = Pipeline::new(self.factory.clone(), self.repository.clone());
        let resources = self.resources;
        tokio::spawn(async move {
            task::run_session(
                &id,
                &config,
                &session,
                task::Completion { finished, done },
                pipeline,
                resources,
                &mut publish,
            )
            .await;
            remove(&sessions, &id, &session);
        });
        Ok(())
    }
    /// Enqueues a live analysis intent; later request IDs supersede earlier ones.
    pub fn analyze_position(
        &self,
        session_id: &str,
        owner: &str,
        request_id: u64,
        request: LiveRequest,
        settings: LiveSettings,
    ) -> Result<()> {
        validation::validate_live(&settings)?;
        update(
            self.get(session_id, owner)?.as_ref(),
            request_id,
            Some(Job {
                id: request_id,
                request,
                settings,
            }),
        )
    }
    /// Cancels older live work and waits for cleanup.
    pub async fn cancel_live(&self, session_id: &str, owner: &str, request_id: u64) -> Result<()> {
        self.get(session_id, owner)?.cancel_live(request_id).await
    }
    /// Closes a session and waits for cleanup; repeated closure succeeds.
    pub async fn close(&self, session_id: &str, owner: &str) -> Result<()> {
        match self.get(session_id, owner) {
            Ok(session) => session.close().await,
            Err(error) if error.code == ReviewErrorCode::SessionClosed => (),
            Err(error) => return Err(error),
        }
        Ok(())
    }
    fn get(&self, id: &str, owner: &str) -> Result<Arc<Session>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|e| ReviewError::new(ReviewErrorCode::Session, "session.lookup", e))?;
        let s = sessions
            .get(id)
            .filter(|s| s.owner == owner)
            .ok_or_else(|| {
                ReviewError::new(
                    ReviewErrorCode::SessionClosed,
                    "session.lookup",
                    "Sessão encerrada.",
                )
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
                ReviewErrorCode::SessionClosed,
                "session.open",
                "O aplicativo está encerrando.",
            ));
        }
        if id.is_empty() || id.len() > 128 {
            return Err(ReviewError::new(
                ReviewErrorCode::InvalidPayload,
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
            .map_err(|e| ReviewError::new(ReviewErrorCode::Session, "session.open", e))?;
        if self.exiting.load(Ordering::SeqCst) {
            return Err(ReviewError::new(
                ReviewErrorCode::SessionClosed,
                "session.open",
                "O aplicativo está encerrando.",
            ));
        }
        if sessions.contains_key(id) {
            return Err(ReviewError::new(
                ReviewErrorCode::SessionExists,
                "session.open",
                "Sessão já aberta.",
            ));
        }
        sessions.insert(id.into(), s.clone());
        Ok((s, finished_tx, done_tx))
    }
    /// Signals all sessions belonging to an owner.
    pub fn close_owner(&self, owner: &str) {
        if let Ok(sessions) = self.sessions.lock() {
            for s in sessions.values().filter(|s| s.owner == owner) {
                s.close.send_replace(true);
            }
        }
    }
    /// Prevents new sessions and waits for existing sessions to close.
    pub async fn shutdown(&self) {
        self.exiting.store(true, Ordering::SeqCst);
        let sessions: Vec<_> = self
            .sessions
            .lock()
            .expect("session registry lock poisoned")
            .values()
            .cloned()
            .collect();
        for session in &sessions {
            session.close.send_replace(true);
        }
        for session in sessions {
            session.close().await;
        }
    }
}
fn remove(sessions: &Arc<Mutex<HashMap<String, Arc<Session>>>>, id: &str, s: &Arc<Session>) {
    if let Ok(mut sessions) = sessions.lock()
        && sessions
            .get(id)
            .is_some_and(|current| Arc::ptr_eq(current, s))
    {
        sessions.remove(id);
    }
}
fn update(s: &Session, id: u64, job: Option<Job>) -> Result<()> {
    if id == 0 || id > 9_007_199_254_740_991 {
        return Err(ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "session.intent",
            "Identificador do pedido inválido.",
        ));
    }
    let mut latest = s
        .latest
        .lock()
        .map_err(|e| ReviewError::new(ReviewErrorCode::Session, "session.intent", e))?;
    if *s.close.borrow() {
        return Err(ReviewError::new(
            ReviewErrorCode::SessionClosed,
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

#[cfg(test)]
mod tests;
