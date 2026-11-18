use super::{Appearance, HistoryOperation, ImportSource, State};
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

#[test]
fn activating_missing_workspace_keeps_current_session_and_history_request() {
    let mut state = State::fixture();
    state.session.open("active".into());
    state.real_progress = Some((2, 10));
    let history_load_id = state.history_load_id;
    let active = state.active_workspace;

    state.activate_review(u64::MAX);

    assert_eq!(state.active_workspace, active);
    assert_eq!(state.session.id(), Some("active"));
    assert_eq!(state.session.phase, crate::review_session::Phase::Opening);
    assert_eq!(state.real_progress, Some((2, 10)));
    assert_eq!(state.history_load_id, history_load_id);
}

#[test]
fn import_source_change_cancels_request_and_clears_transient_ui() {
    let mut state = State {
        chess_loading: true,
        import_error: Some("old error".into()),
        file_drag_over: true,
        ..State::default()
    };
    let request = state.chess_request;
    state.select_import_source(ImportSource::File);
    assert_eq!(state.import_source, ImportSource::File);
    assert_eq!(state.chess_request, request + 1);
    assert!(!state.chess_loading);
    assert!(!state.file_drag_over);
    assert!(state.import_error.is_none());
}

#[test]
fn playback_intent_restarts_finished_variation_and_clears_selection() {
    let mut state = State::fixture();
    state.explore_candidate();
    let index = state.active_variation.expect("fixture has a candidate");
    state.playing = false;
    state.variations[index].cursor = state.variations[index].moves.len();
    state.selected_square = Some(1);
    state.selected_candidate = 1;
    state.clock = 4.0;
    state.toggle_playback();
    assert!(state.playing);
    assert_eq!(state.variations[index].cursor, 0);
    assert_eq!(state.selected_square, None);
    assert_eq!(state.selected_candidate, 0);
    assert_eq!(state.next_step, 4.65);
    state.toggle_playback();
    assert!(!state.playing);
    state.set_appearance(Appearance::Light);
    state.toggle_live_engine();
    assert_eq!(state.preferences.appearance, Appearance::Light);
    assert!(!state.preferences.live_engine);
}

#[test]
fn history_retry_keeps_the_failed_operation_and_cursor() {
    let mut state = State {
        history_cursor: Some(engineroom_backend::db::games::GameCursor {
            created_at: "2025-01-01".into(),
            id: 42,
        }),
        ..State::default()
    };
    state.next_history();
    let page = HistoryOperation::Page {
        created_at: "2025-01-01".into(),
        id: 42,
    };
    assert_eq!(state.history_operation, Some(page.clone()));
    state.history_cursor.as_mut().unwrap().id = 99;
    state.retry_history();
    assert_eq!(state.history_operation, Some(page));
    assert_eq!(state.history_cursor.as_ref().unwrap().id, 99);
    for reanalyze in [false, true] {
        state.history_open(7, reanalyze);
        state.apply_history_load(
            state.history_load_id,
            Err("database failed".into()),
            reanalyze,
        );
        assert_eq!(state.history_error.as_deref(), Some("database failed"));
        state.retry_history();
        assert_eq!(
            state.history_operation,
            Some(HistoryOperation::Open { id: 7, reanalyze })
        );
    }
    state.refresh_history();
    state.retry_history();
    assert_eq!(state.history_operation, Some(HistoryOperation::List));
}

/// A live Analyze/Cancel failure is recoverable: it must surface through the
/// live-error state without showing the fatal banner (or the sidebar "Erro"
/// badge) or closing a healthy session, and the completed result stays usable.
#[test]
fn recoverable_live_error_keeps_session_banner_and_results() {
    let (bridge, wake) = crate::bridge::tests::bridge(
        Arc::new(crate::bridge::tests::NoEngine),
        Arc::new(crate::bridge::tests::MemoryRepo::default()),
    );
    let mut state = State::fixture();
    state.bridge = Some(bridge);
    // The fixture carries a completed result, so resuming its workspace opens a
    // session that immediately completes and stays Running for live requests.
    let key = state.active_workspace.expect("fixture workspace");
    state.activate_review(key);

    let settle = |state: &mut State| {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            state.poll_bridge();
            if matches!(state.session.phase, crate::review_session::Phase::Running)
                && state.real_result.is_some()
            {
                return;
            }
            let _ = wake.recv_timeout(Duration::from_millis(20));
        }
    };
    settle(&mut state);
    assert_eq!(
        state.session.phase,
        crate::review_session::Phase::Running,
        "fixture session should be running"
    );
    let progress = state.real_progress;

    // Invalid live settings make analyze_position reject the request. The bridge
    // reports that failure as a lifecycle Error while the session is still alive.
    state.preferences.lines = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && state.live_error.is_none() {
        state.poll_bridge();
        let _ = wake.recv_timeout(Duration::from_millis(20));
    }
    assert!(
        state.live_error.is_some(),
        "invalid live settings should surface as a live error"
    );
    assert!(
        state.bridge_error.is_none(),
        "a recoverable live error must not show the fatal banner"
    );
    assert_eq!(
        state.session.phase,
        crate::review_session::Phase::Running,
        "a recoverable live error must not close the session"
    );
    assert_eq!(
        state.real_progress, progress,
        "a recoverable live error must keep the review result"
    );
    assert!(state.real_result.is_some());

    // Correcting the settings changes the live key, so the next poll submits a
    // fresh request. With the failing engine it produces a live error; the point
    // is that the request is accepted at all (a Closed phase would skip it).
    state.preferences.lines = 3;
    state.live_error = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && state.live_error.is_none() {
        state.poll_bridge();
        let _ = wake.recv_timeout(Duration::from_millis(20));
    }
    assert!(
        state.live_error.is_some(),
        "a valid live request after the recoverable error must be submitted"
    );
    assert!(state.bridge_error.is_none());
}

