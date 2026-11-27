//! UCI diagnostic using the same exclusive engine factory as review sessions.
use engineroom_backend::review::engine::{self, Cancellation, EngineFactory};

/// Observable diagnostic state for the settings screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Idle,
    Running,
    Success(Option<String>),
    Error(String),
}

/// Acquires the review lease, checks UCI readiness, and waits for process teardown.
pub async fn run(
    factory: &dyn EngineFactory,
    cancel: &Cancellation,
) -> Result<Option<String>, String> {
    let mut port = factory
        .acquire(cancel)
        .await
        .map_err(|error| error.to_string())?;
    let result = async {
        let lines = engine::ask(port.as_mut(), "uci", "uciok", 10_000, cancel).await?;
        engine::ask(port.as_mut(), "isready", "readyok", 10_000, cancel).await?;
        Ok::<_, engineroom_backend::review::types::ReviewError>(
            lines
                .iter()
                .find_map(|line| line.strip_prefix("id name ").map(str::to_owned)),
        )
    }
    .await;
    port.shutdown().await;
    result.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use engineroom_backend::review::{
        engine::{EnginePort, Task},
        types::{Result as ReviewResult, ReviewError, ReviewErrorCode},
    };
    use std::sync::{Arc, Mutex};

    struct Factory(Arc<Mutex<Vec<String>>>, bool);
    struct Port(Arc<Mutex<Vec<String>>>, bool);
    impl EngineFactory for Factory {
        fn acquire<'a>(
            &'a self,
            _: &'a Cancellation,
        ) -> Task<'a, ReviewResult<Box<dyn EnginePort>>> {
            Box::pin(async { Ok(Box::new(Port(self.0.clone(), self.1)) as Box<dyn EnginePort>) })
        }
    }
    impl EnginePort for Port {
        fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, ReviewResult<()>> {
            self.0.lock().expect("probe test lock").push(command.into());
            Box::pin(async { Ok(()) })
        }
        fn next(&mut self) -> Task<'_, ReviewResult<String>> {
            let last = self.0.lock().expect("probe test lock").last().cloned();
            let failed = self.1;
            Box::pin(async move {
                if failed {
                    return Err(ReviewError::new(
                        ReviewErrorCode::EngineSpawn,
                        "probe",
                        "failed",
                    ));
                }
                Ok(if last.as_deref() == Some("uci") {
                    "uciok"
                } else {
                    "readyok"
                }
                .into())
            })
        }
        fn shutdown(&mut self) -> Task<'_, ()> {
            self.0
                .lock()
                .expect("probe test lock")
                .push("shutdown".into());
            Box::pin(async {})
        }
    }
    #[test]
    fn handshake_and_failure_both_finish_teardown() {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        let (_tx, closed) = tokio::sync::watch::channel(false);
        let cancel = Cancellation {
            closed,
            revision: None,
        };
        for failed in [false, true] {
            let commands = Arc::new(Mutex::new(Vec::new()));
            let result = runtime.block_on(run(&Factory(commands.clone(), failed), &cancel));
            assert_eq!(result.is_err(), failed);
            let commands = commands.lock().expect("probe test lock");
            assert_eq!(commands.last().map(String::as_str), Some("shutdown"));
            if !failed {
                assert_eq!(&commands[..], &["uci", "isready", "shutdown"]);
            }
        }
    }
}
