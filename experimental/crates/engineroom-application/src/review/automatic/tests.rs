use super::*;

#[tokio::test]
async fn closed_worker_reply_requires_actual_cancellation_and_preserves_failure() {
    let (closed, receiver) = tokio::sync::watch::channel(false);
    let cancel = Cancellation {
        closed: receiver,
        revision: None,
    };
    let (sender, reply) = tokio::sync::oneshot::channel::<()>();
    drop(sender);
    let error = reply
        .await
        .map_err(|_| disconnected_worker_error(None, &cancel))
        .unwrap_err();
    assert_eq!(error.code, ReviewErrorCode::EngineExited);

    closed.send_replace(true);
    assert_eq!(
        disconnected_worker_error(None, &cancel).code,
        ReviewErrorCode::Cancelled
    );
    let failure = ReviewError::new(
        ReviewErrorCode::EngineProtocol,
        "test.worker",
        "original failure",
    );
    let error = disconnected_worker_error(Some(failure), &cancel);
    assert_eq!(error.code, ReviewErrorCode::EngineProtocol);
    assert_eq!(error.message, "original failure");
}

#[test]
fn missing_baseline_is_an_error_even_with_a_refinement() {
    let game = core::extract("1. e4").unwrap();
    let state = State {
        game: &game,
        profile: adaptive::profile(AnalysisKind::Fast).unwrap().unwrap(),
        sizing: None,
        phases: core::phases(&game.fens),
        terminals: vec![None; 2],
        baseline: vec![None; 2],
        refined: vec![Some(core::terminal_raw(&game.fens[0], 0)), None],
        budgets: vec![None; 2],
        critical: vec![None],
        book: 0,
        cached: 0,
        searched: 0,
        refinement_stage: false,
        final_targets: None,
        ready_targets: vec![],
    };
    for index in [0, 1, 2] {
        let error = state.evaluation(index).unwrap_err();
        assert_eq!(error.code, ReviewErrorCode::MissingEvaluation);
        assert_eq!(error.operation, "review.triage");
    }
}

#[tokio::test]
async fn native_review_reuses_exact_fen_before_flush_and_publishes_both_plies() {
    use crate::review::tests::{FakeState, MemoryRepo, factory};

    // A validated game normally has unique FENs because its move clocks advance.
    // Duplicate a position explicitly to exercise two consumers of one search.
    let mut game = core::extract("1. Nf3 Nf6 2. Ng1 Ng8").unwrap();
    game.fens[4] = game.fens[0].clone();
    let profile = adaptive::profile(AnalysisKind::Fast).unwrap().unwrap();
    let fen = game.fens[0].clone();
    let mut raw = core::terminal_raw(&fen, 0);
    raw.lines = ["e2e4", "d2d4", "g1f3"]
        .into_iter()
        .enumerate()
        .map(|(i, uci)| RawLine {
            multipv: i as u32 + 1,
            cp: 0,
            depth: Some(10),
            pv: vec![uci.into()],
            unstable: false,
            san: None,
        })
        .collect();
    let factory = factory(FakeState {
        scores: [(fen.clone(), raw)].into(),
        ..FakeState::default()
    });
    let repo = MemoryRepo::default();
    let (_closed, receiver) = tokio::sync::watch::channel(false);
    let cancel = Cancellation {
        closed: receiver,
        revision: None,
    };
    let mut published = Vec::new();
    let (_live_tx, mut demands) = mpsc::channel(1);
    let live = NativeLive {
        cancel: &cancel,
        demands: &mut demands,
    };
    let result = review_native(
        factory.as_ref(),
        &repo,
        &game,
        profile,
        None,
        live,
        &mut |event| {
            if let Event::Progress { progress } = event
                && progress.stage == "triage"
                && let Some(update) = progress.update
            {
                if update.index == 4 {
                    assert!(repo.writes.lock().unwrap().is_empty());
                }
                published.push(update.index);
            }
        },
    )
    .await
    .unwrap();
    assert_eq!(result.positions.len(), 5);
    published.sort_unstable();
    assert_eq!(published, [0, 1, 2, 3, 4]);
    let state = factory.state.lock().unwrap();
    let fen_command = format!("position fen {fen}");
    let go_command = format!("go movetime {}", profile.triage_ms);
    assert_eq!(
        state
            .sent
            .windows(2)
            .filter(|pair| pair[0] == fen_command && pair[1] == go_command)
            .count(),
        1,
    );
}

