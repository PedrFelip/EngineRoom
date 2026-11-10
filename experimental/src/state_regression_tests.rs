use super::{Appearance, HistoryOperation, ImportSource, State};
use crate::components::board::annotations::{Annotation, Brush};

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

#[test]
fn annotations_belong_to_each_workspace_and_clear_on_navigation() {
    let mut state = State::fixture();
    let first = state.active_workspace.unwrap();
    let config = state.workspace[0].config.clone();
    state.sync_annotations();
    let shape = Annotation {
        from: 1,
        to: 2,
        brush: Brush::Green,
    };
    state.board_annotations.toggle(shape);
    state.board_annotations.toggle_drawing();
    state.board_annotations.select_square(3);
    state.park_review();
    state.reset_annotations();
    state.register_review(config);
    let second = state.active_workspace.unwrap();
    state.activate_review(first);
    assert_eq!(state.board_annotations.shapes(), &[shape]);
    assert_eq!(state.board_annotations.origin(), Some(3));
    state.activate_review(second);
    assert!(state.board_annotations.shapes().is_empty());
    assert_eq!(state.board_annotations.origin(), None);
    state.go_to(1);
    state.sync_annotations();
    state.board_annotations.toggle(shape);
    state.activate_review(first);
    assert_eq!(state.board_annotations.shapes(), &[shape]);
    state.activate_review(second);
    assert_eq!(state.board_annotations.shapes(), &[shape]);
    state.go_to(2);
    state.sync_annotations();
    assert!(state.board_annotations.shapes().is_empty());
}
