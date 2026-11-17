//! OS-level teardown evidence for the native Stockfish adapter.
//!
//! The fake engine cannot answer "did the OS process terminate?" because its
//! `shutdown()` is a no-op. This test uses `NativeEngineFactory` against a real
//! executable and asserts that every child PID spawned by the adapter is gone
//! from `/proc` after both graceful shutdown and cancellation of a live search.
//!
//! Run explicitly (it is `#[ignore]` so `cargo test` stays hermetic):
//!
//! ```sh
//! ENGINE_ROOM_STOCKFISH=/path/to/stockfish \
//!   cargo test -p engineroom-uci real_process_lifecycle -- --ignored --nocapture
//! ```
#![cfg(target_os = "linux")]

use engineroom_application::{
    db::mode::Mode,
    review::{
        core,
        engine::{Cancellation, EngineFactory, configure, evaluate},
        types::ReviewErrorCode,
    },
};
use engineroom_uci::native_engine::NativeEngineFactory;
use std::{collections::BTreeSet, fs, path::Path, time::Duration};
use tokio::sync::watch;

const SPAWN_WAIT: Duration = Duration::from_secs(3);
const TEARDOWN_WAIT: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const SEARCH_TIMEOUT_MS: u64 = 10_000;
const CANCELLED_MOVETIME_MS: u32 = 30_000;

/// PIDs in `/proc` whose command line names the exact Stockfish binary under test.
///
/// Matching the configured path (not just the truncated `comm`) avoids counting
/// unrelated processes and works for the sidecar name used by the app.
fn stockfish_pids(binary: &Path) -> BTreeSet<u32> {
    let needle = binary.to_string_lossy();
    let mut found = BTreeSet::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return found;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(cmdline) = fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        if String::from_utf8_lossy(&cmdline).contains(needle.as_ref()) {
            found.insert(pid);
        }
    }
    found
}

/// True while the PID still names a Stockfish process (also survives PID reuse).
fn pid_is_stockfish(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|comm| comm.trim().starts_with("stockfish"))
        .unwrap_or(false)
}

fn configured_stockfish() -> std::path::PathBuf {
    let value = std::env::var_os("ENGINE_ROOM_STOCKFISH")
        .expect("set ENGINE_ROOM_STOCKFISH to a real Stockfish executable");
    let path = std::path::PathBuf::from(value);
    assert!(path.is_file(), "Stockfish not found: {}", path.display());
    path
}

fn cancellation() -> (watch::Sender<bool>, Cancellation) {
    let (sender, closed) = watch::channel(false);
    (
        sender,
        Cancellation {
            closed,
            revision: None,
        },
    )
}

async fn wait_until(mut predicate: impl FnMut() -> bool, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if predicate() {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

#[tokio::test]
#[ignore = "requires ENGINE_ROOM_STOCKFISH; asserts real OS process termination"]
async fn real_process_lifecycle_shutdown_and_cancel_reap_stockfish_pids() {
    let binary = configured_stockfish();
    let factory = NativeEngineFactory::new(binary.clone());
    let fen = core::fen(&shakmaty::Chess::default());
    let baseline = stockfish_pids(&binary);

    // Phase 1: graceful shutdown of a real pool, after a real depth search.
    let (_sender, cancel) = cancellation();
    let mut ports = factory
        .acquire_pool(2, &cancel)
        .await
        .expect("spawn two real Stockfish processes");
    assert!(
        wait_until(
            || stockfish_pids(&binary).difference(&baseline).count() >= ports.len(),
            SPAWN_WAIT
        )
        .await,
        "Stockfish child PIDs did not appear in /proc"
    );
    let spawned: BTreeSet<u32> = stockfish_pids(&binary)
        .difference(&baseline)
        .copied()
        .collect();
    assert_eq!(
        spawned.len(),
        ports.len(),
        "expected one OS process per acquired port"
    );
    for port in &mut ports {
        configure(port.as_mut(), Some(1), Some(16), 1, &cancel)
            .await
            .expect("configure real Stockfish");
        let result = evaluate(
            port.as_mut(),
            &fen,
            Mode::Depth,
            4,
            SEARCH_TIMEOUT_MS,
            &cancel,
        )
        .await
        .expect("real depth search");
        assert!(
            result.depth >= 4 && !result.pv.is_empty(),
            "the engine must perform real work, not a stub reply"
        );
    }
    for port in &mut ports {
        port.shutdown().await;
    }
    assert!(
        wait_until(
            || spawned.iter().all(|pid| !pid_is_stockfish(*pid)),
            TEARDOWN_WAIT
        )
        .await,
        "a Stockfish PID survived graceful shutdown: {spawned:?}"
    );
    assert!(
        stockfish_pids(&binary).is_subset(&baseline),
        "unexpected Stockfish processes after graceful shutdown"
    );

    // The lease is a proxy only because the adapter releases it after wait().
    let (fresh_sender, fresh) = cancellation();
    let mut probe = factory
        .acquire(&fresh)
        .await
        .expect("lease released after all children were reaped");
    let probe_pids: BTreeSet<u32> = stockfish_pids(&binary)
        .difference(&baseline)
        .copied()
        .collect();
    assert_eq!(probe_pids.len(), 1, "probe must own exactly one process");
    probe.shutdown().await;
    assert!(
        wait_until(
            || probe_pids.iter().all(|pid| !pid_is_stockfish(*pid)),
            TEARDOWN_WAIT
        )
        .await,
        "probe process survived shutdown: {probe_pids:?}"
    );
    drop(fresh_sender);

    // Phase 2: cancel a long live search, then tear the same process down.
    let pre_cancel = stockfish_pids(&binary);
    let (sender, cancel) = cancellation();
    let mut port = factory
        .acquire(&cancel)
        .await
        .expect("spawn Stockfish for the cancellation case");
    assert!(
        wait_until(
            || stockfish_pids(&binary).difference(&pre_cancel).count() >= 1,
            SPAWN_WAIT
        )
        .await,
        "cancellation-case PID did not appear in /proc"
    );
    let cancelled_pid: BTreeSet<u32> = stockfish_pids(&binary)
        .difference(&pre_cancel)
        .copied()
        .collect();
    assert_eq!(cancelled_pid.len(), 1, "one process for the live search");
    configure(port.as_mut(), Some(1), Some(16), 1, &cancel)
        .await
        .expect("configure Stockfish for the live search");
    let running_cancel = cancel.clone();
    let running = tokio::spawn(async move {
        let outcome = evaluate(
            port.as_mut(),
            &fen,
            Mode::Time,
            CANCELLED_MOVETIME_MS,
            SEARCH_TIMEOUT_MS + u64::from(CANCELLED_MOVETIME_MS),
            &running_cancel,
        )
        .await;
        (port, outcome)
    });
    // Let the search actually start before invalidating it, otherwise the
    // cancellation would be observed before any `go movetime` reached the OS process.
    tokio::time::sleep(Duration::from_millis(200)).await;
    sender.send_replace(true);
    let (mut port, outcome) = running.await.expect("cancelled search task");
    assert_eq!(
        outcome
            .expect_err("live search must observe cancellation")
            .code,
        ReviewErrorCode::Cancelled
    );
    port.shutdown().await;
    assert!(
        wait_until(
            || cancelled_pid.iter().all(|pid| !pid_is_stockfish(*pid)),
            TEARDOWN_WAIT
        )
        .await,
        "Stockfish PID survived cancellation teardown: {cancelled_pid:?}"
    );
    assert!(
        stockfish_pids(&binary).is_subset(&baseline),
        "no Stockfish process may remain beyond the original baseline"
    );
    drop(sender);
}
