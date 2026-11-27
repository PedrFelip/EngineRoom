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

#[tokio::test]
async fn native_and_pipeline_schedulers_produce_identical_reviews_while_native_serves_live_earlier_and_deduplicates_fens()
 {
    use crate::review::engine::{Cancellation, EngineFactory, EnginePort, Task};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::sync::Notify;

    struct GatedPort {
        inner: Box<dyn EnginePort>,
        gate: Option<Arc<Notify>>,
        entered: Arc<AtomicUsize>,
        open: Arc<AtomicBool>,
    }

    impl EnginePort for GatedPort {
        fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, Result<()>> {
            Box::pin(async move {
                if command.starts_with("go ")
                    && let Some(gate) = self.gate.take()
                {
                    self.entered.fetch_add(1, Ordering::SeqCst);
                    if !self.open.load(Ordering::SeqCst) {
                        gate.notified().await;
                    }
                }
                self.inner.send(command).await
            })
        }

        fn next(&mut self) -> Task<'_, Result<String>> {
            self.inner.next()
        }

        fn shutdown(&mut self) -> Task<'_, ()> {
            self.inner.shutdown()
        }
    }

    struct GatedFactory {
        inner: Arc<crate::review::tests::FakeFactory>,
        gate: Arc<Notify>,
        entered: Arc<AtomicUsize>,
        open: Arc<AtomicBool>,
    }

    impl EngineFactory for GatedFactory {
        fn acquire<'a>(
            &'a self,
            cancel: &'a Cancellation,
        ) -> Task<'a, Result<Box<dyn EnginePort>>> {
            Box::pin(async move {
                let port = self.inner.acquire(cancel).await?;
                Ok(Box::new(GatedPort {
                    inner: port,
                    gate: Some(self.gate.clone()),
                    entered: self.entered.clone(),
                    open: self.open.clone(),
                }) as Box<dyn EnginePort>)
            })
        }
    }

    let pgn = "1. e4 e5 2. Nf3 Nc6 3. d4 exd4 4. Nxd4 Nf6";
    let game = super::super::core::extract(pgn).unwrap();
    assert_eq!(game.fens.len(), 9);
    // Ply 7 (`4. Nxd4`) resets halfmove clock to 0; changing fullmove counter
    // creates an equivalent 5-field FEN that shares evidence only in Native mode.
    let live_fen = game.fens[7].replacen(" 0 4", " 0 99", 1);
    assert_ne!(live_fen, game.fens[7]);

    let make_raw = |fen: &str, cp: i32| RawPosition {
        fen: fen.into(),
        cp,
        depth: 12,
        pv: vec!["e2e4".into()],
        lines: ["e2e4", "d2d4", "g1f3"]
            .into_iter()
            .enumerate()
            .map(|(i, mv)| RawLine {
                multipv: i as u32 + 1,
                cp: cp - (i as i32 * 10),
                depth: Some(12),
                pv: vec![mv.into()],
                unstable: false,
                san: None,
            })
            .collect(),
    };

    let mut scores = std::collections::HashMap::new();
    for (ply, fen) in game.fens.iter().enumerate() {
        let cp = if ply % 2 == 0 { 25 } else { -25 };
        scores.insert(fen.clone(), make_raw(fen, cp));
    }
    scores.insert(live_fen.clone(), make_raw(&live_fen, -25));

    let run_mode = |mode: SchedulerMode| {
        let scores = scores.clone();
        let live_fen = live_fen.clone();
        async move {
            let fake = factory(FakeState {
                scores,
                ..Default::default()
            });
            let gate = Arc::new(Notify::new());
            let entered = Arc::new(AtomicUsize::new(0));
            let open = Arc::new(AtomicBool::new(false));
            let gated = Arc::new(GatedFactory {
                inner: fake.clone(),
                gate: gate.clone(),
                entered: entered.clone(),
                open: open.clone(),
            });
            let manager = ReviewSessions::new(
                gated,
                Arc::new(MemoryRepo::default()),
                Some(Resources {
                    threads: 4,
                    memory_mb: 2048,
                }),
            )
            .with_scheduler(mode);
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            manager
                .open(
                    "compare",
                    "window",
                    config(pgn, AnalysisKind::Fast),
                    move |event| tx.send(event).is_ok(),
                )
                .unwrap();

            tokio::time::timeout(Duration::from_secs(3), async {
                while entered.load(Ordering::SeqCst) == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("review should enter initial worker search");

            let mut live_settings = settings();
            live_settings.fast_pass = false;
            live_settings.search_seconds = 1;
            live_settings.lines = 3;
            manager
                .analyze_position("compare", "window", 1, request(&live_fen), live_settings)
                .unwrap();

            let mut completed_result = None;
            let mut live_analysis = None;
            let mut live_before_completed = None;
            if mode == SchedulerMode::Native {
                tokio::time::timeout(Duration::from_secs(3), async {
                    loop {
                        let envelope = rx.recv().await.unwrap();
                        if matches!(envelope.event, Event::LiveStarted { .. }) {
                            break;
                        }
                    }
                })
                .await
                .expect("native mode should accept live navigation while workers are gated");
                for _ in 0..8 {
                    tokio::task::yield_now().await;
                }
            }
            open.store(true, Ordering::SeqCst);
            gate.notify_waiters();

            tokio::time::timeout(Duration::from_secs(3), async {
                while completed_result.is_none() || live_analysis.is_none() {
                    let envelope = rx.recv().await.unwrap();
                    match envelope.event {
                        Event::Completed { result } => {
                            if live_before_completed.is_none() {
                                live_before_completed = Some(false);
                            }
                            completed_result = Some(result);
                        }
                        Event::LiveCompleted { analysis, .. } => {
                            if live_before_completed.is_none() {
                                live_before_completed = Some(true);
                            }
                            live_analysis = Some(analysis);
                        }
                        Event::Error { error, .. } => panic!("unexpected session error: {error}"),
                        _ => {}
                    }
                }
            })
            .await
            .expect("review and live navigation should both complete");

            manager.close("compare", "window").await.unwrap();
            let searches = fake.state.lock().unwrap().searches;
            (
                completed_result.unwrap(),
                live_analysis.unwrap(),
                live_before_completed.unwrap(),
                searches,
            )
        }
    };

    let (pipeline_result, pipeline_live, pipeline_order, pipeline_searches) =
        run_mode(SchedulerMode::Pipeline).await;
    let (native_result, native_live, native_order, native_searches) =
        run_mode(SchedulerMode::Native).await;

    assert!(
        !pipeline_order,
        "Pipeline mode must finish the review before serving live"
    );
    assert!(
        native_order,
        "Native mode must serve live navigation before completing the review"
    );
    assert!(
        native_searches < pipeline_searches,
        "Native mode ({native_searches}) must run fewer searches than Pipeline mode ({pipeline_searches}) via 5-field FenId sharing"
    );
    assert_eq!(native_live.fen, live_fen);
    assert_eq!(pipeline_live.fen, live_fen);
    assert_eq!(native_result.positions[7].fen, game.fens[7]);
    assert_eq!(
        serde_json::to_value(&native_result).unwrap(),
        serde_json::to_value(&pipeline_result).unwrap()
    );
}
