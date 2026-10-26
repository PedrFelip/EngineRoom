//! Exclusive engine ownership, configuration reuse, and acknowledged teardown.
use crate::review::{
    engine::{self, Cancellation, EngineFactory, EnginePort},
    types::{Result, ReviewError, ReviewErrorCode},
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EngineSettings {
    pub threads: u32,
    pub memory_mb: u32,
    pub multipv: u32,
}

pub(super) struct OwnedEngine {
    factory: Arc<dyn EngineFactory>,
    port: Option<Box<dyn EnginePort>>,
    settings: Option<EngineSettings>,
}

impl OwnedEngine {
    pub(super) fn new(factory: Arc<dyn EngineFactory>) -> Self {
        Self {
            factory,
            port: None,
            settings: None,
        }
    }

    pub(super) fn factory(&self) -> &dyn EngineFactory {
        self.factory.as_ref()
    }

    pub(super) async fn discard(&mut self) {
        if let Some(mut port) = self.port.take() {
            port.shutdown().await;
        }
        self.settings = None;
    }

    pub(super) async fn ensure(
        &mut self,
        next: EngineSettings,
        cancel: &Cancellation,
    ) -> Result<&mut (dyn EnginePort + 'static)> {
        if let Some(port) = self.port.as_mut() {
            let previous = self.settings;
            if previous != Some(next) {
                if previous.map(|s| s.threads) != Some(next.threads) {
                    port.send(&format!("setoption name Threads value {}", next.threads))?;
                }
                if previous.map(|s| s.memory_mb) != Some(next.memory_mb) {
                    port.send(&format!("setoption name Hash value {}", next.memory_mb))?;
                }
                if previous.map(|s| s.multipv) != Some(next.multipv) {
                    port.send(&format!("setoption name MultiPV value {}", next.multipv))?;
                }
                engine::ask(port.as_mut(), "isready", "readyok", 10000, cancel).await?;
            }
        } else {
            // Retain ownership before setup so a failed handshake is still
            // closed by the caller's acknowledged teardown.
            let port = self.port.insert(self.factory.acquire(cancel).await?);
            engine::configure(
                port.as_mut(),
                Some(next.threads),
                Some(next.memory_mb),
                next.multipv,
                cancel,
            )
            .await?;
        }
        self.settings = Some(next);
        match self.port.as_mut() {
            Some(port) => Ok(port.as_mut()),
            None => Err(ReviewError::new(
                ReviewErrorCode::EngineExited,
                "engine.ensure",
                "Engine indisponível.",
            )),
        }
    }
}
