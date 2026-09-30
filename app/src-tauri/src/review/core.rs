//! Pure chess and review transformations. No Tauri, process or database access.
use super::{scoring::*, types::*};
use pgn_reader::{Outcome, RawTag, Reader, SanPlus, Visitor};
use shakmaty::{fen::Fen, san, uci::UciMove, CastlingMode, Chess, EnPassantMode, Position};
use std::{collections::HashMap, io::Cursor, ops::ControlFlow, sync::OnceLock};

pub struct Game {
    pub fens: Vec<String>,
    pub moves: Vec<PlayedMove>,
    pub meta: PgnMeta,
}
struct Reading {
    pos: Chess,
    game: Game,
}
struct PgnVisitor;
fn invalid(message: impl ToString) -> ReviewError {
    ReviewError::new("invalidPgn", "pgn", message)
}
pub fn position(fen: &str) -> Result<Chess> {
    let fen: Fen = fen
        .parse()
        .map_err(|e| invalid(format!("FEN inválido: {e}")))?;
    fen.into_position(CastlingMode::Standard)
        .map_err(|e| invalid(format!("Posição inválida: {e}")))
}
pub fn fen(pos: &Chess) -> String {
    Fen::from_position(pos, EnPassantMode::Legal).to_string()
}
impl Visitor for PgnVisitor {
    type Tags = HashMap<String, String>;
    type Movetext = Reading;
    type Output = Result<Game>;
    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(HashMap::new())
    }
    fn tag(
        &mut self,
        tags: &mut Self::Tags,
        name: &[u8],
        value: RawTag<'_>,
    ) -> ControlFlow<Self::Output> {
        tags.insert(
            String::from_utf8_lossy(name).into_owned(),
            value.decode_utf8_lossy().into_owned(),
        );
        ControlFlow::Continue(())
    }
    fn begin_movetext(&mut self, tags: Self::Tags) -> ControlFlow<Self::Output, Reading> {
        let pos = if let Some(start) = tags.get("FEN") {
            match position(start) {
                Ok(p) => p,
                Err(e) => return ControlFlow::Break(Err(e)),
            }
        } else {
            Chess::default()
        };
        let player = |key: &str| {
            tags.get(key)
                .filter(|s| !s.is_empty() && *s != "?")
                .map(|s| s.trim().to_owned())
                .unwrap_or_else(|| "Jogador".into())
        };
        let meta = PgnMeta {
            white: player("White"),
            black: player("Black"),
            white_elo: tags.get("WhiteElo").cloned(),
            black_elo: tags.get("BlackElo").cloned(),
            result: tags
                .get("Result")
                .filter(|s| *s != "?")
                .cloned()
                .unwrap_or_else(|| "*".into()),
            event: tags.get("Event").filter(|s| *s != "?").cloned(),
            plies: 0,
        };
        let start = fen(&pos);
        ControlFlow::Continue(Reading {
            pos,
            game: Game {
                fens: vec![start],
                moves: vec![],
                meta,
            },
        })
    }
    fn san(&mut self, state: &mut Reading, supplied: SanPlus) -> ControlFlow<Self::Output> {
        let m = match supplied.san.to_move(&state.pos) {
            Ok(m) => m,
            Err(e) => return ControlFlow::Break(Err(invalid(format!("Lance inválido: {e}")))),
        };
        let color = if state.pos.turn().is_white() {
            "w"
        } else {
            "b"
        }
        .into();
        let before = fen(&state.pos);
        let uci = m.to_uci(CastlingMode::Standard).to_string();
        let san = san::SanPlus::from_move_and_play_unchecked(&mut state.pos, m).to_string();
        state.game.moves.push(PlayedMove {
            ply: state.game.moves.len() + 1,
            color,
            san,
            uci,
            fen_before: before,
        });
        state.game.fens.push(fen(&state.pos));
        ControlFlow::Continue(())
    }
    fn outcome(&mut self, state: &mut Reading, outcome: Outcome) -> ControlFlow<Self::Output> {
        state.game.meta.result = outcome.to_string();
        ControlFlow::Continue(())
    }
    fn end_game(&mut self, mut state: Reading) -> Self::Output {
        if state.game.moves.is_empty() {
            return Err(invalid("O PGN não contém lances."));
        }
        state.game.meta.plies = state.game.moves.len();
        Ok(state.game)
    }
}
pub fn extract(pgn: &str) -> Result<Game> {
    let mut reader = Reader::new(Cursor::new(pgn.as_bytes()));
    let game = reader
        .read_game(&mut PgnVisitor)
        .map_err(invalid)?
        .ok_or_else(|| invalid("PGN inválido."))??;
    if reader
        .read_game(&mut PgnVisitor)
        .map_err(invalid)?
        .is_some()
    {
        return Err(invalid("Importe apenas uma partida por revisão."));
    }
    Ok(game)
}
pub fn terminal(fen: &str) -> Result<Option<i32>> {
    let p = position(fen)?;
    Ok(if p.is_checkmate() {
        Some(-100000)
    } else if p.is_stalemate() || insufficient(&p) || p.halfmoves() >= 100 {
        Some(0)
    } else {
        None
    })
}
// Match chess.js (same-color bishops only; K+NN is not automatically a draw).
fn insufficient(p: &Chess) -> bool {
    let pieces: Vec<_> = p
        .board()
        .iter()
        .filter(|(_, piece)| piece.role != shakmaty::Role::King)
        .collect();
    if pieces.is_empty() {
        return true;
    }
    if pieces.len() == 1 {
        return matches!(
            pieces[0].1.role,
            shakmaty::Role::Bishop | shakmaty::Role::Knight
        );
    }
    pieces
        .iter()
        .all(|(_, piece)| piece.role == shakmaty::Role::Bishop)
        && pieces
            .iter()
            .all(|(s, _)| s.is_light() == pieces[0].0.is_light())
}
pub fn add_san(raw: &mut RawPosition) {
    for line in &mut raw.lines {
        line.san = line.pv.first().and_then(|uci| {
            let p = position(&raw.fen).ok()?;
            let m = uci.parse::<UciMove>().ok()?.to_move(&p).ok()?;
            Some(san::SanPlus::from_move(p, m).to_string())
        });
    }
}
pub fn terminal_raw(fen: &str, cp: i32) -> RawPosition {
    RawPosition {
        fen: fen.into(),
        cp,
        depth: 0,
        pv: vec![],
        lines: vec![RawLine {
            multipv: 1,
            cp,
            pv: vec![],
            san: None,
            depth: None,
        }],
    }
}
fn region(y: i32, w: usize, b: usize) -> i32 {
    match (w, b) {
        (0, 1) => 1 + y,
        (0, 2) if y < 6 => 2 + 6 - y,
        (0, 3 | 4) if y < 7 => 3 + 7 - y,
        (1, 0) => 1 + 8 - y,
        (1, 1) => 5 + (4 - y).abs(),
        (1, 2) => 4 + 7 - y,
        (1, 3) => 5 + 7 - y,
        (2, 0) if y > 2 => 2 + y - 2,
        (2, 1) => 4 + y - 1,
        (2, 2) => 7,
        (3, 0) if y > 1 => 3 + y - 1,
        (3, 1) => 5 + y - 1,
        (4, 0) if y > 1 => 3 + y - 1,
        _ => 0,
    }
}
pub fn phase(fen: &str) -> Phase {
    let ranks: Vec<_> = fen
        .split_whitespace()
        .next()
        .unwrap_or("")
        .split('/')
        .collect();
    if ranks.len() != 8 {
        return Phase::Opening;
    }
    let pieces = ranks
        .iter()
        .flat_map(|s| s.chars())
        .filter(|c| "nbrqNBRQ".contains(*c))
        .count();
    if pieces <= 6 {
        return Phase::Endgame;
    }
    if pieces <= 10
        || ranks[7].chars().filter(|c| c.is_ascii_uppercase()).count() < 4
        || ranks[0].chars().filter(|c| c.is_ascii_lowercase()).count() < 4
    {
        return Phase::Middlegame;
    }
    let mut board = [[None; 8]; 8];
    for (i, rank) in ranks.iter().enumerate() {
        let mut file = 0;
        for c in rank.chars() {
            if let Some(n) = c.to_digit(10) {
                file += n as usize;
            } else if file < 8 {
                board[7 - i][file] = Some(c.is_ascii_uppercase());
                file += 1;
            }
        }
    }
    let mut mixed = 0;
    for y in 0..7 {
        for x in 0..7 {
            let mut w = 0;
            let mut b = 0;
            for dy in 0..2 {
                for dx in 0..2 {
                    match board[y + dy][x + dx] {
                        Some(true) => w += 1,
                        Some(false) => b += 1,
                        _ => {}
                    }
                }
            }
            mixed += region(y as i32 + 1, w, b);
        }
    }
    if mixed > 150 {
        Phase::Middlegame
    } else {
        Phase::Opening
    }
}
pub fn phases(fens: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<Phase> {
    let mut current = Phase::Opening;
    fens.into_iter()
        .map(|f| {
            let next = phase(f.as_ref());
            if next as u8 > current as u8 {
                current = next;
            }
            current
        })
        .collect()
}
#[derive(Clone, serde::Deserialize)]
struct Entry {
    code: String,
    name: String,
    moves: Vec<String>,
}
pub fn opening(moves: &[PlayedMove]) -> Option<(usize, Eco)> {
    static DATA: OnceLock<Vec<Entry>> = OnceLock::new();
    let data = DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../src/data/eco.json")).unwrap_or_default()
    });
    let mut best: Option<&Entry> = None;
    for entry in data {
        if entry.moves.is_empty() || entry.moves.len() > moves.len() {
            continue;
        }
        if entry.moves.iter().zip(moves).all(|(s, m)| *s == m.san)
            && best.is_none_or(|b| entry.moves.len() > b.moves.len())
        {
            best = Some(entry);
        }
    }
    best.map(|b| {
        (
            b.moves.len(),
            Eco {
                code: b.code.clone(),
                name: b.name.clone(),
            },
        )
    })
}
pub fn position_analysis(raw: &RawPosition, ply: usize, phase: Phase) -> PositionAnalysis {
    let white = raw.fen.split_whitespace().nth(1) != Some("b");
    PositionAnalysis {
        ply,
        fen: raw.fen.clone(),
        phase,
        depth: raw.depth,
        cp: raw.cp,
        win_pct: white_win_pct(raw.cp, white),
        pv: raw.pv.clone(),
        search: None,
        lines: raw
            .lines
            .iter()
            .map(|l| PvLine {
                multipv: l.multipv,
                san: l.san.clone(),
                cp: if white { l.cp } else { -l.cp },
                win_pct: white_win_pct(l.cp, white),
                pv: l.pv.clone(),
            })
            .collect(),
    }
}
pub fn build(game: &Game, raw: &[RawPosition]) -> Result<ReviewResult> {
    if raw.len() != game.moves.len() + 1 {
        return Err(invalid("Avaliações incompletas."));
    }
    let phases = phases(raw.iter().map(|r| &r.fen));
    let positions: Vec<_> = raw
        .iter()
        .enumerate()
        .map(|(i, r)| position_analysis(r, i, phases[i]))
        .collect();
    let book = opening(&game.moves);
    let moves: Vec<_> = game
        .moves
        .iter()
        .map(|m| {
            let before = &raw[m.ply - 1];
            let after = &raw[m.ply];
            let win_pct_before = win_pct(before.cp);
            let win_pct_after = 100.0 - win_pct(after.cp);
            let win_pct_loss = (win_pct_before - win_pct_after).max(0.0);
            let is_book = book.as_ref().is_some_and(|(n, _)| m.ply <= *n);
            MoveAnalysis {
                played: m.clone(),
                classification: classify(win_pct_loss, is_book),
                win_pct_before,
                win_pct_after,
                win_pct_loss,
                cp_loss: (before.cp as f64 + after.cp as f64).max(0.0),
                best_uci: before.pv.first().cloned(),
                is_book,
                eco: if is_book {
                    book.as_ref().map(|(_, e)| e.clone())
                } else {
                    None
                },
            }
        })
        .collect();
    let values: Vec<_> = positions.iter().map(|p| p.win_pct).collect();
    let colors: Vec<_> = moves.iter().map(|m| m.played.color.as_str()).collect();
    Ok(ReviewResult {
        accuracy_model: ACCURACY_MODEL.into(),
        accuracy: accuracy(&colors, &values),
        accuracy_by_phase: phase_accuracy(&moves, &phases, &values),
        positions,
        moves,
    })
}
