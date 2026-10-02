//! Assemble evaluated positions into a complete review.
use super::{invalid, opening, phases, Game};
use crate::review::{scoring::*, types::*};

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
        triage_lines: None,
        lines: raw
            .lines
            .iter()
            .map(|l| PvLine {
                depth: l.depth,
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
