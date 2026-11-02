//! Rust owns the entire UCI exchange and exclusive process lease.
use super::types::*;
use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    pin::Pin,
    sync::Arc,
    time::Duration,
};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};
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
            _ = closed.wait_for(|v| *v) => {},
            _ = async {
                if let Some((rx, id)) = &mut revision { let _ = rx.wait_for(|v| *v != *id).await; }
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
    fn send(&mut self, command: &str) -> Result<()>;
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
pub struct SidecarFactory<R: tauri::Runtime = tauri::Wry> {
    pub app: tauri::AppHandle<R>,
    pub permit: Arc<Semaphore>,
}
struct Sidecar {
    child: Option<CommandChild>,
    rx: tauri::async_runtime::Receiver<CommandEvent>,
    permit: Option<Arc<OwnedSemaphorePermit>>,
    stdout: Vec<u8>,
    lines: VecDeque<String>,
    exited: bool,
}
impl<R: tauri::Runtime> EngineFactory for SidecarFactory<R> {
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
            let permit = tokio::select! {
                _ = cancel.cancelled() => return Err(ReviewError::cancelled()),
                permit = self.permit.clone().acquire_owned() => permit.map_err(|e| ReviewError::new(ReviewErrorCode::EngineSpawn, "engine.acquire", e))?,
            };
            let permit = Arc::new(permit);
            let mut ports: Vec<Box<dyn EnginePort>> = Vec::new();
            for _ in 0..count.max(1) {
                let spawned = cancel.check().and_then(|_| {
                    self.app
                        .shell()
                        .sidecar("stockfish")
                        .and_then(|c| c.set_raw_out(true).spawn())
                        .map_err(|e| {
                            ReviewError::new(
                                ReviewErrorCode::EngineSpawn,
                                "engine.spawn",
                                format!("Falha ao iniciar o Stockfish: {e}"),
                            )
                        })
                });
                match spawned {
                    Ok((rx, child)) => ports.push(Box::new(Sidecar {
                        child: Some(child),
                        rx,
                        permit: Some(permit.clone()),
                        stdout: vec![],
                        lines: VecDeque::new(),
                        exited: false,
                    })),
                    Err(error) => {
                        for port in &mut ports {
                            port.shutdown().await;
                        }
                        return Err(error);
                    }
                }
            }
            Ok(ports)
        })
    }
}
impl Drop for Sidecar {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            let _ = child.kill();
        }
    }
}
impl EnginePort for Sidecar {
    /// Sends a UCI command without waiting for a response.
    ///
    /// # Errors
    /// Returns a transport error if the command cannot be delivered.
    fn send(&mut self, command: &str) -> Result<()> {
        self.child
            .as_mut()
            .ok_or_else(|| {
                ReviewError::new(
                    ReviewErrorCode::EngineExited,
                    "engine.send",
                    "A engine encerrou.",
                )
            })?
            .write(format!("{command}\n").as_bytes())
            .map_err(|e| ReviewError::new(ReviewErrorCode::EngineCommand, "engine.send", e))
    }
    fn next(&mut self) -> Task<'_, Result<String>> {
        Box::pin(async move {
            loop {
                if let Some(line) = self.lines.pop_front() {
                    return Ok(line);
                }
                if self.exited {
                    return Err(ReviewError::new(
                        ReviewErrorCode::EngineExited,
                        "engine.read",
                        "A engine encerrou durante a análise.",
                    ));
                }
                match self.rx.recv().await {
                    Some(CommandEvent::Stdout(bytes)) => {
                        self.stdout.extend_from_slice(&bytes);
                        crate::engine::consume_stdout_lines(&mut self.stdout, |line| {
                            self.lines.push_back(line)
                        });
                    }
                    Some(CommandEvent::Terminated(p)) => {
                        self.exited = true;
                        return Err(ReviewError::new(
                            ReviewErrorCode::EngineExited,
                            "engine.read",
                            format!(
                                "A engine encerrou (código {:?}, sinal {:?}).",
                                p.code, p.signal
                            ),
                        ));
                    }
                    Some(CommandEvent::Error(e)) => {
                        return Err(ReviewError::new(
                            ReviewErrorCode::EngineExited,
                            "engine.read",
                            e,
                        ))
                    }
                    None => {
                        self.exited = true;
                        return Err(ReviewError::new(
                            ReviewErrorCode::EngineExited,
                            "engine.read",
                            "stdout da engine fechado.",
                        ));
                    }
                    _ => {}
                }
            }
        })
    }
    fn shutdown(&mut self) -> Task<'_, ()> {
        Box::pin(async move {
            if let Some(child) = self.child.take() {
                let _ = child.kill();
            }
            // The shell waits for child termination and sends this event. Never
            // release the permit before termination/EOF has actually completed.
            if !self.exited {
                while let Some(event) = self.rx.recv().await {
                    if matches!(event, CommandEvent::Terminated(_)) {
                        break;
                    }
                }
            }
            self.exited = true;
            self.permit.take();
        })
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
    port.send(command)?;
    let read = async {
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
        port.send(&format!("setoption name Threads value {n}"))?;
    }
    if let Some(n) = memory {
        port.send(&format!("setoption name Hash value {n}"))?;
    }
    port.send(&format!("setoption name MultiPV value {multipv}"))?;
    ask(port, "isready", "readyok", 10000, cancel).await?;
    Ok(())
}
struct BorrowedInfo<'a> {
    depth: u32,
    multipv: u32,
    cp: i32,
    pv: std::str::SplitWhitespace<'a>,
}

