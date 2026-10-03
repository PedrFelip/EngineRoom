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
