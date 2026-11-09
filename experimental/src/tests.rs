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
    rt.state = State::fixture();
    rt.invalidate();
    rt.frame(0.0);
    rt
}
fn tap(rt: &mut Runtime<State>, key: &str) {
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
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
    rt.state.load_fixture();
    rt.invalidate();
    rt.frame(0.0);
    tap(&mut rt, "continue");
    assert_eq!(rt.state.page, Page::Configure);
    tap(&mut rt, "profile/segment2");
    assert_eq!(rt.state.profile, state::Profile::Manual);
    tap(&mut rt, "start-review");
    rt.state.real_result = Some(fixtures::result());
    rt.state.bridge_error = None;
    rt.invalidate();
    rt.frame(0.0);
    assert_eq!(rt.state.page, Page::Review);
    tap(&mut rt, "next");
    assert_eq!(rt.state.ply, 1);
    tap(&mut rt, "move-16");
    assert_eq!(rt.state.ply, 16);
    tap(&mut rt, "flip");
    assert!(rt.state.flipped);
    tap(&mut rt, "review-tabs/segment2");
    assert_eq!(rt.state.review_tab, state::ReviewTab::Summary);
    tap(&mut rt, "home-nav");
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
    tap(&mut rt, "session-1");
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
    tap(&mut rt, "session-1");
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
        let inspector = rt.presented("review-inspector").map(|p| p.0);
        assert_eq!(board[2], board[3], "square board at {size:?}");
        assert_eq!(board[2] % 8.0, 0.0, "whole-point squares at {size:?}");
        assert!(
            board[0] >= body[0] && board[0] + board[2] <= body[0] + body[2] + 0.5,
            "board fits horizontally at {size:?}"
        );
        if size.0 == 720.0 {
            assert!(
                inspector.is_none(),
                "narrow layout initially prioritizes the board"
            );
            tap(&mut rt, "compact-review-tabs/segment1");
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
                inspector.unwrap()[0] >= board[0] + board[2],
                "wide layout keeps the inspector alongside"
            );
        }
        assert_eq!(rt.state.ply, 16, "resizing preserves the selected move");
    }
}

#[test]
fn board_lines_follow_toggle_and_variation_feedback_is_visible() {
    let mut rt = runtime();
    rt.state.page = Page::Review;
    rt.state.review_tab = state::ReviewTab::Moves;
    rt.invalidate();
    rt.frame(0.0);
    assert!(
        rt.find("candidate-arrow-0").is_some(),
        "lines must be visible on the board outside the analysis tab"
    );
    tap(&mut rt, "review-engine-toggle");
    assert!(!rt.state.preferences.live_engine);
    assert!(
        rt.find("candidate-arrow-0").is_none(),
        "turning lines off must hide arrows"
    );
    rt.state.preferences.live_engine = true;
    rt.state.click_square(52);
    rt.state.click_square(36);
    let i = rt.state.active_variation.unwrap();
    rt.state.variations[i].classifications[0] =
        Some(engineroom_backend::review::types::Classification::Bom);
    rt.invalidate();
    rt.frame(0.0);
    assert!(
        rt.find("move-feedback").is_some(),
        "variation feedback should appear in the inspector"
    );
    assert_eq!(
        rt.state.displayed_classification(),
        Some(review_data::Classification::Good)
    );
}

