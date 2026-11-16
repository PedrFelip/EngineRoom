//! Open review snapshots and their independently running sessions.
use super::{Page, Profile, ReviewTab, State};
use crate::{game::Game, variation::Variation};
use engineroom_backend::review::types::{ReviewConfig, ReviewResult};

#[derive(Clone)]
pub(crate) struct WorkspaceReview {
    pub key: u64,
    pub title: String,
    pub config: ReviewConfig,
    game: Game,
    ply: usize,
    flipped: bool,
    tab: ReviewTab,
    compact_panel: bool,
    variations: Vec<Variation>,
    active_variation: Option<usize>,
    selected_candidate: usize,
    selected_square: Option<usize>,
    promotion_pending: Option<(usize, usize)>,
    result: Option<ReviewResult>,
    error: Option<String>,
    session: crate::review_session::ReviewSession,
    progress: Option<(usize, usize)>,
    live_positions:
        std::collections::HashMap<String, engineroom_backend::review::types::PositionAnalysis>,
    live_running: bool,
    live_error: Option<String>,
    live_key: Option<String>,
    profile: Profile,
    depth: usize,
    manual_time: bool,
}
impl WorkspaceReview {
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.progress
    }
    pub fn status(&self) -> &'static str {
        if self.error.is_some() {
            "Erro"
        } else if self.result.is_some() {
            "Concluída"
        } else if self.progress.is_some() {
            "Em andamento"
        } else {
            "Prévia"
        }
    }
    /// Full status for the row tooltip, where the subtitle would truncate.
    pub fn status_detail(&self) -> &'static str {
        if self.error.is_some() {
            "Erro · abra para tentar novamente"
        } else if self.result.is_some() {
            "Análise concluída"
        } else if self.progress.is_some() {
            "Análise em andamento"
        } else {
            "Prévia · nenhuma análise iniciada"
        }
    }
}
impl State {
    fn snapshot(&self, key: u64, config: ReviewConfig) -> WorkspaceReview {
        WorkspaceReview {
            key,
            title: format!("{} × {}", self.game.white, self.game.black),
            config,
            game: self.game.clone(),
            ply: self.ply,
            flipped: self.flipped,
            tab: self.review_tab,
            compact_panel: self.compact_panel,
            variations: self.variations.clone(),
            active_variation: self.active_variation,
            selected_candidate: self.selected_candidate,
            selected_square: self.selected_square,
            promotion_pending: self.promotion_pending,
            result: self.real_result.clone(),
            error: self.bridge_error.clone(),
            session: self.session.clone(),
            progress: self.real_progress,
            live_positions: self.live_positions.clone(),
            live_running: self.live_running,
            live_error: self.live_error.clone(),
            live_key: self.live_key.clone(),
            profile: self.profile,
            depth: self.depth,
            manual_time: self.manual_time,
        }
    }
    pub(crate) fn register_review(&mut self, config: ReviewConfig) {
        self.next_workspace_key = self
            .next_workspace_key
            .checked_add(1)
            .expect("workspace ID exhausted");
        let key = self.next_workspace_key;
        self.workspace.push(self.snapshot(key, config));
        self.active_workspace = Some(key);
    }
    pub(super) fn park_review(&mut self) {
        if let Some(key) = self.active_workspace
            && let Some(index) = self.workspace.iter().position(|s| s.key == key)
        {
            let config = self.workspace[index].config.clone();
            let mut snapshot = self.snapshot(key, config);
            if self.page != Page::Review {
                snapshot.selected_square = self.workspace[index].selected_square;
                snapshot.promotion_pending = self.workspace[index].promotion_pending;
            }
            self.workspace[index] = snapshot;
        }
    }
    pub fn activate_review(&mut self, key: u64) {
        if !self.workspace.iter().any(|review| review.key == key) {
            return;
        }
        if self.page == Page::Play {
            self.park_play();
        }
        self.invalidate_history_load();
        if self.active_workspace == Some(key) && self.page == Page::Review {
            return;
        }
        self.park_review();
        let Some(saved) = self.workspace.iter().find(|s| s.key == key).cloned() else {
            return;
        };
        self.game = saved.game;
        self.ply = saved.ply;
        self.flipped = saved.flipped;
        self.review_tab = saved.tab;
        self.compact_panel = saved.compact_panel;
        self.variations = saved.variations;
        self.active_variation = saved.active_variation;
        self.selected_candidate = saved.selected_candidate;
        self.session = saved.session;
        self.real_progress = saved.progress;
        self.live_positions = saved.live_positions;
        self.live_running = saved.live_running;
        self.live_error = saved.live_error;
        self.live_key = saved.live_key;
        self.real_result = saved.result;
        self.bridge_error = saved.error;
        self.profile = saved.profile;
        self.depth = saved.depth;
        self.manual_time = saved.manual_time;
        self.active_workspace = Some(key);
        self.page = Page::Review;
        self.playing = false;
        self.analysis_settings_open = false;
        self.promotion_pending = saved.promotion_pending;
        self.selected_square = saved.selected_square;
        if self.session.id().is_none() && self.bridge.is_some() {
            self.resume_review(saved.config);
        }
    }
    pub(super) fn restore_review_promotion(&mut self) {
        if let Some(saved) = self
            .workspace
            .iter()
            .find(|entry| Some(entry.key) == self.active_workspace)
        {
            self.promotion_pending = saved.promotion_pending;
            self.selected_square = saved.selected_square;
        }
    }
    fn resume_review(&mut self, mut config: ReviewConfig) {
        config.initial_result = self.real_result.clone();
        if let Some(bridge) = &self.bridge {
            let id = format!(
                "workspace-{}-{}",
                self.active_workspace.unwrap_or(0),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            match bridge.open(&id, config) {
                Ok(()) => {
                    self.session.open(id);
                    self.bridge_error = None;
                    if self.real_result.is_none() {
                        self.real_progress = Some((0, self.game.moves.len() + 1));
                    }
                }
                Err(error) => self.bridge_error = Some(error),
            }
        }
    }
    pub(super) fn with_review_session(&mut self, id: &str, f: impl FnOnce(&mut Self)) -> bool {
        if self.active_workspace.is_some() && self.session.id() == Some(id) {
            f(self);
            return true;
        }
        let Some(index) = self
            .workspace
            .iter()
            .position(|review| review.session.id() == Some(id))
        else {
            return false;
        };
        let mut saved = self.workspace.remove(index);
        self.swap_review_event_state(&mut saved);
        let playing = self.playing;
        f(self);
        self.playing = playing;
        self.swap_review_event_state(&mut saved);
        self.workspace.insert(index, saved);
        true
    }
    fn swap_review_event_state(&mut self, saved: &mut WorkspaceReview) {
        std::mem::swap(&mut self.session, &mut saved.session);
        std::mem::swap(&mut self.real_result, &mut saved.result);
        std::mem::swap(&mut self.real_progress, &mut saved.progress);
        std::mem::swap(&mut self.bridge_error, &mut saved.error);
        std::mem::swap(&mut self.live_positions, &mut saved.live_positions);
        std::mem::swap(&mut self.live_running, &mut saved.live_running);
        std::mem::swap(&mut self.live_error, &mut saved.live_error);
        std::mem::swap(&mut self.live_key, &mut saved.live_key);
        std::mem::swap(&mut self.variations, &mut saved.variations);
        std::mem::swap(&mut self.active_variation, &mut saved.active_variation);
    }
    pub fn retry_review(&mut self) {
        if let Some(key) = self.active_workspace
            && let Some(saved) = self.workspace.iter().find(|s| s.key == key)
        {
            let config = saved.config.clone();
            self.cancel_review();
            self.resume_review(config);
        } else {
            self.start_review();
        }
    }
    pub fn close_review(&mut self, key: u64) {
        let original_page = self.page;
        let Some(index) = self.workspace.iter().position(|s| s.key == key) else {
            return;
        };
        let active = self.active_workspace == Some(key);
        if active {
            self.cancel_review();
            self.active_workspace = None;
        } else if let Some(id) = self.workspace[index].session.close()
            && let Some(bridge) = &self.bridge
        {
            bridge.close(id);
        }
        self.workspace.remove(index);
        if active && original_page != Page::Review {
            self.real_result = None;
            self.bridge_error = None;
            self.game = Game::empty();
            self.variations.clear();
            self.active_variation = None;
            self.ply = 0;
            if self.previous_page == Page::Review {
                self.previous_page = Page::Home;
            }
        }
        if active && original_page == Page::Review {
            if let Some(next) = self
                .workspace
                .get(index.min(self.workspace.len().saturating_sub(1)))
            {
                self.activate_review(next.key);
            } else {
                self.real_result = None;
                self.bridge_error = None;
                self.game = Game::empty();
                self.variations.clear();
                self.active_variation = None;
                self.ply = 0;
                self.page = Page::Home;
            }
        }
    }
}

impl WorkspaceReview {
    pub(crate) fn checkpoint_hash(&self, hash: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        (
            self.key,
            self.ply,
            self.flipped,
            self.tab as u8,
            self.compact_panel,
            self.active_variation,
            self.selected_candidate,
            self.selected_square,
            self.promotion_pending,
        )
            .hash(hash);
        (
            self.result.is_some(),
            self.progress,
            self.profile as u8,
            self.depth,
            self.manual_time,
        )
            .hash(hash);
        crate::session_store::hash_variations(&self.variations, hash);
    }
    pub(crate) fn persisted(&self) -> crate::session_store::SavedReview {
        use crate::session_store::{ReviewView, SavedReview, SavedVariation};
        let mut config = self.config.clone();
        config.initial_result = self.result.clone();
        SavedReview {
            key: self.key,
            config,
            resume: self.progress.is_some() && self.error.is_none(),
            view: ReviewView {
                avatar_key: self.game.avatar_key.clone(),
                ply: self.ply,
                flipped: self.flipped,
                tab: self.tab,
                compact_panel: self.compact_panel,
                variations: self
                    .variations
                    .iter()
                    .map(SavedVariation::capture)
                    .collect(),
                active_variation: self.active_variation,
                selected_candidate: self.selected_candidate,
                selected_square: self.selected_square,
                promotion: self.promotion_pending,
                profile: self.profile,
                depth: self.depth,
                manual_time: self.manual_time,
            },
        }
    }
    pub(crate) fn restore(saved: crate::session_store::SavedReview) -> Result<Self, String> {
        let mut game = crate::game::parse(&saved.config.pgn)?;
        game.avatar_key = saved.view.avatar_key.clone();
        if !(1..=128).contains(&saved.config.engine.depth) || !(1..=5).contains(&saved.config.lines)
        {
            return Err("Configuração de revisão inválida.".into());
        }
        let result = saved
            .config
            .initial_result
            .as_ref()
            .map(|result| {
                engineroom_backend::review::repository::validate_review(result)
                    .map_err(|e| e.to_string())?;
                if result.positions.len() != game.positions.len()
                    || result
                        .positions
                        .iter()
                        .zip(&game.positions)
                        .any(|(a, p)| a.fen != engineroom_backend::review::core::fen(p))
                {
                    return Err("Resultado salvo não corresponde ao PGN.".into());
                }
                Ok::<_, String>(result.clone())
            })
            .transpose()?;
        if !(1..=128).contains(&saved.view.depth) || saved.view.variations.len() > 2000 {
            return Err("Revisão salva fora dos limites.".into());
        }
        let mut variations = Vec::new();
        for (index, v) in saved.view.variations.into_iter().enumerate() {
            variations.push(v.restore(&game, index, &variations)?);
        }
        let active_variation = saved
            .view
            .active_variation
            .filter(|i| *i < variations.len());
        let ply = saved.view.ply.min(game.moves.len());
        let position = active_variation.map_or(&game.positions[ply], |i| variations[i].position());
        let promotion_pending =
            crate::session_store::valid_promotion(position, saved.view.promotion);
        let progress = (saved.resume && result.is_none()).then_some((0, game.positions.len()));
        Ok(Self {
            key: saved.key,
            title: format!("{} × {}", game.white, game.black),
            config: saved.config,
            game,
            ply,
            flipped: saved.view.flipped,
            tab: saved.view.tab,
            compact_panel: saved.view.compact_panel,
            variations,
            active_variation,
            selected_candidate: saved.view.selected_candidate.min(4),
            selected_square: saved.view.selected_square.filter(|s| *s < 64),
            promotion_pending,
            result,
            error: None,
            session: Default::default(),
            progress,
            live_positions: Default::default(),
            live_running: false,
            live_error: None,
            live_key: None,
            profile: saved.view.profile,
            depth: saved.view.depth,
            manual_time: saved.view.manual_time,
        })
    }
}
impl State {
    pub(super) fn resume_restored_reviews(&mut self) {
        let Some(bridge) = &self.bridge else {
            return;
        };
        for entry in &mut self.workspace {
            if self.active_workspace == Some(entry.key) && self.session.id().is_some() {
                entry.session = self.session.clone();
            }
            if entry.session.id().is_some()
                || (entry.progress.is_none() && self.active_workspace != Some(entry.key))
            {
                continue;
            }
            let id = format!("restored-{}-{}", entry.key, self.clock.to_bits());
            let mut config = entry.config.clone();
            config.initial_result = entry.result.clone();
            match bridge.open(&id, config) {
                Ok(()) => entry.session.open(id),
                Err(error) => {
                    entry.error = Some(error);
                    entry.progress = None;
                }
            }
            if self.active_workspace == Some(entry.key) {
                self.session = entry.session.clone();
                self.bridge_error = entry.error.clone();
            }
        }
    }
}
