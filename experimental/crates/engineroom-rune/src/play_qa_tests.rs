//! Adversarial play QA. Register as a child module of `state` during integration.
use super::{Page, State};
use crate::{game, play::Side};
use shakmaty::Role;

fn promotion_position(state: &mut State) {
    let parsed =
        game::parse("[SetUp \"1\"]\n[FEN \"7k/P7/8/8/8/8/4K3/8 w - - 0 1\"]\n\n1. a8=Q+ *")
            .expect("valid promotion fixture");
    let game = &mut state.play.game.as_mut().expect("active match").game;
    game.positions = vec![parsed.positions[0].clone()];
    state.click_square(8);
    state.click_square(0);
    assert_eq!(state.promotion_pending, Some((8, 0)));
}

#[test]
fn returning_to_play_from_home_preserves_pending_promotion() {
    let mut state = State::default();
    state.start_play();
    promotion_position(&mut state);
    state.navigate(Page::Home);
    state.navigate(Page::Play);
    assert_eq!(
        state.promotion_pending,
        Some((8, 0)),
        "returning to the same match must restore its pending promotion"
    );
    state.promote(Role::Knight);
    assert_eq!(
        state.play.game.as_ref().expect("match").game.moves,
        ["a8=N"]
    );
}

#[test]
fn closing_parked_play_does_not_clear_review_promotion() {
    let mut state = State::fixture();
    let review = state.active_workspace.expect("review");
    state.open_play_setup();
    state.start_play();
    let key = state.play.active.expect("match");
    state.activate_review(review);
    state.game =
        game::parse("[SetUp \"1\"]\n[FEN \"7k/P7/8/8/8/8/4K3/8 w - - 0 1\"]\n\n1. a8=Q+ *")
            .expect("valid promotion fixture");
    state.go_to(0);
    state.click_square(8);
    state.click_square(0);
    assert_eq!(state.promotion_pending, Some((8, 0)));
    state.close_play(key);
    assert_eq!(
        state.promotion_pending,
        Some((8, 0)),
        "closing a parked match must not mutate the active review"
    );
    state.promote(Role::Knight);
    assert_eq!(state.variations[0].san, ["a8=N"]);
}

#[test]
fn fifteen_matches_retain_historical_cursor_and_close_independently() {
    let mut state = State::default();
    for _ in 0..15 {
        state.open_play_setup();
        state.play.side = Side::White;
        state.start_play();
        state.click_square(52);
        state.click_square(36);
        state.first();
    }
    for key in 1..=15 {
        state.activate_play(key);
        assert_eq!(state.play.cursor, 0, "match {key}");
        assert!(!state.play_can_move(), "historical position in match {key}");
        assert_eq!(state.play.game.as_ref().expect("match").game.moves, ["e4"]);
    }
    state.close_play(1);
    assert_eq!(state.play.active, Some(15));
    state.close_play(15);
    assert_eq!(state.play.active, Some(14));
    assert_eq!(state.play.sessions.len(), 13);
}

#[test]
fn resign_from_history_analyzes_complete_game() {
    let mut state = State::default();
    state.start_play();
    state.click_square(52);
    state.click_square(36);
    state.first();
    state.resign_play();
    state.analyze_play();
    assert_eq!(state.page, Page::Configure);
    assert_eq!(state.game.moves, ["e4"]);
}