/// SessionClosed (even with a live origin) and fatal origins still end the
/// session and surface the fatal error instead of the live-error state.
#[test]
fn fatal_and_session_closed_lifecycle_errors_still_close_the_session() {
    use crate::bridge::LifecycleOrigin;
    use engineroom_backend::review::types::{ReviewError, ReviewErrorCode};

    for (origin, code) in [
        (LifecycleOrigin::Analyze, ReviewErrorCode::SessionClosed),
        (LifecycleOrigin::Cancel, ReviewErrorCode::SessionClosed),
        (LifecycleOrigin::Open, ReviewErrorCode::EngineSpawn),
    ] {
        let mut state = State::default();
        state.session.open("live".into());
        state.session.acquired("live");
        state.real_progress = Some((1, 9));
        state.live_key = Some("fen:None:1:true".into());
        let error = ReviewError::new(code, "analyze", "fatal");
        state.handle_lifecycle_error("live", &error, origin);
        assert_eq!(
            state.session.phase,
            crate::review_session::Phase::Closed,
            "{origin:?}/{code:?} must close the session"
        );
        assert!(state.bridge_error.is_some());
        assert_eq!(state.real_progress, None);
        assert!(state.live_error.is_none());
    }
}

/// The preference checkpoint advances on the worker's saved-snapshot ACK, not at
/// enqueue, and a transient failure is retried on the worker without the UI
/// re-enqueueing (which would spin).
#[test]
fn preference_checkpoint_advances_only_on_ack_without_ui_retry_loop() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let path = std::env::temp_dir().join(format!(
        "engineroom-pref-state-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let attempts = Arc::new(AtomicUsize::new(0));
    let seen = attempts.clone();
    let (wake_tx, wake_rx) = mpsc::channel();
    let (store, initial) = crate::preferences::Store::open_with(
        path.clone(),
        move || {
            let _ = wake_tx.send(());
        },
        move |p, preferences| {
            if seen.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(crate::preferences::WriteError::transient(
                    "falha transitória",
                ))
            } else {
                crate::preferences::write(p, preferences)
            }
        },
        crate::preferences::Retry::immediate(),
    );
    assert_eq!(initial.unwrap(), Some(super::Preferences::default()));

    let mut state = State::default();
    state.set_preference_store(store);
    // setup_preferences normally applies the synchronous initial read.
    state.preference_saved = Some(super::Preferences::default());
    let desired = super::Preferences {
        hash_mb: 512,
        ..super::Preferences::default()
    };
    state.preferences = desired.clone();

    // Enqueue alone must not advance the checkpoint.
    state.poll_preferences();
    assert_eq!(
        state.preference_saved.as_ref(),
        Some(&super::Preferences::default())
    );
    assert_eq!(state.preference_queued.as_ref(), Some(&desired));

    // The worker fails once, then retries the same snapshot and acknowledges it.
    wake_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("worker wake");
    state.poll_preferences();
    assert_eq!(state.preference_saved.as_ref(), Some(&desired));
    assert_eq!(state.preference_error, None);
    assert_eq!(state.preference_queued, None);
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        2,
        "one failure and one retry, with no UI-side re-enqueue loop"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn reverting_preferences_while_a_save_is_pending_persists_the_latest_value() {
    let (started, first_write) = std::sync::mpsc::channel();
    let (release, resume) = std::sync::mpsc::channel();
    let (written, snapshots) = std::sync::mpsc::channel();
    let mut first = true;
    let (store, _) = crate::preferences::Store::open_with(
        std::env::temp_dir().join("engineroom-preference-revert-test.json"),
        || {},
        move |_, preferences| {
            if first {
                first = false;
                started.send(()).unwrap();
                resume.recv().unwrap();
            }
            written.send(preferences.clone()).unwrap();
            Ok(())
        },
        crate::preferences::Retry::immediate(),
    );
    let original = super::Preferences::default();
    let mut state = State::default();
    state.set_preference_store(store);
    state.preference_saved = Some(original.clone());
    state.preferences.hash_mb = 512;
    state.poll_preferences();
    first_write
        .recv_timeout(Duration::from_secs(5))
        .expect("pending write started");
    state.preferences = original.clone();
    state.poll_preferences();
    let queued = state.preference_queued.clone();
    release.send(()).unwrap();
    drop(state);
    let saved: Vec<_> = snapshots.try_iter().collect();
    assert_eq!(
        queued,
        Some(original.clone()),
        "reverting must supersede the pending write"
    );
    assert_eq!(
        saved.last(),
        Some(&original),
        "last snapshot on disk must match the UI"
    );
}

#[test]
fn new_and_history_reviews_start_unfiled_without_changing_existing_sessions() {
    use crate::{session_style::SessionColor, state::Page};
    let mut state = State::fixture();
    let first = state.workspace[0].key;
    let config = state.workspace[0].config.clone();
    crate::fixtures::file_session(&mut state, first, SessionColor::Blue);
    state.navigate(Page::Home);
    state.pgn.replace_text(&config.pgn);
    state.prepare();
    state.start_review();
    assert!(
        state
            .session_folder(state.active_workspace.unwrap())
            .is_none()
    );
    assert_eq!(state.session_folder(first).unwrap().id, "fixture-blue");
    for reanalyze in [false, true] {
        state.navigate(Page::History);
        state.history_open(7, reanalyze);
        state.apply_history_load(
            state.history_load_id,
            Ok(Some(Box::new(config.clone()))),
            reanalyze,
        );
        assert!(
            state
                .session_folder(state.active_workspace.unwrap())
                .is_none()
        );
        assert_eq!(state.session_folder(first).unwrap().id, "fixture-blue");
    }
}
