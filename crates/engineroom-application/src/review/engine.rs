//! Rust owns the entire UCI exchange and exclusive process lease.
use super::types::*;
use crate::db::mode::Mode;
use shakmaty::{Chess, Move, Position, fen::Fen, uci::UciMove};
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
    /// Exact engine artifact identity for contextual cache reuse.
    fn artifact_signature(&self) -> Task<'_, Result<Option<String>>> {
        Box::pin(async { Ok(None) })
    }

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
/// Configures the engine, discarding the identity reported by the UCI handshake.
///
/// Kept for existing callers; use [`configure_with_identity`] when the actual
/// engine name is needed from the same handshake.
pub async fn configure(
    port: &mut dyn EnginePort,
    threads: Option<u32>,
    memory: Option<u32>,
    multipv: u32,
    cancel: &Cancellation,
) -> Result<()> {
    configure_with_identity(port, threads, memory, multipv, cancel)
        .await
        .map(|_| ())
}

/// Extracts the engine name from UCI handshake lines reported by `id name`.
///
/// Returns `None` when the engine did not identify itself, so callers can keep
/// the field absent rather than storing a fabricated name.
pub fn parse_engine_identity(lines: &[String]) -> Option<String> {
    lines.iter().find_map(|line| {
        line.strip_prefix("id name ")
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    })
}

/// Performs the UCI handshake and captures the actual engine identity.
pub async fn engine_identity(
    port: &mut dyn EnginePort,
    cancel: &Cancellation,
) -> Result<Option<String>> {
    let lines = ask(port, "uci", "uciok", 10_000, cancel).await?;
    Ok(parse_engine_identity(&lines))
}

/// Configures the engine and returns the identity captured from the same
/// UCI handshake, avoiding a second `uci` round trip.
pub async fn configure_with_identity(
    port: &mut dyn EnginePort,
    threads: Option<u32>,
    memory: Option<u32>,
    multipv: u32,
    cancel: &Cancellation,
) -> Result<Option<String>> {
    let lines = ask(port, "uci", "uciok", 10_000, cancel).await?;
    let identity = parse_engine_identity(&lines);
    ask(port, "isready", "readyok", 10_000, cancel).await?;
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
    ask(port, "isready", "readyok", 10_000, cancel).await?;
    Ok(identity)
}

fn invalid_position(operation: &str, detail: &str) -> ReviewError {
    ReviewError::new(
        ReviewErrorCode::InvalidPayload,
        operation,
        format!("Comando de posição UCI inválido: {detail}"),
    )
}

/// Returns true when `movement` is a syntactically valid, non-null UCI move.
///
/// Piece drops and the null move (`0000`) are rejected: a historical prefix
/// for a standard chess game must contain only normal UCI moves, and a null
/// move would silently shift the position the engine actually evaluates.
pub fn is_valid_uci_move(movement: &str) -> bool {
    if movement.is_empty()
        || movement
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return false;
    }
    movement.parse::<UciMove>().is_ok_and(UciMove::is_normal)
}

/// Builds a validated UCI `position` command from an optional setup FEN and a
/// list of historical UCI moves.
///
/// - `base = None` produces `position startpos ...`.
/// - `base = Some(fen)` produces `position fen <fen> ...`.
/// - Moves are appended as `moves <m1> <m2> ...` when the prefix is non-empty.
///
/// Both the FEN and every move are validated before being appended, so untrusted
/// game data cannot inject extra UCI commands through newlines or stray tokens.
///
/// # Errors
/// Returns `InvalidPayload` when the FEN or any move is invalid.
pub fn position_command(base: Option<&str>, moves: &[String]) -> Result<String> {
    let mut command = match base {
        None => "position startpos".to_owned(),
        Some(fen) => {
            if fen.is_empty() || fen.trim() != fen || fen.chars().any(char::is_control) {
                return Err(invalid_position("engine.position_command", "FEN inválido"));
            }
            if fen.parse::<Fen>().is_err() {
                return Err(invalid_position("engine.position_command", "FEN inválido"));
            }
            format!("position fen {fen}")
        }
    };
    if !moves.is_empty() {
        command.push_str(" moves");
        for movement in moves {
            if !is_valid_uci_move(movement) {
                return Err(invalid_position(
                    "engine.position_command",
                    &format!("movimento UCI inválido: {movement:?}"),
                ));
            }
            command.push(' ');
            command.push_str(movement);
        }
    }
    Ok(command)
}

