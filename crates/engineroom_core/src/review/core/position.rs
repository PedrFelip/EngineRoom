//! FEN, terminal positions and SAN conversion.
use super::invalid;
use crate::review::types::*;
use shakmaty::{fen::Fen, san, uci::UciMove, CastlingMode, Chess, EnPassantMode, Position};

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
pub fn terminal(fen: &str) -> Result<Option<i32>> {
    Ok(terminal_position(&position(fen)?))
}
pub fn terminal_position(p: &Chess) -> Option<i32> {
    if p.is_checkmate() {
        Some(-100000)
    } else if p.is_stalemate() || insufficient(p) || p.halfmoves() >= 100 {
        Some(0)
    } else {
        None
    }
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
    let Ok(position) = position(&raw.fen) else {
        return;
    };
    for line in &mut raw.lines {
        line.san = line.pv.first().and_then(|uci| {
            let p = position.clone();
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
            unstable: false,
            multipv: 1,
            cp,
            pv: vec![],
            san: None,
            depth: None,
        }],
    }
}
