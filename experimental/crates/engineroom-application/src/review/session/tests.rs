use super::task::run_session;
use super::*;
use crate::review::tests::{FakeState, MemoryRepo, config, factory, request, settings};
use crate::review::{pipeline::Pipeline, types::*};
use std::time::Duration;

fn restored() -> ReviewConfig {
    let mut c = config("1. e4 e5", AnalysisKind::Manual);
    let game = super::super::core::extract(&c.pgn).unwrap();
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|f| super::super::core::terminal_raw(f, 0))
        .collect();
    c.initial_result = Some(super::super::core::build(&game, &raw).unwrap());
    c
}
async fn receive(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Envelope>,
    wanted: impl Fn(&Envelope) -> bool,
) -> Envelope {
    let mut seen = Vec::new();
    receive_recording(rx, wanted, &mut seen).await
}

// Keep every consumed event so callers can check for spurious errors without
// assuming which legitimate events (progress, completion, warnings) came first.
async fn receive_recording(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Envelope>,
    wanted: impl Fn(&Envelope) -> bool,
    seen: &mut Vec<Envelope>,
) -> Envelope {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let event = rx.recv().await.expect("session event channel closed");
            let matched = wanted(&event);
            seen.push(event);
            if matched {
                return seen.last().expect("event was just pushed").clone();
            }
        }
    })
    .await
    .expect("session event timeout")
}

fn assert_no_error_events(seen: &[Envelope], scenario: &str) {
    let errors: Vec<_> = seen
        .iter()
        .filter(|e| matches!(&e.event, Event::Error { .. }))
        .collect();
    assert!(
        errors.is_empty(),
        "{scenario}: spurious Event::Error: {errors:?}"
    );
}
#[tokio::test]
async fn session_intents_wait_for_their_cleanup_acknowledgments() {
    let manager = ReviewSessions::new(
        factory(FakeState::default()),
        Arc::new(MemoryRepo::default()),
        None,
    );
    let (session, finished, done) = manager.register("session", "main").unwrap();
    let owned = session.clone();
    let cancellation = tokio::spawn(async move { owned.cancel_live(7).await });
    tokio::task::yield_now().await;
    assert_eq!(*session.revision.borrow(), 7);
    assert!(!cancellation.is_finished());
    finished.send_replace(7);
    cancellation.await.unwrap().unwrap();

    let owned = session.clone();
    let closing = tokio::spawn(async move { owned.close().await });
    tokio::task::yield_now().await;
    assert!(*session.close.borrow());
    assert!(!closing.is_finished());
    done.send_replace(true);
    closing.await.unwrap();
    session.close().await;
}

