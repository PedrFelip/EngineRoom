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
        context: None,
        baseline_evidence: Vec::new(),
        refined_evidence: Vec::new(),
        contextual_pending: Vec::new(),
        game: &game,
        profile: adaptive::profile(AnalysisKind::Fast).unwrap().unwrap(),
        sizing: None,
        phases: core::phases(&game.fens),
        terminals: vec![None; 2],
        baseline: vec![None; 2],
        refined: vec![Some(core::terminal_raw(&game.fens[0], 0)), None],
        budgets: vec![None; 2],
        attempted: vec![None; 2],
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

    // Share positions with matching halfmove clocks even when fullmove counters
    // differ, while keeping differing halfmove clocks distinct.
    let mut game = core::extract("1. Nf3 Nf6 2. Ng1 Ng8").unwrap();
    game.fens[2] = game.fens[0].replace(" 0 1", " 2 1");
    game.positions[2] = core::position(&game.fens[2]).unwrap();
    game.fens[4] = game.fens[0].replace(" 0 1", " 0 3");
    game.positions[4] = core::position(&game.fens[4]).unwrap();
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
    for warm in [false, true] {
        let mut raw_different_clock = raw.clone();
        raw_different_clock.fen = game.fens[2].clone();
        let factory = factory(FakeState {
            scores: [
                (fen.clone(), raw.clone()),
                (game.fens[2].clone(), raw_different_clock),
            ]
            .into(),
            ..FakeState::default()
        });
        let repo = MemoryRepo::default();
        if warm {
            repo.hits.lock().unwrap().insert(fen.clone(), raw.clone());
        }
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
        for (index, position) in result.positions.iter().enumerate() {
            assert_eq!(position.fen, game.fens[index], "ply {index}");
        }
        published.sort_unstable();
        assert_eq!(published, [0, 1, 2, 3, 4]);
        let state = factory.state.lock().unwrap();
        let go_command = format!("go movetime {}", profile.triage_ms);
        for (expected_fen, count) in [(&game.fens[0], usize::from(!warm)), (&game.fens[2], 1)] {
            let fen_command = format!("position fen {expected_fen}");
            assert_eq!(
                state
                    .sent
                    .windows(2)
                    .filter(|pair| pair[0] == fen_command && pair[1] == go_command)
                    .count(),
                count,
            );
        }
        let shared_fen_command = format!("position fen {}", game.fens[4]);
        assert!(state.sent.iter().all(|cmd| cmd != &shared_fen_command));
    }
}

#[tokio::test]
async fn native_refinement_shares_stronger_evidence_without_overwriting_triage() {
    use crate::review::tests::{FakeState, MemoryRepo, factory};

    let pgn = "[FEN \"7k/8/8/8/8/8/8/R6K w - - 0 1\"]\n1. Ra2 Kh7 2. Ra1 Kh8 3. Ra2";
    let mut game = core::extract(pgn).unwrap();
    game.fens[4] = game.fens[0].replace(" 0 1", " 0 3");
    game.positions[4] = core::position(&game.fens[4]).unwrap();
    game.fens[5] = game.fens[1].replace(" 1 1", " 1 3");
    game.positions[5] = core::position(&game.fens[5]).unwrap();
    let fen = &game.fens[0];
    let scored = |target_fen: &str, moves: &[&str], cp, depth| {
        let mut raw = core::terminal_raw(target_fen, cp);
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
            (fen.clone(), scored(fen, &["a1a2", "a1a3", "a1a4"], 700, 10)),
            (
                game.fens[4].clone(),
                scored(&game.fens[4], &["a1a2", "a1a3", "a1a4"], 700, 10),
            ),
            (
                game.fens[1].clone(),
                scored(&game.fens[1], &["h8g8", "h8g7", "h8h7"], 0, 10),
            ),
            (
                game.fens[5].clone(),
                scored(&game.fens[5], &["h8g8", "h8g7", "h8h7"], 0, 10),
            ),
        ]
        .into(),
        refined_scores: [
            (fen.clone(), scored(fen, &["a1a8", "a1a7"], 800, 20)),
            (
                game.fens[4].clone(),
                scored(&game.fens[4], &["a1a8", "a1a7"], 800, 20),
            ),
        ]
        .into(),
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

    for (index, position) in result.positions.iter().enumerate() {
        assert_eq!(position.fen, game.fens[index], "ply {index}");
    }
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
    let shared_commands = [
        format!("position fen {}", game.fens[0]),
        format!("position fen {}", game.fens[4]),
    ];
    let state = factory.state.lock().unwrap();
    let budgets: Vec<_> = state
        .sent
        .windows(2)
        .filter_map(|pair| {
            shared_commands
                .contains(&pair[0])
                .then_some(pair[1].as_str())
        })
        .collect();
    assert_eq!(budgets, ["go movetime 180", "go movetime 2000"]);
}

