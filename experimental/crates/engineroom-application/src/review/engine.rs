//! Rust owns the entire UCI exchange and exclusive process lease.
use super::types::*;
use std::{future::Future, ops::Range, pin::Pin, time::Duration};
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
    let info = parse_info_header(line)?;
    Some(info.into_raw(line, false))
}

struct InfoHeader {
    depth: u32,
    multipv: u32,
    cp: i32,
    pv: Range<usize>,
    first_move_end: usize,
}

impl InfoHeader {
    fn first_move<'a>(&self, line: &'a str) -> Option<&'a str> {
        (self.pv.start < self.first_move_end).then(|| &line[self.pv.start..self.first_move_end])
    }

    fn into_raw(self, line: &str, unstable: bool) -> RawLine {
        RawLine {
            unstable,
            depth: Some(self.depth),
            multipv: self.multipv,
            cp: self.cp,
            pv: line[self.pv]
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            san: None,
        }
    }
}

struct LatestInfo {
    line: String,
    header: InfoHeader,
    unstable: bool,
}

fn parse_info_header(line: &str) -> Option<InfoHeader> {
    let mut words = line.split_whitespace();
    if words.next()? != "info" {
        return None;
    }
    let mut depth = 0;
    let mut multipv = 1;
    let mut cp = None;
    let mut pv = line.len()..line.len();
    let mut first_move_end = line.len();
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
                if let Some(first_move) = words.next() {
                    // The token is a slice of line, so its address gives its byte offset.
                    let start = first_move.as_ptr() as usize - line.as_ptr() as usize;
                    pv = start..line.len();
                    first_move_end = start + first_move.len();
                }
                break;
            }
            // Bounds from an unfinished aspiration window are not exact
            // evaluations and must not overwrite a completed score.
            "lowerbound" | "upperbound" => return None,
            "string" => break,
            _ => {}
        }
    }
    Some(InfoHeader {
        depth,
        multipv,
        cp: cp?,
        pv,
        first_move_end,
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
        // Stockfish supports up to 500 PVs. Keep larger ranks from custom
        // engines sparse so one rank cannot force an unbounded resize.
        const DENSE_MULTIPV: u32 = 500;
        let mut latest: Vec<Option<LatestInfo>> = Vec::new();
        let mut extra: Vec<(u32, Option<LatestInfo>)> = Vec::new();
        loop {
            let line = port.next().await?;
            if line.trim().starts_with("bestmove") {
                break;
            }
            let Some(header) = parse_info_header(&line) else {
                continue;
            };
            let slot = if header.multipv <= DENSE_MULTIPV {
                let index = (header.multipv - 1) as usize;
                if index >= latest.len() {
                    latest.resize_with(index + 1, || None);
                }
                &mut latest[index]
            } else {
                let index = match extra.binary_search_by_key(&header.multipv, |entry| entry.0) {
                    Ok(index) => index,
                    Err(index) => {
                        extra.insert(index, (header.multipv, None));
                        index
                    }
                };
                &mut extra[index].1
            };
            let unstable = match slot {
                Some(previous) if header.depth < previous.header.depth => continue,
                Some(previous) if header.depth == previous.header.depth => previous.unstable,
                Some(previous) => {
                    (super::scoring::win_pct(header.cp)
                        - super::scoring::win_pct(previous.header.cp))
                    .abs()
                        >= 2.0
                        || header.first_move(&line) != previous.header.first_move(&previous.line)
                }
                None => false,
            };
            *slot = Some(LatestInfo {
                line,
                header,
                unstable,
            });
        }
        let lines: Vec<_> = latest
            .into_iter()
            .chain(extra.into_iter().map(|(_, info)| info))
            .flatten()
            .map(|info| info.header.into_raw(&info.line, info.unstable))
            .collect();
        let principal = lines.first().filter(|l| l.multipv == 1).ok_or_else(|| {
            ReviewError::new(
                ReviewErrorCode::MissingEvaluation,
                "engine.evaluate",
                "A engine encerrou a busca sem avaliação da posição.",
            )
        })?;
        Ok(RawPosition {
            fen: fen.into(),
            cp: principal.cp,
            depth: principal.depth.unwrap_or_default(),
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
mod tests {
    use super::*;

    struct Transcript(std::vec::IntoIter<&'static str>);

    impl EnginePort for Transcript {
        fn send<'a>(&'a mut self, _command: &'a str) -> Task<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn next(&mut self) -> Task<'_, Result<String>> {
            Box::pin(async { Ok(self.0.next().expect("transcript includes bestmove").into()) })
        }

        fn shutdown(&mut self) -> Task<'_, ()> {
            Box::pin(async {})
        }
    }

    #[tokio::test]
    async fn retains_latest_exact_depth_and_instability_per_rank() {
        let mut port = Transcript(
            vec![
                "info depth 5 multipv 3 score cp 0 pv d2d4",
                "info depth 5 score cp 0 pv e2e4 e7e5",
                "info depth 6 multipv 3 score cp 150 pv d2d4 d7d5",
                "info depth 7 multipv 3 score cp 150 pv d2d4 g8f6",
                "info depth 6 score cp 0 pv c2c4",
                "info depth 6 score cp 0 pv c2c4   e7e5",
                "info depth 5 score cp 500 pv g2g4",
                "info depth 7 score cp 500 lowerbound pv g2g4",
                "info depth 7 score cp 500 upperbound pv g2g4",
                "info depth 7 multipv 0 score cp 500 pv g2g4",
                "info depth 5 multipv 4 score cp 0",
                "info depth 6 multipv 4 score cp 150 pv",
                "info depth 9 multipv 4294967295 score cp -25 pv g1f3",
                "bestmove c2c4",
            ]
            .into_iter(),
        );
        let (_closed, receiver) = watch::channel(false);
        let cancel = Cancellation {
            closed: receiver,
            revision: None,
        };

        let result = evaluate(
            &mut port,
            "",
            crate::db::mode::Mode::Depth,
            7,
            1000,
            &cancel,
        )
        .await
        .expect("transcript contains exact principal evaluation");
        assert_eq!((result.depth, result.cp), (6, 0));
        assert_eq!(result.pv, ["c2c4", "e7e5"]);
        assert_eq!(
            result
                .lines
                .iter()
                .map(|line| (line.multipv, line.depth, line.unstable))
                .collect::<Vec<_>>(),
            [
                (1, Some(6), true),
                (3, Some(7), false),
                (4, Some(6), true),
                (u32::MAX, Some(9), false),
            ]
        );
        assert_eq!(result.lines[1].pv, ["d2d4", "g8f6"]);
        assert!(result.lines[2].pv.is_empty());
    }
}
