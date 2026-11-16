//! UI intents and stale-response rejection for games against Stockfish.
use super::{Page, State};
use crate::play::{Match, PlaySession, Side};
use shakmaty::Color;
impl State {
    pub fn start_play(&mut self) {
        use std::hash::BuildHasher;
        self.park_play();
        self.park_review();
        self.active_workspace = None;
        let human = match self.play.side {
            Side::White => Color::White,
            Side::Black => Color::Black,
            Side::Random => {
                if std::collections::hash_map::RandomState::new().hash_one(self.play.request) & 1
                    == 0
                {
                    Color::White
                } else {
                    Color::Black
                }
            }
        };
        self.play.next_key += 1;
        self.play.active = Some(self.play.next_key);
        self.play.cursor = 0;
        self.play.game = Some(Match::new(human, self.play.rating));
        self.active_variation = None;
        self.compact_panel = false;
        self.review_sidebar.set_open(true);
        self.play.error = None;
        self.selected_square = None;
        self.promotion_pending = None;
        self.flipped = human.is_black();
        self.page = Page::Play;
        self.save_play();
        self.sync_play();
    }
    pub fn play_can_move(&self) -> bool {
        self.play
            .game
            .as_ref()
            .is_some_and(|g| g.human_turn() && self.play.cursor == g.moves.len())
            && !self.play.thinking
    }
    pub(super) fn push_play_move(&mut self, m: shakmaty::Move) {
        if !self.play_can_move() {
            return;
        }
        if let Some(game) = &mut self.play.game {
            self.play.error = game.push(m).err();
            self.play.cursor = game.moves.len();
        }
        self.selected_square = None;
        self.sync_play();
    }
    pub(super) fn stop_play_search(&mut self) {
        if let Some(entry) = self
            .play
            .sessions
            .iter_mut()
            .find(|entry| Some(entry.key) == self.play.active)
            && let Some(request) = entry.request.take()
            && let Some(bridge) = &self.bridge
        {
            bridge.cancel_play(request);
        }
        self.play.thinking = false;
    }
    // Request identity belongs to the match, never to the visible page.
    pub(super) fn queue_play(&mut self, key: u64) -> Option<(u64, u32, String)> {
        let entry = self
            .play
            .sessions
            .iter_mut()
            .find(|entry| entry.key == key)?;
        if entry.game.result.is_some()
            || entry.game.human_turn()
            || entry.request.is_some()
            || entry.error.is_some()
        {
            return None;
        }
        self.play.request += 1;
        let request = self.play.request;
        entry.request = Some(request);
        if self.play.active == Some(key) {
            self.play.thinking = true;
        }
        Some((request, entry.game.rating, entry.game.position_command()))
    }
    pub(super) fn sync_play(&mut self) {
        self.save_play();
        if self.bridge.is_none() {
            return;
        }
        let keys: Vec<_> = self.play.sessions.iter().map(|entry| entry.key).collect();
        for key in keys {
            if let Some((request, rating, position)) = self.queue_play(key)
                && let Some(bridge) = &self.bridge
            {
                bridge.play_move(request, rating, position);
            }
        }
    }
    pub(super) fn receive_play_move(&mut self, request: u64, result: Result<String, String>) {
        self.save_play();
        let Some(entry) = self
            .play
            .sessions
            .iter_mut()
            .find(|entry| entry.request == Some(request))
        else {
            return;
        };
        entry.request = None;
        let following = entry.cursor == entry.game.moves.len();
        entry.error = result.and_then(|text| entry.game.engine_move(&text)).err();
        if following {
            entry.cursor = entry.game.moves.len();
        }
        if self.play.active == Some(entry.key) {
            self.play.game = Some(entry.game.clone());
            self.play.cursor = entry.cursor;
            self.play.thinking = false;
            self.play.error = entry.error.clone();
            if self.page == Page::Play {
                self.selected_square = None;
            }
        }
    }
    pub fn retry_play(&mut self) {
        self.play.error = None;
        self.sync_play();
    }
    pub fn resign_play(&mut self) {
        self.stop_play_search();
        if let Some(game) = &mut self.play.game {
            game.resign();
        }
        self.selected_square = None;
        self.promotion_pending = None;
    }
    pub fn reset_play(&mut self) {
        self.park_play();
        self.play.active = None;
        self.play.thinking = false;
        self.play.cursor = 0;
        self.play.game = None;
        self.play.error = None;
        self.selected_square = None;
        self.promotion_pending = None;
    }
    pub(super) fn save_play(&mut self) {
        if self.page != Page::Play {
            return;
        }
        let (Some(key), Some(game)) = (self.play.active, &self.play.game) else {
            return;
        };
        // Match history is append-only. Avoid cloning every position on each UI
        // poll when only another session has advanced.
        if self.play.sessions.iter().any(|entry| {
            entry.key == key
                && entry.cursor == self.play.cursor
                && entry.flipped == self.flipped
                && entry.error == self.play.error
                && entry.promotion == self.promotion_pending
                && entry.game.moves.len() == game.moves.len()
                && entry.game.result == game.result
                && entry.game.game.positions.len() == game.game.positions.len()
        }) {
            return;
        }
        let request = self
            .play
            .sessions
            .iter()
            .find(|entry| entry.key == key)
            .and_then(|entry| entry.request);
        let saved = PlaySession {
            request,
            key,
            game: game.clone(),
            cursor: self.play.cursor,
            flipped: self.flipped,
            error: self.play.error.clone(),
            promotion: self.promotion_pending,
        };
        if let Some(entry) = self.play.sessions.iter_mut().find(|entry| entry.key == key) {
            *entry = saved;
        } else {
            self.play.sessions.push(saved);
        }
    }
    pub(super) fn park_play(&mut self) {
        self.save_play();
    }
    pub fn open_play_setup(&mut self) {
        if self.page != Page::Play {
            self.park_review();
            self.active_workspace = None;
        }
        self.reset_play();
        self.navigate(Page::Play);
    }
    pub fn activate_play(&mut self, key: u64) {
        if !self.play.sessions.iter().any(|entry| entry.key == key) {
            return;
        }
        self.invalidate_history_load();
        self.file_context = None;
        if self.page == Page::Play {
            self.park_play();
        } else {
            self.park_review();
            self.active_workspace = None;
        }
        let Some(saved) = self
            .play
            .sessions
            .iter()
            .find(|entry| entry.key == key)
            .cloned()
        else {
            return;
        };
        self.play.active = Some(key);
        self.play.thinking = saved.request.is_some();
        self.play.game = Some(saved.game);
        self.play.cursor = saved.cursor;
        self.play.error = saved.error;
        self.flipped = saved.flipped;
        self.promotion_pending = saved.promotion;
        self.selected_square = None;
        self.active_variation = None;
        self.playing = false;
        self.page = Page::Play;
        self.compact_panel = false;
        self.sync_play();
    }
    pub fn close_play(&mut self, key: u64) {
        let visible =
            self.page == Page::Play && self.play.active == Some(key) && self.play.game.is_some();
        if let Some(entry) = self.play.sessions.iter().find(|entry| entry.key == key)
            && let Some(request) = entry.request
            && let Some(bridge) = &self.bridge
        {
            bridge.cancel_play(request);
        }
        if self.play.active == Some(key) {
            self.play.active = None;
            self.play.game = None;
            self.play.thinking = false;
            if self.page == Page::Play {
                self.selected_square = None;
                self.promotion_pending = None;
            }
        }
        self.play.sessions.retain(|entry| entry.key != key);
        if visible
            && self.play.game.is_none()
            && let Some(next) = self.play.sessions.last()
        {
            self.activate_play(next.key);
        }
    }
    pub(super) fn restore_play_promotion(&mut self) {
        self.promotion_pending = self
            .play
            .sessions
            .iter()
            .find(|entry| Some(entry.key) == self.play.active)
            .and_then(|entry| entry.promotion);
    }
    pub fn play_go_to(&mut self, ply: usize) {
        if let Some(game) = &self.play.game {
            self.play.cursor = ply.min(game.moves.len());
        }
        self.selected_square = None;
        self.promotion_pending = None;
    }
    pub fn analyze_play(&mut self) {
        let Some(game) = &self.play.game else {
            return;
        };
        if game.result.is_none() || game.moves.is_empty() {
            return;
        }
        self.pgn.replace_text(game.pgn());
        self.prepare();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn human_moves_cannot_change_review_or_play_out_of_turn() {
        let mut state = State::default();
        state.play.side = Side::Black;
        state.start_play();
        assert!(state.flipped);
        state.click_square(52); // e2 belongs to the engine
        assert!(state.selected_square.is_none());
        let key = state.play.active.expect("match key");
        let (request, _, _) = state.queue_play(key).expect("engine turn");
        state.receive_play_move(request + 1, Ok("e2e4".into()));
        assert!(state.play.game.as_ref().expect("match").moves.is_empty());
        state.receive_play_move(request, Ok("e2e4".into()));
        assert!(state.play_can_move());
        state.click_square(12); // e7
        state.click_square(28); // e5
        assert_eq!(state.play.game.as_ref().expect("match").moves.len(), 2);
        assert!(state.game.moves.is_empty());
        let (old, _, _) = state.queue_play(key).expect("engine turn");
        state.resign_play();
        state.receive_play_move(old, Ok("g1f3".into()));
        assert_eq!(state.play.game.as_ref().expect("match").moves.len(), 2);
        state.analyze_play();
        assert_eq!(state.page, Page::Configure);
        assert_eq!(state.game.moves, ["e4", "e5"]);
    }
    #[test]
    fn navigation_preserves_search_and_applies_the_background_move() {
        let mut state = State::default();
        state.start_play();
        state.click_square(52);
        state.click_square(36);
        let key = state.play.active.expect("match");
        let (request, _, _) = state.queue_play(key).expect("engine turn");
        state.navigate(Page::Home);
        assert!(state.play.thinking);
        state.receive_play_move(request, Ok("e7e5".into()));
        assert_eq!(state.page, Page::Home);
        state.activate_play(key);
        assert_eq!(
            state.play.game.as_ref().expect("match").game.moves,
            ["e4", "e5"]
        );
        assert!(state.play_can_move());
    }
}

#[cfg(test)]
mod workspace_tests {
    use super::*;
    #[test]
    fn fifteen_games_preserve_moves_colors_cursors_and_ignore_superseded_searches() {
        let mut s = State::default();
        for index in 0..15 {
            s.open_play_setup();
            s.play.side = if index % 2 == 0 {
                Side::White
            } else {
                Side::Black
            };
            s.play.rating = 1400 + index * 50;
            s.start_play();
            if s.play_can_move() {
                s.click_square(52);
                s.click_square(36);
            } else {
                let key = s.play.active.expect("match");
                let (request, _, _) = s.queue_play(key).expect("engine turn");
                s.receive_play_move(request, Ok("e2e4".into()));
            }
            s.first();
        }
        assert_eq!(s.play.sessions.len(), 15);
        for key in 1..=15 {
            s.activate_play(key);
            assert_eq!(s.play.cursor, 0);
            assert_eq!(s.play.game.as_ref().expect("game").game.moves, ["e4"]);
            assert_eq!(s.flipped, key % 2 == 0);
            assert!(
                !s.play_can_move(),
                "historical positions cannot change the match"
            );
            s.last();
            assert_eq!(s.play.cursor, 1);
        }
        s.activate_play(1);
        let (stale, _, _) = s.queue_play(1).expect("engine turn");
        s.activate_play(2);
        s.receive_play_move(stale, Ok("e7e5".into()));
        assert_eq!(s.play.game.as_ref().expect("game").moves.len(), 1);
        assert_eq!(
            s.play
                .sessions
                .iter()
                .find(|e| e.key == 1)
                .expect("first")
                .game
                .moves
                .len(),
            2
        );
        s.close_play(1);
        assert_eq!(s.play.sessions.len(), 14);
        assert_eq!(s.play.active, Some(2));
        s.close_play(2);
        assert_eq!(s.play.sessions.len(), 13);
        assert!(s.play.game.is_some());
    }
    #[test]
    fn review_and_play_switch_without_overwriting_either_game() {
        let mut s = State::fixture();
        let review = s.active_workspace.expect("fixture review");
        s.open_play_setup();
        s.start_play();
        let key = s.play.active.expect("play session");
        s.click_square(52);
        s.click_square(36);
        s.flipped = true;
        s.activate_review(review);
        assert_eq!(s.game.moves.len(), 16);
        s.activate_play(key);
        assert!(s.flipped);
        assert_eq!(s.displayed_game().moves, ["e4"]);
        s.resign_play();
        s.analyze_play();
        assert_eq!(s.page, Page::Configure);
        s.start_review();
        assert_eq!(s.workspace.len(), 2);
        assert_eq!(s.play.sessions.len(), 1);
        s.activate_play(key);
        assert!(s.play.game.as_ref().expect("game").result.is_some());
    }
}
