//! Integration test: drives the bundled Stockfish binary directly through a real
//! UCI handshake (uci -> uciok, position+go -> bestmove). This guards both the
//! shipped engine binary and the UCI plumbing assumptions used by `engine.rs`.
//!
//! The binary is fetched out-of-band (`scripts/fetch-stockfish.mjs`) into
//! `src-tauri/binaries/stockfish-<triple>`. If it is absent the test is skipped
//! rather than failed, so CI without the binary still passes.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Best-effort host target triple: prefer the one tauri-build bakes in at
/// compile time, then fall back to deriving it from `std::env::consts`.
fn target_triple() -> String {
    if let Some(triple) = option_env!("TAURI_ENV_TARGET_TRIPLE") {
        if !triple.is_empty() {
            return triple.to_string();
        }
    }
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "x86" => "i686",
        "aarch64" => "aarch64",
        "arm" => "armv7",
        other => other,
    };
    let os = match std::env::consts::OS {
        "linux" => "unknown-linux-gnu",
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        other => other,
    };
    format!("{arch}-{os}")
}

fn sidecar_path() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest)
        .join("binaries")
        .join(format!("stockfish-{}", target_triple()))
}

const TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::test]
async fn stockfish_uci_handshake_and_bestmove() {
    let bin = sidecar_path();
    if !bin.exists() {
        eprintln!(
            "[skip] Stockfish sidecar não encontrado em {}, pulando teste.",
            bin.display()
        );
        return;
    }

    let mut child = Command::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("falha ao spawnar a engine");

    let stdout = child.stdout.take().expect("sem stdout");
    let mut stdin = child.stdin.take().expect("sem stdin");
    let mut lines = BufReader::new(stdout).lines();
    let deadline = Instant::now() + TIMEOUT;

    // 1) UCI handshake.
    stdin.write_all(b"uci\n").await.expect("write uci");
    stdin.flush().await.expect("flush uci");

    let mut got_uciok = false;
    let mut name = String::new();
    while Instant::now() < deadline {
        let next = tokio::time::timeout(Duration::from_millis(500), lines.next_line()).await;
        match next {
            Ok(Ok(Some(line))) => {
                if let Some(rest) = line.strip_prefix("id name") {
                    name = rest.trim().to_string();
                }
                if line.starts_with("uciok") {
                    got_uciok = true;
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(got_uciok, "engine não respondeu `uciok` a tempo");
    assert!(
        name.contains("Stockfish"),
        "id name inesperado: {name:?}"
    );

    // 2) Ask for a move from the start position.
    stdin
        .write_all(b"position startpos\ngo depth 8\n")
        .await
        .expect("write position/go");
    stdin.flush().await.expect("flush position/go");

    let mut bestmove: Option<String> = None;
    while Instant::now() < deadline {
        let next = tokio::time::timeout(Duration::from_millis(500), lines.next_line()).await;
        match next {
            Ok(Ok(Some(line))) => {
                if let Some(rest) = line.strip_prefix("bestmove ") {
                    bestmove = Some(rest.split_whitespace().next().unwrap_or("").to_string());
                    break;
                }
            }
            _ => {}
        }
    }

    // cleanup
    let _ = stdin.write_all(b"quit\n").await;
    let _ = child.kill().await;

    let bestmove = bestmove.expect("engine não devolveu `bestmove`");
    assert!(
        bestmove.len() >= 4 && bestmove != "(none)",
        "bestmove inválido: {bestmove:?}"
    );
}
