use engineroom_application::{
    db::mode::Mode,
    review::{core, engine::*, types::ReviewErrorCode},
};
use engineroom_uci::native_engine::NativeEngineFactory;
use std::time::Duration;
use tokio::sync::watch;
fn cancel() -> (watch::Sender<bool>, Cancellation) {
    let (tx, closed) = watch::channel(false);
    (
        tx,
        Cancellation {
            closed,
            revision: None,
        },
    )
}

// Explicit integration run: ENGINE_ROOM_STOCKFISH=/path/to/stockfish cargo test -p
// engineroom-uci real_native_stockfish -- --ignored
#[tokio::test]
#[ignore = "requires ENGINE_ROOM_STOCKFISH pointing to a real Stockfish executable"]
async fn real_native_stockfish_search_and_exclusive_pool_teardown() {
    let path = std::env::var_os("ENGINE_ROOM_STOCKFISH")
        .expect("set ENGINE_ROOM_STOCKFISH to an executable Stockfish path");
    let factory = NativeEngineFactory::new(path.into());
    let (_sender, cancel) = cancel();
    let mut ports = factory.acquire_pool(2, &cancel).await.expect("spawn pool");
    for port in &mut ports {
        configure(port.as_mut(), Some(1), Some(16), 1, &cancel)
            .await
            .expect("configure Stockfish");
        let fen = core::fen(&shakmaty::Chess::default());
        let result = evaluate(port.as_mut(), &fen, Mode::Depth, 4, 10000, &cancel)
            .await
            .expect("search position");
        assert!(result.depth >= 4);
        assert!(!result.pv.is_empty());
    }
    ports[0].shutdown().await;
    assert!(
        tokio::time::timeout(Duration::from_millis(30), factory.acquire(&cancel))
            .await
            .is_err()
    );
    ports[1].shutdown().await;
    let mut next = factory
        .acquire(&cancel)
        .await
        .expect("lease released after both exits");
    next.shutdown().await;
}

#[tokio::test]
async fn closed_cancellation_senders_do_not_cancel_an_active_request() {
    let (sender, cancel) = cancel();
    drop(sender);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), cancel.cancelled())
            .await
            .is_err()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn native_transport_frames_partial_output_and_drains_stderr() {
    use std::os::unix::fs::PermissionsExt;
    let path =
        std::env::temp_dir().join(format!("engineroom-engine-fixture-{}", std::process::id()));
    std::fs::write(&path, "#!/bin/sh\nprintf partial\nsleep 0.05\nprintf ' line\\n'\ni=0; while [ $i -lt 10000 ]; do printf 'diagnostic output\\n' >&2; i=$((i+1)); done\nprintf 'readyok\\n'\nwhile read line; do :; done\n").expect("write engine fixture");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
        .expect("make fixture executable");
    let factory = NativeEngineFactory::new(path.clone());
    let (_sender, cancel) = cancel();
    let mut port = factory.acquire(&cancel).await.expect("spawn fixture");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), port.next())
            .await
            .expect("partial line timeout")
            .expect("partial line"),
        "partial line"
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), port.next())
            .await
            .expect("stderr blocked process")
            .expect("ready line"),
        "readyok"
    );
    port.shutdown().await;
    std::fs::remove_file(path).expect("remove fixture");
}

#[cfg(unix)]
#[tokio::test]
async fn blocked_stdin_times_out_and_cancels_without_blocking_runtime() {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::temp_dir().join(format!("engineroom-blocked-{}", std::process::id()));
    std::fs::write(&path, "#!/bin/sh\nexec sleep 60\n").expect("write fixture");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).expect("chmod fixture");
    let factory = NativeEngineFactory::new(path.clone());
    let (sender, cancellation) = cancel();
    let mut ports = factory
        .acquire_pool(2, &cancellation)
        .await
        .expect("spawn pool");
    let command = "x".repeat(4 * 1024 * 1024);
    let error = ask(ports[0].as_mut(), &command, "readyok", 30, &cancellation)
        .await
        .expect_err("blocked write must time out");
    assert_eq!(error.code, ReviewErrorCode::EngineTimeout);
    let trigger = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        sender.send(true).expect("cancel");
    });
    let error = ask(ports[1].as_mut(), &command, "readyok", 3000, &cancellation)
        .await
        .expect_err("blocked write must cancel");
    assert_eq!(error.code, ReviewErrorCode::Cancelled);
    trigger.await.expect("cancellation task");
    let (_sender, fresh) = cancel();
    ports[0].shutdown().await;
    assert!(
        tokio::time::timeout(Duration::from_millis(30), factory.acquire(&fresh))
            .await
            .is_err(),
        "remaining child retains pool lease"
    );
    // Poll teardown once, then abandon its waiter. The spawned reaper keeps
    // ownership of the process and lease until wait completes.
    let mut shutdown = ports[1].shutdown();
    std::future::poll_fn(|cx| {
        let _ = shutdown.as_mut().poll(cx);
        std::task::Poll::Ready(())
    })
    .await;
    drop(shutdown);
    drop(ports);
    let mut next = tokio::time::timeout(Duration::from_secs(2), factory.acquire(&fresh))
        .await
        .expect("lease released")
        .expect("spawn after cleanup");
    next.shutdown().await;
    std::fs::remove_file(path).expect("remove fixture");
}
