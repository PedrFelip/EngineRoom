//! OS-level teardown evidence for the native Stockfish adapter.
//!
//! The fake engine cannot answer "did the OS process terminate?" because its
//! `shutdown()` is a no-op. This opt-in test drives `NativeEngineFactory`
//! against a real executable and asserts that every child PID it spawns is gone
//! from `/proc` after an awaited `EnginePort::shutdown()`.
//!
//! Scope — read this before trusting a green result:
//! * It covers **explicit shutdown** only: graceful shutdown after a real depth
//!   search, and cancel-then-shutdown of a live `go movetime` search. It does
//!   NOT exercise `Drop` (the adapter's `Drop` is best-effort) and it is NOT a
//!   CI gate — the test is `#[ignore]`d and must be requested explicitly.
//! * Cancellation happens only after deterministic evidence that `go movetime`
//!   reached the OS process: a test-only `EnginePort` decorator observes an
//!   `info` answer. No fixed-duration sleep gates the cancellation.
//! * Process identity is the exact argv[0] of the configured binary, so renamed
//!   / sidecar binaries are tracked consistently. Every wait has a deadline.
//! * Requires Linux because it reads `/proc`; on other hosts the suite says so
//!   explicitly instead of silently reporting zero tests as a pass.
//!
//! Run explicitly (Linux only):
//!
//! ```sh
//! ENGINE_ROOM_STOCKFISH=/path/to/stockfish \
//!   cargo test -p engineroom-uci real_process_lifecycle -- --ignored --nocapture
//! ```

#[cfg(target_os = "linux")]
mod linux {
    use engineroom_application::{
        db::mode::Mode,
        review::{
            core,
            engine::{Cancellation, EngineFactory, EnginePort, Task, configure, evaluate},
            types::{Result, ReviewErrorCode},
        },
    };
    use engineroom_uci::native_engine::NativeEngineFactory;
    use std::{
        collections::BTreeSet,
        fs,
        future::Future,
        path::{Path, PathBuf},
        time::Duration,
    };
    use tokio::sync::watch;

    const SPAWN_WAIT: Duration = Duration::from_secs(5);
    const TEARDOWN_WAIT: Duration = Duration::from_secs(5);
    const ACQUIRE_WAIT: Duration = Duration::from_secs(20);
    const CONFIGURE_WAIT: Duration = Duration::from_secs(20);
    const SHUTDOWN_WAIT: Duration = Duration::from_secs(15);
    const EVALUATE_WAIT: Duration = Duration::from_secs(60);
    const EVIDENCE_WAIT: Duration = Duration::from_secs(20);
    const POLL_INTERVAL: Duration = Duration::from_millis(10);

    const DEPTH: u32 = 4;
    const SEARCH_TIMEOUT_MS: u64 = 10_000;
    const CANCELLED_MOVETIME_MS: u32 = 30_000;
    const LIVE_TIMEOUT_MS: u64 = SEARCH_TIMEOUT_MS + CANCELLED_MOVETIME_MS as u64;

    /// Exact argv[0] identifying every process spawned from `binary`.
    fn identity(binary: &Path) -> String {
        binary.to_string_lossy().into_owned()
    }

    /// argv[0] of one `/proc` entry. An empty cmdline (zombie / exited) is not
    /// an owned process, which also protects against PID reuse.
    fn proc_argv0(pid: u32) -> Option<String> {
        let bytes = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
        let argv0 = bytes.split(|byte| *byte == 0).next()?;
        Some(String::from_utf8_lossy(argv0).into_owned())
    }

    /// Single source of truth for process identity, used for discovery and for
    /// teardown checks alike (renamed/sidecar binaries included).
    fn is_owned(pid: u32, identity: &str) -> bool {
        proc_argv0(pid).as_deref() == Some(identity)
    }

    /// All live `/proc` processes whose argv[0] is the configured binary.
    fn owned_pids(identity: &str) -> BTreeSet<u32> {
        let Ok(entries) = fs::read_dir("/proc") else {
            return BTreeSet::new();
        };
        entries
            .flatten()
            .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
            .filter(|pid| is_owned(*pid, identity))
            .collect()
    }

