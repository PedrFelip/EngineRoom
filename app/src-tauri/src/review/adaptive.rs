use super::{
    core::{opening, position, Game},
    scoring::win_pct,
    types::*,
};
use shakmaty::{Chess, Color, Position, Role};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy)]
pub struct Profile {
    pub triage_ms: u32,
    pub triage_multipv: u32,
    pub medium_ms: u32,
    pub high_ms: u32,
    pub refinement_multipv: u32,
    pub medium_multipv: u32,
    pub fraction: f64,
    pub minimum: usize,
    pub context_plies: usize,
}
pub fn profile(kind: AnalysisKind) -> Option<Profile> {
    match kind {
        AnalysisKind::Manual => None,
        AnalysisKind::Fast => Some(Profile {
            triage_ms: 180,
            triage_multipv: 3,
            medium_ms: 500,
            high_ms: 2000,
            refinement_multipv: 2,
            medium_multipv: 1,
            fraction: 0.15,
            minimum: 4,
            context_plies: 2,
        }),
        AnalysisKind::Deep => Some(Profile {
            triage_ms: 450,
            triage_multipv: 5,
            medium_ms: 1800,
            high_ms: 6000,
            refinement_multipv: 2,
            medium_multipv: 1,
            fraction: 0.25,
            minimum: 6,
            context_plies: 4,
        }),
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum RefinementKind {
    #[default]
    Context,
    Complex,
    Tactical,
    Uncertain,
    Critical,
    Promotion,
    Mate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchBudget {
    pub ms: u32,
    pub multipv: u32,
}
impl Profile {
    pub fn budget(self, kind: RefinementKind) -> SearchBudget {
        use RefinementKind::*;
        let (ms, multipv) = match kind {
            Context => (self.medium_ms, self.medium_multipv),
            Uncertain => (self.medium_ms * 3 / 2, self.refinement_multipv),
            Tactical => (self.medium_ms * 2, self.refinement_multipv),
            Complex => (self.medium_ms * 3 / 2, self.refinement_multipv),
            Critical => (self.high_ms, self.refinement_multipv),
            Promotion => (self.high_ms, self.refinement_multipv),
            Mate => (self.high_ms, self.medium_multipv),
        };
        SearchBudget { ms, multipv }
    }
}
fn refinement_kind(m: &Critical) -> RefinementKind {
    let has = |reason: &str| m.reasons.iter().any(|r| r == reason);
    if has("sequência de mate") {
        RefinementKind::Mate
    } else if m.hard {
        // Promotion is tracked independently of captures and checks.
        if m.promotion {
            RefinementKind::Promotion
        } else {
            RefinementKind::Critical
        }
    } else if has("classificação incerta") {
        RefinementKind::Uncertain
    } else if has("sequência tática") {
        RefinementKind::Tactical
    } else {
        RefinementKind::Complex
    }
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Critical {
    pub ply: usize,
    pub score: u32,
    pub hard: bool,
    pub reasons: Vec<String>,
    #[serde(skip)]
    pub promotion: bool,
}
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub position_index: usize,
    pub score: u32,
    pub budget: String,
    #[serde(skip)]
    pub hard: bool,
    #[serde(skip)]
    pub kind: RefinementKind,
    #[serde(skip)]
    pub search: SearchBudget,
}
pub fn rank(game: &Game, raw: &[RawPosition]) -> Vec<Critical> {
    let book = opening(&game.moves).map_or(0, |(n, _)| n);
    game.moves
        .iter()
        .map(|m| rank_move(m, &raw[m.ply - 1], &raw[m.ply], book))
        .collect()
}
pub fn rank_move(
    m: &PlayedMove,
    before: &RawPosition,
    after: &RawPosition,
    book: usize,
) -> Critical {
    if m.ply <= book {
        return Critical {
            ply: m.ply,
            score: 0,
            hard: false,
            reasons: vec![],
            promotion: false,
        };
    }
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
                    .filter(|(_, piece)| piece.role != Role::Pawn && piece.role != Role::King)
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
        promotion,
    }
}

pub fn targets(critical: &[Critical], count: usize, profile: Profile) -> Vec<Target> {
    let mut ranked: Vec<_> = critical
        .iter()
        .filter(|m| m.score >= 32 || m.hard)
        .collect();
    ranked.sort_by(|a, b| {
        b.hard
            .cmp(&a.hard)
            .then_with(|| refinement_kind(b).cmp(&refinement_kind(a)))
            .then(b.score.cmp(&a.score))
    });
    let limit = profile
        .minimum
        .max((count as f64 * profile.fraction).ceil() as usize);
    // Preserve insertion order, including stable sorting of ties.
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
            selected.insert(
                i,
                Target {
                    position_index: i,
                    score: selected.get(&i).map_or(m.score, |t| t.score.max(m.score)),
                    budget: if selected.get(&i).is_some_and(|t| t.budget == "high") {
                        "high"
                    } else {
                        budget
                    }
                    .into(),
                    hard: m.hard || selected.get(&i).is_some_and(|t| t.hard),
                    search: {
                        let next = profile.budget(refinement_kind(m));
                        selected.get(&i).map_or(next, |t| SearchBudget {
                            ms: t.search.ms.max(next.ms),
                            multipv: t.search.multipv.max(next.multipv),
                        })
                    },
                    kind: refinement_kind(m)
                        .max(selected.get(&i).map_or(RefinementKind::Context, |t| t.kind)),
                },
            );
        }
    }
    let mut result: Vec<_> = order.iter().filter_map(|i| selected.remove(i)).collect();
    result.sort_by(|a, b| {
        b.hard
            .cmp(&a.hard)
            .then_with(|| b.kind.cmp(&a.kind))
            .then(b.score.cmp(&a.score))
    });
    result
}

