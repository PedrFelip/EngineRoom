use super::{
    core::{opening, position, Game},
    scoring::win_pct,
    types::*,
};
use shakmaty::{Position, Role};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy)]
pub struct Profile {
    pub triage_ms: u32,
    pub triage_multipv: u32,
    pub medium_ms: u32,
    pub high_ms: u32,
    pub refinement_multipv: u32,
    pub fraction: f64,
    pub minimum: usize,
}
pub fn profile(kind: AnalysisKind) -> Option<Profile> {
    match kind {
        AnalysisKind::Manual => None,
        AnalysisKind::Fast => Some(Profile {
            triage_ms: 120,
            triage_multipv: 2,
            medium_ms: 600,
            high_ms: 1500,
            refinement_multipv: 2,
            fraction: 0.2,
            minimum: 6,
        }),
        AnalysisKind::Deep => Some(Profile {
            triage_ms: 300,
            triage_multipv: 3,
            medium_ms: 1500,
            high_ms: 4000,
            refinement_multipv: 3,
            fraction: 0.35,
            minimum: 10,
        }),
    }
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Critical {
    pub ply: usize,
    pub score: u32,
    pub hard: bool,
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub position_index: usize,
    pub score: u32,
    pub budget: String,
}
pub fn rank(game: &Game, raw: &[RawPosition]) -> Vec<Critical> {
    let book = opening(&game.moves).map_or(0, |(n, _)| n);
    game.moves
        .iter()
        .map(|m| {
            if m.ply <= book {
                return Critical {
                    ply: m.ply,
                    score: 0,
                    hard: false,
                    reasons: vec![],
                };
            }
            let before = &raw[m.ply - 1];
            let after = &raw[m.ply];
            let delta = win_pct(before.cp) - (100.0 - win_pct(after.cp));
            let loss = delta.max(0.0);
            let swing = delta.abs();
            let gap = before
                .lines
                .iter()
                .find(|l| l.multipv == 1)
                .zip(before.lines.iter().find(|l| l.multipv == 2))
                .map(|(a, b)| (a.cp as f64 - b.cp as f64).abs());
            let (legal, pieces, in_check) = position(&m.fen_before)
                .map(|p| {
                    (
                        p.legal_moves().len(),
                        p.board()
                            .iter()
                            .filter(|(_, piece)| {
                                piece.role != Role::Pawn && piece.role != Role::King
                            })
                            .count(),
                        p.is_check(),
                    )
                })
                .unwrap_or((0, 0, false));
            let capture = m.san.contains('x');
            let check = m.san.contains('+') || m.san.contains('#');
            let promotion = m.san.contains('=');
            let mate = before.cp.abs() >= 90000 || after.cp.abs() >= 90000;
            let mut score =
                ((loss / 20.0) * 24.0).clamp(0.0, 24.0) + ((swing / 20.0) * 11.0).clamp(0.0, 11.0);
            let mut reasons = Vec::new();
            if loss >= 5.0 {
                reasons.push("perda de avaliação".into());
            }
            if swing >= 10.0 {
                reasons.push("virada de avaliação".into());
            }
            if capture {
                score += 4.0;
            }
            if check {
                score += 7.0;
            }
            if promotion {
                score += 12.0;
            }
            if in_check {
                score += 7.0;
            }
            if capture || check || promotion {
                reasons.push("sequência tática".into());
            }
            score += (((legal as f64 - 18.0) / 22.0) * 10.0).clamp(0.0, 10.0)
                + ((pieces as f64 / 10.0) * 8.0).clamp(0.0, 8.0);
            if legal >= 30 || pieces >= 8 {
                reasons.push("posição complexa".into());
            }
            if let Some(gap) = gap {
                score += ((gap / 180.0) * 12.0).clamp(0.0, 12.0);
                if gap >= 80.0 {
                    reasons.push("melhor lance se destaca".into());
                }
            }
            if [2.0, 5.0, 10.0, 20.0]
                .iter()
                .any(|b| (loss - b).abs() <= 1.25)
            {
                score += 8.0;
                reasons.push("classificação incerta".into());
            }
            if mate {
                score += 20.0;
                reasons.push("sequência de mate".into());
            }
            Critical {
                ply: m.ply,
                score: score.round().min(100.0) as u32,
                hard: loss >= 10.0 || swing >= 15.0 || mate || promotion,
                reasons,
            }
        })
        .collect()
}
pub fn targets(critical: &[Critical], count: usize, profile: Profile) -> Vec<Target> {
    let mut ranked: Vec<_> = critical
        .iter()
        .filter(|m| m.score >= 32 || m.hard)
        .collect();
    ranked.sort_by(|a, b| b.hard.cmp(&a.hard).then(b.score.cmp(&a.score)));
    let limit = profile
        .minimum
        .max((count as f64 * profile.fraction).ceil() as usize);
    // Insertion order matches JS Map, including stable sorting of ties.
    let mut order = Vec::new();
    let mut selected: BTreeMap<usize, Target> = BTreeMap::new();
    for m in ranked {
        let indexes = [m.ply - 1, m.ply];
        if !m.hard && indexes.iter().any(|i| !selected.contains_key(i)) && selected.len() >= limit {
            continue;
        }
        for i in indexes {
            if i >= count {
                continue;
            }
            let budget = if m.hard || m.score >= 65 {
                "high"
            } else {
                "medium"
            };
            if !selected.contains_key(&i) {
                order.push(i);
            }
            if selected
                .get(&i)
                .is_none_or(|p| p.score < m.score || (p.budget == "medium" && budget == "high"))
            {
                selected.insert(
                    i,
                    Target {
                        position_index: i,
                        score: m.score,
                        budget: budget.into(),
                    },
                );
            }
        }
    }
    let mut result: Vec<_> = order.iter().filter_map(|i| selected.remove(i)).collect();
    result.sort_by(|a, b| {
        (b.budget == "high")
            .cmp(&(a.budget == "high"))
            .then(b.score.cmp(&a.score))
    });
    result
}