#[test]
fn analyzed_feedback_graph_and_critical_navigation_are_interactive() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
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
        "Shift+Right finds the next critical analyzed move"
    );
    rt.state.real_result = None;
    rt.invalidate();
    rt.frame(0.0);
    assert!(rt.presented("move-feedback").is_none());
}
#[test]
fn candidates_play_legal_variations_without_changing_the_mainline() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
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
    tap(&mut rt, "session-1");
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
fn variation_moves_navigate_directly_and_restore_their_origin() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
    tap(&mut rt, "move-8");
    tap(&mut rt, "review-tabs/segment1");
    tap(&mut rt, "explore-line");
    let index = rt.state.active_variation.expect("candidate variation");
    assert!(!rt.state.variations[index].moves.is_empty());
    tap(&mut rt, "review-tabs/segment0");
    tap(&mut rt, "move-2");
    tap(&mut rt, "variation-0-move-1");
    assert_eq!(rt.state.active_variation, Some(index));
    assert_eq!(rt.state.variations[index].cursor, 1);
    assert_eq!(rt.state.ply, 8);
    assert!(!rt.state.playing);
    tap(&mut rt, "return-mainline");
    assert_eq!(rt.state.ply, 8);
    assert_eq!(rt.state.active_variation, None);
}
#[test]
fn review_settings_and_analysis_states_work() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
    tap(&mut rt, "review-settings");
    tap(&mut rt, "dialog-feedback");
    assert!(!rt.state.preferences.feedback);
    tap(&mut rt, "dialog-lines/segment2");
    assert_eq!(rt.state.preferences.lines, 3);
    tap(&mut rt, "close");
    rt.state.real_result = None;
    rt.state.real_progress = Some((2, 17));
    rt.invalidate();
    rt.frame(0.0);
    assert!(rt.presented("analysis-progress").is_some());
    tap(&mut rt, "cancel-analysis");
    assert!(rt.state.real_progress.is_none());
    rt.state.bridge_error = Some("Falha de teste".into());
    rt.invalidate();
    rt.frame(0.0);
    assert!(rt.presented("analysis-error").is_some());
    assert!(rt.presented("retry-analysis").is_some());
}

#[test]
fn focus_layout_and_sidebar_toggle_keep_navigation_intact() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
    tap(&mut rt, "move-11");
    tap(&mut rt, "sidebar-toggle");
    assert!(!rt.state.sidebar.is_open());
    tap(&mut rt, "focus-board");
    assert!(!rt.state.review_sidebar.is_open());
    assert!(rt.presented("review-inspector").is_none());
    tap(&mut rt, "focus-board");
    assert!(rt.state.review_sidebar.is_open());
    assert_eq!(rt.state.ply, 11);
}

#[test]
fn sidebar_navigation_preserves_draft_and_settings_return_target() {
    let mut rt = runtime();
    rt.state.load_fixture();
    rt.invalidate();
    rt.frame(0.0);
    tap(&mut rt, "continue");
    assert_eq!(rt.state.page, Page::Configure);
    tap(&mut rt, "home-nav");
    assert_eq!(rt.state.page, Page::Home);
    assert_eq!(rt.state.pgn.text, game::DEMO_PGN);
    tap(&mut rt, "continue");
    tap(&mut rt, "settings-nav");
    tap(&mut rt, "settings-nav");
    tap(&mut rt, "settings-close");
    assert_eq!(rt.state.page, Page::Configure);
    tap(&mut rt, "history-nav");
    tap(&mut rt, "home-nav");
    assert_eq!(rt.state.pgn.text, game::DEMO_PGN);
}

#[test]
fn sidebar_width_and_visibility_survive_navigation() {
    let mut rt = runtime();
    rt.state.sidebar.set_width(290.0);
    rt.state.sidebar.set_open(false);
    rt.state.navigate(Page::History);
    assert!(!rt.state.sidebar.is_open());
    rt.state.sidebar.set_open(true);
    assert_eq!(rt.state.sidebar.width(), 290.0);
    rt.state.sidebar.set_width(999.0);
    assert_eq!(rt.state.sidebar.width(), 290.0);
    rt.state.sidebar.reset_width();
    assert_eq!(rt.state.sidebar.width(), 218.0);
}

#[test]
fn sidebar_toggles_with_command_b() {
    let mut rt = runtime();
    let key = rune::KeyEvent {
        key: rune::Key::Command('b'),
        mods: rune::Mods {
            cmd: true,
            ..Default::default()
        },
    };
    rt.key(key.clone());
    rt.frame(1.0 / 60.0);
    assert!(!rt.state.sidebar.is_open());
    rt.key(key);
    rt.frame(1.0 / 60.0);
    assert!(rt.state.sidebar.is_open());
}

fn toggle_review_key() -> rune::KeyEvent {
    rune::KeyEvent {
        key: rune::Key::Command('b'),
        mods: rune::Mods {
            ctrl: true,
            cmd: !cfg!(target_os = "macos"),
            alt: true,
            ..Default::default()
        },
    }
}

