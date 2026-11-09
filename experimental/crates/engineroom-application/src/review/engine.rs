//! Rust owns the entire UCI exchange and exclusive process lease.
use super::types::*;
use std::{collections::BTreeMap, future::Future, pin::Pin, time::Duration};
use tokio::sync::watch;
pub type Task<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone)]
pub struct Cancellation {
    pub closed: watch::Receiver<bool>,
    pub revision: Option<(watch::Receiver<u64>, u64)>,
}
impl Cancellation {
    pub fn check(&self) -> Result<()> {
        if *self.closed.borrow()
            || self
                .revision
                .as_ref()
                .is_some_and(|(r, id)| *r.borrow() != *id)
        {
            Err(ReviewError::cancelled())
        } else {
            Ok(())
        }
    }
    pub async fn cancelled(&self) {
        let mut closed = self.closed.clone();
        let mut revision = self.revision.clone();
        if self.check().is_err() {
            return;
        }
        tokio::select! {
            _ = async { if closed.wait_for(|v| *v).await.is_err() { std::future::pending::<()>().await; } } => {},
            _ = async {
                if let Some((rx, id)) = &mut revision { if rx.wait_for(|v| *v != *id).await.is_err() { std::future::pending::<()>().await; } }
                else { std::future::pending::<()>().await; }
            } => {},
        }
    }
}
/// Owns one process until explicitly awaited shutdown acknowledges its termination.
///
/// Transport failures leave teardown with the owner; dropping a future is not cleanup.
pub trait EnginePort: Send {
    /// Sends a UCI command without waiting for a response.
    ///
    /// # Errors
    /// Returns a transport error if the command cannot be delivered.
    fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, Result<()>>;
    /// Receives the next complete stdout line.
    ///
    /// # Errors
    /// Returns a transport error on unexpected process exit or receive failure.
    fn next(&mut self) -> Task<'_, Result<String>>;
    /// Kills and awaits actual process termination before releasing its lease share.
    /// Await this future to completion; Drop is only best-effort cleanup.
    fn shutdown(&mut self) -> Task<'_, ()>;
}
/// Acquires exclusive engine ownership shared with review sessions and Settings probes.
///
/// A group retains its exclusive lease until every process finishes teardown.
pub trait EngineFactory: Send + Sync {
    /// Acquires one process, transferring teardown responsibility to the caller.
    ///
    /// # Errors
    /// Returns cancellation or acquisition/spawn errors. Partial acquisition must be
    /// fully torn down by the implementation before returning an error.
    fn acquire<'a>(&'a self, cancel: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>>;
    /// One owner leases the whole group. Adapters without pool support stay serial.
    ///
    /// # Errors
    /// Returns cancellation or acquisition/spawn errors after cleaning up any partial group.
    fn acquire_pool<'a>(
        &'a self,
        _count: usize,
        cancel: &'a Cancellation,
    ) -> Task<'a, Result<Vec<Box<dyn EnginePort>>>> {
        Box::pin(async move { Ok(vec![self.acquire(cancel).await?]) })
    }
}
pub async fn ask(
    port: &mut dyn EnginePort,
    command: &str,
    done: &str,
    timeout: u64,
    cancel: &Cancellation,
) -> Result<Vec<String>> {
    cancel.check()?;
    let read = async {
        port.send(command).await?;
        let mut lines = Vec::new();
        loop {
            let line = port.next().await?;
            if line.trim() == done {
                return Ok(lines);
            }
            lines.push(line);
        }
    };
    tokio::select! {
        _ = cancel.cancelled() => Err(ReviewError::cancelled()),
        result = tokio::time::timeout(Duration::from_millis(timeout), read) => result.unwrap_or_else(|_| Err(ReviewError::new(ReviewErrorCode::EngineTimeout, command, format!("A engine não respondeu a '{command}' em {timeout}ms.")))),
    }
}
pub async fn send(port: &mut dyn EnginePort, command: &str, cancel: &Cancellation) -> Result<()> {
    cancel.check()?;
    tokio::select! {
        _ = cancel.cancelled() => Err(ReviewError::cancelled()),
        result = tokio::time::timeout(Duration::from_secs(10), port.send(command)) => result.unwrap_or_else(|_| Err(ReviewError::new(ReviewErrorCode::EngineTimeout, "engine.send", format!("A engine não aceitou '{command}'.")))),
    }
}
pub async fn configure(
    port: &mut dyn EnginePort,
    threads: Option<u32>,
    memory: Option<u32>,
    multipv: u32,
    cancel: &Cancellation,
) -> Result<()> {
    ask(port, "uci", "uciok", 10000, cancel).await?;
    ask(port, "isready", "readyok", 10000, cancel).await?;
    if let Some(n) = threads.filter(|n| *n > 1) {
        send(port, &format!("setoption name Threads value {n}"), cancel).await?;
    }
    if let Some(n) = memory {
        send(port, &format!("setoption name Hash value {n}"), cancel).await?;
    }
    send(
        port,
        &format!("setoption name MultiPV value {multipv}"),
        cancel,
    )
    .await?;
    ask(port, "isready", "readyok", 10000, cancel).await?;
    Ok(())
}
pub fn parse_info(line: &str) -> Option<RawLine> {
    let mut words = line.split_whitespace();
    if words.next()? != "info" {
        return None;
    }
    let mut depth = 0;
    let mut multipv = 1;
    let mut cp = None;
    let mut pv = Vec::new();
    while let Some(token) = words.next() {
        match token {
            "depth" => depth = words.next()?.parse().ok()?,
            "multipv" => {
                multipv = words.next()?.parse().ok()?;
                if multipv == 0 {
                    return None;
                }
            }
            "score" => {
                let kind = words.next()?;
                let n: i32 = words.next()?.parse().ok()?;
                cp = Some(match kind {
                    "cp" => n,
                    "mate" => {
                        // Mate distances must fit the reserved score encoding.
                        let distance = n.checked_abs()?;
                        if distance > 100000 {
                            return None;
                        }
                        (if n > 0 { 1 } else { -1 }) * (100000 - distance)
                    }
                    _ => return None,
                });
            }
            "pv" => {
                pv.extend(words.by_ref().map(str::to_owned));
                break;
            }
            // Bounds from an unfinished aspiration window are not exact
            // evaluations and must not overwrite a completed score.
            "lowerbound" | "upperbound" => return None,
            "string" => break,
            _ => {}
        }
    }
    Some(RawLine {
        unstable: false,
        depth: Some(depth),
        multipv,
        cp: cp?,
        pv,
        san: None,
    })
}
pub async fn evaluate(
    port: &mut dyn EnginePort,
    fen: &str,
    mode: crate::db::mode::Mode,
    value: u32,
    timeout: u64,
    cancel: &Cancellation,
) -> Result<RawPosition> {
    cancel.check()?;
    let go = format!(
        "go {} {value}",
        if mode == crate::db::mode::Mode::Depth {
            "depth"
        } else {
            "movetime"
        }
    );
    let read = async {
        port.send(&format!("position fen {fen}")).await?;
        port.send(&go).await?;
        let mut latest: BTreeMap<u32, RawLine> = BTreeMap::new();
        loop {
            let line = port.next().await?;
            if let Some(mut info) = parse_info(&line)
                && latest
                    .get(&info.multipv)
                    .is_none_or(|p| info.depth >= p.depth)
            {
                if let Some(previous) = latest.get(&info.multipv) {
                    info.unstable = if info.depth > previous.depth {
                        (super::scoring::win_pct(info.cp) - super::scoring::win_pct(previous.cp))
                            .abs()
                            >= 2.0
                            || info.pv.first() != previous.pv.first()
                    } else {
                        previous.unstable
                    };
                }
                latest.insert(info.multipv, info);
            }
            if line.trim().starts_with("bestmove") {
                break;
            }
        }
        let depth = latest.get(&1).and_then(|l| l.depth).unwrap_or(0);
        let lines: Vec<_> = latest.into_values().collect();
        let principal = lines.iter().find(|l| l.multipv == 1).ok_or_else(|| {
            ReviewError::new(
                ReviewErrorCode::MissingEvaluation,
                "engine.evaluate",
                "A engine encerrou a busca sem avaliação da posição.",
            )
        })?;
        Ok(RawPosition {
            fen: fen.into(),
            cp: principal.cp,
            depth,
            pv: principal.pv.clone(),
            lines,
        })
    };
    tokio::select! {
        _ = cancel.cancelled() => Err(ReviewError::cancelled()),
        result = tokio::time::timeout(Duration::from_millis(timeout), read) => result.unwrap_or_else(|_| Err(ReviewError::new(ReviewErrorCode::EngineTimeout, "engine.evaluate", format!("A engine não respondeu a '{go}' em {timeout}ms.")))),
    }
}