#[tokio::test]
async fn pending_navigation_attaches_to_matching_search_or_queues_stronger_budget() {
    for demand in [Demand::time(180, 1), Demand::time(500, 3)] {
        let (sender, mut receiver) = mpsc::channel(1);
        let (reply, mut result) = oneshot::channel();
        let game = core::extract("1. e4").unwrap();
        let fen = game.fens[0].clone();
        sender
            .try_send(LiveDemand {
                fen: fen.clone(),
                value: demand.value,
                multipv: demand.multipv,
                reply,
            })
            .unwrap();
        let mut native = Native {
            scheduler: Scheduler::new(&game),
            demands: &mut receiver,
            latest: None,
            receiving: true,
            live_running: false,
            latest_dispatched: false,
        };
        native.refresh(None);
        let id = native.scheduler.ply_id(0).unwrap();
        native.scheduler.reserve(id);
        native.resolve_live();
        assert!(native.next_live(1).is_none());
        assert!(matches!(
            result.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));

        let mut raw = core::terminal_raw(&fen, 0);
        raw.depth = 10;
        native.scheduler.complete(id, Demand::time(180, 1), &raw);
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
    let game = core::extract("1. e4").unwrap();
    let fen = game.fens[0].clone();
    let mut native = Native {
        scheduler: Scheduler::new(&game),
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
    let id = native.scheduler.ply_id(0).unwrap();
    native.scheduler.reserve(id);

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
    assert!(!native.scheduler.available(id));
    assert!(native.next_live(1).is_none());

    let raw = core::terminal_raw(&fen, 0);
    native.scheduler.complete(id, first.demand, &raw);
    native.complete_live(raw);
    native.resolve_live();
    assert!(matches!(
        replacement.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert!(native.scheduler.available(id));
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

#[tokio::test]
async fn fully_cached_native_review_leaves_fen_index_uninitialized() {
    use crate::review::tests::{FakeState, MemoryRepo, factory};

    let game = core::extract("1. e4 e5").unwrap();
    let repo = MemoryRepo {
        hits: game
            .fens
            .iter()
            .map(|fen| (fen.clone(), core::terminal_raw(fen, 0)))
            .collect::<std::collections::HashMap<_, _>>()
            .into(),
        ..MemoryRepo::default()
    };
    let factory = factory(FakeState::default());
    let (_closed, receiver) = tokio::sync::watch::channel(false);
    let cancel = Cancellation {
        closed: receiver,
        revision: None,
    };
    let (_live_tx, mut demands) = mpsc::channel(1);
    let mut native = Native {
        scheduler: Scheduler::new(&game),
        demands: &mut demands,
        latest: None,
        receiving: true,
        live_running: false,
        latest_dispatched: false,
    };
    run_review(
        Resources {
            factory: factory.as_ref(),
            repository: &repo,
            cancel: &cancel,
        },
        &game,
        adaptive::profile(AnalysisKind::Fast).unwrap().unwrap(),
        None,
        &mut |_| {},
        Some(&mut native),
    )
    .await
    .unwrap();

    assert!(native.scheduler.ply_id(0).is_none());
    assert!(factory.state.lock().unwrap().sent.is_empty());
}

#[test]
fn only_valid_nonterminal_live_demand_initializes_fen_index() {
    let game = core::extract("1. e4").unwrap();
    let (_sender, mut demands) = mpsc::channel(1);
    let mut native = Native {
        scheduler: Scheduler::new(&game),
        demands: &mut demands,
        latest: None,
        receiving: true,
        live_running: false,
        latest_dispatched: false,
    };
    let mut request = |fen: &str| {
        let (reply, result) = oneshot::channel();
        native.accept(LiveDemand {
            fen: fen.into(),
            value: 180,
            multipv: 1,
            reply,
        });
        (
            result,
            native.scheduler.ply_id(0),
            native.latest.as_ref().map(|live| live.id),
        )
    };

    let (mut malformed, mainline, live) = request("invalid\n");
    assert!(malformed.try_recv().unwrap().is_err());
    assert!(mainline.is_none() && live.is_none());

    let (mut terminal, mainline, live) = request("7k/8/8/8/8/8/8/K7 w - - 0 1");
    assert_eq!(terminal.try_recv().unwrap().unwrap().cp, 0);
    assert!(mainline.is_none() && live.is_none());

    let off_mainline = core::extract("1. d4").unwrap();
    let (_result, mainline, live) = request(&off_mainline.fens[1]);
    assert!(mainline.is_some() && live.is_some());
    assert_ne!(mainline, live);
    let (_replacement, _, repeated) = request(&off_mainline.fens[1]);
    assert_eq!(live, repeated);
}

#[test]
fn sqlite_time_misses_only_rule_out_stricter_demands_on_the_same_ply() {
    let budget = |ms, multipv| SearchBudget { ms, multipv };
    let mut misses = CacheMisses::default();
    misses.record(0, budget(100, 3), 2);
    misses.record(0, budget(200, 1), 2);
    assert!(misses.rules_out(0, budget(150, 3)));
    assert!(misses.rules_out(0, budget(300, 2)));
    assert!(!misses.rules_out(0, budget(150, 2)));
    assert!(!misses.rules_out(1, budget(300, 3)));

    misses.record(0, budget(100, 1), 2);
    assert_eq!(misses.0[0], [budget(100, 1)]);
    assert!(misses.rules_out(0, budget(150, 2)));
}

#[test]
fn live_demand_canonicalizes_pseudo_legal_en_passant_and_preserves_six_field_fen() {
    let game = core::extract("1. Nf3 Nf6 2. Ng1 Ng8 3. e4").unwrap();
    let (_sender, mut demands) = mpsc::channel(1);
    let mut native = Native {
        scheduler: Scheduler::new(&game),
        demands: &mut demands,
        latest: None,
        receiving: true,
        live_running: false,
        latest_dispatched: false,
    };
    let pseudo_ep = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    let canonical = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
    let (reply, mut result) = oneshot::channel();
    native.accept(LiveDemand {
        fen: pseudo_ep.into(),
        value: 180,
        multipv: 1,
        reply,
    });
    let mainline_id = native.scheduler.ply_id(5).unwrap();
    let pending = native.latest.as_ref().expect("live demand accepted");
    assert_eq!(pending.id, mainline_id);
    assert_eq!(pending.request.fen, canonical);
    assert_ne!(pending.request.fen, game.fens[5]);

    native.scheduler.reserve(mainline_id);
    let mut mainline_raw = core::terminal_raw(&game.fens[5], -30);
    mainline_raw.depth = 12;
    mainline_raw.lines[0].depth = Some(12);
    native
        .scheduler
        .complete(mainline_id, Demand::time(180, 1), &mainline_raw);
    native.resolve_live();
    let resolved = result.try_recv().unwrap().unwrap();
    assert_eq!(resolved.fen, canonical);
    assert_eq!(resolved.depth, 12);
}

#[tokio::test]
async fn refinement_records_effective_multipv_without_repeating_partial_searches() {
    use crate::review::tests::{FakeState, MemoryRepo, factory};

    let pgn = "[FEN \"7k/6p1/8/8/8/8/8/R5K1 w - - 0 1\"]\n1. Ra8+ Kh7";
    let game = core::extract(pgn).unwrap();
    assert!(game.positions[0].legal_moves().len() > 1);
    assert_eq!(game.positions[1].legal_moves().len(), 1);
    let profile = adaptive::profile(AnalysisKind::Fast).unwrap().unwrap();
    let single_pv = |fen: &str, uci: &str, cp, depth| {
        let mut raw = core::terminal_raw(fen, cp);
        raw.depth = depth;
        raw.pv = vec![uci.into()];
        raw.lines = vec![RawLine {
            multipv: 1,
            cp,
            depth: Some(depth),
            pv: vec![uci.into()],
            unstable: false,
            san: None,
        }];
        raw
    };

    for native_mode in [false, true] {
        let factory = factory(FakeState {
            scores: [
                (
                    game.fens[0].clone(),
                    single_pv(&game.fens[0], "a1a8", 700, 10),
                ),
                (
                    game.fens[1].clone(),
                    single_pv(&game.fens[1], "h8h7", 0, 10),
                ),
                (
                    game.fens[2].clone(),
                    single_pv(&game.fens[2], "a8a7", 0, 10),
                ),
            ]
            .into(),
            refined_scores: [
                (
                    game.fens[0].clone(),
                    single_pv(&game.fens[0], "a1a7", 750, 20),
                ),
                (
                    game.fens[1].clone(),
                    single_pv(&game.fens[1], "h8h7", 0, 20),
                ),
            ]
            .into(),
            ..FakeState::default()
        });
        let repo = MemoryRepo::default();
        let (_closed, receiver) = tokio::sync::watch::channel(false);
        let cancel = Cancellation {
            closed: receiver,
            revision: None,
        };
        let result = if native_mode {
            let (live_tx, mut demands) = mpsc::channel(1);
            let (reply, _live_rx) = oneshot::channel();
            // Queue a prior partial Live search at the refinement budget on ply 0
            // (>1 legal moves): it must not satisfy the refinement target.
            live_tx
                .try_send(LiveDemand {
                    fen: game.fens[0].clone(),
                    value: profile.high_ms,
                    multipv: profile.refinement_multipv,
                    reply,
                })
                .unwrap();
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
            .unwrap()
        } else {
            review(
                factory.as_ref(),
                &repo,
                &game,
                profile,
                None,
                &cancel,
                &mut |_| {},
            )
            .await
            .unwrap()
        };

        assert_eq!(
            result.positions[0].search.as_ref().unwrap().multipv,
            1,
            "native={native_mode}"
        );
        assert_eq!(
            result.positions[1].search.as_ref().unwrap().multipv,
            profile.refinement_multipv,
            "native={native_mode}"
        );
        let state = factory.state.lock().unwrap();
        let ply0_refinements = state
            .sent
            .windows(2)
            .filter(|pair| {
                pair[0] == format!("position fen {}", game.fens[0]) && pair[1] == "go movetime 2000"
            })
            .count();
        // In native mode, ply 0 runs once for the partial Live demand and once
        // for Purpose::Refinement(0) (not satisfied by partial Live, not looping).
        assert_eq!(
            ply0_refinements,
            if native_mode { 2 } else { 1 },
            "native={native_mode}"
        );
    }
}

#[test]
fn attempted_refinement_blocks_equal_budget_but_allows_stronger_target() {
    let game = core::extract("1. e4").unwrap();
    let profile = adaptive::profile(AnalysisKind::Fast).unwrap().unwrap();
    let mut state = State {
        context: None,
        baseline_evidence: Vec::new(),
        refined_evidence: Vec::new(),
        contextual_pending: Vec::new(),
        game: &game,
        profile,
        sizing: None,
        phases: core::phases(&game.fens),
        terminals: vec![None; 2],
        baseline: vec![None; 2],
        refined: vec![None; 2],
        budgets: vec![
            Some(SearchBudget {
                ms: 2000,
                multipv: 1,
            }),
            None,
        ],
        attempted: vec![
            Some(SearchBudget {
                ms: 2000,
                multipv: 2,
            }),
            None,
        ],
        critical: vec![None],
        book: 0,
        cached: 0,
        searched: 1,
        refinement_stage: true,
        final_targets: None,
        ready_targets: vec![],
    };
    let target = |ms, multipv| Target {
        position_index: 0,
        score: 50,
        hard: true,
        kind: adaptive::RefinementKind::Critical,
        search: SearchBudget { ms, multipv },
    };
    assert!(state.satisfied(&target(2000, 2)));
    assert!(state.satisfied(&target(1000, 2)));
    assert!(!state.satisfied(&target(6000, 2)));
    assert!(!state.satisfied(&target(2000, 3)));

    state.attempted[0] = None;
    assert!(!state.satisfied(&target(2000, 2)));
}
