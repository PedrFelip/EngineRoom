use super::*;

#[tokio::test]
async fn quiet_critical_move_refines_only_pair_and_flushes_before_failure() {
    let pgn = "1. a3 a6 2. h3 h6 3. f3 f6 4. g3 g6 5. Kf2 Kf7";
    for failure in [None, Some(13)] {
        let f = factory(FakeState {
            fail_at: failure,
            ..Default::default()
        });
        let repo = Arc::new(MemoryRepo::default());
        let mut pipeline = Pipeline::new(f.clone(), repo.clone());
        let (_tx, cancel) = cancel();
        let mut events = Vec::new();
        let review = pipeline
            .review(&config(pgn, AnalysisKind::Fast), None, &cancel, &mut |e| {
                events.push(e)
            })
            .await;
        if failure.is_none() {
            assert_eq!(review.unwrap().positions.len(), 11);
            let budgets: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    if let Event::Progress { progress: p } = e {
                        if p.stage == "refinement" {
                            p.remaining_budget_ms
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(budgets, vec![4000, 2000, 0]);
            assert_eq!(f.state.lock().unwrap().searches, 13);
            assert!(f
                .state
                .lock()
                .unwrap()
                .sent
                .iter()
                .all(|c| c != "go movetime 1800"));
        } else {
            assert_eq!(review.unwrap_err().message, "engine failed");
            let writes = repo.writes.lock().unwrap();
            assert_eq!(writes.iter().map(|(_, count)| count).sum::<usize>(), 11);
            assert!(writes.iter().all(|(value, _)| *value == 180));
        }
        assert_eq!(f.state.lock().unwrap().stopped, 1);
    }
}
#[tokio::test]
async fn cache_failures_preserve_root_cause_and_discard_engine() {
    for read in [true, false] {
        let f = factory(FakeState::default());
        let repo = Arc::new(MemoryRepo {
            fail_read: read,
            fail_put: !read,
            ..Default::default()
        });
        let mut pipeline = Pipeline::new(f.clone(), repo);
        let (_tx, cancel) = cancel();
        let e = pipeline
            .review(
                &config("1. e4 e5", AnalysisKind::Manual),
                None,
                &cancel,
                &mut |_| {},
            )
            .await
            .unwrap_err();
        assert_eq!(
            e.message,
            if read {
                "cache read failed"
            } else {
                "cache write failed"
            }
        );
        assert_eq!(f.state.lock().unwrap().stopped, usize::from(!read));
    }
}
#[tokio::test]
async fn interruption_discards_stalled_search_before_next_owner_can_acquire() {
    let f = factory(FakeState {
        stall: true,
        ..Default::default()
    });
    let mut pipeline = Pipeline::new(f.clone(), Arc::new(MemoryRepo::default()));
    let fen = core::extract("1. e4").unwrap().fens[0].clone();
    let (close, cancel) = cancel();
    let started = f.state.clone();
    let cancel_task = tokio::spawn(async move {
        while started.lock().unwrap().searches == 0 {
            tokio::task::yield_now().await;
        }
        close.send_replace(true);
    });
    let e = pipeline
        .live(&request(&fen), &settings(), &cancel, &mut |_| {})
        .await
        .unwrap_err();
    cancel_task.await.unwrap();
    assert_eq!(e.code, "cancelled");
    assert_eq!(f.state.lock().unwrap().stopped, 1);
    assert_eq!(f.permit.available_permits(), 1);
    f.state.lock().unwrap().stall = false;
    let (_tx, fresh) = self::cancel();
    pipeline
        .live(&request(&fen), &settings(), &fresh, &mut |_| {})
        .await
        .unwrap();
    assert_eq!(f.acquired.load(Ordering::SeqCst), 2);
    pipeline.discard().await;
}
#[tokio::test]
async fn cancelled_waiter_does_not_spawn_or_stop_another_owners_process() {
    let f = factory(FakeState::default());
    let (_tx, root) = cancel();
    let mut first = f.acquire(&root).await.unwrap();
    let (tx, second) = cancel();
    let pending = f.acquire(&second);
    tokio::pin!(pending);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut pending)
            .await
            .is_err()
    );
    tx.send_replace(true);
    assert_eq!(pending.await.err().unwrap().code, "cancelled");
    assert_eq!(f.acquired.load(Ordering::SeqCst), 1);
    assert_eq!(f.state.lock().unwrap().stopped, 0);
    first.shutdown().await;
    assert_eq!(f.permit.available_permits(), 1);
}
#[tokio::test]
async fn live_retry_playback_and_source_classification() {
    let f = factory(FakeState {
        empty_once: true,
        ..Default::default()
    });
    let mut pipeline = Pipeline::new(f.clone(), Arc::new(MemoryRepo::default()));
    let (_tx, cancel) = cancel();
    let game = core::extract("1. e4").unwrap();
    let mut settings = settings();
    settings.fast_pass = true;
    settings.move_feedback_enabled = true;
    let mut request = request(&game.fens[1]);
    request.source_fen = Some(game.fens[0].clone());
    request.variation_node_id = Some("node".into());
    let mut events = Vec::new();
    pipeline
        .live(&request, &settings, &cancel, &mut |e| events.push(e))
        .await
        .unwrap();
    assert_eq!(f.state.lock().unwrap().searches, 2);
    assert_eq!(
        f.state
            .lock()
            .unwrap()
            .sent
            .iter()
            .filter(|c| *c == "go movetime 500")
            .count(),
        2
    );
    assert!(events.iter().any(|e|matches!(e,Event::LiveCompleted {analysis:a,..} if a.search.as_ref().unwrap().purpose=="playback")));
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::Classification { .. })));
    settings.fast_pass = false;
    pipeline
        .live(&request, &settings, &cancel, &mut |e| events.push(e))
        .await
        .unwrap();
    assert!(events
        .iter()
        .any(|e| matches!(e,Event::Classification {node_id,..} if node_id=="node")));
    pipeline.discard().await;
}
#[tokio::test]
async fn uci_timeout_mate_scores_and_latest_multipv() {
    assert_eq!(
        parse_info("info depth 10 score mate 0 pv").unwrap().cp,
        -100000
    );
    assert_eq!(
        parse_info("info depth 10 score mate -3 pv").unwrap().cp,
        -99997
    );
    assert!(parse_info("info string score cp 99").is_none());
    assert!(parse_info("info depth 12 score cp 90 lowerbound pv e2e4").is_none());
    assert!(parse_info("info depth 12 score cp 90 upperbound pv e2e4").is_none());
    let f = factory(FakeState {
        stall: true,
        ..Default::default()
    });
    let (_tx, cancel) = cancel();
    let mut p = f.acquire(&cancel).await.unwrap();
    let e = evaluate(
        p.as_mut(),
        &core::fen(&shakmaty::Chess::default()),
        Mode::Time,
        1,
        10,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "engineTimeout");
    p.shutdown().await;
}

