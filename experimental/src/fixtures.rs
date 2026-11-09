//! Deterministic inputs for headless scenes and tests, using production review records.
use crate::review_data::{Candidate, Classification as DisplayClassification};
use crate::{game, review_data, state::State};
use engineroom_backend::review::{
    core,
    types::{Classification, RawLine, RawPosition, ReviewResult},
};
use shakmaty::{CastlingMode, Chess, Move, Position, san::SanPlus};

pub fn result() -> ReviewResult {
    let game = core::extract(game::DEMO_PGN).expect("fixture PGN is legal");
    let raw = game
        .positions
        .iter()
        .enumerate()
        .map(|(ply, board)| {
            let lines = candidates(board, ply)
                .into_iter()
                .enumerate()
                .map(|(i, c)| RawLine {
                    unstable: false,
                    multipv: i as u32 + 1,
                    cp: c.cp,
                    pv: c
                        .moves
                        .iter()
                        .map(|m| m.to_uci(CastlingMode::Standard).to_string())
                        .collect(),
                    san: Some(c.san.join(" ")),
                    depth: Some(c.depth as u32),
                })
                .collect::<Vec<_>>();
            RawPosition {
                fen: game.fens[ply].clone(),
                cp: cp(ply),
                depth: 22,
                pv: lines.first().map_or_else(Vec::new, |l| l.pv.clone()),
                lines,
            }
        })
        .collect::<Vec<_>>();
    let mut result = core::build(&game, &raw).expect("fixture evaluations match game");
    for m in &mut result.moves {
        use review_data::Classification as C;
        m.classification = match classification(m.played.ply) {
            C::Book => Classification::Livro,
            C::Best => Classification::Melhor,
            C::Excellent => Classification::Excelente,
            C::Good => Classification::Bom,
            C::Inaccuracy => Classification::Imprecisao,
            C::Mistake => Classification::Erro,
            C::Blunder => Classification::Blunder,
        };
    }
    result
}
impl State {
    pub fn fixture() -> Self {
        let mut state = Self::default();
        state.game = game::parse(game::DEMO_PGN).expect("fixture PGN");
        state.real_result = Some(result());
        state.register_review(engineroom_backend::review::types::ReviewConfig {
            pgn: game::DEMO_PGN.into(),
            meta: core::extract(game::DEMO_PGN).unwrap().meta,
            engine: engineroom_backend::review::types::EngineTier {
                id: "fixture".into(),
                depth: 12,
                label: String::new(),
                hint: String::new(),
            },
            mode: engineroom_backend::db::mode::Mode::Depth,
            analysis_kind: engineroom_backend::review::types::AnalysisKind::Manual,
            movetime_ms: None,
            lines: 3,
            initial_result: state.real_result.clone(),
        });
        state
    }
    pub fn load_fixture(&mut self) {
        self.pgn.replace_text(game::DEMO_PGN);
        self.import_error = None;
    }
}

pub fn classification(ply: usize) -> DisplayClassification {
    const PATTERN: [DisplayClassification; 16] = [
        DisplayClassification::Book,
        DisplayClassification::Book,
        DisplayClassification::Best,
        DisplayClassification::Good,
        DisplayClassification::Excellent,
        DisplayClassification::Inaccuracy,
        DisplayClassification::Best,
        DisplayClassification::Mistake,
        DisplayClassification::Good,
        DisplayClassification::Best,
        DisplayClassification::Blunder,
        DisplayClassification::Good,
        DisplayClassification::Best,
        DisplayClassification::Inaccuracy,
        DisplayClassification::Excellent,
        DisplayClassification::Mistake,
    ];
    PATTERN[ply.saturating_sub(1) % PATTERN.len()]
}
pub fn cp(ply: usize) -> i32 {
    const SCORES: [i32; 17] = [
        18, 24, 12, 38, 20, 64, 48, 82, 164, 126, 102, -142, -88, -64, 34, 76, 152,
    ];
    SCORES[ply % SCORES.len()]
}
pub fn candidates(position: &Chess, ply: usize) -> Vec<Candidate> {
    let mut legal: Vec<Move> = position.legal_moves().into_iter().collect();
    legal.sort_by_key(|m| m.to_uci(CastlingMode::Standard).to_string());
    legal
        .into_iter()
        .take(5)
        .enumerate()
        .map(|(index, first)| {
            let mut p = position.clone();
            let mut moves = Vec::new();
            let mut san = Vec::new();
            let mut next = Some(first);
            for step in 0..5 {
                let Some(m) = next else {
                    break;
                };
                san.push(SanPlus::from_move_and_play_unchecked(&mut p, m).to_string());
                moves.push(m);
                let choices = p.legal_moves();
                next = choices
                    .get((ply + index + step) % choices.len().max(1))
                    .copied();
            }
            Candidate {
                moves,
                san,
                cp: cp(ply) - index as i32 * 36,
                win_pct: engineroom_backend::review::scoring::win_pct(cp(ply) - index as i32 * 36),
                depth: 22 - index * 2,
            }
        })
        .collect()
}

pub fn workspace_scene() -> State {
    let mut state = State::fixture();
    let first = state.workspace[0].key;
    state.page = crate::state::Page::Review;
    state.navigate(crate::state::Page::Home);
    state.pgn.replace_text(
        game::DEMO_PGN
            .replace("Jogador de brancas", "Alex")
            .replace("Jogador de pretas", "Marina"),
    );
    state.prepare();
    state.start_review();
    state.bridge_error = None;
    state.real_progress = Some((5, 17));
    state.activate_review(first);
    state
}