#[test]
fn right_sidebar_shortcut_is_scoped_and_independent() {
    let mut rt = runtime();
    rt.key(toggle_review_key());
    assert!(
        rt.state.review_sidebar.is_open(),
        "home does not toggle review panels"
    );
    tap(&mut rt, "session-1");
    tap(&mut rt, "move-11");
    tap(&mut rt, "review-tabs/segment1");
    let orientation = rt.state.flipped;
    for opened in [false, true, false, true] {
        rt.key(toggle_review_key());
        rt.frame(1.0);
        assert_eq!(rt.state.review_sidebar.is_open(), opened);
        assert!(
            rt.state.sidebar.is_open(),
            "right shortcut must leave left sidebar open"
        );
        assert_eq!(rt.state.ply, 11);
        assert_eq!(rt.state.review_tab, state::ReviewTab::Analysis);
        assert_eq!(rt.state.flipped, orientation);
    }
    let mut shifted = toggle_review_key();
    shifted.mods.shift = true;
    rt.key(shifted);
    rt.frame(1.0);
    assert!(
        rt.state.review_sidebar.is_open(),
        "extra Shift must not toggle right sidebar"
    );
    tap(&mut rt, "review-settings");
    rt.key(toggle_review_key());
    rt.frame(1.0);
    assert!(
        rt.state.review_sidebar.is_open(),
        "modal owns keyboard while open"
    );
}

#[test]
fn right_sidebar_width_survives_visibility_and_navigation() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
    rt.state.review_sidebar.set_width(392.0);
    rt.key(toggle_review_key());
    rt.frame(1.0);
    tap(&mut rt, "home-nav");
    tap(&mut rt, "session-1");
    assert!(!rt.state.review_sidebar.is_open());
    rt.key(toggle_review_key());
    rt.frame(1.0);
    assert_eq!(rt.state.review_sidebar.width(), 392.0);
    assert!(rt.presented("review-sidebar-grip").is_some());
    rt.state.review_sidebar.set_width(1000.0);
    assert_eq!(rt.state.review_sidebar.width(), 392.0);
    rt.state.review_sidebar.reset_width();
    assert_eq!(rt.state.review_sidebar.width(), 352.0);
    rt.state.review_sidebar.set_width(392.0);
    rt.invalidate();
    rt.frame(0.0);
    tap(&mut rt, "review-sidebar-grip");
    let ply = rt.state.ply;
    rt.key(rune::KeyEvent {
        key: rune::Key::Left,
        mods: Default::default(),
    });
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.review_sidebar.width(), 384.0);
    assert_eq!(
        rt.state.ply, ply,
        "resizing the sidebar must not navigate the game"
    );
}

#[test]
fn toggling_review_panels_keeps_the_active_variation() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
    tap(&mut rt, "move-8");
    tap(&mut rt, "review-tabs/segment1");
    rt.state.explore_candidate();
    rt.state.playing = false;
    let active = rt
        .state
        .active_variation
        .expect("candidate creates a branch");
    let cursor = rt.state.variations[active].cursor;
    let moves = rt.state.variations[active].moves.clone();
    for _ in 0..2 {
        rt.key(toggle_review_key());
        rt.frame(1.0);
        assert_eq!(rt.state.active_variation, Some(active));
        assert_eq!(rt.state.variations[active].cursor, cursor);
        assert_eq!(rt.state.variations[active].moves, moves);
        assert_eq!(rt.state.review_tab, state::ReviewTab::Analysis);
    }
}

fn summary(id: i64) -> engineroom_backend::db::games::GameSummary {
    engineroom_backend::db::games::GameSummary {
        id,
        white: "White".into(),
        black: "Black".into(),
        result: "1-0".into(),
        plies: 40,
        engine_tier: "custom".into(),
        mode: engineroom_backend::db::mode::Mode::Depth,
        analysis_kind: "manual".into(),
        depth: 20,
        multipv: 2,
        accuracy_white: 80.0,
        accuracy_black: 70.0,
        created_at: format!("2026-11-0{}T00:00:00Z", id + 1),
    }
}