#[tokio::test]
async fn only_latest_navigation_runs_and_close_finishes_all_resources() {
    let manager = ReviewSessions::new(
        factory(FakeState::default()),
        Arc::new(MemoryRepo::default()),
        None,
    );
    let (s, finished, done) = manager.register("session", "main").unwrap();
    let config = restored();
    let f = factory(FakeState::default());
    let repo = Arc::new(MemoryRepo::default());
    let fen = config.initial_result.as_ref().unwrap().positions[0]
        .fen
        .clone();
    for id in 1..=100 {
        update(
            &s,
            id,
            Some(Job {
                id,
                request: request(&fen),
                settings: settings(),
            }),
        )
        .unwrap();
    }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let owned = s.clone();
    let factory = f.clone();
    let running = tokio::spawn(async move {
        run_session(
            "session",
            &config,
            &owned,
            super::task::Completion { finished, done },
            Pipeline::new(factory, repo),
            None,
            SchedulerMode::Pipeline,
            &mut |e| tx.send(e).is_ok(),
        )
        .await;
    });
    let mut seen = Vec::new();
    let completed = receive_recording(
        &mut rx,
        |e| matches!(e.event, Event::LiveCompleted { .. }),
        &mut seen,
    )
    .await;
    assert_eq!(completed.request_id, Some(100));
    assert_no_error_events(&seen, "superseded navigation intents");
    assert_eq!(f.state.lock().unwrap().searches, 1);
    s.close.send_replace(true);
    running.await.unwrap();
    assert!(*s.done.borrow());
    assert_eq!(f.state.lock().unwrap().stopped, 1);
    assert_eq!(f.permit.available_permits(), 1);
}
#[tokio::test]
async fn replacing_live_search_waits_for_discard_and_does_not_emit_old_result() {
    let manager = ReviewSessions::new(
        factory(FakeState::default()),
        Arc::new(MemoryRepo::default()),
        None,
    );
    let (s, finished, done) = manager.register("session", "main").unwrap();
    let config = restored();
    let fen = config.initial_result.as_ref().unwrap().positions[0]
        .fen
        .clone();
    let f = factory(FakeState {
        stall: true,
        ..Default::default()
    });
    update(
        &s,
        1,
        Some(Job {
            id: 1,
            request: request(&fen),
            settings: settings(),
        }),
    )
    .unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let owned = s.clone();
    let factory = f.clone();
    let running = tokio::spawn(async move {
        run_session(
            "session",
            &config,
            &owned,
            super::task::Completion { finished, done },
            Pipeline::new(factory, Arc::new(MemoryRepo::default())),
            None,
            SchedulerMode::Pipeline,
            &mut |e| tx.send(e).is_ok(),
        )
        .await;
    });
    let mut seen = Vec::new();
    receive_recording(
        &mut rx,
        |e| matches!(e.event, Event::LiveStarted { .. }),
        &mut seen,
    )
    .await;
    while f.state.lock().unwrap().searches == 0 {
        tokio::task::yield_now().await;
    }
    f.state.lock().unwrap().stall = false;
    update(
        &s,
        2,
        Some(Job {
            id: 2,
            request: request(&fen),
            settings: settings(),
        }),
    )
    .unwrap();
    let event = receive_recording(
        &mut rx,
        |e| matches!(e.event, Event::LiveCompleted { .. }),
        &mut seen,
    )
    .await;
    assert_eq!(event.request_id, Some(2));
    assert_no_error_events(&seen, "live search replacement");
    assert_eq!(f.state.lock().unwrap().stopped, 1);
    assert_eq!(f.acquired.load(std::sync::atomic::Ordering::SeqCst), 2);
    s.close.send_replace(true);
    running.await.unwrap();
    assert_eq!(f.state.lock().unwrap().stopped, 2);
}
#[tokio::test]
async fn provisional_progress_handles_maximum_untrusted_ply_count() {
    let manager = ReviewSessions::new(
        factory(FakeState::default()),
        Arc::new(MemoryRepo::default()),
        None,
    );
    let (session, finished, done) = manager.register("session", "main").unwrap();
    let mut config = config("1. e4", AnalysisKind::Manual);
    config.meta.plies = usize::MAX;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let owned = session.clone();
    let running = tokio::spawn(async move {
        run_session(
            "session",
            &config,
            &owned,
            super::task::Completion { finished, done },
            Pipeline::new(
                factory(FakeState::default()),
                Arc::new(MemoryRepo::default()),
            ),
            None,
            SchedulerMode::Pipeline,
            &mut |event| tx.send(event).is_ok(),
        )
        .await;
    });
    let preparing = receive(&mut rx, |event| {
        matches!(&event.event, Event::Progress { progress } if progress.stage == "preparing")
    })
    .await;
    assert!(
        matches!(preparing.event, Event::Progress { progress } if progress.total == usize::MAX)
    );
    let completed = receive(&mut rx, |event| {
        matches!(event.event, Event::Completed { .. })
    })
    .await;
    assert!(matches!(completed.event, Event::Completed { result } if result.positions.len() == 2));
    session.close.send_replace(true);
    running
        .await
        .expect("session must complete without a metadata overflow");
    assert!(*session.done.borrow());
}

#[tokio::test]
async fn persistence_failure_does_not_invalidate_completed_review() {
    let manager = ReviewSessions::new(
        factory(FakeState::default()),
        Arc::new(MemoryRepo::default()),
        None,
    );
    let (s, finished, done) = manager.register("session", "main").unwrap();
    let f = factory(FakeState::default());
    let repo = Arc::new(MemoryRepo {
        fail_save: true,
        ..Default::default()
    });
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let owned = s.clone();
    let factory = f.clone();
    let running = tokio::spawn(async move {
        run_session(
            "session",
            &config("1. e4", AnalysisKind::Manual),
            &owned,
            super::task::Completion { finished, done },
            Pipeline::new(factory, repo),
            None,
            SchedulerMode::Pipeline,
            &mut |e| tx.send(e).is_ok(),
        )
        .await;
    });
    let mut seen = Vec::new();
    receive_recording(
        &mut rx,
        |e| matches!(e.event, Event::Completed { .. }),
        &mut seen,
    )
    .await;
    let warning = receive_recording(
        &mut rx,
        |e| matches!(e.event, Event::Warning { .. }),
        &mut seen,
    )
    .await;
    assert!(
        matches!(warning.event,Event::Warning {error:e} if e.code==ReviewErrorCode::Persistence)
    );
    assert_no_error_events(&seen, "best-effort history save");
    s.close.send_replace(true);
    running.await.unwrap();
    assert!(*s.done.borrow());
}

