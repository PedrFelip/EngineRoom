use super::{HistoryOperation, State};
use crate::components::board::annotations::{Annotation, Brush};

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
