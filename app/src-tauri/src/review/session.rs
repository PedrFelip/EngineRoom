//! Window-owned sessions. IPC records intent; the task serializes all engine work.
use super::{
    engine,
    types::{LiveRequest, LiveSettings, Result, ReviewError, ReviewErrorCode},
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::{watch, Semaphore};

mod ipc;
mod task;
mod validation;

pub use ipc::{
    __cmd__engine_probe, __cmd__review_session_analyze_position, __cmd__review_session_cancel_live,
    __cmd__review_session_close, __cmd__review_session_open, __tauri_command_name_engine_probe,
    __tauri_command_name_review_session_analyze_position,
    __tauri_command_name_review_session_cancel_live, __tauri_command_name_review_session_close,
    __tauri_command_name_review_session_open, engine_probe, review_session_analyze_position,
    review_session_cancel_live, review_session_close, review_session_open,
};

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
    permit: Arc<Semaphore>,
    exiting: AtomicBool,
    shutdown_complete: Arc<AtomicBool>,
}

/// The action required for an application exit request.
pub enum ShutdownAction {
    /// Prevent exit and run this cleanup task once.
    Cleanup(engine::Task<'static, ()>),
    /// Prevent repeated exit requests until cleanup finishes.
    Wait,
    /// Allow exit after all sessions acknowledge cleanup.
    Exit,
}
impl Default for ReviewSessions {
    fn default() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            permit: Arc::new(Semaphore::new(1)),
            exiting: AtomicBool::new(false),
            shutdown_complete: Arc::new(AtomicBool::new(false)),
        }
    }
}
impl ReviewSessions {
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
    pub fn close_window(&self, owner: &str) {
        if let Ok(sessions) = self.sessions.lock() {
            for s in sessions.values().filter(|s| s.owner == owner) {
                s.close.send_replace(true);
            }
        }
    }
    pub fn begin_shutdown(&self) -> ShutdownAction {
        if self.exiting.swap(true, Ordering::SeqCst) {
            return if self.shutdown_complete.load(Ordering::SeqCst) {
                ShutdownAction::Exit
            } else {
                ShutdownAction::Wait
            };
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
        let shutdown_complete = self.shutdown_complete.clone();
        ShutdownAction::Cleanup(Box::pin(async move {
            for session in sessions {
                let mut done = session.done.clone();
                let _ = done.wait_for(|v| *v).await;
            }
            shutdown_complete.store(true, Ordering::SeqCst);
        }))
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
