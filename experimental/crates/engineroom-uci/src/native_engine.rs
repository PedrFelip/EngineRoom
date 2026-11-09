//! Native UCI processes with an exclusive lease shared by reviews and probes.
use engineroom_application::review::{
    engine::{Cancellation, EngineFactory, EnginePort, Task},
    types::{Result, ReviewError, ReviewErrorCode},
};
use std::{path::PathBuf, process::Stdio, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{OwnedSemaphorePermit, Semaphore},
};

/// Factory bound to a shell-resolved executable path and one shared lease.
#[derive(Clone)]
pub struct NativeEngineFactory {
    path: PathBuf,
    permit: Arc<Semaphore>,
}
impl NativeEngineFactory {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            permit: Arc::new(Semaphore::new(1)),
        }
    }
}
struct NativeEngine {
    child: Option<Child>,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    permit: Option<Arc<OwnedSemaphorePermit>>,
}
impl EngineFactory for NativeEngineFactory {
    fn acquire<'a>(&'a self, cancel: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        Box::pin(async move {
            self.acquire_pool(1, cancel).await?.pop().ok_or_else(|| {
                ReviewError::new(
                    ReviewErrorCode::EngineSpawn,
                    "engine.acquire",
                    "Pool vazio.",
                )
            })
        })
    }
    fn acquire_pool<'a>(
        &'a self,
        count: usize,
        cancel: &'a Cancellation,
    ) -> Task<'a, Result<Vec<Box<dyn EnginePort>>>> {
        Box::pin(async move {
            cancel.check()?;
            let permit = tokio::select! { _ = cancel.cancelled() => return Err(ReviewError::cancelled()), p = self.permit.clone().acquire_owned() => p.map_err(|e| ReviewError::new(ReviewErrorCode::EngineSpawn, "engine.acquire", e))? };
            let permit = Arc::new(permit);
            let mut ports: Vec<Box<dyn EnginePort>> = Vec::new();
            for _ in 0..count.max(1) {
                let spawned = cancel
                    .check()
                    .and_then(|()| spawn(&self.path, permit.clone()));
                match spawned {
                    Ok(port) => ports.push(Box::new(port)),
                    Err(error) => {
                        for port in &mut ports {
                            port.shutdown().await;
                        }
                        return Err(error);
                    }
                }
            }
            if let Err(error) = cancel.check() {
                for port in &mut ports {
                    port.shutdown().await;
                }
                return Err(error);
            }
            Ok(ports)
        })
    }
}
fn spawn(path: &PathBuf, permit: Arc<OwnedSemaphorePermit>) -> Result<NativeEngine> {
    let mut child = Command::new(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ReviewError::new(
                    ReviewErrorCode::EngineSpawn,
                    "engine.spawn",
                    format!(
                        "Stockfish não encontrado em '{}'. Defina ENGINE_ROOM_STOCKFISH ou coloque o binário em experimental/resources/stockfish.",
                        path.display()
                    ),
                )
            } else {
                ReviewError::new(ReviewErrorCode::EngineSpawn, "engine.spawn", e)
            }
        })?;
    let stdin = child.stdin.take().ok_or_else(|| {
        ReviewError::new(
            ReviewErrorCode::EngineSpawn,
            "engine.spawn",
            "stdin unavailable",
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        ReviewError::new(
            ReviewErrorCode::EngineSpawn,
            "engine.spawn",
            "stdout unavailable",
        )
    })?;
    let lines = BufReader::new(stdout).lines();
    Ok(NativeEngine {
        child: Some(child),
        stdin,
        lines,
        permit: Some(permit),
    })
}
impl Drop for NativeEngine {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let permit = self.permit.take();
            tokio::spawn(async move {
                let _ = child.start_kill();
                let _ = child.wait().await;
                drop(permit);
            });
        }
    }
}
impl EnginePort for NativeEngine {
    fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, Result<()>> {
        Box::pin(async move {
            self.stdin
                .write_all(command.as_bytes())
                .await
                .map_err(|e| ReviewError::new(ReviewErrorCode::EngineCommand, "engine.send", e))?;
            self.stdin
                .write_all(b"\n")
                .await
                .map_err(|e| ReviewError::new(ReviewErrorCode::EngineCommand, "engine.send", e))
        })
    }
    fn next(&mut self) -> Task<'_, Result<String>> {
        Box::pin(async move {
            self.lines
                .next_line()
                .await
                .map_err(|e| ReviewError::new(ReviewErrorCode::EngineExited, "engine.read", e))?
                .ok_or_else(|| {
                    ReviewError::new(
                        ReviewErrorCode::EngineExited,
                        "engine.read",
                        "stdout da engine fechado.",
                    )
                })
        })
    }
    fn shutdown(&mut self) -> Task<'_, ()> {
        Box::pin(async move {
            if let Some(mut child) = self.child.take() {
                let permit = self.permit.take();
                let _ = tokio::spawn(async move {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    drop(permit);
                })
                .await;
            }
            self.permit.take();
        })
    }
}