#[tokio::test]
async fn native_refinement_shares_stronger_evidence_without_overwriting_triage() {
    use crate::review::tests::{FakeState, MemoryRepo, factory};

    let pgn = "[FEN \"7k/8/8/8/8/8/8/R6K w - - 0 1\"]\n1. Ra2 Kh7 2. Ra1 Kh8 3. Ra2";
    let mut game = core::extract(pgn).unwrap();
    game.fens[4] = game.fens[0].clone();
    game.fens[5] = game.fens[1].clone();
    let fen = &game.fens[0];
    let scored = |moves: &[&str], cp, depth| {
        let mut raw = core::terminal_raw(fen, cp);
        raw.depth = depth;
        raw.lines = moves
            .iter()
            .enumerate()
            .map(|(index, &uci)| RawLine {
                multipv: index as u32 + 1,
                cp,
                depth: Some(depth),
                pv: vec![uci.into()],
                unstable: false,
                san: None,
            })
            .collect();
        raw
    };
    let factory = factory(FakeState {
        scores: [
            (fen.clone(), scored(&["a1a2", "a1a3", "a1a4"], 700, 10)),
            (game.fens[1].clone(), core::terminal_raw(&game.fens[1], 0)),
        ]
        .into(),
        refined_scores: [(fen.clone(), scored(&["a1a8", "a1a7"], 800, 20))].into(),
        ..FakeState::default()
    });
    let (_closed, receiver) = tokio::sync::watch::channel(false);
    let cancel = Cancellation {
        closed: receiver,
        revision: None,
    };
    let profile = adaptive::profile(AnalysisKind::Fast).unwrap().unwrap();
    let (_live_tx, mut demands) = mpsc::channel(1);
    let live = NativeLive {
        cancel: &cancel,
        demands: &mut demands,
    };
    let result = review_native(
        factory.as_ref(),
        &MemoryRepo::default(),
        &game,
        profile,
        None,
        live,
        &mut |_| {},
    )
    .await
    .unwrap();

    for index in [0, 4] {
        let position = &result.positions[index];
        assert_eq!(position.depth, 20, "ply {index}");
        assert_eq!(position.lines[0].pv[0], "a1a8", "ply {index}");
        assert_eq!(
            position.triage_lines.as_ref().unwrap()[0].pv[0],
            "a1a2",
            "ply {index}"
        );
    }
    let fen_command = format!("position fen {fen}");
    let state = factory.state.lock().unwrap();
    let budgets: Vec<_> = state
        .sent
        .windows(2)
        .filter_map(|pair| (pair[0] == fen_command).then_some(pair[1].as_str()))
        .collect();
    assert_eq!(budgets, ["go movetime 180", "go movetime 2000"]);
}

#[tokio::test]
async fn pending_navigation_attaches_to_matching_search_or_queues_stronger_budget() {
    for demand in [Demand::time(180, 1), Demand::time(500, 3)] {
        let (sender, mut receiver) = mpsc::channel(1);
        let (reply, mut result) = oneshot::channel();
        let fen = core::extract("1. e4").unwrap().fens[0].clone();
        sender
            .try_send(LiveDemand {
                fen: fen.clone(),
                value: demand.value,
                multipv: demand.multipv,
                reply,
            })
            .unwrap();
        let mut native = Native {
            scheduler: Scheduler::default(),
            demands: &mut receiver,
            latest: None,
            receiving: true,
            live_running: false,
            latest_dispatched: false,
        };
        native.refresh(None);
        native.scheduler.reserve(&fen);
        native.resolve_live();
        assert!(native.next_live(1).is_none());
        assert!(matches!(
            result.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));

        let mut raw = core::terminal_raw(&fen, 0);
        raw.depth = 10;
        native.scheduler.complete(Demand::time(180, 1), &raw);
        native.resolve_live();
        if demand.value == 180 {
            assert_eq!(result.try_recv().unwrap().unwrap().fen, fen);
            assert!(native.next_live(1).is_none());
        } else {
            let job = native
                .next_live(1)
                .expect("stronger demand is still pending");
            assert_eq!(job.demand.value, 500);
            assert_eq!(job.demand.multipv, 3);
        }
    }
}