fn parse_info_borrowed(line: &str) -> Option<BorrowedInfo<'_>> {
    let mut words = line.split_whitespace();
    if words.next()? != "info" {
        return None;
    }
    let mut depth = 0;
    let mut multipv = 1;
    let mut cp = None;
    let mut pv = "".split_whitespace();
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
                pv = words;
                break;
            }
            // Bounds from an unfinished aspiration window are not exact
            // evaluations and must not overwrite a completed score.
            "lowerbound" | "upperbound" => return None,
            "string" => break,
            _ => {}
        }
    }
    Some(BorrowedInfo {
        depth,
        multipv,
        cp: cp?,
        pv,
    })
}

#[cfg(test)]
pub fn parse_info(line: &str) -> Option<RawLine> {
    let info = parse_info_borrowed(line)?;
    Some(RawLine {
        unstable: false,
        depth: Some(info.depth),
        multipv: info.multipv,
        cp: info.cp,
        pv: info.pv.map(str::to_owned).collect(),
        san: None,
    })
}

fn update_latest(latest: &mut BTreeMap<u32, RawLine>, info: BorrowedInfo<'_>) {
    use std::collections::btree_map::Entry;
    match latest.entry(info.multipv) {
        Entry::Vacant(entry) => {
            entry.insert(RawLine {
                unstable: false,
                depth: Some(info.depth),
                multipv: info.multipv,
                cp: info.cp,
                pv: info.pv.map(str::to_owned).collect(),
                san: None,
            });
        }
        Entry::Occupied(mut entry) => {
            let previous = entry.get_mut();
            if Some(info.depth) < previous.depth {
                return;
            }
            if Some(info.depth) > previous.depth {
                previous.unstable =
                    (super::scoring::win_pct(info.cp) - super::scoring::win_pct(previous.cp)).abs()
                        >= 2.0
                        || info.pv.clone().next() != previous.pv.first().map(String::as_str);
            }
            previous.depth = Some(info.depth);
            previous.cp = info.cp;
            let mut len = 0;
            for (index, mv) in info.pv.enumerate() {
                if let Some(existing) = previous.pv.get_mut(index) {
                    existing.clear();
                    existing.push_str(mv);
                } else {
                    previous.pv.push(mv.to_owned());
                }
                len = index + 1;
            }
            previous.pv.truncate(len);
        }
    }
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
    port.send(&format!("position fen {fen}"))?;
    let go = format!(
        "go {} {value}",
        if mode == crate::db::mode::Mode::Depth {
            "depth"
        } else {
            "movetime"
        }
    );
    port.send(&go)?;
    let read = async {
        let mut latest: BTreeMap<u32, RawLine> = BTreeMap::new();
        loop {
            let line = port.next().await?;
            if let Some(info) = parse_info_borrowed(&line) {
                update_latest(&mut latest, info);
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

#[cfg(test)]
mod info_update_tests {
    use super::*;

    #[test]
    fn borrowed_updates_match_owned_updates_across_depths_and_pv_lengths() {
        let inputs = [
            "info depth 8 multipv 1 score cp 10 pv e2e4 e7e5",
            "info depth 8 multipv 2 score mate -3 pv d2d4",
            "info depth 9 multipv 1 score cp 80 pv d2d4 d7d5 c2c4",
            "info depth 9 multipv 1 score cp 12 pv e2e4",
            "info depth 7 multipv 1 score cp 999 pv a2a4",
            "info depth 10 multipv 1 score cp 12 pv e2e4 e7e5 g1f3 b8c6",
            "info depth 11 multipv 2 score cp 5 lowerbound pv a2a4",
            "info string score cp 99 pv a2a4",
            "info depth 12 multipv 2 score mate 0 pv",
        ];
        let mut actual = BTreeMap::new();
        let mut expected: BTreeMap<u32, RawLine> = BTreeMap::new();
        for line in inputs {
            if let Some(info) = parse_info_borrowed(line) {
                update_latest(&mut actual, info);
            }
            if let Some(mut info) = parse_info(line) {
                if expected
                    .get(&info.multipv)
                    .is_none_or(|p| info.depth >= p.depth)
                {
                    if let Some(previous) = expected.get(&info.multipv) {
                        info.unstable = if info.depth > previous.depth {
                            (super::super::scoring::win_pct(info.cp)
                                - super::super::scoring::win_pct(previous.cp))
                            .abs()
                                >= 2.0
                                || info.pv.first() != previous.pv.first()
                        } else {
                            previous.unstable
                        };
                    }
                    expected.insert(info.multipv, info);
                }
            }
            assert_eq!(
                serde_json::to_value(&actual).unwrap(),
                serde_json::to_value(&expected).unwrap(),
                "after {line}"
            );
        }
    }
}
