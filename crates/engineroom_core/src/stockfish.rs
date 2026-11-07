//! Minimal synchronous manual-analysis adapter for the GPUI experiment.
//! Call on a worker thread. Every exit kills and reaps the owned process.
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex, TryLockError,
    },
    thread,
    time::{Duration, Instant},
};

static ENGINE_LEASE: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnalysisLine {
    pub depth: u32,
    pub multipv: u32,
    /// UCI score, from the side-to-move perspective.
    pub score_cp: Option<i32>,
    pub mate: Option<i32>,
    pub pv: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Analysis {
    pub best_move: String,
    pub lines: Vec<AnalysisLine>,
}

struct Process {
    child: Child,
    input: ChildStdin,
    output: mpsc::Receiver<String>,
    reader: Option<thread::JoinHandle<()>>,
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Process {
    fn send(&mut self, command: &str) -> Result<(), String> {
        writeln!(self.input, "{command}")
            .and_then(|_| self.input.flush())
            .map_err(|error| format!("Stockfish input: {error}"))
    }

    fn receive(&self, cancelled: &AtomicBool, deadline: Instant) -> Result<String, String> {
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("cancelled".into());
            }
            if Instant::now() >= deadline {
                return Err("Stockfish timed out".into());
            }
            match self.output.recv_timeout(Duration::from_millis(25)) {
                Ok(line) => return Ok(line),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Stockfish closed its output".into());
                }
            }
        }
    }

    fn wait_for(&self, expected: &str, cancelled: &AtomicBool) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.receive(cancelled, deadline)? != expected {}
        Ok(())
    }
}

/// Analyze one validated FEN using a fresh Stockfish process.
///
/// The experimental adapter serializes its callers and retains its lease until
/// process termination and stdout-reader cleanup. This lease does not coordinate
/// with a separately running Tauri application. Cancellation returns `cancelled`;
/// consumers should suppress that outcome in their error UI.
pub fn analyze(
    executable: &Path,
    fen: &str,
    depth: u32,
    multipv: u32,
    cancelled: &AtomicBool,
) -> Result<Analysis, String> {
    if fen.contains(['\n', '\r']) || !(1..=30).contains(&depth) || !(1..=5).contains(&multipv) {
        return Err("Invalid manual analysis request".into());
    }
    let _lease = loop {
        if cancelled.load(Ordering::Acquire) {
            return Err("cancelled".into());
        }
        match ENGINE_LEASE.try_lock() {
            Ok(lease) => break lease,
            Err(TryLockError::Poisoned(error)) => break error.into_inner(),
            Err(TryLockError::WouldBlock) => thread::sleep(Duration::from_millis(25)),
        }
    };
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Cannot start Stockfish: {error}"))?;
    let input = child.stdin.take().expect("piped stdin");
    let output = child.stdout.take().expect("piped stdout");
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        // BufRead preserves partial chunks until a complete UCI line arrives.
        for line in BufReader::new(output).lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
    });
    let mut process = Process {
        child,
        input,
        output: receiver,
        reader: Some(reader),
    };
    process.send("uci")?;
    process.wait_for("uciok", cancelled)?;
    process.send("setoption name Threads value 1")?;
    process.send("setoption name Hash value 32")?;
    process.send(&format!("setoption name MultiPV value {multipv}"))?;
    process.send("isready")?;
    process.wait_for("readyok", cancelled)?;
    process.send(&format!("position fen {fen}"))?;
    process.send(&format!("go depth {depth}"))?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut lines = BTreeMap::new();
    loop {
        let line = process.receive(cancelled, deadline)?;
        if let Some(best) = line.strip_prefix("bestmove ") {
            return Ok(Analysis {
                best_move: best.split_whitespace().next().unwrap_or("(none)").into(),
                lines: lines.into_values().collect(),
            });
        }
        if let Some(info) = parse_info(&line) {
            lines.insert(info.multipv, info);
        }
    }
}

fn parse_info(line: &str) -> Option<AnalysisLine> {
    let tokens: Vec<_> = line.split_whitespace().collect();
    if tokens.first() != Some(&"info")
        || tokens.contains(&"lowerbound")
        || tokens.contains(&"upperbound")
    {
        return None;
    }
    let value = |key| {
        tokens
            .iter()
            .position(|token| *token == key)
            .and_then(|index| tokens.get(index + 1).copied())
    };
    let pv = tokens.iter().position(|token| *token == "pv")?;
    let score = tokens.iter().position(|token| *token == "score")?;
    let amount = tokens.get(score + 2)?.parse().ok()?;
    let mut info = AnalysisLine {
        depth: value("depth")?.parse().ok()?,
        multipv: value("multipv").unwrap_or("1").parse().ok()?,
        pv: tokens[pv + 1..]
            .iter()
            .map(|token| (*token).into())
            .collect(),
        ..Default::default()
    };
    match *tokens.get(score + 1)? {
        "cp" => info.score_cp = Some(amount),
        "mate" => info.mate = Some(amount),
        _ => return None,
    }
    Some(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scores_and_rejects_bounds() {
        let line =
            parse_info("info depth 14 multipv 2 score cp -25 nodes 100 pv e2e4 e7e5").unwrap();
        assert_eq!(
            (line.depth, line.multipv, line.score_cp),
            (14, 2, Some(-25))
        );
        assert_eq!(line.pv, ["e2e4", "e7e5"]);
        assert_eq!(
            parse_info("info depth 8 score mate -3 pv h1h8")
                .unwrap()
                .mate,
            Some(-3)
        );
        assert!(parse_info("info depth 8 score cp 45 lowerbound pv e2e4").is_none());
    }

    #[test]
    fn cancellation_before_spawn() {
        assert_eq!(
            analyze(
                Path::new("missing"),
                "startpos",
                8,
                1,
                &AtomicBool::new(true)
            )
            .unwrap_err(),
            "cancelled"
        );
    }

    #[test]
    #[ignore = "requires bundled Stockfish; run explicitly"]
    fn real_stockfish_manual_analysis() {
        let executable = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu");
        let analysis = analyze(
            &executable,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            6,
            2,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(analysis.lines.len(), 2);
        assert!(analysis
            .lines
            .iter()
            .all(|line| line.depth >= 6 && !line.pv.is_empty()));
        assert_eq!(analysis.best_move.len(), 4);
    }

    #[test]
    #[ignore = "requires bundled Stockfish; run explicitly"]
    fn real_stockfish_cancel_and_reuse() {
        let executable = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../app/src-tauri/binaries/stockfish-x86_64-unknown-linux-gnu");
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let cancelled = AtomicBool::new(false);
        let start = Instant::now();
        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(Duration::from_millis(150));
                cancelled.store(true, Ordering::Release);
            });
            assert_eq!(
                analyze(&executable, fen, 30, 1, &cancelled).unwrap_err(),
                "cancelled"
            );
        });
        assert!(start.elapsed() < Duration::from_secs(5));
        // A fresh process can acquire the lease after interrupted teardown.
        assert!(analyze(&executable, fen, 4, 1, &AtomicBool::new(false)).is_ok());
    }

    #[test]
    fn queued_request_can_cancel_without_starting_a_process() {
        let _lease = ENGINE_LEASE
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let cancelled = AtomicBool::new(false);
        thread::scope(|scope| {
            let queued = scope.spawn(|| analyze(Path::new("missing"), "unused", 4, 1, &cancelled));
            thread::sleep(Duration::from_millis(75));
            cancelled.store(true, Ordering::Release);
            assert_eq!(queued.join().unwrap().unwrap_err(), "cancelled");
        });
    }
}