#[test]
fn superseded_running_navigation_detaches_and_replacement_requires_coverage() {
    let (sender, mut receiver) = mpsc::channel(1);
    let fen = core::extract("1. e4").unwrap().fens[0].clone();
    let mut native = Native {
        scheduler: Scheduler::default(),
        demands: &mut receiver,
        latest: None,
        receiving: true,
        live_running: false,
        latest_dispatched: false,
    };
    let (reply, mut original) = oneshot::channel();
    native.accept(LiveDemand {
        fen: fen.clone(),
        value: 500,
        multipv: 2,
        reply,
    });
    let first = native.next_live(0).unwrap();
    native.scheduler.reserve(&fen);

    let (reply, mut replacement) = oneshot::channel();
    sender
        .try_send(LiveDemand {
            fen: fen.clone(),
            value: 500,
            multipv: 2,
            reply,
        })
        .unwrap();
    native.refresh(None);
    assert!(matches!(
        original.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
    ));
    assert!(!native.scheduler.available(&fen));
    assert!(native.next_live(1).is_none());

    let raw = core::terminal_raw(&fen, 0);
    native.scheduler.complete(first.demand, &raw);
    native.complete_live(raw);
    native.resolve_live();
    assert!(matches!(
        replacement.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert!(native.scheduler.available(&fen));
    assert!(native.next_live(1).is_some());
}

#[tokio::test]
async fn native_failure_and_cancellation_drain_the_pool_before_detaching_live() {
    use crate::review::tests::{FakeState, MemoryRepo, factory};

    for closing in [false, true] {
        let game = core::extract("1. e4").unwrap();
        let factory = factory(FakeState {
            fail_at: (!closing).then_some(1),
            stall: closing,
            ..FakeState::default()
        });
        let repo = MemoryRepo::default();
        let (closed, receiver) = tokio::sync::watch::channel(false);
        let cancel = Cancellation {
            closed: receiver,
            revision: None,
        };
        let (sender, mut demands) = mpsc::channel(1);
        let (reply, result) = oneshot::channel();
        sender
            .try_send(LiveDemand {
                fen: game.fens[0].clone(),
                value: 500,
                multipv: 2,
                reply,
            })
            .unwrap();

        let profile = adaptive::profile(AnalysisKind::Fast).unwrap().unwrap();
        let verify = async {
            if closing {
                while factory.state.lock().unwrap().searches == 0 {
                    tokio::task::yield_now().await;
                }
                closed.send_replace(true);
            }
            assert!(result.await.is_err());
            assert_eq!(factory.state.lock().unwrap().stopped, 1);
            assert_eq!(factory.permit.available_permits(), 1);
        };
        let review = async {
            review_native(
                factory.as_ref(),
                &repo,
                &game,
                profile,
                None,
                NativeLive {
                    cancel: &cancel,
                    demands: &mut demands,
                },
                &mut |_| {},
            )
            .await
            .unwrap_err()
        };

        let (error, ()) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            tokio::join!(review, verify)
        })
        .await
        .unwrap();
        assert_eq!(
            error.code,
            if closing {
                ReviewErrorCode::Cancelled
            } else {
                ReviewErrorCode::EngineExited
            }
        );
        assert_eq!(factory.state.lock().unwrap().searches, 1);
        assert!(repo.writes.lock().unwrap().is_empty());
    }
}
