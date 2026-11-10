//! Typed navigation and view state. No engine or database work runs in UI handlers.
use crate::{
    game::{self, Game},
    review_data,
    variation::Variation,
};
use rune::TextEdit;
use rune_kit::shell::PaneState;
use shakmaty::{Chess, Position, Role, Square};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Home,
    Configure,
    Review,
    History,
    Settings,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Appearance {
    #[default]
    Dark,
    Light,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsSection {
    #[default]
    Preferences,
    Analysis,
    Storage,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImportSource {
    File,
    #[default]
    Paste,
    ChessCom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Profile {
    #[default]
    Fast,
    Deep,
    Manual,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReviewTab {
    #[default]
    Moves,
    Analysis,
    Summary,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LearningSide {
    #[default]
    All,
    White,
    Black,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub appearance: Appearance,
    pub sound: bool,
    pub volume: f32,
    pub live_engine: bool,
    pub feedback: bool,
    pub reduced_motion: bool,
    pub lines: usize,
    pub threads: usize,
    pub hash_mb: usize,
    pub search_seconds: usize,
    pub threads_auto: bool,
    pub chess_username: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            appearance: Appearance::Dark,
            sound: true,
            volume: 0.6,
            live_engine: true,
            feedback: true,
            reduced_motion: false,
            lines: 2,
            threads: 2,
            hash_mb: 256,
            search_seconds: 3,
            threads_auto: true,
            chess_username: String::new(),
        }
    }
}
#[path = "workspace.rs"]
mod workspace;
pub(crate) use workspace::WorkspaceReview;

#[derive(Clone, Debug, PartialEq)]
enum HistoryOperation {
    List,
    Page { created_at: String, id: i64 },
    Open { id: i64, reanalyze: bool },
}

pub struct State {
    avatars: Option<crate::avatar::Avatars>,
    pub workspace: Vec<WorkspaceReview>,
    pub active_workspace: Option<u64>,
    next_workspace_key: u64,
    preference_store: Option<crate::preferences::Store>,
    preference_saved: Option<Preferences>,
    pub preference_error: Option<String>,
    pub bridge: Option<crate::bridge::Bridge>,
    pub bridge_error: Option<String>,
    pub real_result: Option<engineroom_backend::review::types::ReviewResult>,
    pub real_progress: Option<(usize, usize)>,
    session: crate::review_session::ReviewSession,
    pub live_positions:
        std::collections::HashMap<String, engineroom_backend::review::types::PositionAnalysis>,
    pub live_running: bool,
    pub live_error: Option<String>,
    live_key: Option<String>,
    pub page: Page,
    pub previous_page: Page,
    pub section: SettingsSection,
    pub preferences: Preferences,
    pub sidebar: PaneState,
    pub pgn: TextEdit,
    pub import_source: ImportSource,
    pub import_error: Option<String>,
    pub file_loading: bool,
    pub file_drag_over: bool,
    pub pending_picks: usize,
    pub file_dialog: Option<crate::file_dialog::FileDialog>,
    pub chess_username: TextEdit,
    pub chess_filter: TextEdit,
    pub chess_loading: bool,
    pub chess_searched: bool,
    pub chess_games: Vec<engineroom_backend::imports::RemoteGame>,
    pub chess_selected: Option<usize>,
    chess_selected_key: Option<String>,
    pub chess_speed: usize,
    pub chess_cursor: Option<engineroom_backend::imports::Cursor>,
    pub chess_warning: Option<String>,
    chess_request: u64,
    pub engine_probe: crate::engine_probe::Status,
    pub history_filter: TextEdit,
    pub history_confirm_delete: Option<i64>,
    pub history_delete_pending: bool,
    pub history_delete_error: Option<String>,
    pub history_focus_after_delete: bool,
    pub history: Vec<engineroom_backend::db::games::GameSummary>,
    pub history_cursor: Option<engineroom_backend::db::games::GameCursor>,
    pub history_total: i64,
    pub storage_stats: Option<engineroom_backend::db::stats::StorageStats>,
    pub storage_loading: bool,
    pub storage_error: Option<String>,
    pub storage_message: Option<String>,
    pub confirm_clear_history: bool,
    history_load_id: u64,
    pub history_loading: bool,
    pub history_error: Option<String>,
    history_operation: Option<HistoryOperation>,
    pub game: Game,
    pub ply: usize,
    pub flipped: bool,
    pub review_tab: ReviewTab,
    pub compact_panel: bool,
    pub learning_side: LearningSide,
    pub selected_candidate: usize,
    pub analysis_settings_open: bool,
    pub review_sidebar: PaneState,
    annotation_fen: Option<String>,
    pub(crate) board_annotations: crate::components::board::annotations::Annotations,
    pub selected_square: Option<usize>,
    pub promotion_pending: Option<(usize, usize)>,
    pub variations: Vec<Variation>,
    pub active_variation: Option<usize>,
    pub playing: bool,
    pub clock: f64,
    pub next_step: f64,
    pub profile: Profile,
    pub depth: usize,
    pub manual_time: bool,
    pub select_open: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            avatars: None,
            workspace: Vec::new(),
            active_workspace: None,
            next_workspace_key: 0,
            preference_store: None,
            preference_saved: None,
            preference_error: None,
            bridge: None,
            bridge_error: None,
            real_result: None,
            real_progress: None,
            session: crate::review_session::ReviewSession::default(),
            live_positions: std::collections::HashMap::new(),
            live_running: false,
            live_error: None,
            live_key: None,
            page: Page::Home,
            previous_page: Page::Home,
            section: SettingsSection::Preferences,
            preferences: Preferences::default(),
            sidebar: PaneState::new(218.0).range(190.0, 290.0),
            pgn: TextEdit::new(""),
            import_source: ImportSource::Paste,
            import_error: None,
            file_loading: false,
            file_drag_over: false,
            pending_picks: 0,
            file_dialog: None,
            chess_username: TextEdit::new(""),
            chess_filter: TextEdit::new(""),
            chess_loading: false,
            chess_searched: false,
            chess_games: Vec::new(),
            chess_selected: None,
            chess_selected_key: None,
            chess_speed: 0,
            chess_cursor: None,
            chess_warning: None,
            chess_request: 0,
            engine_probe: crate::engine_probe::Status::Idle,
            history_filter: TextEdit::new(""),
            history_confirm_delete: None,
            history_delete_pending: false,
            history_delete_error: None,
            history_focus_after_delete: false,
            history: Vec::new(),
            history_cursor: None,
            history_total: 0,
            storage_stats: None,
            storage_loading: false,
            storage_error: None,
            storage_message: None,
            confirm_clear_history: false,
            history_load_id: 0,
            history_loading: false,
            history_error: None,
            history_operation: None,
            game: Game::empty(),
            ply: 0,
            flipped: false,
            review_tab: ReviewTab::Moves,
            compact_panel: false,
            learning_side: LearningSide::All,
            selected_candidate: 0,
            analysis_settings_open: false,
            review_sidebar: PaneState::new(352.0).range(304.0, 392.0),
            annotation_fen: None,
            board_annotations: Default::default(),
            selected_square: None,
            promotion_pending: None,
            variations: Vec::new(),
            active_variation: None,
            playing: false,
            clock: 0.0,
            next_step: 0.0,
            profile: Profile::Fast,
            depth: 20,
            manual_time: false,
            select_open: false,
        }
    }
}
impl State {
    pub fn fixture_avatars(&mut self) {
        self.avatars = Some(crate::avatar::Avatars::new(
            std::sync::Arc::new(|| {}),
            None,
        ));
        if let Some(avatars) = &mut self.avatars {
            avatars.fixture("white");
        }
        self.game.avatar_key = Some("white".into());
        self.game.ratings = [Some("1842".into()), Some("1768".into())];
    }
    pub fn setup_avatars(&mut self, wake: rune::Waker) {
        self.avatars = Some(crate::avatar::Avatars::new(
            wake,
            crate::desktop_paths::data_dir()
                .ok()
                .map(|dir| dir.join("avatars")),
        ));
    }
    pub fn avatar(&self, index: usize) -> Option<&rune::Image> {
        let key = self.game.avatar_key.as_ref()?;
        self.avatars.as_ref()?.get(key, index)
    }
    pub fn setup_preferences(&mut self, wake: rune::Waker) {
        match crate::desktop_paths::data_dir() {
            Ok(base) => {
                let (store, initial) =
                    crate::preferences::Store::open(base.join("preferences.json"), move || {
                        (wake)()
                    });
                match initial {
                    Ok(Some(preferences)) => {
                        self.chess_username
                            .replace_text(&preferences.chess_username);
                        self.preferences = preferences.clone();
                        self.preference_saved = Some(preferences);
                    }
                    Ok(None) => {}
                    Err(error) => self.preference_error = Some(error),
                }
                self.preference_store = Some(store);
            }
            Err(error) => {
                self.preference_error =
                    Some(format!("Diretório de preferências indisponível: {error}"));
            }
        }
    }
    fn poll_preferences(&mut self) -> bool {
        let Some(store) = &self.preference_store else {
            return false;
        };
        let mut changed = false;
        for event in store.drain() {
            match event {
                // Setup applies the initial read synchronously; the worker only
                // reports save outcomes, so a document never racy-hydrates here.
                Ok(_) => {}
                Err(error) => {
                    self.preference_error = Some(error);
                    changed = true;
                }
            }
        }
        if self
            .preference_saved
            .as_ref()
            .is_some_and(|saved| saved != &self.preferences)
        {
            store.save(self.preferences.clone());
            self.preference_saved = Some(self.preferences.clone());
        }
        changed
    }
    #[expect(dead_code, reason = "component wiring is owned by the design work")]
    pub fn set_appearance(&mut self, appearance: Appearance) {
        self.preferences.appearance = appearance;
    }
    #[expect(dead_code, reason = "component wiring is owned by the design work")]
    pub fn toggle_live_engine(&mut self) {
        self.preferences.live_engine = !self.preferences.live_engine;
    }
    pub fn setup_bridge(&mut self, wake: rune::Waker) {
        match crate::bridge::Bridge::new(wake) {
            Ok(bridge) => self.bridge = Some(bridge),
            Err(error) => self.bridge_error = Some(error),
        }
    }
    pub fn setup_file_dialog(&mut self, wake: rune::Waker) {
        match crate::file_dialog::FileDialog::new(move || (wake)()) {
            Ok(dialog) => self.file_dialog = Some(dialog),
            Err(error) => self.import_error = Some(format!("Seletor de arquivos: {error}")),
        }
    }
    pub fn start_review(&mut self) {
        use engineroom_backend::{
            db::mode::Mode,
            review::{
                core,
                types::{AnalysisKind, EngineTier, ReviewConfig},
            },
        };
        self.park_review();
        self.active_workspace = None;
        self.review_tab = ReviewTab::Moves;
        self.compact_panel = false;
        self.real_result = None;
        self.real_progress = None;
        let game = match core::extract(&self.pgn.text) {
            Ok(game) => game,
            Err(e) => {
                self.bridge_error = Some(e.to_string());
                self.navigate(Page::Review);
                return;
            }
        };
        let config = ReviewConfig {
            pgn: self.pgn.text.clone(),
            meta: game.meta,
            engine: EngineTier {
                id: "custom".into(),
                depth: self.depth as u32,
                label: String::new(),
                hint: String::new(),
            },
            mode: if self.manual_time {
                Mode::Time
            } else {
                Mode::Depth
            },
            analysis_kind: match self.profile {
                Profile::Fast => AnalysisKind::Fast,
                Profile::Deep => AnalysisKind::Deep,
                Profile::Manual => AnalysisKind::Manual,
            },
            movetime_ms: self
                .manual_time
                .then_some((self.preferences.search_seconds * 1000) as u32),
            lines: self.preferences.lines as u32,
            initial_result: None,
        };
        self.reset_annotations();
        self.register_review(config.clone());
        let id = format!(
            "review-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        if let Some(bridge) = &self.bridge {
            match bridge.open(&id, config) {
                Ok(()) => {
                    self.session.open(id);
                    self.bridge_error = None;
                    self.real_progress = Some((0, self.game.moves.len() + 1));
                }
                Err(e) => self.bridge_error = Some(e),
            }
        } else {
            self.bridge_error = Some("Service de análise indisponível.".into());
        }
        self.navigate(Page::Review);
    }
    pub fn cancel_review(&mut self) {
        if let Some(id) = self.session.close()
            && let Some(bridge) = &self.bridge
        {
            bridge.close(id);
        }
        self.real_progress = None;
        self.live_positions.clear();
        self.live_running = false;
        self.live_error = None;
        self.live_key = None;
    }
    fn sync_annotations(&mut self) {
        let fen =
            shakmaty::fen::Fen::from_position(self.position(), shakmaty::EnPassantMode::Legal)
                .to_string();
        if self
            .annotation_fen
            .as_ref()
            .is_some_and(|previous| previous != &fen)
        {
            self.board_annotations.clear();
        }
        self.annotation_fen = Some(fen);
    }
    fn reset_annotations(&mut self) {
        self.board_annotations = Default::default();
        self.annotation_fen = None;
    }
    pub fn poll_bridge(&mut self) -> bool {
        self.sync_live();
        self.sync_annotations();
        let preferences_changed = self.poll_preferences();
        let avatars_changed = self.avatars.as_mut().is_some_and(|avatars| avatars.poll());
        use engineroom_backend::review::types::Event;
        let events = self
            .bridge
            .as_ref()
            .map_or_else(Vec::new, |bridge| bridge.drain());
        let lifecycle = self
            .bridge
            .as_ref()
            .map_or_else(Vec::new, |bridge| bridge.drain_lifecycle());
        let files = self
            .bridge
            .as_ref()
            .map_or_else(Vec::new, |bridge| bridge.drain_files());
        let picks = self
            .file_dialog
            .as_ref()
            .map_or_else(Vec::new, |dialog| dialog.drain());
        let data = self
            .bridge
            .as_ref()
            .map_or_else(Vec::new, |bridge| bridge.drain_data());
        let imports = self
            .bridge
            .as_ref()
            .map_or_else(Vec::new, |bridge| bridge.drain_imports());
        let mut changed = preferences_changed | avatars_changed;
        for event in imports {
            if event.request_id != self.chess_request.to_string() {
                continue;
            }
            self.chess_loading = false;
            match event.body {
                engineroom_backend::imports::EventBody::Completed { result } => {
                    self.chess_searched = true;
                    self.preferences.chess_username = self.chess_username.text.trim().to_owned();
                    self.chess_games.extend(
                        result
                            .items
                            .into_iter()
                            .take(100usize.saturating_sub(self.chess_games.len())),
                    );
                    self.chess_cursor = if self.chess_games.len() < 100 {
                        result.next_cursor
                    } else {
                        None
                    };
                    self.chess_warning = result.warning.map(|e| e.to_string());
                    if self.chess_games.len() >= 100 && self.chess_warning.is_none() {
                        self.chess_warning =
                            Some("Limite de 100 partidas carregadas atingido.".into());
                    }
                    if self.chess_warning.is_none()
                        && (result.skipped_invalid > 0 || result.skipped_unsupported > 0)
                    {
                        self.chess_warning = Some(format!(
                            "Algumas partidas foram ignoradas: {} inválidas e {} não suportadas.",
                            result.skipped_invalid, result.skipped_unsupported
                        ));
                    }
                    if result.stop_reason == engineroom_backend::imports::StopReason::Limit
                        && self.chess_warning.is_none()
                    {
                        self.chess_warning =
                            Some("Busca parcial: limite de arquivos atingido.".into());
                    }
                }
                engineroom_backend::imports::EventBody::Error { error } => {
                    self.import_error = Some(error.to_string())
                }
            }
            changed = true;
        }
        for event in data {
            match event {
                crate::bridge::DataEvent::Probe(result) => {
                    self.engine_probe = match result {
                        Ok(name) => crate::engine_probe::Status::Success(name),
                        Err(error) => crate::engine_probe::Status::Error(error),
                    };
                }
                crate::bridge::DataEvent::Deleted(id, result) => {
                    self.history_delete_pending = false;
                    match result {
                        Ok(()) => {
                            self.history.retain(|game| game.id != id);
                            self.history_total = self.history_total.saturating_sub(1);
                            if self.history_confirm_delete == Some(id) {
                                self.history_confirm_delete = None;
                                self.history_focus_after_delete = true;
                            }
                            self.history_delete_error = None;
                        }
                        Err(error) => self.history_delete_error = Some(error),
                    }
                }
                crate::bridge::DataEvent::Games(Ok(page), append) => {
                    self.apply_history_page(page, append);
                }
                crate::bridge::DataEvent::Storage(result) => {
                    self.storage_loading = false;
                    match result {
                        Ok(stats) => self.storage_stats = Some(stats),
                        Err(error) => self.storage_error = Some(error),
                    }
                }
                crate::bridge::DataEvent::Cleared(result, history) => {
                    self.storage_loading = false;
                    match result {
                        Ok(()) => {
                            self.storage_message = Some(
                                if history {
                                    "Histórico apagado."
                                } else {
                                    "Cache limpo."
                                }
                                .into(),
                            );
                            if history {
                                self.history.clear();
                                self.history_cursor = None;
                                self.history_total = 0;
                            }
                            self.refresh_storage();
                        }
                        Err(error) => self.storage_error = Some(error),
                    }
                }
                crate::bridge::DataEvent::Game(result, reanalyze, request_id) => {
                    self.apply_history_load(request_id, result, reanalyze);
                }
                crate::bridge::DataEvent::Games(Err(error), append) => {
                    self.history_loading = false;
                    self.history_error = Some(error);
                    if !matches!(
                        self.history_operation,
                        Some(HistoryOperation::List | HistoryOperation::Page { .. })
                    ) {
                        self.history_operation = if append {
                            self.history_cursor
                                .as_ref()
                                .map(|cursor| HistoryOperation::Page {
                                    created_at: cursor.created_at.clone(),
                                    id: cursor.id,
                                })
                        } else {
                            Some(HistoryOperation::List)
                        };
                    }
                }
            }
            changed = true;
        }
        for result in files {
            self.file_loading = false;
            match result {
                Ok(pgn) => {
                    self.pgn.replace_text(&pgn);
                    self.prepare();
                }
                Err(error) => self.import_error = Some(error),
            }
            changed = true;
        }
        for result in picks {
            changed = true;
            self.pending_picks = self.pending_picks.saturating_sub(1);
            match result {
                Ok(Some(path)) => {
                    self.read_pgn_path(path);
                }
                Ok(None) => {}
                Err(error) => self.import_error = Some(format!("Seletor de arquivos: {error}")),
            }
        }
        for event in lifecycle {
            use crate::bridge::BridgeLifecycle;
            match event {
                BridgeLifecycle::Opened { id } => self.session.acquired(&id),
                BridgeLifecycle::Closed { id } => self.session.closed(&id),
                BridgeLifecycle::Error { id, error } if self.session.id() == Some(id.as_str()) => {
                    self.bridge_error = Some(error.to_string());
                    self.real_progress = None;
                    self.live_running = false;
                    self.playing = false;
                    self.session.closed(&id);
                }
                BridgeLifecycle::Error { .. } => continue,
            }
            changed = true;
        }
        for envelope in events {
            if !self
                .session
                .accepts(&envelope.session_id, envelope.sequence, envelope.request_id)
            {
                continue;
            }
            changed = true;
            match envelope.event {
                Event::Progress { progress } => {
                    self.real_progress = Some((progress.completed, progress.total))
                }
                Event::Completed { result } => {
                    self.real_result = Some(result);
                    self.real_progress = None;
                    self.live_key = None;
                }
                Event::Error {
                    error,
                    fen: Some(_),
                } => {
                    self.stop_live_with_error(error.to_string());
                }
                Event::Error { error, fen: None } => {
                    self.bridge_error = Some(error.to_string());
                    self.real_progress = None;
                    self.live_running = false;
                    self.playing = false;
                }
                Event::LiveStarted { .. } => {
                    self.live_running = true;
                    self.live_error = None;
                }
                Event::LiveCompleted { analysis, .. } => {
                    self.live_running = false;
                    let replace = self
                        .live_positions
                        .get(&analysis.fen)
                        .is_none_or(|old| prefer_live(old, &analysis));
                    if replace {
                        self.live_positions.insert(analysis.fen.clone(), analysis);
                    }
                }
                Event::Classification {
                    node_id,
                    classification,
                } => {
                    self.apply_variation_classification(&node_id, classification);
                }
                Event::Warning { error } => eprintln!("review warning: {error}"),
            }
        }
        self.sync_live();
        changed
    }
    fn sync_live(&mut self) {
        use engineroom_backend::review::types::{LiveRequest, LiveSettings};
        use shakmaty::{EnPassantMode, fen::Fen};
        let Some(id) = self.session.id().map(str::to_owned) else {
            return;
        };
        if matches!(
            self.session.phase,
            crate::review_session::Phase::Closing | crate::review_session::Phase::Closed
        ) {
            return;
        }
        let fen = Fen::from_position(self.position(), EnPassantMode::Legal).to_string();
        let node_id = self.variation_node_id();
        let source_fen = self.active_variation.and_then(|i| {
            let v = &self.variations[i];
            v.cursor
                .checked_sub(1)
                .map(|j| engineroom_backend::review::core::fen(&v.positions[j]))
        });
        let source_analysis = source_fen
            .as_deref()
            .and_then(|fen| self.analysis_for_fen(fen))
            .cloned();
        let p = &self.preferences;
        let key = format!(
            "{fen}:{node_id:?}:{}:{}:{}:{}:{}:{}:{}:{}",
            p.live_engine,
            p.search_seconds,
            p.lines,
            p.threads_auto,
            p.threads,
            p.hash_mb,
            p.feedback,
            self.playing
        );
        if self.live_key.as_ref() == Some(&key) {
            return;
        }
        self.live_key = Some(key);
        let request_id = self.session.invalidate_live();
        self.live_running = p.live_engine && self.real_result.is_some();
        self.live_error = None;
        if let Some(bridge) = &self.bridge {
            if p.live_engine && self.real_result.is_some() {
                let request = LiveRequest {
                    fen,
                    variation_node_id: node_id,
                    source_fen,
                    source_analysis,
                };
                let settings = LiveSettings {
                    search_seconds: p.search_seconds as u32,
                    lines: p.lines as u32,
                    threads_auto: p.threads_auto,
                    threads: p.threads as u32,
                    memory_mb: p.hash_mb as u32,
                    move_feedback_enabled: p.feedback,
                    fast_pass: self.playing,
                };
                if let Err(error) = bridge.analyze(&id, request_id, request, settings) {
                    self.stop_live_with_error(error);
                }
            } else {
                bridge.cancel_live(id, request_id);
            }
        }
    }
    fn stop_live_with_error(&mut self, error: String) {
        self.live_running = false;
        self.playing = false;
        if let Some(key) = &mut self.live_key
            && let Some(prefix) = key.strip_suffix(":true")
        {
            *key = format!("{prefix}:false");
        }
        self.live_error = Some(error);
    }
    pub fn classification(&self, ply: usize) -> Option<review_data::Classification> {
        if ply == 0 {
            return None;
        }

        let class = self
            .real_result
            .as_ref()?
            .moves
            .iter()
            .find(|m| m.played.ply == ply)?
            .classification;
        Some(display_classification(class))
    }

    pub(crate) fn invalidate_history_load(&mut self) {
        self.history_load_id = self
            .history_load_id
            .checked_add(1)
            .expect("history request ID exhausted");
        self.history_loading = false;
    }
    pub(crate) fn apply_history_load(
        &mut self,
        request_id: u64,
        result: Result<Option<Box<engineroom_backend::review::types::ReviewConfig>>, String>,
        reanalyze: bool,
    ) {
        if request_id != self.history_load_id {
            return;
        }
        match result {
            Ok(Some(mut config)) => {
                self.history_loading = false;
                match game::parse(&config.pgn) {
                    Ok(mut game) => {
                        self.history_error = None;
                        self.history_operation = None;
                        let key = crate::avatar::game_key(&config.pgn);
                        if let Some(avatars) = &mut self.avatars {
                            for side in 0..2 {
                                avatars.request(&key, side, None);
                            }
                        }
                        game.avatar_key = Some(key);
                        self.park_review();
                        self.game = game;
                        self.reset_annotations();
                        self.go_to(0);
                        self.variations.clear();
                        self.real_result = None;
                        self.bridge_error = None;
                        if reanalyze {
                            config.initial_result = None;
                        }
                        self.register_review((*config).clone());
                        let id = format!(
                            "history-{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos()
                        );
                        if let Some(bridge) = &self.bridge {
                            match bridge.open(&id, *config) {
                                Ok(()) => {
                                    self.session.open(id);
                                    self.real_progress = Some((0, self.game.moves.len() + 1));
                                }
                                Err(e) => self.bridge_error = Some(e),
                            }
                        }
                        self.navigate(Page::Review);
                    }
                    Err(error) => self.history_error = Some(error),
                }
            }
            Ok(None) => {
                self.history_loading = false;
                self.history_error = Some("Partida não encontrada.".into());
            }
            Err(error) => {
                self.history_loading = false;
                self.history_error = Some(error);
            }
        }
    }
    pub fn navigate(&mut self, page: Page) {
        self.invalidate_history_load();
        if page == Page::Home && self.page != Page::Home {
            self.park_review();
            self.active_workspace = None;
        }
        if page == Page::History && self.page != Page::History {
            self.refresh_history();
        }
        self.page = page;
        self.playing = false;
        self.analysis_settings_open = false;
        self.promotion_pending = None;
        self.select_open = false;
        self.confirm_clear_history = false;
    }
    pub(crate) fn apply_history_page(
        &mut self,
        page: engineroom_backend::db::games::GamePage,
        append: bool,
    ) {
        self.history_loading = false;
        self.history_error = None;
        self.history_operation = None;
        self.history_total = page.total;
        if append {
            for game in page.games {
                if !self.history.iter().any(|existing| existing.id == game.id) {
                    self.history.push(game);
                }
            }
        } else {
            self.history = page.games;
        }
        self.history_cursor = page.next_cursor;
    }
    fn run_history_operation(&mut self, operation: HistoryOperation) {
        if self.history_loading {
            return;
        }
        self.history_operation = Some(operation.clone());
        let result = match (&self.bridge, operation) {
            (Some(bridge), HistoryOperation::List) => bridge.list_games(None),
            (Some(bridge), HistoryOperation::Page { created_at, id }) => {
                bridge.list_games(Some(engineroom_backend::db::games::GameCursor {
                    created_at,
                    id,
                }))
            }
            (Some(bridge), HistoryOperation::Open { id, reanalyze }) => {
                bridge.load_game(id, reanalyze, self.history_load_id)
            }
            (None, _) => {
                self.history_error = None;
                return;
            }
        };
        match result {
            Ok(()) => {
                self.history_loading = true;
                self.history_error = None;
            }
            Err(error) => self.history_error = Some(error),
        }
    }
    pub fn refresh_history(&mut self) {
        self.run_history_operation(HistoryOperation::List);
    }
    pub fn next_history(&mut self) {
        if let Some(cursor) = &self.history_cursor {
            self.run_history_operation(HistoryOperation::Page {
                created_at: cursor.created_at.clone(),
                id: cursor.id,
            });
        }
    }
    pub(crate) fn history_error_context(&self) -> &'static str {
        match self.history_operation {
            Some(HistoryOperation::List) => "Não foi possível carregar o histórico",
            Some(HistoryOperation::Page { .. }) => "Não foi possível carregar mais partidas",
            Some(HistoryOperation::Open {
                reanalyze: false, ..
            }) => "Não foi possível abrir a partida",
            Some(HistoryOperation::Open {
                reanalyze: true, ..
            }) => "Não foi possível reanalisar a partida",
            None => "Erro no histórico",
        }
    }
    pub fn retry_history(&mut self) {
        if let Some(operation) = self.history_operation.clone() {
            if matches!(operation, HistoryOperation::Open { .. }) {
                self.invalidate_history_load();
            }
            self.run_history_operation(operation);
        }
    }
    pub fn refresh_storage(&mut self) {
        if let Some(bridge) = &self.bridge {
            match bridge.storage_stats() {
                Ok(()) => {
                    self.storage_loading = true;
                    self.storage_error = None;
                }
                Err(e) => self.storage_error = Some(e),
            }
        }
    }
    pub fn request_clear_history(&mut self) {
        if self.storage_loading {
            return;
        }
        self.confirm_clear_history = true;
        self.storage_message = None;
    }
    pub fn cancel_clear_history(&mut self) {
        self.confirm_clear_history = false;
    }
    pub fn clear_storage(&mut self, history: bool) {
        if self.storage_loading {
            return;
        }
        self.confirm_clear_history = false;
        if let Some(bridge) = &self.bridge {
            let result = if history {
                bridge.clear_history()
            } else {
                bridge.clear_cache()
            };
            match result {
                Ok(()) => {
                    self.storage_loading = true;
                    self.storage_error = None;
                    self.storage_message = None;
                }
                Err(e) => self.storage_error = Some(e),
            }
        }
    }
    pub fn history_open(&mut self, id: i64, reanalyze: bool) {
        self.invalidate_history_load();
        self.run_history_operation(HistoryOperation::Open { id, reanalyze });
    }
    pub fn start_engine_probe(&mut self) {
        if matches!(self.engine_probe, crate::engine_probe::Status::Running) {
            return;
        }
        if let Some(bridge) = &self.bridge {
            self.engine_probe = crate::engine_probe::Status::Running;
            bridge.start_probe();
        } else {
            self.engine_probe =
                crate::engine_probe::Status::Error("Serviço de motor indisponível.".into());
        }
    }
    pub fn request_history_delete(&mut self, id: i64) {
        if !self.history_delete_pending {
            self.history_confirm_delete = Some(id);
            self.history_delete_error = None;
        }
    }
    pub fn cancel_history_delete(&mut self) {
        if !self.history_delete_pending {
            self.history_confirm_delete = None;
            self.history_delete_error = None;
        }
    }
    pub fn confirm_history_delete(&mut self) {
        if self.history_delete_pending {
            return;
        }
        let Some(id) = self.history_confirm_delete else {
            return;
        };
        let result = self
            .bridge
            .as_ref()
            .ok_or_else(|| "Histórico indisponível".to_string())
            .and_then(|bridge| bridge.delete_game(id));
        match result {
            Ok(()) => {
                self.history_delete_pending = true;
                self.history_delete_error = None;
            }
            Err(error) => self.history_delete_error = Some(error),
        }
    }
    pub fn open_settings(&mut self) {
        if self.page != Page::Settings {
            self.previous_page = self.page;
            self.navigate(Page::Settings);
        }
    }
    pub fn cancel_chesscom(&mut self) {
        if let Some(bridge) = &self.bridge {
            bridge.cancel_chesscom(self.chess_request.to_string());
        }
        self.chess_request += 1;
        self.chess_loading = false;
    }
    #[expect(dead_code, reason = "component wiring is owned by the design work")]
    pub fn select_import_source(&mut self, source: ImportSource) {
        self.cancel_chesscom();
        self.import_source = source;
        self.import_error = None;
        self.file_drag_over = false;
        if source == ImportSource::ChessCom
            && !self.preferences.chess_username.is_empty()
            && self.chess_games.is_empty()
            && !self.chess_searched
        {
            self.search_chesscom();
        }
    }
    pub fn chess_username_changed(&mut self) {
        self.chess_filter.replace_text("");
        self.chess_searched = false;
        self.chess_selected = None;
        self.chess_selected_key = None;
        self.cancel_chesscom();
        self.chess_games.clear();
        self.chess_cursor = None;
        self.chess_warning = None;
        self.import_error = None;
    }
    pub fn forget_chesscom(&mut self) {
        self.preferences.chess_username.clear();
        self.chess_username.replace_text("");
        self.chess_username_changed();
    }
    pub fn search_chesscom(&mut self) {
        self.chess_filter.replace_text("");
        self.chess_searched = false;
        self.chess_selected = None;
        self.chess_selected_key = None;
        self.chess_games.clear();
        self.chess_cursor = None;
        self.start_chess_search(None);
    }
    pub fn next_chesscom(&mut self) {
        if let Some(cursor) = self.chess_cursor.take() {
            self.start_chess_search(Some(cursor));
        }
    }
    fn start_chess_search(&mut self, cursor: Option<engineroom_backend::imports::Cursor>) {
        self.cancel_chesscom();
        self.import_error = None;
        self.chess_warning = None;
        let request = engineroom_backend::imports::SearchRequest {
            owner: "window".into(),
            request_id: self.chess_request.to_string(),
            username: self.chess_username.text.trim().into(),
            cursor,
        };
        match self
            .bridge
            .as_ref()
            .ok_or_else(|| "Serviço de importação indisponível.".to_string())
            .and_then(|bridge| bridge.search_chesscom(request))
        {
            Ok(()) => self.chess_loading = true,
            Err(error) => {
                self.chess_loading = false;
                self.import_error = Some(error);
            }
        }
    }
    pub fn select_chesscom(&mut self, index: usize) {
        if let Some(game) = self.chess_games.get(index) {
            let players = [game.white.username.clone(), game.black.username.clone()];
            self.pgn.replace_text(&game.pgn);
            self.cancel_chesscom();
            self.prepare();
            if self.page == Page::Configure {
                let key = crate::avatar::game_key(&self.pgn.text);
                if let Some(avatars) = &mut self.avatars {
                    for (side, name) in players.iter().enumerate() {
                        avatars.request(&key, side, Some(name));
                    }
                }
                self.game.avatar_key = Some(key);
            }
        }
    }
    pub fn chess_select(&mut self, index: usize) {
        if let Some(game) = self.chess_games.get(index) {
            self.chess_selected = Some(index);
            self.chess_selected_key = Some(chess_game_key(game));
        }
    }
    pub fn chess_sync_selection(&mut self, speed: usize, query: &str) {
        let query = query.trim().to_lowercase();
        let Some(key) = self.chess_selected_key.as_deref() else {
            self.chess_selected = None;
            return;
        };
        self.chess_selected = self.chess_games.iter().position(|game| {
            chess_game_key(game) == key
                && (speed == 0
                    || game.time_class.as_deref()
                        == Some(["", "bullet", "blitz", "rapid", "daily"][speed]))
                && (query.is_empty()
                    || game.white.username.to_lowercase().contains(&query)
                    || game.black.username.to_lowercase().contains(&query))
        });
    }
    pub fn pick_pgn_file(&mut self) {
        let Some(dialog) = &self.file_dialog else {
            self.import_error = Some("Seletor de arquivos indisponível.".into());
            return;
        };
        match dialog.request() {
            Ok(()) => {
                self.pending_picks += 1;
                self.import_error = None;
            }
            Err(error) => self.import_error = Some(format!("Seletor de arquivos: {error}")),
        }
    }
    pub fn drop_pgn_files(&mut self, files: &[std::path::PathBuf]) {
        if self.file_loading || self.pending_picks > 0 {
            return;
        }
        if let [path] = files {
            self.read_pgn_path(path.clone());
        } else {
            self.import_error = Some("Escolha apenas um arquivo PGN.".into());
        }
    }
    fn read_pgn_path(&mut self, path: std::path::PathBuf) {
        if !crate::file_dialog::accepts_pgn_path(&path) {
            self.import_error = Some("Escolha um arquivo com extensão .pgn ou .txt.".into());
            return;
        }
        match self
            .bridge
            .as_ref()
            .ok_or_else(|| "Serviço de importação indisponível.".to_string())
            .and_then(|bridge| bridge.read_file(path))
        {
            Ok(()) => {
                self.file_loading = true;
                self.import_error = None;
            }
            Err(error) => self.import_error = Some(error),
        }
    }
    pub fn prepare(&mut self) {
        match game::parse(&self.pgn.text) {
            Ok(game) => {
                self.park_review();
                self.active_workspace = None;
                self.game = game;
                self.reset_annotations();
                self.go_to(0);
                self.variations.clear();
                self.import_error = None;
                self.navigate(Page::Configure);
            }
            Err(error) => self.import_error = Some(error),
        }
    }

    pub fn go_to(&mut self, ply: usize) {
        self.promotion_pending = None;
        let previous_ply = self.ply;
        let was_mainline = self.active_variation.is_none();
        self.ply = ply.min(self.game.moves.len());
        if was_mainline
            && self.bridge.is_some()
            && let Some(sound) = crate::audio::forward_sound(
                previous_ply,
                self.ply,
                self.ply
                    .checked_sub(1)
                    .and_then(|i| self.game.moves.get(i))
                    .map(String::as_str),
                self.preferences.sound,
            )
        {
            crate::audio::play(sound, self.preferences.volume);
        }
        self.active_variation = None;
        self.playing = false;
        self.selected_square = None;
        self.selected_candidate = 0;
    }
    pub fn go_to_variation(&mut self, index: usize, cursor: usize) {
        let Some(variation) = self.variations.get_mut(index) else {
            return;
        };
        variation.cursor = cursor.min(variation.moves.len());
        self.ply = variation.root_ply;
        self.active_variation = Some(index);
        self.playing = false;
        self.selected_square = None;
        self.selected_candidate = 0;
    }
    pub fn next(&mut self) {
        self.playing = false;
        self.selected_square = None;
        self.selected_candidate = 0;
        if let Some(index) = self.active_variation {
            let v = &mut self.variations[index];
            v.cursor = (v.cursor + 1).min(v.moves.len());
        } else {
            self.go_to(self.ply + 1);
        }
    }
    pub fn previous(&mut self) {
        self.playing = false;
        self.selected_square = None;
        self.selected_candidate = 0;
        if let Some(index) = self.active_variation {
            if let Some((parent, cursor)) = self.variations[index].branch_from
                && self.variations[index].cursor <= cursor
            {
                self.go_to_variation(parent, cursor);
                self.previous();
                return;
            }
            if self.variations[index].cursor == 0 {
                self.go_to(self.variations[index].root_ply);
                self.previous();
                return;
            }
            let v = &mut self.variations[index];
            v.cursor = v.cursor.saturating_sub(1);
        } else {
            self.go_to(self.ply.saturating_sub(1));
        }
    }
    pub fn first(&mut self) {
        self.selected_candidate = 0;
        self.selected_square = None;
        if let Some(i) = self.active_variation {
            self.variations[i].cursor = self.variations[i]
                .branch_from
                .map_or(0, |(_, cursor)| cursor);
            self.playing = false;
        } else {
            self.go_to(0);
        }
    }
    pub fn last(&mut self) {
        self.selected_candidate = 0;
        self.selected_square = None;
        if let Some(i) = self.active_variation {
            self.variations[i].cursor = self.variations[i].moves.len();
            self.playing = false;
        } else {
            self.go_to(self.game.moves.len());
        }
    }
    pub fn position(&self) -> &Chess {
        match self.active_variation {
            Some(i) => self.variations[i].position(),
            None => &self.game.positions[self.ply],
        }
    }
    pub fn visible_variation_cursor(&self) -> Option<(usize, usize)> {
        let mut index = self.active_variation?;
        let cursor = self.variations[index].cursor;
        while let Some((parent, split)) = self.variations[index].branch_from {
            if cursor > split {
                break;
            }
            index = parent;
        }
        Some((index, cursor))
    }
    pub fn displayed_ply(&self) -> usize {
        self.active_variation.map_or(self.ply, |i| {
            self.variations[i].root_ply + self.variations[i].cursor
        })
    }
    pub fn last_move(&self) -> Option<(usize, usize)> {
        if let Some(i) = self.active_variation {
            let v = &self.variations[i];
            if let Some(j) = v.cursor.checked_sub(1) {
                return Some(review_data::squares(v.moves[j]));
            }
        }
        self.ply
            .checked_sub(1)
            .and_then(|i| {
                self.game.moves[i]
                    .parse::<shakmaty::san::SanPlus>()
                    .ok()?
                    .san
                    .to_move(&self.game.positions[i])
                    .ok()
            })
            .map(review_data::squares)
    }
    fn variation_node_id(&self) -> Option<String> {
        let i = self.active_variation?;
        let variation = self.variations.get(i)?;
        if variation.cursor == 0 {
            return None;
        }
        Some(format!(
            "{}:{}:{}:{}",
            i,
            variation.cursor,
            engineroom_backend::review::core::fen(&variation.positions[variation.cursor - 1]),
            engineroom_backend::review::core::fen(variation.position())
        ))
    }
    fn apply_variation_classification(
        &mut self,
        node_id: &str,
        classification: engineroom_backend::review::types::Classification,
    ) {
        if self.variation_node_id().as_deref() != Some(node_id) {
            return;
        }
        if let Some((i, cursor)) = self.visible_variation_cursor()
            && let Some(j) = cursor.checked_sub(1)
            && let Some(slot) = self.variations[i].classifications.get_mut(j)
        {
            *slot = Some(classification);
        }
    }
    fn analysis_for_fen(
        &self,
        fen: &str,
    ) -> Option<&engineroom_backend::review::types::PositionAnalysis> {
        let original = self
            .real_result
            .as_ref()
            .and_then(|result| result.positions.iter().find(|p| p.fen == fen));
        let live = self.live_positions.get(fen);
        match (original, live) {
            (Some(a), Some(b)) => Some(if prefer_incoming(a, b) { b } else { a }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
    pub fn position_analysis(
        &self,
    ) -> Option<&engineroom_backend::review::types::PositionAnalysis> {
        self.analysis_for_fen(&engineroom_backend::review::core::fen(self.position()))
    }
    pub fn displayed_classification(&self) -> Option<review_data::Classification> {
        if let Some((i, cursor)) = self.visible_variation_cursor() {
            let variation = self.variations.get(i)?;
            if cursor == 0 {
                return self.classification(variation.root_ply);
            }
            let class = variation
                .classifications
                .get(cursor.checked_sub(1)?)?
                .as_ref()?;
            Some(display_classification(*class))
        } else {
            self.classification(self.ply)
        }
    }
    pub fn candidates(&self) -> Vec<review_data::Candidate> {
        use shakmaty::{CastlingMode, san::SanPlus, uci::UciMove};
        let fen = engineroom_backend::review::core::fen(self.position());
        let original = self
            .real_result
            .as_ref()
            .and_then(|result| result.positions.iter().find(|p| p.fen == fen));
        let live = self.live_positions.get(&fen);
        let analysis = match (original, live) {
            (Some(a), Some(b)) if (b.lines.len(), b.depth) > (a.lines.len(), a.depth) => Some(b),
            (Some(a), _) => Some(a),
            (None, b) => b,
        };
        analysis
            .map(|position| {
                position
                    .lines
                    .iter()
                    .map(|line| {
                        let mut board = self.position().clone();
                        let mut moves = Vec::new();
                        let mut san = Vec::new();
                        for uci in &line.pv {
                            let Ok(m) = uci.parse::<UciMove>().and_then(|u| {
                                u.to_move(&board)
                                    .map_err(|_| shakmaty::uci::ParseUciMoveError)
                            }) else {
                                break;
                            };
                            san.push(
                                SanPlus::from_move_and_play_unchecked(&mut board, m).to_string(),
                            );
                            moves.push(m);
                        }
                        let _ = CastlingMode::Standard;
                        review_data::Candidate {
                            moves,
                            san,
                            cp: line.cp,
                            win_pct: line.win_pct,
                            depth: line.depth.unwrap_or(position.depth) as usize,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn explore_candidate(&mut self) {
        let lines = self.candidates();
        let Some(line) = lines.get(self.selected_candidate) else {
            return;
        };
        if line.moves.is_empty() {
            return;
        }
        let mut variation = self.active_variation.map_or_else(
            || Variation::new(self.ply, self.position().clone()),
            |i| self.variations[i].clone(),
        );
        if let Some(parent) = self.active_variation {
            variation.branch_from = Some((parent, variation.cursor));
        }
        let start = variation.cursor;
        for m in &line.moves {
            variation.append(*m);
        }
        variation.cursor = start;
        self.variations.push(variation);
        self.active_variation = Some(self.variations.len() - 1);
        self.selected_square = None;
        self.playing = true;
        self.next_step = self.clock + 0.65;
    }
    #[expect(dead_code, reason = "component wiring is owned by the design work")]
    pub fn toggle_playback(&mut self) {
        self.selected_square = None;
        self.selected_candidate = 0;
        if !self.playing
            && let Some(i) = self.active_variation
            && self.variations[i].cursor == self.variations[i].moves.len()
        {
            self.variations[i].cursor = self.variations[i]
                .branch_from
                .map_or(0, |(_, cursor)| cursor);
        }
        self.playing = !self.playing;
        self.next_step = self.clock + 0.65;
    }
    pub fn tick(&mut self, now: f64) -> bool {
        self.clock = now;
        if !self.playing || now < self.next_step {
            return false;
        }
        // Keep playback on the current position until its live search has completed;
        // manual navigation still cancels it immediately through the session intent.
        if self.live_running && self.preferences.live_engine && self.real_result.is_some() {
            return false;
        }
        let Some(i) = self.active_variation else {
            self.playing = false;
            return false;
        };
        let v = &mut self.variations[i];
        v.cursor = (v.cursor + 1).min(v.moves.len());
        self.selected_square = None;
        self.selected_candidate = 0;
        self.playing = v.cursor < v.moves.len();
        self.next_step = now + 0.65;
        true
    }
    pub fn click_square(&mut self, index: usize) {
        if self.promotion_pending.is_some() {
            return;
        }
        self.playing = false;
        let target = Square::new((index ^ 56) as u32);
        if let Some(from) = self.selected_square {
            let source = Square::new((from ^ 56) as u32);
            let m = self
                .position()
                .legal_moves()
                .into_iter()
                .find(|m| m.from() == Some(source) && review_data::squares(*m).1 == index);
            if let Some(m) = m {
                if m.promotion().is_some() {
                    self.promotion_pending = Some((from, index));
                    self.selected_square = None;
                    return;
                }
                self.play_exploration_move(m);
                return;
            }
        }
        self.selected_square = self
            .position()
            .board()
            .piece_at(target)
            .filter(|p| p.color == self.position().turn())
            .map(|_| index);
    }
    pub fn promote(&mut self, role: Role) {
        let Some((from, target)) = self.promotion_pending.take() else {
            return;
        };
        let source = Square::new((from ^ 56) as u32);
        if let Some(m) = self.position().legal_moves().into_iter().find(|m| {
            m.from() == Some(source)
                && review_data::squares(*m).1 == target
                && m.promotion() == Some(role)
        }) {
            self.play_exploration_move(m);
        }
    }
    fn play_exploration_move(&mut self, m: shakmaty::Move) {
        if let Some(i) = self.active_variation {
            if self.variations[i].cursor < self.variations[i].moves.len() {
                let mut branch = self.variations[i].clone();
                branch.branch_from = Some((i, branch.cursor));
                self.variations.push(branch);
                self.active_variation = Some(self.variations.len() - 1);
            }
        } else {
            self.variations
                .push(Variation::new(self.ply, self.position().clone()));
            self.active_variation = Some(self.variations.len() - 1);
        }
        self.variations[self
            .active_variation
            .expect("variation created before append")]
        .append(m);
        self.selected_square = None;
        self.selected_candidate = 0;
    }
    pub fn critical_matches(&self, ply: usize) -> bool {
        self.classification(ply)
            .is_some_and(|class| class.critical())
            && match self.learning_side {
                LearningSide::All => true,
                LearningSide::White => self.game.positions[ply - 1].turn().is_white(),
                LearningSide::Black => self.game.positions[ply - 1].turn().is_black(),
            }
    }
    pub fn next_critical(&mut self) {
        if let Some(ply) =
            ((self.ply + 1)..=self.game.moves.len()).find(|&p| self.critical_matches(p))
        {
            self.go_to(ply);
        }
    }
    pub fn previous_critical(&mut self) {
        if let Some(ply) = (1..self.ply).rev().find(|&p| self.critical_matches(p)) {
            self.go_to(ply);
        }
    }
}

fn chess_game_key(game: &engineroom_backend::imports::RemoteGame) -> String {
    if game.url.is_empty() {
        format!("{}\0{}\0{}", game.end_time, game.white.username, game.pgn)
    } else {
        game.url.clone()
    }
}

pub(crate) fn display_classification(
    class: engineroom_backend::review::types::Classification,
) -> review_data::Classification {
    use engineroom_backend::review::types::Classification as C;
    use review_data::Classification as D;
    match class {
        C::Livro => D::Book,
        C::Melhor => D::Best,
        C::Excelente => D::Excellent,
        C::Bom => D::Good,
        C::Imprecisao => D::Inaccuracy,
        C::Erro => D::Mistake,
        C::Blunder => D::Blunder,
    }
}
fn prefer_live(
    old: &engineroom_backend::review::types::PositionAnalysis,
    new: &engineroom_backend::review::types::PositionAnalysis,
) -> bool {
    let old_playback = old.search.as_ref().is_some_and(|s| s.purpose == "playback");
    let new_playback = new.search.as_ref().is_some_and(|s| s.purpose == "playback");
    if old_playback != new_playback && old.lines.len() != new.lines.len() {
        return new.lines.len() > old.lines.len();
    }
    prefer_incoming(old, new)
}

fn prefer_incoming(
    old: &engineroom_backend::review::types::PositionAnalysis,
    new: &engineroom_backend::review::types::PositionAnalysis,
) -> bool {
    if old.fen != new.fen {
        return true;
    }
    if old.depth != new.depth {
        return new.depth > old.depth;
    }
    let old_purpose = old.search.as_ref().map(|search| search.purpose.as_str());
    let new_purpose = new.search.as_ref().map(|search| search.purpose.as_str());
    if old_purpose != new_purpose {
        if new_purpose == Some("playback") {
            return false;
        }
        if old_purpose == Some("playback") {
            return true;
        }
    }
    if old.lines.len() != new.lines.len() {
        return new.lines.len() > old.lines.len();
    }
    !matches!((&old.search, &new.search), (Some(a), Some(b)) if a.multipv == b.multipv && a.movetime_ms > b.movetime_ms)
}

#[cfg(test)]
mod council_regressions {
    use super::*;
    #[test]
    fn classification_arriving_at_branch_origin_updates_its_visible_ancestor() {
        let mut state = State::fixture();
        for square in [52, 36, 12, 28] {
            state.click_square(square);
        }
        state.previous();
        for square in [10, 26] {
            state.click_square(square);
        }
        state.first();
        let id = state.variation_node_id().unwrap();
        state.apply_variation_classification(
            &id,
            engineroom_backend::review::types::Classification::Bom,
        );
        assert_eq!(
            state.displayed_classification(),
            Some(review_data::Classification::Good)
        );
        assert_eq!(
            state.variations[0].classifications[0],
            Some(engineroom_backend::review::types::Classification::Bom)
        );
    }
    #[test]
    fn playback_depth_does_not_hide_requested_candidate_lines() {
        use engineroom_backend::review::types::Search;
        let state = State::fixture();
        let mut full = state.real_result.as_ref().unwrap().positions[0].clone();
        assert!(full.lines.len() > 1);
        full.search = Some(Search {
            purpose: "refinement".into(),
            movetime_ms: 1000,
            multipv: 3,
        });
        let mut playback = full.clone();
        playback.lines.truncate(1);
        playback.depth += 5;
        playback.search = Some(Search {
            purpose: "playback".into(),
            movetime_ms: 500,
            multipv: 1,
        });
        assert!(prefer_live(&playback, &full));
        assert!(!prefer_live(&full, &playback));
    }
    #[test]
    fn candidate_chances_come_from_the_same_analysis_as_the_lines() {
        let mut state = State::fixture();
        let original = state.real_result.as_ref().unwrap().positions[0].clone();
        let mut live = original.clone();
        live.lines.truncate(1);
        live.depth += 10;
        state.live_positions.insert(live.fen.clone(), live);
        let candidates = state.candidates();
        assert_eq!(candidates.len(), original.lines.len());
        assert_eq!(candidates[1].win_pct, original.lines[1].win_pct);
    }
    #[test]
    fn live_error_pauses_playback_and_retains_the_failed_request_key() {
        let mut state = State::fixture();
        state.playing = true;
        state.live_running = true;
        state.live_key = Some("position:settings:true".into());
        state.stop_live_with_error("motor indisponível".into());
        assert!(!state.playing);
        assert!(!state.live_running);
        assert_eq!(state.live_key.as_deref(), Some("position:settings:false"));
        assert_eq!(state.live_error.as_deref(), Some("motor indisponível"));
    }
    #[test]
    fn stronger_evaluations_and_variation_feedback_remain_separate() {
        use engineroom_backend::review::types::{Classification, Search};
        let mut state = State::fixture();
        let original = state.real_result.as_ref().unwrap().positions[0].clone();
        let mut weaker = original.clone();
        weaker.depth = original.depth.saturating_sub(1);
        weaker.cp = 999;
        state.live_positions.insert(weaker.fen.clone(), weaker);
        assert_eq!(state.position_analysis().unwrap().cp, original.cp);
        let mut playback = original.clone();
        playback.search = Some(Search {
            purpose: "playback".into(),
            movetime_ms: 500,
            multipv: 1,
        });
        assert!(!prefer_incoming(&original, &playback));
        state.click_square(52);
        state.click_square(36);
        let node = state
            .variation_node_id()
            .expect("variation move has identity");
        let index = state.active_variation.unwrap();
        state.variations[index].classifications[0] = Some(Classification::Bom);
        assert_eq!(
            state.displayed_classification(),
            Some(review_data::Classification::Good)
        );
        state.previous();
        assert!(state.variation_node_id().is_none());
        assert!(state.displayed_classification().is_none());
        state.next();
        assert_eq!(state.variation_node_id().as_deref(), Some(node.as_str()));
        assert_eq!(
            state.displayed_classification(),
            Some(review_data::Classification::Good)
        );
        state.previous();
        state.click_square(51); // d2
        state.click_square(35); // d4 replaces the old branch
        assert_ne!(state.variation_node_id().as_deref(), Some(node.as_str()));
        assert!(state.displayed_classification().is_none());
    }
    #[test]
    fn selecting_and_closing_reviews_preserves_the_import_draft_and_current_task() {
        for page in [Page::History, Page::Settings] {
            let mut state = crate::fixtures::workspace_scene();
            let first = state.active_workspace.unwrap();
            let draft = "[White \"Rascunho incompleto";
            state.pgn.replace_text(draft);
            state.navigate(Page::Home);
            state.activate_review(first);
            state.navigate(Page::Home);
            assert_eq!(
                state.pgn.text, draft,
                "session selection overwrote import draft"
            );
            state.activate_review(first);
            state.navigate(page);
            state.close_review(first);
            assert_eq!(state.page, page, "closing review stole the current task");
            assert!(state.active_workspace.is_none());
            assert_eq!(state.pgn.text, draft);
        }
    }
    #[test]
    fn delayed_history_load_does_not_override_newer_session_or_navigation() {
        let mut state = crate::fixtures::workspace_scene();
        let first = state.workspace[0].key;
        let config = state.workspace[0].config.clone();
        state.navigate(Page::History);
        state.history_open(7, false);
        let old = state.history_load_id;
        state.activate_review(first);
        state.apply_history_load(old, Ok(Some(Box::new(config.clone()))), false);
        assert_eq!(state.active_workspace, Some(first));
        assert_eq!(state.workspace.len(), 2);
        state.navigate(Page::History);
        state.history_open(7, false);
        let old = state.history_load_id;
        state.history_open(8, false);
        let latest = state.history_load_id;
        state.apply_history_load(old, Err("old failure".into()), false);
        assert!(state.history_error.is_none());
        state.navigate(Page::Settings);
        state.apply_history_load(latest, Ok(Some(Box::new(config))), false);
        assert_eq!(state.page, Page::Settings);
        assert_eq!(state.workspace.len(), 2);
    }
}

#[cfg(test)]
#[path = "state_regression_tests.rs"]
mod regression_tests;
