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
impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Self::Home => "Nova revisão",
            Self::Configure => "Configurar análise",
            Self::Review => "Revisão da partida",
            Self::History => "Histórico",
            Self::Settings => "Configurações",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
pub enum ReviewMode {
    Preview,
    #[default]
    Mock,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MockStatus {
    #[default]
    Ready,
    Running,
    Failed,
    Cancelled,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LearningSide {
    #[default]
    All,
    White,
    Black,
}

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
        }
    }
}
pub struct State {
    pub page: Page,
    pub previous_page: Page,
    pub section: SettingsSection,
    pub preferences: Preferences,
    pub sidebar: PaneState,
    pub pgn: TextEdit,
    pub import_error: Option<String>,
    pub game: Game,
    pub ply: usize,
    pub flipped: bool,
    pub review_tab: ReviewTab,
    pub review_mode: ReviewMode,
    pub mock_status: MockStatus,
    pub mock_progress: f32,
    pub learning_side: LearningSide,
    pub selected_candidate: usize,
    pub triage_open: bool,
    pub analysis_settings_open: bool,
    pub inspector_visible: bool,
    pub selected_square: Option<usize>,
    pub variations: Vec<Variation>,
    pub active_variation: Option<usize>,
    pub playing: bool,
    pub clock: f64,
    pub next_step: f64,
    pub profile: Profile,
    pub depth: usize,
    pub select_open: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            page: Page::Home,
            previous_page: Page::Home,
            section: SettingsSection::Preferences,
            preferences: Preferences::default(),
            sidebar: PaneState::new(218.0).range(190.0, 290.0),
            pgn: TextEdit::new(""),
            import_error: None,
            game: game::parse(game::DEMO_PGN).expect("bundled demo is valid"),
            ply: 0,
            flipped: false,
            review_tab: ReviewTab::Moves,
            review_mode: ReviewMode::Mock,
            mock_status: MockStatus::Ready,
            mock_progress: 0.62,
            learning_side: LearningSide::All,
            selected_candidate: 0,
            triage_open: false,
            analysis_settings_open: false,
            inspector_visible: true,
            selected_square: None,
            variations: Vec::new(),
            active_variation: None,
            playing: false,
            clock: 0.0,
            next_step: 0.0,
            profile: Profile::Fast,
            depth: 20,
            select_open: false,
        }
    }
}
impl State {
    pub fn navigate(&mut self, page: Page) {
        self.page = page;
        self.playing = false;
        self.analysis_settings_open = false;
        self.select_open = false;
    }
    pub fn open_settings(&mut self) {
        self.previous_page = self.page;
        self.navigate(Page::Settings);
    }
    pub fn load_demo(&mut self) {
        self.pgn.replace_text(game::DEMO_PGN);
        self.import_error = None;
    }
    pub fn prepare(&mut self) {
        match game::parse(&self.pgn.text) {
            Ok(game) => {
                self.game = game;
                self.go_to(0);
                self.variations.clear();
                self.review_mode = if self.pgn.text.trim() == game::DEMO_PGN {
                    ReviewMode::Mock
                } else {
                    ReviewMode::Preview
                };
                self.import_error = None;
                self.navigate(Page::Configure);
            }
            Err(error) => self.import_error = Some(error),
        }
    }

    pub fn is_mock(&self) -> bool {
        self.review_mode == ReviewMode::Mock
    }
    pub fn go_to(&mut self, ply: usize) {
        self.ply = ply.min(self.game.moves.len());
        self.active_variation = None;
        self.playing = false;
        self.selected_square = None;
        self.selected_candidate = 0;
    }
    pub fn next(&mut self) {
        self.playing = false;
        self.selected_square = None;
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
        if let Some(index) = self.active_variation {
            let v = &mut self.variations[index];
            v.cursor = v.cursor.saturating_sub(1);
        } else {
            self.go_to(self.ply.saturating_sub(1));
        }
    }
    pub fn first(&mut self) {
        if let Some(i) = self.active_variation {
            self.variations[i].cursor = 0;
            self.playing = false;
        } else {
            self.go_to(0);
        }
    }
    pub fn last(&mut self) {
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
    pub fn displayed_ply(&self) -> usize {
        self.active_variation.map_or(self.ply, |i| {
            self.variations[i].root_ply + self.variations[i].cursor
        })
    }
    pub fn last_move(&self) -> Option<(usize, usize)> {
        if let Some(i) = self.active_variation {
            let v = &self.variations[i];
            return v
                .cursor
                .checked_sub(1)
                .map(|j| review_data::squares(v.moves[j]));
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
    pub fn candidates(&self) -> Vec<review_data::Candidate> {
        review_data::candidates(self.position(), self.displayed_ply())
    }
    pub fn explore_candidate(&mut self) {
        let lines = self.candidates();
        let Some(line) = lines.get(self.selected_candidate) else {
            return;
        };
        let mut variation = Variation::new(self.ply, self.position().clone());
        for m in &line.moves {
            variation.append(*m);
        }
        variation.cursor = 0;
        self.variations.push(variation);
        self.active_variation = Some(self.variations.len() - 1);
        self.selected_square = None;
        self.playing = true;
        self.next_step = self.clock + 0.65;
    }
    pub fn tick(&mut self, now: f64) -> bool {
        let elapsed = (now - self.clock).clamp(0.0, 1.0) as f32;
        self.clock = now;
        let mut changed = false;
        if self.page == Page::Review && self.is_mock() && self.mock_status == MockStatus::Running {
            self.mock_progress = (self.mock_progress + elapsed * 0.2).min(1.0);
            if self.mock_progress >= 1.0 {
                self.mock_status = MockStatus::Ready;
            }
            changed = true;
        }
        if !self.playing || now < self.next_step {
            return changed;
        }
        let Some(i) = self.active_variation else {
            self.playing = false;
            return false;
        };
        let v = &mut self.variations[i];
        v.cursor = (v.cursor + 1).min(v.moves.len());
        self.playing = v.cursor < v.moves.len();
        self.next_step = now + 0.65;
        true
    }
    pub fn click_square(&mut self, index: usize) {
        self.playing = false;
        let target = Square::new((index ^ 56) as u32);
        if let Some(from) = self.selected_square {
            let source = Square::new((from ^ 56) as u32);
            let m = self.position().legal_moves().into_iter().find(|m| {
                m.from() == Some(source)
                    && review_data::squares(*m).1 == index
                    && (m.promotion().is_none() || m.promotion() == Some(Role::Queen))
            });
            if let Some(m) = m {
                if self.active_variation.is_none() {
                    self.variations
                        .push(Variation::new(self.ply, self.position().clone()));
                    self.active_variation = Some(self.variations.len() - 1);
                }
                self.variations[self
                    .active_variation
                    .expect("variation created before append")]
                .append(m);
                self.selected_square = None;
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
    pub fn critical_matches(&self, ply: usize) -> bool {
        self.is_mock()
            && review_data::classification(ply).critical()
            && match self.learning_side {
                LearningSide::All => true,
                LearningSide::White => ply % 2 == 1,
                LearningSide::Black => ply.is_multiple_of(2),
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
