use super::Page;
use engineroom_backend::review::types::{Envelope, Event, Progress};

#[test]
fn parked_sessions_receive_events_without_changing_visible_workspace() {
    let mut state = crate::fixtures::workspace_scene();
    let first = state.active_workspace.expect("fixture has a workspace");
    state.session.open("first".into());
    state.real_progress = Some((0, 10));
    state.navigate(Page::Home);
    let config = state.workspace[0].config.clone();
    state.session = Default::default();
    state.register_review(config);
    let second = state.active_workspace.expect("second workspace exists");
    state.session.open("second".into());
    state.real_progress = Some((1, 10));
    state.page = Page::Review;
    assert!(state.with_review_session("first", |state| {
        state.apply_review_envelope(Envelope {
            session_id: "first".into(),
            sequence: 1,
            request_id: None,
            event: Event::Progress {
                progress: Progress {
                    stage: "analysis".into(),
                    completed: 4,
                    total: 10,
                    current_ply: 4,
                    phase: None,
                    cached_positions: 0,
                    engine_positions: 4,
                    remaining_budget_ms: None,
                    update: None,
                },
            },
        });
    }));
    assert_eq!(state.active_workspace, Some(second));
    assert_eq!(state.real_progress, Some((1, 10)));
    state.activate_review(first);
    assert_eq!(state.session.id(), Some("first"));
    assert_eq!(state.real_progress, Some((4, 10)));
    state.activate_review(second);
    assert_eq!(state.session.id(), Some("second"));
    assert_eq!(state.real_progress, Some((1, 10)));
}

#[test]
fn events_on_home_update_snapshot_and_keep_sequence_order() {
    let mut state = crate::fixtures::workspace_scene();
    let key = state.active_workspace.expect("fixture has a workspace");
    state.session.open("background".into());
    state.navigate(Page::Home);
    let envelope = |sequence, completed| Envelope {
        session_id: "background".into(),
        sequence,
        request_id: None,
        event: Event::Progress {
            progress: Progress {
                stage: "analysis".into(),
                completed,
                total: 10,
                current_ply: completed,
                phase: None,
                cached_positions: 0,
                engine_positions: completed,
                remaining_budget_ms: None,
                update: None,
            },
        },
    };
    state.with_review_session("background", |state| {
        state.apply_review_envelope(envelope(2, 6))
    });
    state.with_review_session("background", |state| {
        state.apply_review_envelope(envelope(1, 2))
    });
    assert_eq!(state.page, Page::Home);
    assert!(state.active_workspace.is_none());
    state.activate_review(key);
    assert_eq!(state.real_progress, Some((6, 10)));
}

#[test]
fn review_promotion_survives_switching_sessions_and_settings() {
    let mut state = crate::fixtures::workspace_scene();
    let first = state.active_workspace.expect("review");
    state.promotion_pending = Some((8, 0));
    state.selected_square = Some(8);
    state.navigate(Page::Settings);
    state.navigate(Page::Review);
    assert_eq!(state.promotion_pending, Some((8, 0)));
    state.navigate(Page::Home);
    state.activate_review(first);
    assert_eq!(state.promotion_pending, Some((8, 0)));
    assert_eq!(state.selected_square, Some(8));
}