#[tokio::test]
async fn cancellation_interrupts_a_blocked_cache_lookup() {
    struct BlockedRepo(tokio::sync::Notify);
    impl Repository for BlockedRepo {
        fn lookup<'a>(
            &'a self,
            _: &'a [String],
            _: Mode,
            _: u32,
            _: u32,
        ) -> Task<'a, Result<Vec<Option<RawPosition>>>> {
            Box::pin(async move {
                self.0.notify_one();
                std::future::pending().await
            })
        }
        fn put<'a>(
            &'a self,
            _: &'a [RawPosition],
            _: Mode,
            _: u32,
            _: u32,
        ) -> Task<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }
        fn save<'a>(&'a self, _: &'a ReviewConfig, _: &'a ReviewResult) -> Task<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }
    }
    let f = factory(FakeState::default());
    let repo = Arc::new(BlockedRepo(tokio::sync::Notify::new()));
    let mut pipeline = Pipeline::new(f.clone(), repo.clone());
    let (closed, cancel) = cancel();
    let cancel_task = tokio::spawn(async move {
        repo.0.notified().await;
        closed.send_replace(true);
    });
    let fen = core::fen(&shakmaty::Chess::default());
    let error = tokio::time::timeout(
        Duration::from_secs(1),
        pipeline.live(&request(&fen), &settings(), &cancel, &mut |_| {}),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(error.code, "cancelled");
    cancel_task.await.unwrap();
    assert_eq!(f.acquired.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn candidate_search_sends_base_roots_to_engine() {
    let f = factory(FakeState::default());
    let (_tx, cancel) = cancel();
    let mut port = f.acquire(&cancel).await.unwrap();
    let roots = vec!["e2e4".into(), "d2d4".into(), "g1f3".into()];
    super::super::engine::evaluate_candidates(
        port.as_mut(),
        &core::fen(&shakmaty::Chess::default()),
        Mode::Time,
        500,
        1000,
        &cancel,
        &roots,
    )
    .await
    .unwrap();
    assert!(f
        .state
        .lock()
        .unwrap()
        .sent
        .iter()
        .any(|c| c == "go movetime 500 searchmoves e2e4 d2d4 g1f3"));
    port.shutdown().await;
}
