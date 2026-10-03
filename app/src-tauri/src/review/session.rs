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

mod task;
mod validation;

use task::run_session;
use validation::{validate, validate_live};

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
        Err(e) if e.code == ReviewErrorCode::SessionClosed => return Ok(()),
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
mod tests;
