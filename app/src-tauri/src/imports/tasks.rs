//! Window-owned import tasks, independent of engine leases.
use super::chess_com::*;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    sync::{watch, Semaphore},
    time::Instant,
};

struct Job {
    owner: String,
    cancel: watch::Sender<bool>,
    done: watch::Receiver<bool>,
}
type Jobs = Arc<Mutex<HashMap<String, Arc<Job>>>>;
pub struct ImportTasks {
    jobs: Jobs,
    http: Arc<dyn Http>,
    gate: Arc<Semaphore>,
    exiting: AtomicBool,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    request_id: String,
    #[serde(flatten)]
    body: EventBody,
}
#[derive(serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum EventBody {
    Completed { result: SearchResult },
    Error { error: ImportError },
}

impl ImportTasks {
    pub fn new(http: Arc<dyn Http>) -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            http,
            gate: Arc::new(Semaphore::new(1)),
            exiting: AtomicBool::new(false),
        }
    }
    pub fn start(
        &self,
        owner: &str,
        id: &str,
        input: String,
        cursor: Option<Cursor>,
        publish: impl FnOnce(Event) + Send + 'static,
    ) -> Result<()> {
        let name = username(&input)?;
        if id.is_empty() || id.len() > 128 {
            return Err(ImportError::new(
                ErrorCode::InvalidInput,
                "Identificador da busca inválido.",
            ));
        }
        let (cancel, mut cancelled) = watch::channel(false);
        let (finished, done) = watch::channel(false);
        let job = Arc::new(Job {
            owner: owner.into(),
            cancel,
            done,
        });
        {
            let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            if self.exiting.load(Ordering::SeqCst)
                || jobs.contains_key(id)
                || jobs.len() >= 8
                || jobs.values().any(|job| job.owner == owner)
            {
                return Err(ImportError::new(
                    ErrorCode::InvalidInput,
                    "Aguarde o encerramento da busca anterior.",
                ));
            }
            jobs.insert(id.into(), job.clone());
        }
        let jobs = self.jobs.clone();
        let http = self.http.clone();
        let gate = self.gate.clone();
        let id = id.to_owned();
        tauri::async_runtime::spawn(async move {
            let limits = Limits::default();
            let deadline = Instant::now() + limits.total_time;
            let result = async {
                let _permit = tokio::select! {
                    biased;
                    _ = cancelled.wait_for(|v| *v) => return Err(ImportError::new(ErrorCode::Cancelled, "Busca cancelada.")),
                    result = tokio::time::timeout_at(deadline, gate.acquire()) => result.map_err(|_| ImportError::new(ErrorCode::Timeout, "A busca demorou demais."))?.map_err(|_| ImportError::new(ErrorCode::Cancelled, "Busca cancelada."))?,
                };
                search(http.as_ref(), &name, cursor, Limits { total_time: deadline.saturating_duration_since(Instant::now()), ..limits }, &mut cancelled).await
            }.await;
            if !*cancelled.borrow() {
                publish(Event {
                    request_id: id.clone(),
                    body: match result {
                        Ok(result) => EventBody::Completed { result },
                        Err(error) => EventBody::Error { error },
                    },
                });
            }
            // The gate and every awaited parser are released before acknowledging close.
            {
                let mut jobs = jobs.lock().unwrap_or_else(|e| e.into_inner());
                if jobs
                    .get(&id)
                    .is_some_and(|current| Arc::ptr_eq(current, &job))
                {
                    jobs.remove(&id);
                }
            }
            finished.send_replace(true);
        });
        Ok(())
    }
    pub async fn cancel(&self, owner: &str, id: &str) -> Result<()> {
        let job = self
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned();
        let Some(job) = job else {
            return Ok(());
        };
        if job.owner != owner {
            return Err(ImportError::new(
                ErrorCode::InvalidInput,
                "A busca pertence a outra janela.",
            ));
        }
        job.cancel.send_replace(true);
        let mut done = job.done.clone();
        let _ = done.wait_for(|v| *v).await;
        Ok(())
    }
    pub fn close_window(&self, owner: &str) {
        for job in self
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .filter(|job| job.owner == owner)
        {
            job.cancel.send_replace(true);
        }
    }
    pub fn begin_shutdown(&self) -> Option<Task<'static, ()>> {
        if self.exiting.swap(true, Ordering::SeqCst) {
            return None;
        }
        let jobs: Vec<_> = self
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        for job in &jobs {
            job.cancel.send_replace(true);
        }
        Some(Box::pin(async move {
            for job in jobs {
                let mut done = job.done.clone();
                let _ = done.wait_for(|v| *v).await;
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imports::tests::PendingHttp;

    #[tokio::test]
    async fn cancellation_is_owned_idempotent_and_acknowledged_after_removal() {
        let state = ImportTasks::new(Arc::new(PendingHttp));
        state
            .start("window-a", "request", "test-player".into(), None, |_| {})
            .unwrap();
        assert!(state.cancel("window-b", "request").await.is_err());
        assert_eq!(state.jobs.lock().unwrap().len(), 1);
        state.cancel("window-a", "request").await.unwrap();
        assert!(state.jobs.lock().unwrap().is_empty());
        state.cancel("window-a", "request").await.unwrap();
        state
            .start("window-a", "request", "test-player".into(), None, |_| {})
            .unwrap();
        state.cancel("window-a", "request").await.unwrap();
    }

    #[tokio::test]
    async fn window_close_cancels_only_its_tasks_and_shutdown_drains_everything() {
        let state = ImportTasks::new(Arc::new(PendingHttp));
        state
            .start("a", "first", "test-player".into(), None, |_| {})
            .unwrap();
        state
            .start("b", "second", "test-player".into(), None, |_| {})
            .unwrap();
        state.close_window("a");
        state.cancel("a", "first").await.unwrap();
        assert_eq!(state.jobs.lock().unwrap().len(), 1);
        state.begin_shutdown().unwrap().await;
        assert!(state.jobs.lock().unwrap().is_empty());
        assert!(state
            .start("a", "third", "test-player".into(), None, |_| {})
            .is_err());
        assert!(state.begin_shutdown().is_none());
    }
}
