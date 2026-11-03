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
        baseline_completed: 0,
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

#[test]
fn progress_counts_satisfied_budgets_and_preserves_deeper_baseline() {
    let game = core::extract("1. e4").unwrap();
    let profile = adaptive::profile(AnalysisKind::Fast).unwrap().unwrap();
    let mut baseline = core::terminal_raw(&game.fens[0], 50);
    baseline.depth = 20;
    let mut refined = core::terminal_raw(&game.fens[0], 100);
    refined.depth = 10;
    let target = Target {
        position_index: 0,
        score: 0,
        hard: true,
        kind: adaptive::RefinementKind::Critical,
        search: SearchBudget {
            ms: 1000,
            multipv: 2,
        },
    };
    let mut state = State {
        game: &game,
        profile,
        sizing: None,
        phases: core::phases(&game.fens),
        terminals: vec![None; 2],
        baseline: vec![Some(baseline), None],
        baseline_completed: 1,
        refined: vec![Some(refined), None],
        budgets: vec![
            Some(SearchBudget {
                ms: 1000,
                multipv: 1,
            }),
            None,
        ],
        critical: vec![None],
        book: 0,
        cached: 1,
        searched: 1,
        refinement_stage: false,
        final_targets: None,
        ready_targets: vec![target],
    };
    assert_eq!(state.evaluation(0).unwrap().cp, 50);
    let mut events = Vec::new();
    state.progress(0, None, &mut |event| events.push(event));
    let Event::Progress { progress } = &events[0] else {
        panic!("expected progress")
    };
    assert_eq!(
        (progress.stage.as_str(), progress.completed, progress.total),
        ("triage", 1, 2)
    );
    assert_eq!(
        (progress.cached_positions, progress.engine_positions),
        (1, 1)
    );
    assert_eq!(
        progress.remaining_budget_ms,
        Some(u64::from(profile.triage_ms) + 1000)
    );

    state.baseline[1] = Some(core::terminal_raw(&game.fens[1], 0));
    state.baseline_completed = 2;
    state.final_targets = Some(state.ready_targets.clone());
    state.refinement_stage = true;
    state.budgets[0] = Some(SearchBudget {
        ms: 1000,
        multipv: 2,
    });
    state.progress(0, None, &mut |event| events.push(event));
    let Event::Progress { progress } = &events[1] else {
        panic!("expected progress")
    };
    assert_eq!(
        (progress.stage.as_str(), progress.completed, progress.total),
        ("refinement", 1, 1)
    );
    assert_eq!(progress.remaining_budget_ms, Some(0));
    assert_eq!(state.evaluation(0).unwrap().cp, 50);
}