/// Validates and sends a historical position command, returning the exact
/// command delivered to the engine.
///
/// # Errors
/// Returns `InvalidPayload` for bad history, or a transport/cancellation error.
pub async fn send_position(
    port: &mut dyn EnginePort,
    base: Option<&str>,
    moves: &[String],
    cancel: &Cancellation,
) -> Result<String> {
    let command = position_command(base, moves)?;
    send(port, &command, cancel).await?;
    Ok(command)
}
pub fn parse_info(line: &str) -> Option<RawLine> {
    let info = parse_info_header(line)?;
    info.exact.then(|| info.to_raw(line, false))
}

struct InfoHeader {
    depth: u32,
    multipv: u32,
    cp: i32,
    exact: bool,
    time_ms: Option<u64>,
    nodes: Option<u64>,
    pv: Range<usize>,
    first_move_end: usize,
}

impl InfoHeader {
    fn first_move<'a>(&self, line: &'a str) -> Option<&'a str> {
        (self.pv.start < self.first_move_end).then(|| &line[self.pv.start..self.first_move_end])
    }

    fn to_raw(&self, line: &str, unstable: bool) -> RawLine {
        RawLine {
            unstable,
            depth: Some(self.depth),
            multipv: self.multipv,
            cp: self.cp,
            pv: line[self.pv.clone()]
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
    let mut exact = true;
    let mut time_ms = None;
    let mut nodes = None;
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
            "time" | "nodes" => {
                if let Some(value) = words.clone().next().and_then(|value| value.parse().ok()) {
                    words.next();
                    if token == "time" {
                        time_ms = Some(value);
                    } else {
                        nodes = Some(value);
                    }
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
            "lowerbound" | "upperbound" => exact = false,
            "string" => break,
            _ => {}
        }
    }
    Some(InfoHeader {
        depth,
        multipv,
        cp: cp?,
        exact,
        time_ms,
        nodes,
        pv,
        first_move_end,
    })
}
/// Exact, complete MultiPV evidence observed during one UCI search.
#[derive(Debug, Clone)]
pub struct SearchCheckpoint {
    /// Completed iteration depth. Published depths increase within a search.
    pub depth: u32,
    /// Greatest elapsed time reported for the lines in this iteration.
    pub time_ms: Option<u64>,
    /// Greatest cumulative node count reported for the lines in this iteration.
    pub nodes: Option<u64>,
    /// Exact lines in MultiPV rank order, all at the checkpoint depth.
    pub lines: Vec<RawLine>,
}

/// A single-search channel. Each receiver identifies its own search, even on a reused worker.
#[derive(Debug)]
pub struct SearchCheckpoints {
    multipv: u32,
    latest: watch::Sender<Option<SearchCheckpoint>>,
}

impl SearchCheckpoints {
    /// Keeps only the latest complete checkpoint, independently of final result delivery.
    pub fn channel(multipv: u32) -> (Self, watch::Receiver<Option<SearchCheckpoint>>) {
        let (latest, receiver) = watch::channel(None);
        (Self { multipv, latest }, receiver)
    }
}

struct CheckpointCollector {
    position: Chess,
    width: usize,
    published_depth: u32,
    depth: u32,
    time_ms: Option<u64>,
    nodes: Option<u64>,
    pending: Vec<(Move, RawLine)>,
}

impl CheckpointCollector {
    fn new(fen: &str, multipv: u32) -> Option<Self> {
        let position = super::core::position(fen).ok()?;
        let width = position.legal_moves().len().min(multipv as usize);
        (width > 0).then(|| Self {
            position,
            width,
            published_depth: 0,
            depth: 0,
            time_ms: None,
            nodes: None,
            pending: Vec::with_capacity(width),
        })
    }

    fn clear_pending(&mut self) {
        self.pending.clear();
        self.time_ms = None;
        self.nodes = None;
    }

    fn observe(&mut self, header: &InfoHeader, line: &str) -> Option<SearchCheckpoint> {
        if !header.exact || header.depth <= self.published_depth || header.depth < self.depth {
            return None;
        }
        if header.multipv == 1 || header.depth > self.depth {
            self.depth = header.depth;
            self.clear_pending();
        }
        let rank = header.multipv as usize;
        if rank != self.pending.len() + 1 || rank > self.width {
            self.clear_pending();
            return None;
        }
        let root = header
            .first_move(line)
            .and_then(|uci| uci.parse::<UciMove>().ok())
            .and_then(|uci| uci.to_move(&self.position).ok())
            .filter(|root| self.pending.iter().all(|(seen, _)| seen != root));
        let Some(root) = root else {
            self.clear_pending();
            return None;
        };
        self.time_ms = self.time_ms.max(header.time_ms);
        self.nodes = self.nodes.max(header.nodes);
        self.pending.push((root, header.to_raw(line, false)));
        if self.pending.len() != self.width {
            return None;
        }
        self.published_depth = self.depth;
        Some(SearchCheckpoint {
            depth: self.depth,
            time_ms: self.time_ms,
            nodes: self.nodes,
            lines: self.pending.drain(..).map(|(_, line)| line).collect(),
        })
    }
}

fn go_command(mode: Mode, value: u32) -> String {
    format!(
        "go {} {value}",
        if mode == Mode::Depth {
            "depth"
        } else {
            "movetime"
        }
    )
}

/// Shared UCI search: sends the position and go commands, then consumes the
/// evaluation lines through `bestmove`. The caller supplies the validated
/// command strings and the FEN recorded on the result.
async fn search(
    port: &mut dyn EnginePort,
    position: &str,
    go: &str,
    fen: &str,
    timeout: u64,
    cancel: &Cancellation,
    checkpoints: Option<SearchCheckpoints>,
) -> Result<RawPosition> {
    cancel.check()?;
    let read = async {
        port.send(position).await?;
        port.send(go).await?;
        // Stockfish supports up to 500 PVs. Keep larger ranks from custom
        // engines sparse so one rank cannot force an unbounded resize.
        const DENSE_MULTIPV: u32 = 500;
        let mut collector = checkpoints.and_then(|observer| {
            Some((
                CheckpointCollector::new(fen, observer.multipv)?,
                observer.latest,
            ))
        });
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
            if let Some((collector, latest)) = &mut collector
                && let Some(checkpoint) = collector.observe(&header, &line)
            {
                let _ = latest.send(Some(checkpoint));
            }
            if !header.exact {
                continue;
            }
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
            .map(|info| info.header.to_raw(&info.line, info.unstable))
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

/// Evaluates a single FEN. Preserved for existing tests and callers.
pub async fn evaluate(
    port: &mut dyn EnginePort,
    fen: &str,
    mode: crate::db::mode::Mode,
    value: u32,
    timeout: u64,
    cancel: &Cancellation,
) -> Result<RawPosition> {
    let go = go_command(mode, value);
    search(
        port,
        &format!("position fen {fen}"),
        &go,
        fen,
        timeout,
        cancel,
        None,
    )
    .await
}

/// Evaluates the position reached by validated historical UCI moves from the
/// standard starting position, recording `result_fen` on the returned raw
/// evaluation.
pub async fn evaluate_history(
    port: &mut dyn EnginePort,
    moves: &[String],
    result_fen: &str,
    mode: crate::db::mode::Mode,
    value: u32,
    timeout: u64,
    cancel: &Cancellation,
) -> Result<RawPosition> {
    evaluate_position(port, None, moves, result_fen, mode, value, timeout, cancel).await
}

/// General history-aware entry point: evaluates
/// `position [startpos | fen <base>] moves ...`.
///
/// `result_fen` is the FEN of the position after `moves` and is stored on the
/// result; the command itself is built from `base` and validated moves.
#[expect(
    clippy::too_many_arguments,
    reason = "UCI configuration separates search budget and position context"
)]
pub async fn evaluate_position(
    port: &mut dyn EnginePort,
    base: Option<&str>,
    moves: &[String],
    result_fen: &str,
    mode: crate::db::mode::Mode,
    value: u32,
    timeout: u64,
    cancel: &Cancellation,
) -> Result<RawPosition> {
    cancel.check()?;
    let position = position_command(base, moves)?;
    let go = go_command(mode, value);
    search(port, &position, &go, result_fen, timeout, cancel, None).await
}

/// Runs one automatic time-budget search, publishing complete checkpoints before bestmove.
///
/// The observer width must match the MultiPV setting on the configured engine.
///
/// # Errors
///
/// Returns a UCI, validation, timeout, or cancellation error. A failed search
/// must be followed by engine teardown before the lease is reused.
pub async fn evaluate_with_checkpoints(
    port: &mut dyn EnginePort,
    fen: &str,
    history: Option<&super::core::PositionContext>,
    value: u32,
    timeout: u64,
    cancel: &Cancellation,
    checkpoints: SearchCheckpoints,
) -> Result<RawPosition> {
    cancel.check()?;
    let position = match history {
        Some(history) => position_command(Some(&history.initial_fen), &history.uci_path)?,
        None => format!("position fen {fen}"),
    };
    let go = go_command(Mode::Time, value);
    search(
        port,
        &position,
        &go,
        fen,
        timeout,
        cancel,
        Some(checkpoints),
    )
    .await
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

    #[tokio::test]
    async fn checkpoints_coalesce_complete_iterations_and_keep_the_final_result_independent() {
        let transcript = vec![
            "info depth 3 multipv 1 score cp 20 time 2 nodes 50 pv e2e4",
            "info depth 3 multipv 2 score cp 10 time 3 nodes 90 pv d2d4",
            "info depth 4 multipv 1 score cp 25 pv d2d4",
            "info depth 4 multipv 2 score cp 15 pv d2d4",
            "info depth 4 multipv 1 score cp 25 pv e2e4",
            "info depth 4 multipv 2 score cp 10 pv e7e5",
            "info depth 4 multipv 2 score cp 30 pv c2c4",
            "info depth 4 multipv 1 score cp 10 time 10 nodes 500 pv e2e4",
            "info depth 4 multipv 3 score cp 40 pv g1f3",
            "info depth 4 multipv 2 score cp 30 pv c2c4",
            "info depth 5 multipv 1 score cp 40 time 999 nodes 9999 pv e2e4",
            "info depth 5 multipv 1 score cp 35 upperbound pv c2c4",
            "info depth 5 multipv 1 score cp 41 pv g1f3",
            "info depth 5 multipv 2 score cp 32 time 15 nodes 613 pv c2c4",
            "info depth 5 multipv 1 score cp 45 pv b1c3",
            "info depth 6 multipv 1 score cp 50 pv e2e4",
            "info depth 7 multipv 2 score cp 60 pv d2d4",
            "info depth 6 multipv 2 score cp 42 pv c2c4",
            "info depth 7 multipv 1 score cp 55 pv c2c4",
            "info depth 7 multipv 1 score cp 56 lowerbound pv c2c4",
            "info depth 7 multipv 2 score cp 53 time 30 nodes 1500 pv d2d4",
            "info depth 8 multipv 1 score cp 70 time 40 nodes 2000 pv e2e4",
            "bestmove e2e4",
        ];
        let (_closed, receiver) = watch::channel(false);
        let cancel = Cancellation {
            closed: receiver,
            revision: None,
        };
        let (publish, observed) = SearchCheckpoints::channel(2);
        let fen = super::super::core::fen(&Chess::default());
        let mut collector = CheckpointCollector::new(&fen, 2).expect("valid starting position");
        let iterations: Vec<_> = transcript
            .iter()
            .filter_map(|line| collector.observe(&parse_info_header(line)?, line))
            .map(|checkpoint| (checkpoint.depth, checkpoint.time_ms, checkpoint.nodes))
            .collect();
        assert_eq!(
            iterations,
            [
                (3, Some(3), Some(90)),
                (5, Some(15), Some(613)),
                (7, Some(30), Some(1500)),
            ]
        );

        let mut port = Transcript(transcript.into_iter());
        let final_result =
            evaluate_with_checkpoints(&mut port, &fen, None, 50, 1000, &cancel, publish)
                .await
                .expect("the completed go returns its final evaluation");
        let observed = observed.borrow();
        let checkpoint = observed.as_ref().expect("at least one complete iteration");
        assert_eq!(
            (checkpoint.depth, checkpoint.time_ms, checkpoint.nodes),
            (7, Some(30), Some(1500))
        );
        assert_eq!(
            checkpoint
                .lines
                .iter()
                .map(|line| (line.multipv, line.depth, line.pv[0].as_str()))
                .collect::<Vec<_>>(),
            [(1, Some(7), "c2c4"), (2, Some(7), "d2d4")],
        );
        assert_eq!(final_result.depth, 8);
        assert_eq!(final_result.pv, ["e2e4"]);
        assert_eq!(final_result.lines[1].pv, ["d2d4"]);
    }

    #[test]
    fn checkpoint_width_is_capped_by_legal_roots_and_metrics_may_be_absent() {
        // Black has only Kh7 available.
        let fen = "R6k/6p1/8/8/8/8/8/6K1 b - - 1 1";
        let mut collector = CheckpointCollector::new(fen, 5).expect("position with a legal move");
        let line = "info depth 8 score cp -520 pv h8h7";
        let checkpoint = collector
            .observe(&parse_info_header(line).expect("valid info"), line)
            .expect("single legal root completes the iteration");
        assert_eq!(checkpoint.lines.len(), 1);
        assert_eq!(
            (checkpoint.depth, checkpoint.time_ms, checkpoint.nodes),
            (8, None, None)
        );
        let older = "info depth 7 score cp -500 pv h8h7";
        assert!(
            collector
                .observe(&parse_info_header(older).unwrap(), older)
                .is_none()
        );
    }

    #[tokio::test]
    async fn checkpoints_require_a_legal_position_even_when_the_engine_returns_scores() {
        let mut port =
            Transcript(vec!["info depth 4 score cp 20 pv e2e4", "bestmove e2e4"].into_iter());
        let cancel = cancel();
        let (publish, observed) = SearchCheckpoints::channel(1);
        let raw =
            evaluate_with_checkpoints(&mut port, "invalid FEN", None, 50, 1000, &cancel, publish)
                .await
                .expect("the unchecked FEN still follows the existing UCI result path");
        assert_eq!(raw.depth, 4);
        assert!(observed.borrow().is_none());
    }

    /// Engine port that records every sent command and replays scripted stdout lines.
    struct Recording {
        sent: Vec<String>,
        lines: std::collections::VecDeque<String>,
    }

    impl Recording {
        fn new(lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
            Self {
                sent: Vec::new(),
                lines: lines.into_iter().map(Into::into).collect(),
            }
        }
    }

    impl EnginePort for Recording {
        fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, Result<()>> {
            self.sent.push(command.to_owned());
            Box::pin(async { Ok(()) })
        }

        fn next(&mut self) -> Task<'_, Result<String>> {
            let line = self.lines.pop_front().unwrap_or_default();
            Box::pin(async { Ok(line) })
        }

        fn shutdown(&mut self) -> Task<'_, ()> {
            Box::pin(async {})
        }
    }

    fn cancel() -> Cancellation {
        let (_closed, receiver) = watch::channel(false);
        Cancellation {
            closed: receiver,
            revision: None,
        }
    }

    fn moves(list: &[&str]) -> Vec<String> {
        list.iter().map(|m| (*m).to_owned()).collect()
    }

    #[test]
    fn parses_actual_engine_identity_from_handshake_lines() {
        let lines = [
            "id name Stockfish 19".to_owned(),
            "id author The Stockfish developers".to_owned(),
            "uciok".to_owned(),
        ];
        assert_eq!(
            parse_engine_identity(&lines).as_deref(),
            Some("Stockfish 19")
        );
        assert_eq!(parse_engine_identity(&[]), None);
        assert_eq!(
            parse_engine_identity(&["option name Hash".to_owned()]),
            None
        );
    }

    #[tokio::test]
    async fn engine_identity_captures_name_from_uci_handshake() {
        let mut port = Recording::new(["id name Stockfish 19", "uciok"]);
        let identity = engine_identity(&mut port, &cancel())
            .await
            .expect("handshake succeeds");
        assert_eq!(identity.as_deref(), Some("Stockfish 19"));
        assert_eq!(port.sent, ["uci"]);
    }

    #[tokio::test]
    async fn configure_with_identity_captures_name_from_one_handshake() {
        let mut port = Recording::new(["id name Stockfish 19", "uciok", "readyok", "readyok"]);
        let identity = configure_with_identity(&mut port, Some(4), Some(128), 3, &cancel())
            .await
            .expect("configuration succeeds");
        assert_eq!(identity.as_deref(), Some("Stockfish 19"));
        assert_eq!(
            port.sent,
            [
                "uci",
                "isready",
                "setoption name Threads value 4",
                "setoption name Hash value 128",
                "setoption name MultiPV value 3",
                "isready",
            ]
        );
    }

    #[test]
    fn position_command_builds_startpos_with_validated_history() {
        assert_eq!(
            position_command(None, &moves(&[])).unwrap(),
            "position startpos"
        );
        assert_eq!(
            position_command(None, &moves(&["e2e4", "e7e5", "g1f3", "e7e8q"])).unwrap(),
            "position startpos moves e2e4 e7e5 g1f3 e7e8q"
        );
    }

    #[test]
    fn position_command_builds_fen_base_with_validated_history() {
        let fen = "7k/8/8/8/8/8/8/R6K w - - 0 1";
        assert_eq!(
            position_command(Some(fen), &moves(&["a2a6"])).unwrap(),
            format!("position fen {fen} moves a2a6")
        );
    }

    #[test]
    fn position_command_rejects_invalid_history_and_fen() {
        for bad in ["", "0000", "e2", "e2e4 extra", "e2e4\nisready", "Q@f7"] {
            let history = moves(&[bad]);
            assert!(!is_valid_uci_move(bad), "accepted move {bad:?}");
            assert!(
                position_command(None, &history).is_err(),
                "accepted history {bad:?}"
            );
        }
        for bad_fen in ["", "7k/8/8/8/8/8/8/R6K w - - 0 1\nisready", "not a fen"] {
            assert!(
                position_command(Some(bad_fen), &[]).is_err(),
                "accepted FEN {bad_fen:?}"
            );
        }
    }

    #[tokio::test]
    async fn send_position_delivers_validated_command() {
        let mut port = Recording::new(Vec::<String>::new());
        let history = moves(&["e2e4", "e7e5"]);
        let command = send_position(&mut port, None, &history, &cancel())
            .await
            .expect("valid history is sent");
        assert_eq!(command, "position startpos moves e2e4 e7e5");
        assert_eq!(port.sent, [command]);
    }

    #[tokio::test]
    async fn evaluate_still_sends_plain_fen_command() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let mut port = Recording::new(["info depth 2 score cp 7 pv e2e4", "bestmove e2e4"]);
        let result = evaluate(&mut port, fen, Mode::Depth, 2, 1000, &cancel())
            .await
            .expect("transcript has an evaluation");
        assert_eq!(result.fen, fen);
        assert_eq!(result.cp, 7);
        assert_eq!(
            port.sent,
            [format!("position fen {fen}"), "go depth 2".to_owned()]
        );
    }

    #[tokio::test]
    async fn evaluate_history_sends_historical_command_and_records_result_fen() {
        let mut port = Recording::new(["info depth 4 score cp 35 pv g1f3", "bestmove g1f3"]);
        let history = moves(&["e2e4", "e7e5", "g1f3"]);
        let result_fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2";
        let result = evaluate_history(
            &mut port,
            &history,
            result_fen,
            Mode::Time,
            100,
            1000,
            &cancel(),
        )
        .await
        .expect("transcript has an evaluation");
        assert_eq!(result.fen, result_fen);
        assert_eq!(result.cp, 35);
        assert_eq!(
            port.sent,
            ["position startpos moves e2e4 e7e5 g1f3", "go movetime 100",]
        );
    }

    #[tokio::test]
    async fn evaluate_position_sends_fen_base_with_moves() {
        let fen = "7k/8/8/8/8/8/8/R6K w - - 0 1";
        let mut port = Recording::new(["info depth 3 score cp 10 pv a2a6", "bestmove a2a6"]);
        let result = evaluate_position(
            &mut port,
            Some(fen),
            &moves(&["a2a6"]),
            "result-fen",
            Mode::Depth,
            5,
            1000,
            &cancel(),
        )
        .await
        .expect("transcript has an evaluation");
        assert_eq!(result.fen, "result-fen");
        assert_eq!(
            port.sent,
            [
                format!("position fen {fen} moves a2a6"),
                "go depth 5".to_owned()
            ]
        );
    }
}