#[tokio::test]
async fn public_open_then_close_acknowledges_cleanup_and_shutdown() {
    let f = factory(FakeState {
        stall: true,
        ..Default::default()
    });
    let manager = ReviewSessions::new(f.clone(), Arc::new(MemoryRepo::default()), None);
    manager
        .open(
            "opening",
            "window",
            config("1. e4", AnalysisKind::Manual),
            |_| true,
        )
        .unwrap();
    manager.close("opening", "window").await.unwrap();
    manager.close("opening", "window").await.unwrap();
    assert_eq!(f.permit.available_permits(), 1);
    manager.shutdown().await;
    assert!(
        manager
            .open(
                "later",
                "window",
                config("1. e4", AnalysisKind::Manual),
                |_| true
            )
            .is_err()
    );
}

#[tokio::test]
async fn stale_intents_cannot_replace_the_latest_request() {
    let manager = ReviewSessions::new(
        factory(FakeState::default()),
        Arc::new(MemoryRepo::default()),
        None,
    );
    let (session, _, done) = manager.register("session", "owner").unwrap();
    let fen = restored().initial_result.unwrap().positions[0].fen.clone();
    manager
        .analyze_position("session", "owner", 9, request(&fen), settings())
        .unwrap();
    manager
        .analyze_position("session", "owner", 3, request(&fen), settings())
        .unwrap();
    assert_eq!(*session.revision.borrow(), 9);
    assert_eq!(session.latest.lock().unwrap().as_ref().unwrap().id, 9);
    assert!(
        manager
            .analyze_position("session", "other", 10, request(&fen), settings())
            .is_err()
    );
    done.send_replace(true);
    manager.close("session", "owner").await.unwrap();
}

#[tokio::test]
async fn native_navigation_detaches_from_an_active_review_without_releasing_its_lease() {
    use std::sync::atomic::Ordering;

    let f = factory(FakeState {
        stall: true,
        ..Default::default()
    });
    let manager = ReviewSessions::new(
        f.clone(),
        Arc::new(MemoryRepo::default()),
        Some(Resources {
            threads: 1,
            memory_mb: 2048,
        }),
    );
    assert_eq!(manager.scheduler(), SchedulerMode::Native);
    let pgn = "1. f3 e5 2. g4 Qh4#";
    let game = super::super::core::extract(pgn).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    manager
        .open(
            "native",
            "window",
            config(pgn, AnalysisKind::Fast),
            move |event| tx.send(event).is_ok(),
        )
        .unwrap();

    tokio::time::timeout(Duration::from_secs(3), async {
        while f.state.lock().unwrap().searches == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the review should start a search");

    manager
        .analyze_position("native", "window", 1, request(&game.fens[0]), settings())
        .unwrap();
    receive(&mut rx, |event| {
        event.request_id == Some(1) && matches!(event.event, Event::LiveStarted { .. })
    })
    .await;

    tokio::time::timeout(
        Duration::from_secs(3),
        manager.cancel_live("native", "window", 2),
    )
    .await
    .expect("live detach should finish while the review engine is still busy")
    .unwrap();
    let terminal = game.fens.last().unwrap();
    manager
        .analyze_position("native", "window", 3, request(terminal), settings())
        .unwrap();
    let event = receive(&mut rx, |event| {
        matches!(
            event.event,
            Event::Completed { .. } | Event::LiveCompleted { .. }
        )
    })
    .await;
    assert_eq!(event.request_id, Some(3));
    assert!(matches!(event.event, Event::LiveCompleted { fen, .. } if fen == *terminal));
    assert_eq!(f.acquired.load(Ordering::SeqCst), 1);
    assert_eq!(f.permit.available_permits(), 0);
    {
        let state = f.state.lock().unwrap();
        assert_eq!(state.searches, 1);
        assert_eq!(state.stopped, 0);
    }

    tokio::time::timeout(Duration::from_secs(3), manager.close("native", "window"))
        .await
        .expect("close should await native worker shutdown")
        .unwrap();
    assert_eq!(f.state.lock().unwrap().stopped, 1);
    assert_eq!(f.permit.available_permits(), 1);
}
