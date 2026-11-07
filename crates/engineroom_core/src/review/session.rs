use super::{
    engine::Cancellation,
    pipeline::{hash_mb, Pipeline},
    repository::normalize,
    types::*,
};
use std::sync::Mutex;
use tokio::sync::watch;
mod task;
mod validation;
pub use task::run_session;
pub use validation::{validate, validate_live};
#[derive(Clone)]
pub struct Job {
    pub id: u64,
    pub request: LiveRequest,
    pub settings: LiveSettings,
}
pub struct Session {
    pub owner: String,
    pub close: watch::Sender<bool>,
    pub revision: watch::Sender<u64>,
    pub latest: Mutex<Option<Job>>,
    pub finished: watch::Receiver<u64>,
    pub done: watch::Receiver<bool>,
}
pub fn update(s: &Session, id: u64, job: Option<Job>) -> Result<()> {
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