/// Context follows the played sequence; Stockfish already searches continuations
/// inside each position. Only selected decisions may open an optional sequence.
pub fn review_targets(game: &Game, raw: &[RawPosition], profile: Profile) -> Vec<Target> {
    let positions: Vec<_> = game.fens.iter().map(|fen| position(fen).ok()).collect();
    let forcing: Vec<_> = game
        .moves
        .iter()
        .enumerate()
        .map(|(i, m)| {
            m.san.contains('x')
                || m.san.contains('+')
                || m.san.contains('#')
                || m.san.contains('=')
                || positions[i]
                    .as_ref()
                    .is_some_and(|p| p.is_check() || p.legal_moves().len() == 1)
        })
        .collect();
    let unstable = |i: usize| (win_pct(raw[i].cp) - (100.0 - win_pct(raw[i + 1].cp))).abs() >= 2.0;
    let mut critical = rank(game, raw);
    let book = opening(&game.moves).map_or(0, |(n, _)| n);
    let sacrifices: Vec<_> = game
        .moves
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if i + 2 >= positions.len() || i + 1 <= book {
                return false;
            }
            let Some(before) = &positions[i] else {
                return false;
            };
            let Some(after) = &positions[i + 2] else {
                return false;
            };
            let color = before.turn();
            // Material surrendered over the reply with evaluation compensation is
            // a candidate sacrifice, not a claim that the sacrifice is sound.
            material(before, color) - material(after, color) >= 300
                && (material(before, color) - material(before, !color))
                    - (material(after, color) - material(after, !color))
                    >= 200
                && win_pct(raw[i].cp) - win_pct(raw[i + 2].cp) <= 5.0
                && game.moves[i + 1].san.contains('x')
        })
        .collect();
    for (i, candidate) in critical.iter_mut().enumerate() {
        if sacrifices[i] && candidate.ply > book {
            candidate.score = candidate.score.max(32);
            candidate.reasons.push("sequência tática".into());
        }
    }
    let mut selected = targets(&critical, raw.len(), profile);
    let limit = profile
        .minimum
        .max((raw.len() as f64 * profile.fraction).ceil() as usize);
    let mut seeds: Vec<_> = critical
        .iter()
        .filter(|m| {
            let i = m.ply - 1;
            m.ply > book
                && (m.hard || m.score >= 32)
                && (forcing[i]
                    || sacrifices[i]
                    || m.reasons.iter().any(|r| r == "sequência de mate"))
                && selected
                    .iter()
                    .any(|t| t.position_index == i && t.kind != RefinementKind::Context)
                && selected
                    .iter()
                    .any(|t| t.position_index == i + 1 && t.kind != RefinementKind::Context)
        })
        .collect();
    seeds.sort_by(|a, b| {
        b.hard
            .cmp(&a.hard)
            .then(refinement_kind(b).cmp(&refinement_kind(a)))
            .then(b.score.cmp(&a.score))
    });
    for seed in seeds {
        let mut left = seed.ply - 1;
        let mut right = seed.ply;
        let mut left_open = true;
        let mut right_open = true;
        for step in 0..profile.context_plies {
            if selected.len() >= limit {
                break;
            }
            let next = if step % 2 == 0 && right_open || !left_open {
                if right < game.moves.len() && (forcing[right] || unstable(right)) {
                    right += 1;
                    Some(right)
                } else {
                    right_open = false;
                    None
                }
            } else {
                if left > book && forcing[left - 1] {
                    left -= 1;
                    Some(left)
                } else {
                    left_open = false;
                    None
                }
            };
            if !left_open && !right_open {
                break;
            }
            if let Some(index) = next {
                if !selected.iter().any(|t| t.position_index == index) {
                    selected.push(Target {
                        position_index: index,
                        score: seed.score,
                        budget: "medium".into(),
                        hard: false,
                        kind: RefinementKind::Context,
                        search: profile.budget(RefinementKind::Context),
                    });
                }
            }
        }
    }
    selected
}

fn material(position: &Chess, color: Color) -> i32 {
    position
        .board()
        .iter()
        .filter(|(_, piece)| piece.color == color)
        .map(|(_, piece)| match piece.role {
            Role::Pawn => 100,
            Role::Knight | Role::Bishop => 300,
            Role::Rook => 500,
            Role::Queen => 900,
            Role::King => 0,
        })
        .sum()
}
