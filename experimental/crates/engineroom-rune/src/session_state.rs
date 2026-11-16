//! Persistent workspace capture and deferred restoration on the UI boundary.
use super::{Page, State};
use crate::session_store::{Document, Event, Restored, SavedPlay, Store};
use std::sync::mpsc;

impl State {
    pub fn setup_startup(&mut self, wake: rune::Waker) {
        self.startup.wake = Some(wake.clone());
        self.start_backend();
        match crate::desktop_paths::data_dir() {
            Ok(dir) => {
                self.startup.store = Some(Store::open(dir.join("workspace.json"), move || (wake)()))
            }
            Err(e) => {
                self.startup.error = Some(e.to_string());
                self.startup.loaded = true;
            }
        }
    }
    fn start_backend(&mut self) {
        if !self.startup.backend_started {
            self.startup.backend_started = true;
            if self.bridge.is_none() {
                let wake = self
                    .startup
                    .wake
                    .as_ref()
                    .expect("native startup waker")
                    .clone();
                let (sender, receiver) = mpsc::channel();
                self.startup.backend = Some(receiver);
                std::thread::spawn(move || {
                    let result = crate::bridge::Bridge::new(wake.clone());
                    let _ = sender.send(result);
                    (wake)();
                });
            }
        }
    }
    pub(crate) fn workspace_document(&mut self) -> Document {
        self.park_review();
        self.save_play();
        Document {
            version: 1,
            reviews: self.workspace.iter().map(|v| v.persisted()).collect(),
            plays: self.play.sessions.iter().map(SavedPlay::capture).collect(),
            active_review: self.active_workspace,
            active_play: self.play.active,
            page: self.page,
            previous_page: self.previous_page,
            section: self.section,
            pgn: self.pgn.text.clone(),
            import_source: self.import_source,
            profile: self.profile,
            depth: self.depth,
            manual_time: self.manual_time,
            sidebar: (self.sidebar.is_open(), self.sidebar.width()),
            inspector: (self.review_sidebar.is_open(), self.review_sidebar.width()),
            draft_fields: crate::session_store::DraftFields {
                history_filter: self.history_filter.text.clone(),
                chess_username: self.chess_username.text.clone(),
                chess_filter: self.chess_filter.text.clone(),
                chess_speed: self.chess_speed,
            },
            play_rating: self.play.rating,
            play_side: self.play.side,
        }
    }
    pub(crate) fn restore_workspace(&mut self, restored: Restored) {
        let Restored {
            document: d,
            reviews,
            plays,
            draft,
        } = restored;
        self.workspace = reviews;
        self.play.sessions = plays;
        self.next_workspace_key = self.workspace.iter().map(|v| v.key).max().unwrap_or(0);
        self.play.next_key = self.play.sessions.iter().map(|v| v.key).max().unwrap_or(0);
        self.active_workspace = None;
        self.play.active = None;
        self.page = Page::Home;
        if let Some(key) = d.active_play {
            self.activate_play(key);
        }
        if let Some(key) = d.active_review {
            self.activate_review(key);
        }
        self.pgn.replace_text(d.pgn);
        self.history_filter
            .replace_text(d.draft_fields.history_filter);
        self.chess_username
            .replace_text(d.draft_fields.chess_username);
        self.chess_filter.replace_text(d.draft_fields.chess_filter);
        self.chess_speed = d.draft_fields.chess_speed.min(4);
        self.play.rating = d
            .play_rating
            .clamp(crate::play::MIN_ELO, crate::play::MAX_ELO);
        self.play.side = d.play_side;
        self.import_source = d.import_source;
        self.profile = d.profile;
        self.depth = d.depth.clamp(1, 128);
        self.manual_time = d.manual_time;
        self.sidebar.set_open(d.sidebar.0);
        self.sidebar.set_width(d.sidebar.1);
        self.review_sidebar.set_open(d.inspector.0);
        self.review_sidebar.set_width(d.inspector.1);
        self.section = d.section;
        self.previous_page = d.previous_page;
        if let Some(game) = draft {
            self.game = game;
            self.ply = 0;
            self.active_variation = None;
        }
        if self.previous_page == Page::Review && self.active_workspace.is_none() {
            self.previous_page = Page::Home;
        }
        self.page = match d.page {
            Page::Review if self.active_workspace.is_none() => Page::Home,
            page => page,
        };
        if self.page == Page::Play {
            self.restore_play_promotion();
        } else if self.page == Page::Review {
            self.restore_review_promotion();
        } else {
            self.promotion_pending = None;
            self.selected_square = None;
        }
    }
    pub fn poll_startup(&mut self, now: f64, tx: &mut rune::Tx) -> bool {
        if !self.startup.enabled() {
            return false;
        }
        if !self.startup.first_poll {
            self.startup.first_poll = true;
            tx.wake_after(std::time::Duration::from_millis(1));
            return false;
        }
        let mut changed = false;
        let events = self
            .startup
            .store
            .as_ref()
            .map_or_else(Vec::new, Store::drain);
        for event in events {
            match event {
                Event::Loaded(result) => {
                    match result {
                        Ok(Some(restored)) => self.restore_workspace(*restored),
                        Ok(None) => {}
                        Err(e) => self.startup.error = Some(e),
                    }
                    self.startup.loaded = true;
                    self.startup.last_checkpoint = now;
                }
                Event::Saved(Err(e)) => self.startup.error = Some(e),
                Event::Saved(Ok(())) => {}
            }
            changed = true;
        }
        if let Some(result) = self
            .startup
            .backend
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
        {
            match result {
                Ok(bridge) => self.bridge = Some(bridge),
                Err(e) => self.bridge_error = Some(e),
            }
            self.startup.backend = None;
            if self.page == Page::History {
                self.refresh_history();
            }
            if self.page == Page::Settings && self.section == super::SettingsSection::Storage {
                self.refresh_storage();
            }
            changed = true;
        }
        if self.startup.loaded && self.bridge.is_some() {
            self.resume_restored_reviews();
            if !self.startup.ready_reported {
                self.startup.ready_reported = true;
                if std::env::var_os("ENGINE_ROOM_STARTUP_METRICS").is_some() {
                    println!("ENGINE_ROOM_READY");
                }
            }
        }
        if self.page == Page::Review && self.avatars.is_none() {
            let wake = self
                .startup
                .wake
                .as_ref()
                .expect("native startup waker")
                .clone();
            self.setup_avatars(wake);
        }
        if self.page == Page::Review && self.startup.avatar_key != self.game.avatar_key {
            self.startup.avatar_key = self.game.avatar_key.clone();
            if let (Some(key), Some(avatars)) = (&self.game.avatar_key, &mut self.avatars) {
                avatars.request(key, 0, None);
                avatars.request(key, 1, None);
            }
        }
        if self.startup.loaded {
            let signature = self.workspace_signature();
            match self.startup.last_signature {
                None => self.startup.last_signature = Some(signature),
                Some(saved) if saved != signature => {
                    let elapsed = now - self.startup.last_checkpoint;
                    if elapsed >= 1.0 {
                        self.startup.last_checkpoint = now;
                        self.startup.last_signature = Some(signature);
                        let doc = self.workspace_document();
                        if let Some(store) = &self.startup.store {
                            store.save(doc);
                        }
                    } else {
                        tx.wake_after(std::time::Duration::from_secs_f64(
                            (1.0 - elapsed).clamp(0.01, 1.0),
                        ));
                    }
                }
                _ => {}
            }
        }
        changed
    }
    fn workspace_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for value in [
            self.page as u8,
            self.previous_page as u8,
            self.section as u8,
            self.import_source as u8,
            self.profile as u8,
            self.review_tab as u8,
        ] {
            value.hash(&mut hash);
        }
        self.pgn.text.hash(&mut hash);
        (
            &self.history_filter.text,
            &self.chess_username.text,
            &self.chess_filter.text,
            self.chess_speed,
            self.play.rating,
            self.play.side as u8,
        )
            .hash(&mut hash);
        (
            self.active_workspace,
            self.play.active,
            self.ply,
            self.flipped,
            self.compact_panel,
            self.depth,
            self.manual_time,
            self.real_result.is_some(),
            self.real_progress,
        )
            .hash(&mut hash);
        (
            self.sidebar.is_open(),
            self.sidebar.width().to_bits(),
            self.review_sidebar.is_open(),
            self.review_sidebar.width().to_bits(),
        )
            .hash(&mut hash);
        (
            self.selected_square,
            self.promotion_pending,
            self.active_variation,
            self.selected_candidate,
        )
            .hash(&mut hash);
        crate::session_store::hash_variations(&self.variations, &mut hash);
        for entry in &self.workspace {
            entry.checkpoint_hash(&mut hash);
        }
        for entry in &self.play.sessions {
            (
                entry.key,
                entry.game.human.is_white(),
                entry.game.rating,
                entry.game.moves.len(),
                entry.game.result,
                entry.cursor,
                entry.flipped,
                entry.promotion,
            )
                .hash(&mut hash);
        }
        (
            self.play.cursor,
            self.play.game.as_ref().map(|g| (g.moves.len(), g.result)),
        )
            .hash(&mut hash);
        hash.finish()
    }
    pub fn flush_workspace(&mut self) {
        if !self.startup.enabled() {
            return;
        }
        // A close before hydration must preserve the existing file rather than
        // replace it with the initial empty screen.
        if !self.startup.loaded {
            return;
        }
        self.poll_bridge();
        let doc = self.workspace_document();
        if let Some(store) = &self.startup.store
            && let Err(error) = store.flush(doc)
        {
            eprintln!("workspace save: {error}");
        }
    }
}