    /// Deadline-bounded observation of `/proc`. It polls because `/proc` offers
    /// no notification; the interval is not a synchronization delay.
    async fn wait_until_deadline(mut predicate: impl FnMut() -> bool, timeout: Duration) -> bool {
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

    /// Bounds every awaited phase: a hang becomes a diagnostic panic, never a stall.
    async fn within<T>(label: &str, timeout: Duration, future: impl Future<Output = T>) -> T {
        tokio::time::timeout(timeout, future)
            .await
            .unwrap_or_else(|_| panic!("{label} did not finish within {timeout:?}"))
    }

    fn configured_stockfish() -> PathBuf {
        let value = std::env::var_os("ENGINE_ROOM_STOCKFISH")
            .expect("set ENGINE_ROOM_STOCKFISH to a real Stockfish executable");
        let path = PathBuf::from(value);
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

    /// Exactly `count` new test-owned PIDs over `baseline`.
    async fn claim_pids(
        identity: &str,
        baseline: &BTreeSet<u32>,
        count: usize,
        label: &str,
    ) -> BTreeSet<u32> {
        assert!(
            wait_until_deadline(
                || owned_pids(identity).difference(baseline).count() >= count,
                SPAWN_WAIT,
            )
            .await,
            "{label}: expected {count} new Stockfish PID(s) in /proc"
        );
        let new: BTreeSet<u32> = owned_pids(identity).difference(baseline).copied().collect();
        assert_eq!(
            new.len(),
            count,
            "{label}: expected exactly {count} owned process(es), got {new:?}"
        );
        new
    }

    /// These PIDs must be reaped and nothing else may remain beyond `baseline`.
    async fn assert_reaped(
        identity: &str,
        pids: &BTreeSet<u32>,
        baseline: &BTreeSet<u32>,
        label: &str,
    ) {
        assert!(
            wait_until_deadline(
                || pids.iter().all(|pid| !is_owned(*pid, identity)),
                TEARDOWN_WAIT
            )
            .await,
            "{label}: PID(s) {pids:?} survived explicit shutdown"
        );
        assert!(
            owned_pids(identity).is_subset(baseline),
            "{label}: unexpected Stockfish process(es) beyond baseline"
        );
    }

    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    struct SearchProgress {
        go_sent: bool,
        info_seen: bool,
    }

    impl SearchProgress {
        fn ready(self) -> bool {
            self.go_sent && self.info_seen
        }
    }

    /// Test-only decorator over any `EnginePort`: records that `evaluate`
    /// forwarded `go movetime` and that the OS process answered with `info`.
    /// It touches no production engine code and adds no dependency.
    struct ObservingPort {
        inner: Box<dyn EnginePort>,
        progress: watch::Sender<SearchProgress>,
    }

    impl ObservingPort {
        fn wrap(inner: Box<dyn EnginePort>) -> (Self, watch::Receiver<SearchProgress>) {
            let (progress, receiver) = watch::channel(SearchProgress::default());
            (Self { inner, progress }, receiver)
        }
    }

    impl EnginePort for ObservingPort {
        fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, Result<()>> {
            Box::pin(async move {
                if command.starts_with("go movetime") {
                    self.progress.send_modify(|state| state.go_sent = true);
                }
                self.inner.send(command).await
            })
        }

        fn next(&mut self) -> Task<'_, Result<String>> {
            Box::pin(async move {
                let line = self.inner.next().await?;
                if line.starts_with("info ") {
                    self.progress.send_modify(|state| state.info_seen = true);
                }
                Ok(line)
            })
        }

        fn shutdown(&mut self) -> Task<'_, ()> {
            self.inner.shutdown()
        }
    }

