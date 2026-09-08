use super::{consume_stdout_lines, enqueue_uci_lines, UciOutputFilter};

#[test]
fn consumes_all_complete_stdout_lines_and_preserves_partial_suffix() {
    let mut stdout = b"info depth 1\r\nbestmove e2e4\npartial".to_vec();
    let mut lines = Vec::new();

    consume_stdout_lines(&mut stdout, |line| lines.push(line));

    assert_eq!(lines, ["info depth 1", "bestmove e2e4"]);
    assert_eq!(stdout, b"partial");
}

#[test]
fn consumes_stdout_lines_across_chunk_boundaries() {
    let mut stdout = b"info depth 1".to_vec();
    let mut lines = Vec::new();

    consume_stdout_lines(&mut stdout, |line| lines.push(line));
    stdout.extend_from_slice(b" score cp 20\nbestmove e2e4\n");
    consume_stdout_lines(&mut stdout, |line| lines.push(line));

    assert_eq!(lines, ["info depth 1 score cp 20", "bestmove e2e4"]);
    assert!(stdout.is_empty());
}

#[test]
fn forwards_protocol_responses_outside_searches() {
    let mut filter = UciOutputFilter::default();

    assert_eq!(filter.on_line("uciok".into()), ["uciok"]);
    assert_eq!(filter.on_line("readyok".into()), ["readyok"]);
}

#[test]
fn compacts_verbose_search_to_the_deepest_line_per_multipv() {
    let mut filter = UciOutputFilter::default();
    filter.on_command("go depth 20");

    assert!(filter
        .on_line("info depth 10 multipv 1 score cp 12 pv e2e4".into())
        .is_empty());
    assert!(filter
        .on_line("info depth 10 multipv 2 score cp 4 pv d2d4".into())
        .is_empty());
    assert!(filter
        .on_line("info depth 11 multipv 2 score cp 8 pv c2c4".into())
        .is_empty());
    assert!(filter
        .on_line("info depth 11 multipv 1 score cp 20 pv e2e4".into())
        .is_empty());
    assert!(filter
        .on_line("info depth 11 nodes 42 nps 1000".into())
        .is_empty());

    assert_eq!(
        filter.on_line("bestmove e2e4".into()),
        [
            "info depth 11 multipv 1 score cp 20 pv e2e4",
            "info depth 11 multipv 2 score cp 8 pv c2c4",
            "bestmove e2e4",
        ]
    );
}

#[test]
fn preserves_older_multipv_lines_when_final_depth_is_partial() {
    let mut filter = UciOutputFilter::default();
    filter.on_command("go movetime 1000");

    for multipv in 1..=3 {
        assert!(filter
            .on_line(format!(
                "info depth 18 multipv {multipv} score cp 0 pv e2e4"
            ))
            .is_empty());
    }
    assert!(filter
        .on_line("info depth 19 multipv 1 score cp 12 pv d2d4".into())
        .is_empty());

    assert_eq!(
        filter.on_line("bestmove d2d4".into()),
        [
            "info depth 19 multipv 1 score cp 12 pv d2d4",
            "info depth 18 multipv 2 score cp 0 pv e2e4",
            "info depth 18 multipv 3 score cp 0 pv e2e4",
            "bestmove d2d4",
        ]
    );
}

#[test]
fn bounds_events_for_a_reproducible_verbose_trace() {
    let mut filter = UciOutputFilter::default();
    filter.on_command("go depth 100");

    for depth in 1..=100 {
        for multipv in 1..=3 {
            assert!(filter
                .on_line(format!(
                    "info depth {depth} multipv {multipv} score cp 0 pv e2e4"
                ))
                .is_empty());
        }
    }

    let output = filter.on_line("bestmove e2e4".into());
    assert_eq!(output.len(), 4, "300 infos + bestmove viram 4 eventos");
}

#[test]
fn production_batch_preserves_uci_command_order() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let lines = vec!["position fen first".to_string(), "go depth 20".to_string()];

    enqueue_uci_lines(&tx, lines.clone()).unwrap();

    assert_eq!(rx.try_recv().unwrap(), lines[0]);
    assert_eq!(rx.try_recv().unwrap(), lines[1]);
}

#[tokio::test]
async fn stop_waits_for_writer_and_terminates_reader() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::sync::{mpsc, oneshot};

    let (tx, _incoming) = mpsc::unbounded_channel();
    let (shutdown, shutdown_rx) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let (ready, started) = oneshot::channel();
    let reader_alive = Arc::new(AtomicBool::new(true));
    struct ReaderGuard(Arc<AtomicBool>);
    impl Drop for ReaderGuard {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let alive = Arc::clone(&reader_alive);
    let reader = tauri::async_runtime::spawn(async move {
        let _guard = ReaderGuard(alive);
        let _ = ready.send(());
        std::future::pending::<()>().await;
    });
    started.await.unwrap();
    let writer = tauri::async_runtime::spawn(async move {
        shutdown_rx.await.unwrap();
        released.await.unwrap();
    });
    let handle = super::EngineHandle {
        tx,
        shutdown,
        reader,
        writer,
    };
    let stopping = tokio::spawn(handle.stop());
    while reader_alive.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
    assert!(
        !stopping.is_finished(),
        "stop must await writer termination"
    );
    release.send(()).unwrap();
    stopping.await.unwrap();
    assert!(!reader_alive.load(Ordering::SeqCst));
}
