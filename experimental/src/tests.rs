use super::*;
use rune::{Env, Runtime};
fn runtime() -> Runtime<State> {
    let mut rt = rune::App::new(spec()).headless_runtime(
        (1220.0, 900.0),
        Env {
            system_dark: true,
            scale: 1.0,
        },
    );
    rt.frame(0.0);
    rt
}
fn tap(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("interactive element {key} must exist"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..60 {
        rt.frame(1.0 / 60.0);
    }
}
#[test]
fn imports_configures_reviews_and_returns_to_the_draft() {
    let mut rt = runtime();
    tap(&mut rt, "continue");
    assert_eq!(rt.state.page, Page::Home);
    tap(&mut rt, "load-demo");
    tap(&mut rt, "continue");
    assert_eq!(rt.state.page, Page::Configure);
    tap(&mut rt, "profile/segment2");
    assert_eq!(rt.state.profile, state::Profile::Manual);
    tap(&mut rt, "start-review");
    assert_eq!(rt.state.page, Page::Review);
    tap(&mut rt, "next");
    assert_eq!(rt.state.ply, 1);
    tap(&mut rt, "move-16");
    assert_eq!(rt.state.ply, 16);
    tap(&mut rt, "flip");
    assert!(rt.state.flipped);
    tap(&mut rt, "review-tabs/segment2");
    assert_eq!(rt.state.review_tab, state::ReviewTab::Summary);
    tap(&mut rt, "review-back");
    assert_eq!(rt.state.pgn.text, game::DEMO_PGN);
}
#[test]
fn settings_change_theme_and_remember_preferences_across_navigation() {
    let mut rt = runtime();
    tap(&mut rt, "settings-nav");
    tap(&mut rt, "appearance/segment1");
    assert_eq!(rt.state.preferences.appearance, Appearance::Light);
    tap(&mut rt, "sound");
    assert!(!rt.state.preferences.sound);
    let volume = rt.state.preferences.volume;
    tap(&mut rt, "volume");
    assert_eq!(
        rt.state.preferences.volume, volume,
        "disabled volume does not change"
    );
    tap(&mut rt, "reduce-motion");
    assert!(rt.state.preferences.reduced_motion);
    tap(&mut rt, "settings-tabs/segment1");
    tap(&mut rt, "live-engine");
    assert!(!rt.state.preferences.live_engine);
    tap(&mut rt, "lines-trigger");
    tap(&mut rt, "option4");
    assert_eq!(rt.state.preferences.lines, 5);
    tap(&mut rt, "settings-close");
    assert_eq!(rt.state.page, Page::Home);
    tap(&mut rt, "settings-nav");
    assert!(!rt.state.preferences.sound);
    tap(&mut rt, "history-nav");
    assert_eq!(rt.state.page, Page::History);
    tap(&mut rt, "open-demo");
    assert_eq!(rt.state.page, Page::Review);
}
#[test]
fn invalid_import_stays_on_the_form() {
    let mut rt = runtime();
    tap(&mut rt, "pgn-editor/scroll/input");
    rt.key(rune::KeyEvent {
        key: rune::Key::Text("1. e4 e5 2. Ke5 *".into()),
        mods: Default::default(),
    });
    rt.frame(1.0 / 60.0);
    tap(&mut rt, "continue");
    assert_eq!(rt.state.page, Page::Home);
    assert!(rt.state.import_error.is_some());
}

#[test]
fn resizing_reflows_the_board_without_resetting_navigation() {
    let mut rt = runtime();
    tap(&mut rt, "review-nav");
    tap(&mut rt, "move-16");
    for size in [
        (1440.0, 1080.0),
        (1024.0, 768.0),
        (720.0, 560.0),
        (1220.0, 900.0),
    ] {
        rt.set_viewport(size, 1.0);
        for _ in 0..60 {
            rt.frame(1.0 / 60.0);
        }
        let rect = |key: &str| {
            rt.presented(key)
                .unwrap_or_else(|| panic!("{key} exists at {size:?}"))
                .0
        };
        let board = rect("chessboard");
        let body = rect("page-body");
        let inspector = rect("review-inspector");
        assert_eq!(board[2], board[3], "square board at {size:?}");
        assert_eq!(board[2] % 8.0, 0.0, "whole-point squares at {size:?}");
        assert!(
            board[0] >= body[0] && board[0] + board[2] <= body[0] + body[2] + 0.5,
            "board fits horizontally at {size:?}"
        );
        if size.0 == 720.0 {
            assert!(
                inspector[1] >= board[1] + board[3],
                "narrow layout stacks the inspector"
            );
            tap(&mut rt, "show-analysis");
            let inspector = rt
                .presented("review-inspector")
                .expect("inspector remains mounted")
                .0;
            assert!(
                inspector[1] < size.1,
                "analysis shortcut reveals the compact inspector"
            );
        } else {
            assert!(
                inspector[0] >= board[0] + board[2],
                "wide layout keeps the inspector alongside"
            );
        }
        assert_eq!(rt.state.ply, 16, "resizing preserves the selected move");
    }
}

#[test]
fn mock_feedback_graph_and_critical_navigation_are_interactive() {
    let mut rt = runtime();
    tap(&mut rt, "review-nav");
    tap(&mut rt, "move-11");
    assert!(rt.presented("move-feedback").is_some());
    tap(&mut rt, "review-tabs/segment2");
    tap(&mut rt, "evaluation-graph");
    assert_eq!(rt.state.ply, 8, "chart click selects the midpoint position");
    rt.key(rune::KeyEvent {
        key: rune::Key::Right,
        mods: rune::Mods {
            shift: true,
            ..Default::default()
        },
    });
    for _ in 0..60 {
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(
        rt.state.ply, 11,
        "Shift+Right finds the next critical mock move"
    );
    tap(&mut rt, "mock-toggle");
    assert!(!rt.state.is_mock());
    assert!(rt.presented("move-feedback").is_none());
}
#[test]
fn candidates_play_legal_variations_without_changing_the_mainline() {
    let mut rt = runtime();
    tap(&mut rt, "review-nav");
    tap(&mut rt, "move-8");
    let original = rt.state.game.board(8);
    tap(&mut rt, "review-tabs/segment1");
    tap(&mut rt, "candidate-1");
    assert_eq!(rt.state.selected_candidate, 1);
    tap(&mut rt, "explore-line");
    assert!(rt.state.active_variation.is_some());
    for _ in 0..240 {
        rt.frame(1.0 / 60.0);
    }
    let i = rt
        .state
        .active_variation
        .expect("candidate creates a variation");
    assert_eq!(
        rt.state.variations[i].cursor,
        rt.state.variations[i].moves.len()
    );
    assert!(!rt.state.playing);
    assert_eq!(rt.state.game.board(8), original);
    tap(&mut rt, "return-mainline");
    assert!(rt.state.active_variation.is_none());
    assert_eq!(rt.state.ply, 8);
}
#[test]
fn board_moves_create_a_legal_branch_and_reject_illegal_destinations() {
    let mut rt = runtime();
    tap(&mut rt, "review-nav");
    tap(&mut rt, "square-g1");
    tap(&mut rt, "square-f5");
    assert!(rt.state.active_variation.is_none(), "illegal move rejected");
    tap(&mut rt, "square-g1");
    tap(&mut rt, "square-f3");
    assert!(rt.state.active_variation.is_some());
    assert_eq!(crate::variation::board(rt.state.position())[45], 'N');
    assert_eq!(rt.state.game.board(0)[62], 'N', "mainline unchanged");
    tap(&mut rt, "previous");
    assert_eq!(crate::variation::board(rt.state.position())[62], 'N');
}
#[test]
fn review_settings_and_mock_failure_recovery_work() {
    let mut rt = runtime();
    tap(&mut rt, "review-nav");
    tap(&mut rt, "review-settings");
    tap(&mut rt, "dialog-feedback");
    assert!(!rt.state.preferences.feedback);
    tap(&mut rt, "dialog-lines/segment2");
    assert_eq!(rt.state.preferences.lines, 3);
    tap(&mut rt, "close");
    tap(&mut rt, "mock-scenario");
    assert_eq!(rt.state.mock_status, state::MockStatus::Running);
    tap(&mut rt, "cancel-analysis");
    assert_eq!(rt.state.mock_status, state::MockStatus::Cancelled);
    tap(&mut rt, "mock-scenario");
    tap(&mut rt, "mock-scenario");
    tap(&mut rt, "mock-scenario");
    assert_eq!(rt.state.mock_status, state::MockStatus::Failed);
    tap(&mut rt, "retry-analysis");
    assert_eq!(rt.state.mock_status, state::MockStatus::Running);
}
#[test]
fn focus_layout_and_sidebar_toggle_keep_navigation_intact() {
    let mut rt = runtime();
    tap(&mut rt, "review-nav");
    tap(&mut rt, "move-11");
    tap(&mut rt, "sidebar-toggle");
    assert!(!rt.state.sidebar.is_open());
    tap(&mut rt, "focus-board");
    assert!(!rt.state.inspector_visible);
    assert!(rt.presented("review-inspector").is_none());
    tap(&mut rt, "focus-board");
    assert!(rt.state.inspector_visible);
    assert_eq!(rt.state.ply, 11);
}
