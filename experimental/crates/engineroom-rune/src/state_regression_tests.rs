use super::{Appearance, HistoryOperation, ImportSource, State};
use std::{
    sync::Arc,
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

/// A live request whose settings fail validation is recoverable: the session is
/// still running and a later, valid request must be accepted. The lifecycle error
/// must not tear the session down.
#[test]
fn recoverable_live_settings_error_keeps_session_for_next_request() {
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

    // Invalid live settings make analyze_position reject the request. The bridge
    // reports that failure as a lifecycle Error while the session is still alive.
    state.preferences.lines = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && state.bridge_error.is_none() {
        state.poll_bridge();
        let _ = wake.recv_timeout(Duration::from_millis(20));
    }
    assert!(
        state.bridge_error.is_some(),
        "invalid live settings should surface a recoverable error"
    );
    assert_eq!(
        state.session.phase,
        crate::review_session::Phase::Running,
        "a recoverable live settings error must not close the session"
    );

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
}

/// The preference checkpoint advances on the worker's saved-snapshot ACK, not at
/// enqueue, and a transient failure is retried on the worker without the UI
/// re-enqueueing (which would spin).
#[test]
fn preference_checkpoint_advances_only_on_ack_without_ui_retry_loop() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
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
    let (store, initial) = crate::preferences::Store::open_with(
        path.clone(),
        || {},
        move |p, preferences| {
            if seen.fetch_add(1, Ordering::SeqCst) == 0 {
                Err("falha transitória".into())
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
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && state.preference_saved.as_ref() != Some(&desired) {
        let before = state.preference_saved.clone();
        state.poll_preferences();
        if state.preference_saved == before {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
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