    /// Waits for the deterministic evidence without timers: `borrow()` marks the
    /// current version seen, so `changed()` cannot miss an intervening update.
    async fn wait_for_evidence(
        mut receiver: watch::Receiver<SearchProgress>,
    ) -> Option<SearchProgress> {
        loop {
            if receiver.borrow().ready() {
                return Some(*receiver.borrow());
            }
            if receiver.changed().await.is_err() {
                return None;
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires ENGINE_ROOM_STOCKFISH on Linux; explicit shutdown evidence only, not CI"]
    async fn real_process_lifecycle_shutdown_and_cancel_reap_stockfish_pids() {
        let binary = configured_stockfish();
        let identity = identity(&binary);
        let factory = NativeEngineFactory::new(binary);
        let fen = core::fen(&shakmaty::Chess::default());
        let baseline = owned_pids(&identity);

        // Phase 1: graceful shutdown of a real pool after a real depth search.
        let (pool_sender, pool_cancel) = cancellation();
        let mut ports = within(
            "acquire pool",
            ACQUIRE_WAIT,
            factory.acquire_pool(2, &pool_cancel),
        )
        .await
        .expect("spawn two real Stockfish processes");
        let spawned = claim_pids(&identity, &baseline, 2, "graceful shutdown").await;
        for port in &mut ports {
            within(
                "configure (depth)",
                CONFIGURE_WAIT,
                configure(port.as_mut(), Some(1), Some(16), 1, &pool_cancel),
            )
            .await
            .expect("configure real Stockfish");
            let result = within(
                "evaluate (depth)",
                EVALUATE_WAIT,
                evaluate(
                    port.as_mut(),
                    &fen,
                    Mode::Depth,
                    DEPTH,
                    SEARCH_TIMEOUT_MS,
                    &pool_cancel,
                ),
            )
            .await
            .expect("real depth search");
            assert!(
                result.depth >= DEPTH && !result.pv.is_empty(),
                "the engine must perform real work, not a stub reply"
            );
        }
        for port in &mut ports {
            within("shutdown (pool)", SHUTDOWN_WAIT, port.shutdown()).await;
        }
        assert_reaped(&identity, &spawned, &baseline, "graceful shutdown").await;
        drop(pool_sender);

        // Lease proxy: the adapter releases its permit only after every child
        // was reaped, so a fresh acquire proves teardown actually completed.
        let (probe_sender, probe_cancel) = cancellation();
        let mut probe = within(
            "acquire (lease probe)",
            ACQUIRE_WAIT,
            factory.acquire(&probe_cancel),
        )
        .await
        .expect("lease released after all children were reaped");
        let probe_pids = claim_pids(&identity, &baseline, 1, "lease probe").await;
        within("shutdown (probe)", SHUTDOWN_WAIT, probe.shutdown()).await;
        assert_reaped(&identity, &probe_pids, &baseline, "lease probe").await;
        drop(probe_sender);

        // Phase 2: cancel a live `go movetime` search, then tear the process down.
        let (sender, cancel) = cancellation();
        let mut port = within(
            "acquire (live search)",
            ACQUIRE_WAIT,
            factory.acquire(&cancel),
        )
        .await
        .expect("spawn Stockfish for the cancellation case");
        let cancelled_pids = claim_pids(&identity, &baseline, 1, "live search").await;
        within(
            "configure (live search)",
            CONFIGURE_WAIT,
            configure(port.as_mut(), Some(1), Some(16), 1, &cancel),
        )
        .await
        .expect("configure Stockfish for the live search");

        let (mut observer, progress) = ObservingPort::wrap(port);
        let running_cancel = cancel.clone();
        // Cancellation is owned by this watcher and happens only after it sees
        // `info` output — deterministic proof that `go movetime` was processed.
        let watcher = tokio::spawn(async move {
            match tokio::time::timeout(EVIDENCE_WAIT, wait_for_evidence(progress)).await {
                Ok(Some(state)) => {
                    sender.send_replace(true);
                    state
                }
                Ok(None) => panic!("progress observer dropped before go movetime evidence"),
                Err(_) => panic!(
                    "go movetime evidence (info output) not observed within {EVIDENCE_WAIT:?}"
                ),
            }
        });
        let outcome = within(
            "evaluate (live)",
            EVALUATE_WAIT,
            evaluate(
                &mut observer,
                &fen,
                Mode::Time,
                CANCELLED_MOVETIME_MS,
                LIVE_TIMEOUT_MS,
                &running_cancel,
            ),
        )
        .await;
        let evidence = within("evidence watcher", EVALUATE_WAIT, watcher).await;
        within("shutdown (live)", SHUTDOWN_WAIT, observer.shutdown()).await;
        // Propagates a panic (and fails the test) if proof never arrived.
        let evidence = evidence.expect("evidence watcher task");
        assert!(
            evidence.go_sent && evidence.info_seen,
            "cancellation must be gated by observed go movetime evidence"
        );
        assert_eq!(
            outcome
                .expect_err("live search must observe cancellation")
                .code,
            ReviewErrorCode::Cancelled
        );
        assert_reaped(
            &identity,
            &cancelled_pids,
            &baseline,
            "cancellation teardown",
        )
        .await;

        assert!(
            owned_pids(&identity).is_subset(&baseline),
            "no Stockfish process may remain beyond the original baseline"
        );
    }
}

/// `/proc` does not exist off Linux. An explicit ignored test that fails when
/// requested prevents a zero-test run from being mistaken for a pass.
#[cfg(not(target_os = "linux"))]
#[tokio::test]
#[ignore = "Linux-only: this suite reads /proc; a skip here is NOT a pass"]
async fn real_process_lifecycle_is_linux_only() {
    panic!(
        "real_process_lifecycle requires Linux (/proc); \
         ignoring it on this platform is not a passing result"
    );
}
