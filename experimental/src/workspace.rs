//! Open review snapshots; the bridge retains exclusive ownership of one active analysis.
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
    result: Option<ReviewResult>,
    error: Option<String>,
    paused: bool,
    profile: Profile,
    depth: usize,
    manual_time: bool,
    annotation_fen: Option<String>,
    board_annotations: crate::components::board::annotations::Annotations,
}
impl WorkspaceReview {
    pub fn status(&self) -> &'static str {
        if self.error.is_some() {
            "Erro"
        } else if self.result.is_some() {
            "Concluída"
        } else if self.paused {
            "Pausada"
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
        } else if self.paused {
            "Pausada · retoma do cache"
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
            result: self.real_result.clone(),
            error: self.bridge_error.clone(),
            paused: self.real_progress.is_some(),
            profile: self.profile,
            depth: self.depth,
            manual_time: self.manual_time,
            annotation_fen: self.annotation_fen.clone(),
            board_annotations: self.board_annotations.clone(),
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
        self.sync_annotations();
        if let Some(key) = self.active_workspace
            && let Some(index) = self.workspace.iter().position(|s| s.key == key)
        {
            let config = self.workspace[index].config.clone();
            self.workspace[index] = self.snapshot(key, config);
        }
        self.cancel_review();
    }
    pub fn activate_review(&mut self, key: u64) {
        if !self.workspace.iter().any(|review| review.key == key) {
            return;
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
        self.real_result = saved.result;
        self.bridge_error = saved.error;
        self.profile = saved.profile;
        self.depth = saved.depth;
        self.manual_time = saved.manual_time;
        self.annotation_fen = saved.annotation_fen;
        self.board_annotations = saved.board_annotations;
        self.active_workspace = Some(key);
        self.page = Page::Review;
        self.playing = false;
        self.analysis_settings_open = false;
        self.promotion_pending = None;
        self.selected_square = None;
        self.resume_review(saved.config);
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
        self.invalidate_history_load();
        let original_page = self.page;
        let Some(index) = self.workspace.iter().position(|s| s.key == key) else {
            return;
        };
        let active = self.active_workspace == Some(key);
        if active {
            self.cancel_review();
            self.active_workspace = None;
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