#[test]
fn history_pages_append_without_duplicates_and_track_total() {
    use engineroom_backend::db::games::{GameCursor, GamePage};
    let mut state = State::default();
    state.history_loading = true;
    state.apply_history_page(
        GamePage {
            games: vec![summary(2), summary(1)],
            total: 3,
            next_cursor: Some(GameCursor {
                created_at: "2026-11-02T00:00:00Z".into(),
                id: 1,
            }),
        },
        false,
    );
    assert_eq!(state.history_total, 3);
    assert_eq!(
        state.history.iter().map(|g| g.id).collect::<Vec<_>>(),
        [2, 1]
    );
    assert!(state.history_cursor.is_some());
    assert!(!state.history_loading);
    state.apply_history_page(
        GamePage {
            games: vec![summary(1), summary(0)],
            total: 3,
            next_cursor: None,
        },
        true,
    );
    assert_eq!(
        state.history.iter().map(|g| g.id).collect::<Vec<_>>(),
        [2, 1, 0],
        "appending a page must not duplicate a boundary review"
    );
    assert!(state.history_cursor.is_none());
    assert_eq!(state.history_total, 3);
}

#[test]
fn missing_file_picker_and_empty_path_are_reported_without_io() {
    let mut state = State::default();
    state.pick_pgn_file();
    assert!(
        state
            .import_error
            .as_deref()
            .is_some_and(|error| error.contains("Seletor de arquivos")),
        "headless scenes have no native picker"
    );
    state.import_error = None;
    state.import_path.replace_text("   ");
    state.import_file();
    assert!(
        state
            .import_error
            .as_deref()
            .is_some_and(|error| error.contains("caminho do arquivo")),
        "an empty path must not reach the file bridge"
    );
}

#[test]
fn history_clear_confirmation_is_dismissed_by_navigation() {
    let mut state = State::default();
    state.request_clear_history();
    assert!(state.confirm_clear_history);
    state.navigate(Page::History);
    assert!(!state.confirm_clear_history);
}

#[test]
fn production_starts_without_sample_or_synthetic_analysis() {
    let state = State::default();
    assert!(state.game.moves.is_empty());
    assert!(state.real_result.is_none());
    assert!(state.workspace.is_empty());
    assert!(state.candidates().is_empty());
}

#[test]
fn workspace_switch_restores_board_variations_and_result_without_losing_other_reviews() {
    let mut state = State::fixture();
    let first = state.workspace[0].key;
    state.go_to(8);
    state.explore_candidate();
    state.playing = false;
    let variations = state.variations.len();
    state.page = Page::Review;
    state.navigate(Page::Home);
    state.pgn.replace_text("1. d4 d5 2. c4 e6 *");
    state.prepare();
    state.start_review();
    let second = state.active_workspace.unwrap();
    assert_ne!(first, second);
    state.go_to(3);
    state.flipped = true;
    state.activate_review(first);
    assert_eq!(state.ply, 8);
    assert_eq!(state.variations.len(), variations);
    assert!(state.real_result.is_some());
    assert!(!state.flipped);
    state.activate_review(second);
    assert_eq!(state.ply, 3);
    assert!(state.flipped);
    assert_eq!(state.game.moves.len(), 4);
    state.close_review(first);
    assert_eq!(state.active_workspace, Some(second));
    state.close_review(second);
    assert!(state.workspace.is_empty());
    assert!(state.real_result.is_none());
    assert_eq!(state.page, Page::Home);
}

#[test]
fn production_ui_has_no_demo_controls_and_session_close_is_keyboard_accessible() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
    for key in ["mock-toggle", "mock-scenario", "load-demo", "open-demo"] {
        assert!(
            rt.presented(key).is_none(),
            "removed scaffold control {key}"
        );
    }
    assert!(rt.presented("close-session-1").is_some());
    tap(&mut rt, "close-session-1");
    assert!(rt.state.workspace.is_empty());
    assert_eq!(rt.state.page, Page::Home);
}

