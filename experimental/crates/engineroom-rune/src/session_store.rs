//! Versioned workspace checkpoints. Decode, replay and atomic writes stay off the UI thread.
use crate::{
    game::Game,
    play::{Match, PlaySession},
    state::{ImportSource, Page, Profile, ReviewTab, SettingsSection, WorkspaceReview},
    variation::Variation,
};
use engineroom_backend::review::types::{Classification, ReviewConfig};
use serde::{Deserialize, Serialize};
use shakmaty::{CastlingMode, Chess, Color, EnPassantMode, Position, fen::Fen, uci::UciMove};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

const VERSION: u32 = 1;
const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ReviewView {
    #[serde(default)]
    pub avatar_key: Option<String>,
    pub ply: usize,
    pub flipped: bool,
    pub tab: ReviewTab,
    pub compact_panel: bool,
    pub variations: Vec<SavedVariation>,
    pub active_variation: Option<usize>,
    pub selected_candidate: usize,
    pub selected_square: Option<usize>,
    pub promotion: Option<(usize, usize)>,
    pub profile: Profile,
    pub depth: usize,
    pub manual_time: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SavedReview {
    pub key: u64,
    pub config: ReviewConfig,
    pub view: ReviewView,
    pub resume: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SavedVariation {
    root_ply: usize,
    branch_from: Option<(usize, usize)>,
    fen: String,
    moves: Vec<String>,
    cursor: usize,
    classifications: Vec<Option<Classification>>,
}
impl SavedVariation {
    pub fn capture(v: &Variation) -> Self {
        Self {
            root_ply: v.root_ply,
            branch_from: v.branch_from,
            fen: Fen::from_position(&v.positions[0], EnPassantMode::Legal).to_string(),
            moves: v
                .moves
                .iter()
                .map(|m| UciMove::from_standard(*m).to_string())
                .collect(),
            cursor: v.cursor,
            classifications: v.classifications.clone(),
        }
    }
    pub fn restore(
        self,
        game: &Game,
        index: usize,
        previous: &[Variation],
    ) -> Result<Variation, String> {
        if self.root_ply > game.moves.len() || self.moves.len() > 2000 {
            return Err("Variação fora dos limites.".into());
        }
        if let Some((parent, split)) = self.branch_from
            && (parent >= index || previous.get(parent).is_none_or(|v| split > v.moves.len()))
        {
            return Err("Ramo de variação inválido.".into());
        }
        let p: Chess = self
            .fen
            .parse::<Fen>()
            .map_err(|e| e.to_string())?
            .into_position(CastlingMode::Standard)
            .map_err(|e| e.to_string())?;
        let mut v = Variation::new(self.root_ply, p);
        for text in self.moves {
            let m = text
                .parse::<UciMove>()
                .map_err(|e| e.to_string())?
                .to_move(v.position())
                .map_err(|e| e.to_string())?;
            v.append(m);
        }
        v.cursor = self.cursor.min(v.moves.len());
        v.branch_from = self.branch_from;
        v.classifications = self.classifications;
        v.classifications.resize(v.moves.len(), None);
        v.classifications.truncate(v.moves.len());
        Ok(v)
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SavedPlay {
    key: u64,
    white: bool,
    rating: u32,
    moves: Vec<String>,
    resigned: bool,
    cursor: usize,
    flipped: bool,
    promotion: Option<(usize, usize)>,
}
impl SavedPlay {
    pub fn capture(s: &PlaySession) -> Self {
        Self {
            key: s.key,
            white: s.game.human.is_white(),
            rating: s.game.rating,
            moves: s
                .game
                .moves
                .iter()
                .map(|m| UciMove::from_standard(*m).to_string())
                .collect(),
            resigned: s.game.reason == Some("Desistência"),
            cursor: s.cursor,
            flipped: s.flipped,
            promotion: s.promotion,
        }
    }
    fn restore(self) -> Result<PlaySession, String> {
        if self.key == 0
            || self.key > 1_000_000_000
            || !(crate::play::MIN_ELO..=crate::play::MAX_ELO).contains(&self.rating)
            || self.moves.len() > 2000
        {
            return Err("Partida salva fora dos limites.".into());
        }
        let mut game = Match::new(
            if self.white {
                Color::White
            } else {
                Color::Black
            },
            self.rating,
        );
        for text in self.moves {
            let m = text
                .parse::<UciMove>()
                .map_err(|e| e.to_string())?
                .to_move(game.position())
                .map_err(|e| e.to_string())?;
            game.push(m)?;
        }
        if self.resigned {
            game.resign();
        }
        let cursor = self.cursor.min(game.moves.len());
        let promotion = valid_promotion(&game.game.positions[cursor], self.promotion);
        Ok(PlaySession {
            key: self.key,
            request: None,
            game,
            cursor,
            flipped: self.flipped,
            error: None,
            promotion,
        })
    }
}
pub(crate) fn valid_promotion(
    p: &Chess,
    candidate: Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    candidate.filter(|(from, to)| {
        *from < 64
            && *to < 64
            && p.legal_moves().iter().any(|m| {
                m.promotion().is_some()
                    && m.from().is_some_and(|f| f.to_usize() ^ 56 == *from)
                    && crate::review_data::squares(*m).1 == *to
            })
    })
}

pub(crate) fn hash_variations(variations: &[Variation], hash: &mut impl std::hash::Hasher) {
    use std::hash::Hash;
    variations.len().hash(hash);
    for v in variations {
        (v.root_ply, v.branch_from, v.cursor).hash(hash);
        v.san.hash(hash);
        for class in &v.classifications {
            class.as_ref().map(std::mem::discriminant).hash(hash);
        }
    }
}

fn default_rating() -> u32 {
    1500
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct DraftFields {
    pub history_filter: String,
    pub chess_username: String,
    pub chess_filter: String,
    pub chess_speed: usize,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Document {
    pub version: u32,
    pub reviews: Vec<SavedReview>,
    pub plays: Vec<SavedPlay>,
    pub active_review: Option<u64>,
    pub active_play: Option<u64>,
    pub page: Page,
    pub previous_page: Page,
    pub section: SettingsSection,
    pub pgn: String,
    pub import_source: ImportSource,
    pub profile: Profile,
    pub depth: usize,
    pub manual_time: bool,
    pub sidebar: (bool, f32),
    pub inspector: (bool, f32),
    #[serde(default)]
    pub draft_fields: DraftFields,
    #[serde(default = "default_rating")]
    pub play_rating: u32,
    #[serde(default)]
    pub play_side: crate::play::Side,
}
pub(crate) struct Restored {
    pub document: Document,
    pub reviews: Vec<WorkspaceReview>,
    pub plays: Vec<PlaySession>,
    pub draft: Option<Game>,
}
impl Document {
    pub fn hydrate(self) -> Result<Restored, String> {
        if self.version != VERSION {
            return Err(format!("Versão de sessões não suportada: {}", self.version));
        }
        if self.reviews.len() + self.plays.len() > 256
            || self.pgn.len() > 1_000_000
            || !self.sidebar.1.is_finite()
            || !self.inspector.1.is_finite()
        {
            return Err("Workspace fora dos limites.".into());
        }
        let mut reviews = Vec::new();
        for saved in &self.reviews {
            if saved.key == 0 || reviews.iter().any(|v: &WorkspaceReview| v.key == saved.key) {
                return Err("Identificador de revisão duplicado ou inválido.".into());
            }
            reviews.push(WorkspaceReview::restore(saved.clone())?);
        }
        let mut plays = Vec::new();
        for saved in &self.plays {
            if plays.iter().any(|v: &PlaySession| v.key == saved.key) {
                return Err("Identificador de partida duplicado.".into());
            }
            plays.push(saved.clone().restore()?);
        }
        let draft = if self.page == Page::Configure
            || (self.page == Page::Settings && self.previous_page == Page::Configure)
        {
            Some(crate::game::parse(&self.pgn)?)
        } else {
            None
        };
        Ok(Restored {
            document: self,
            reviews,
            plays,
            draft,
        })
    }
}

enum Intent {
    Save(Box<Document>),
    Flush(Box<Document>, mpsc::Sender<Result<(), String>>),
    Stop,
}
pub(crate) enum Event {
    Saved(Result<(), String>),
}
pub(crate) struct Store {
    sender: mpsc::Sender<Intent>,
    events: mpsc::Receiver<Event>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Store {
    /// Reads the current workspace before starting the save worker.
    ///
    /// The initial read is synchronous so setup can restore the workspace
    /// without racing the first UI frame; saves stay on the worker thread.
    pub fn open(
        path: PathBuf,
        wake: impl Fn() + Send + 'static,
    ) -> (Self, Result<Option<Box<Restored>>, String>) {
        let initial = read(&path);
        let writable = initial.is_ok();
        let (sender, intents) = mpsc::channel();
        let (tx, events) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut previous = None;
            while let Ok(intent) = intents.recv() {
                let (mut doc, mut ack) = match intent {
                    Intent::Save(d) => (d, None),
                    Intent::Flush(d, ack) => (d, Some(ack)),
                    Intent::Stop => break,
                };
                let mut stop = false;
                while ack.is_none() {
                    match intents.try_recv() {
                        Ok(Intent::Save(new)) => doc = new,
                        Ok(Intent::Flush(new, reply)) => {
                            doc = new;
                            ack = Some(reply);
                        }
                        Ok(Intent::Stop) => {
                            stop = true;
                            break;
                        }
                        Err(_) => break,
                    }
                }
                let result = if writable {
                    write(&path, *doc, &mut previous)
                } else {
                    Err("O arquivo original de sessões foi preservado após erro de leitura.".into())
                };
                if let Some(ack) = ack {
                    let _ = ack.send(result.clone());
                }
                let _ = tx.send(Event::Saved(result));
                wake();
                if stop {
                    break;
                }
            }
        });
        (
            Self {
                sender,
                events,
                worker: Some(worker),
            },
            initial,
        )
    }
    pub fn save(&self, doc: Document) {
        let _ = self.sender.send(Intent::Save(Box::new(doc)));
    }
    pub fn flush(&self, doc: Document) -> Result<(), String> {
        let (ack, rx) = mpsc::channel();
        self.sender
            .send(Intent::Flush(Box::new(doc), ack))
            .map_err(|_| "Gravador de sessões indisponível.".to_string())?;
        rx.recv()
            .map_err(|_| "Gravação de sessões interrompida.".to_string())?
    }
    pub fn drain(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.sender.send(Intent::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn read(path: &Path) -> Result<Option<Box<Restored>>, String> {
    match fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Ler sessões: {e}")),
        Ok(m) if m.len() > MAX_BYTES => return Err("Arquivo de sessões excede 64 MiB.".into()),
        _ => {}
    }
    let bytes = fs::read(path).map_err(|e| format!("Ler sessões: {e}"))?;
    let doc: Document =
        serde_json::from_slice(&bytes).map_err(|e| format!("Sessões inválidas: {e}"))?;
    doc.hydrate().map(|r| Some(Box::new(r)))
}
fn write(path: &Path, doc: Document, previous: &mut Option<Vec<u8>>) -> Result<(), String> {
    let bytes = serde_json::to_vec(&doc).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Workspace excede 64 MiB.".into());
    }
    if previous.as_ref() == Some(&bytes) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = fs::File::create(&temporary).map_err(|e| e.to_string())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(&temporary, path).map_err(|e| e.to_string())?;
    *previous = Some(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ReviewTab, State};

    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "engineroom-workspace-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }
    fn roundtrip(state: &mut State) -> State {
        let bytes = serde_json::to_vec(&state.workspace_document()).expect("serialize workspace");
        let doc: Document = serde_json::from_slice(&bytes).expect("decode workspace");
        let mut restored = State::default();
        restored.restore_workspace(doc.hydrate().expect("replay valid workspace"));
        restored
    }
    #[test]
    fn restores_fifteen_games_review_variations_selection_and_draft() {
        let mut s = State::fixture();
        let key = s.active_workspace.expect("review");
        s.activate_review(key);
        s.go_to(4);
        s.explore_candidate();
        let variation = s.variations[s.active_variation.expect("candidate variation")]
            .san
            .clone();
        s.review_tab = ReviewTab::Analysis;
        for i in 0..15 {
            s.open_play_setup();
            s.play.rating = 1400 + i * 50;
            s.start_play();
            s.click_square(52);
            s.click_square(36);
            if i % 2 == 0 {
                s.first();
            }
        }
        s.sidebar.set_open(false);
        s.review_sidebar.set_width(376.0);
        s.pgn
            .replace_text("[Event \"Unsaved draft\"]\n\n1. d4 d5 *");
        let mut restored = roundtrip(&mut s);
        assert_eq!(restored.play.sessions.len(), 15);
        assert_eq!(restored.play.active, Some(15));
        assert_eq!(restored.play.cursor, 0);
        assert_eq!(
            restored.play.game.as_ref().expect("game").game.moves,
            ["e4"]
        );
        assert!(!restored.sidebar.is_open());
        assert_eq!(restored.review_sidebar.width(), 376.0);
        assert!(restored.pgn.text.contains("Unsaved draft"));
        assert!(restored.play.sessions.iter().all(|v| v.request.is_none()));
        restored.activate_play(2);
        assert_eq!(restored.play.cursor, 1);
        restored.activate_review(key);
        assert_eq!(restored.review_tab, ReviewTab::Analysis);
        assert_eq!(
            restored.variations[restored.active_variation.expect("variation restored")].san,
            variation
        );
        assert!(restored.real_result.is_some());
    }
    #[test]
    fn restores_promotion_and_resignation_without_persisting_process_requests() {
        let mut s = State::default();
        s.start_play();
        for text in [
            "a2a4", "h7h5", "a4a5", "h5h4", "a5a6", "h4h3", "a6b7", "h3g2",
        ] {
            let g = s.play.game.as_mut().expect("game");
            let m = text
                .parse::<UciMove>()
                .expect("UCI")
                .to_move(g.position())
                .expect("legal move");
            g.push(m).expect("match move");
            s.play.cursor = g.moves.len();
        }
        s.click_square(9);
        s.click_square(0);
        assert_eq!(s.promotion_pending, Some((9, 0)));
        s.open_settings();
        let mut restored = roundtrip(&mut s);
        assert_eq!(restored.page, Page::Settings);
        restored.navigate(Page::Play);
        assert_eq!(restored.promotion_pending, Some((9, 0)));
        restored.promote(shakmaty::Role::Knight);
        assert!(
            restored
                .play
                .game
                .as_ref()
                .expect("game")
                .game
                .moves
                .last()
                .expect("promotion")
                .contains("=N")
        );
        restored.resign_play();
        let again = roundtrip(&mut restored);
        assert_eq!(
            again.play.game.as_ref().expect("game").reason,
            Some("Desistência")
        );
    }
    #[test]
    fn restores_incomplete_analysis_as_resumable_not_completed() {
        let mut s = State::fixture();
        s.activate_review(s.active_workspace.expect("review"));
        s.real_result = None;
        s.real_progress = Some((4, 17));
        s.bridge_error = None;
        let restored = roundtrip(&mut s);
        assert!(restored.real_result.is_none());
        assert_eq!(restored.real_progress, Some((0, 17)));
        assert_eq!(restored.workspace[0].status(), "Em andamento");
    }
    #[test]
    fn closing_a_session_keeps_it_closed_after_reopening() {
        let mut s = State::fixture();
        let review = s.active_workspace.expect("review");
        s.open_play_setup();
        s.start_play();
        s.open_play_setup();
        s.start_play();
        s.close_play(1);
        s.close_review(review);
        let restored = roundtrip(&mut s);
        assert!(restored.workspace.is_empty());
        assert_eq!(restored.play.sessions.len(), 1);
        assert_eq!(restored.play.sessions[0].key, 2);
    }
    #[test]
    fn flush_is_a_barrier_and_drop_drains_the_last_queued_checkpoint() {
        let path = path();
        let (store, initial) = Store::open(path.clone(), || {});
        assert!(matches!(initial, Ok(None)));
        let mut s = State::default();
        s.start_play();
        store.save(s.workspace_document());
        s.click_square(52);
        s.click_square(36);
        store
            .flush(s.workspace_document())
            .expect("durable final checkpoint");
        let loaded = read(&path).expect("read").expect("document");
        assert_eq!(loaded.plays[0].game.game.moves, ["e4"]);
        s.resign_play();
        store.save(s.workspace_document());
        drop(store);
        assert_eq!(
            read(&path).expect("read").expect("document").plays[0]
                .game
                .reason,
            Some("Desistência")
        );
        fs::remove_file(path).expect("cleanup");
    }
    #[test]
    fn unsupported_or_corrupt_workspaces_are_preserved() {
        let path = path();
        for bytes in [b"{broken".as_slice(), b"{\"version\":999}".as_slice()] {
            fs::write(&path, bytes).expect("seed invalid file");
            let (store, initial) = Store::open(path.clone(), || {});
            assert!(initial.is_err());
            assert!(store.flush(State::default().workspace_document()).is_err());
            drop(store);
            assert_eq!(fs::read(&path).expect("preserved file"), bytes);
        }
        fs::remove_file(path).expect("cleanup");
    }
    #[test]
    fn corrupt_moves_and_duplicate_keys_are_rejected_before_hydration() {
        let mut s = State::default();
        s.start_play();
        let mut doc = s.workspace_document();
        doc.plays[0].moves.push("e2e5".into());
        assert!(doc.hydrate().is_err());
        let mut doc = s.workspace_document();
        doc.plays.push(doc.plays[0].clone());
        assert!(doc.hydrate().is_err());
    }
}

#[cfg(test)]
mod perf_fixture {
    #[test]
    #[ignore = "writes the reproducible startup workspace fixture on request"]
    fn write_startup_fixture() {
        let path = std::env::var_os("ENGINE_ROOM_PERF_WORKSPACE").expect("set fixture output path");
        let mut state = crate::fixtures::play_scene(15);
        let doc = state.workspace_document();
        std::fs::write(path, serde_json::to_vec(&doc).expect("fixture JSON"))
            .expect("write fixture");
    }
}