#[test]
fn fifteen_background_moves_route_to_their_sessions_without_touching_review_state() {
    let mut state = State::fixture();
    let review = state.active_workspace.expect("review");
    let mut requests = Vec::new();
    for _ in 0..15 {
        state.open_play_setup();
        state.start_play();
        state.click_square(52);
        state.click_square(36);
        let key = state.play.active.expect("match");
        if key == 1 {
            state.first();
        }
        let (request, _, _) = state.queue_play(key).expect("engine turn");
        requests.push((key, request));
    }
    state.activate_review(review);
    let selected = Some(12);
    state.selected_square = selected;
    state.promotion_pending = Some((8, 0));
    let review_moves = state.game.moves.clone();
    for (_, request) in requests.iter().rev() {
        state.receive_play_move(*request, Ok("e7e5".into()));
        state.receive_play_move(*request, Ok("b8c6".into())); // duplicate is ignored
    }
    assert_eq!(state.page, Page::Review);
    assert_eq!(state.game.moves, review_moves);
    assert_eq!(state.selected_square, selected);
    assert_eq!(state.promotion_pending, Some((8, 0)));
    for (key, _) in requests {
        state.activate_play(key);
        assert_eq!(
            state.play.game.as_ref().expect("game").game.moves,
            ["e4", "e5"]
        );
        assert_eq!(state.play.cursor, if key == 1 { 0 } else { 2 });
        assert!(!state.play.thinking);
    }
}

#[test]
fn closed_and_resigned_games_ignore_late_results_while_other_matches_complete() {
    let mut state = State::default();
    let mut requests = Vec::new();
    for _ in 0..3 {
        state.open_play_setup();
        state.start_play();
        state.click_square(52);
        state.click_square(36);
        let key = state.play.active.expect("match");
        requests.push(state.queue_play(key).expect("engine turn").0);
    }
    state.close_play(1);
    state.activate_play(2);
    state.resign_play();
    state.receive_play_move(requests[0], Ok("e7e5".into()));
    state.receive_play_move(requests[1], Ok("e7e5".into()));
    state.receive_play_move(requests[2], Ok("e7e5".into()));
    assert!(!state.play.sessions.iter().any(|entry| entry.key == 1));
    assert_eq!(
        state.play.game.as_ref().expect("resigned game").moves.len(),
        1
    );
    state.activate_play(3);
    assert_eq!(state.play.game.as_ref().expect("other game").moves.len(), 2);
}

#[test]
fn background_engine_failure_is_local_and_retry_gets_a_new_request() {
    let mut state = State::default();
    state.start_play();
    state.click_square(52);
    state.click_square(36);
    let (request, _, _) = state.queue_play(1).expect("engine turn");
    state.open_play_setup();
    state.start_play();
    state.receive_play_move(request, Err("engine exited".into()));
    assert!(
        state.play.error.is_none(),
        "failure must not leak to visible game"
    );
    state.activate_play(1);
    assert_eq!(state.play.error.as_deref(), Some("engine exited"));
    state.retry_play();
    let (retry, _, _) = state.queue_play(1).expect("retry ready");
    assert_ne!(retry, request);
    state.receive_play_move(request, Ok("e7e5".into()));
    assert!(state.play.thinking);
    state.receive_play_move(retry, Ok("e7e5".into()));
    assert!(state.play_can_move());
}

#[test]
fn reviewing_a_short_played_game_resets_the_previous_review_cursor() {
    let mut state = State::fixture();
    state.activate_review(state.active_workspace.expect("review"));
    state.last();
    state.open_play_setup();
    state.start_play();
    state.click_square(52);
    state.click_square(36);
    state.resign_play();
    state.analyze_play();
    assert_eq!(state.ply, 0);
    assert_eq!(state.position(), &state.game.positions[0]);
}

#[test]
fn closing_a_parked_match_leaves_the_new_game_form_open() {
    let mut s = State::default();
    s.start_play();
    s.open_play_setup();
    s.start_play();
    s.open_play_setup();
    s.close_play(1);
    assert!(s.play.game.is_none());
    assert_eq!(s.page, Page::Play);
    assert_eq!(s.play.sessions.len(), 1);
}

#[test]
fn activating_play_invalidates_a_pending_history_open() {
    let mut s = State::fixture();
    s.open_play_setup();
    s.start_play();
    let key = s.play.active.expect("match");
    s.navigate(Page::History);
    let request = s.history_load_id;
    let config = s.workspace[0].config.clone();
    s.activate_play(key);
    s.apply_history_load(request, Ok(Some(Box::new(config))), false);
    assert_eq!(s.page, Page::Play);
    assert_eq!(s.play.active, Some(key));
}