#[test]
fn session_close_requires_explicit_keyboard_activation() {
    let mut rt = runtime();
    rt.set_tab_visits_controls(true);
    for _ in 0..16 {
        rt.key(rune::KeyEvent {
            key: rune::Key::Tab,
            mods: Default::default(),
        });
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.workspace.len(),
            1,
            "focusing an action must not close the review"
        );
        if rt
            .elements()
            .iter()
            .any(|e| e.focused && e.key.as_deref() == Some("close-session-1"))
        {
            rt.key(rune::KeyEvent {
                key: rune::Key::Text(" ".into()),
                mods: Default::default(),
            });
            rt.frame(1.0 / 60.0);
            assert!(rt.state.workspace.is_empty());
            return;
        }
    }
    panic!("session close could not be reached with Tab");
}

#[test]
fn board_navigation_is_compact_and_disables_boundary_actions() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
    let elements = rt.elements();
    let bar = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("playback"))
        .unwrap();
    assert!(
        bar.rect[3] <= 36.0,
        "navigation consumes too much vertical space"
    );
    for key in ["first", "previous"] {
        let button = elements
            .iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap();
        assert!(!button.focusable, "boundary action {key} must be disabled");
    }
    for key in ["next", "last", "flip"] {
        let button = elements
            .iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap();
        assert!(button.focusable && button.label.is_some());
        assert!(button.rect[2] >= 32.0 && button.rect[3] >= 32.0);
    }
    tap(&mut rt, "last");
    assert_eq!(rt.state.ply, rt.state.game.moves.len());
    assert!(
        !rt.elements()
            .iter()
            .find(|e| e.key.as_deref() == Some("next"))
            .unwrap()
            .focusable
    );
}

#[test]
fn compact_review_keeps_board_navigation_and_tools_inside_the_viewport() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
    rt.set_viewport((720.0, 560.0), 1.0);
    for _ in 0..60 {
        rt.frame(1.0 / 60.0);
    }
    let board = rt.presented("chessboard").unwrap().0;
    assert!(board[2] >= 280.0);
    let playback = rt.presented("playback").unwrap().0;
    assert!(playback[1] + playback[3] <= 560.0);
    tap(&mut rt, "compact-review-tabs/segment2");
    assert!(rt.presented("chessboard").is_none());
    let panel = rt.presented("review-inspector").unwrap().0;
    assert!(panel[3] >= 200.0 && panel[1] + panel[3] <= 560.0);
    tap(&mut rt, "next");
    assert_eq!(rt.state.ply, 1);
    rt.key(toggle_review_key());
    rt.frame(1.0);
    assert!(rt.presented("chessboard").is_some());
    rt.key(toggle_review_key());
    rt.frame(1.0);
    assert_eq!(rt.state.review_tab, state::ReviewTab::Analysis);
    assert!(rt.presented("review-inspector").is_some());
    tap(&mut rt, "compact-review-tabs/segment0");
    assert!(rt.presented("chessboard").is_some());
    assert_eq!(rt.state.ply, 1);
}

#[test]
fn review_shortcuts_work_from_inspector_and_respect_dialogs() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
    tap(&mut rt, "review-tabs/segment2");
    let key = |key| rune::KeyEvent {
        key,
        mods: Default::default(),
    };
    rt.key(key(rune::Key::Right));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.ply, 1, "arrow moves the board with a tab focused");
    assert_eq!(rt.state.review_tab, state::ReviewTab::Summary);
    tap(&mut rt, "evaluation-graph");
    rt.key(key(rune::Key::Left));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.ply, 7, "graph focus moves exactly one position");
    rt.key(key(rune::Key::Text("f".into())));
    rt.frame(1.0 / 60.0);
    assert!(rt.state.flipped, "F flips while the inspector has focus");
    rt.key(key(rune::Key::Home));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.ply, 0);
    rt.key(key(rune::Key::End));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.ply, rt.state.game.moves.len());
    tap(&mut rt, "review-settings");
    assert!(rt.state.analysis_settings_open);
    let ply = rt.state.ply;
    rt.key(key(rune::Key::Left));
    rt.frame(1.0 / 60.0);
    rt.key(key(rune::Key::Text("f".into())));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.ply, ply, "dialog retains keyboard ownership");
    assert!(rt.state.flipped);
}
