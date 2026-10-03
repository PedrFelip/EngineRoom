use super::{
    core::{opening, Game},
    scoring::win_pct,
    types::*,
};
use shakmaty::{Chess, Color, Position, Role};
use std::{collections::BTreeMap, sync::LazyLock};
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub triage_ms: u32,
    pub triage_multipv: u32,
    pub medium_ms: u32,
    pub high_ms: u32,
    pub refinement_multipv: u32,
    pub medium_multipv: u32,
    #[serde(rename = "maxRefineFraction")]
    pub fraction: f64,
    #[serde(rename = "minRefinePositions")]
    pub minimum: usize,
    pub context_plies: usize,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Profiles {
    fast: Profile,
    deep: Profile,
}

// Embedded in the binary and also imported by the frontend.
static PROFILES: LazyLock<Result<Profiles>> =
    LazyLock::new(|| parse_profiles(include_str!("profiles.json")));

fn parse_profiles(json: &str) -> Result<Profiles> {
    serde_json::from_str(json).map_err(|error| {
        ReviewError::with_source(ReviewErrorCode::Session, "review.profiles", error)
    })
}
pub fn profile(kind: AnalysisKind) -> Result<Option<Profile>> {
    if kind == AnalysisKind::Manual {
        return Ok(None);
    }
    let profiles = PROFILES.as_ref().map_err(Clone::clone)?;
    Ok(Some(match kind {
        AnalysisKind::Fast => profiles.fast,
        AnalysisKind::Deep => profiles.deep,
        AnalysisKind::Manual => return Ok(None),
    }))
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Reason {
    #[serde(rename = "perda de avaliação")]
    Loss,
    #[serde(rename = "virada de avaliação")]
    Swing,
    #[serde(rename = "sequência tática")]
    Tactical,
    #[serde(rename = "posição complexa")]
    Complex,
    #[serde(rename = "melhor lance se destaca")]
    BestMoveGap,
    #[serde(rename = "proximidade do limite de classificação")]
    ClassificationBoundary,
    #[serde(rename = "busca instável")]
    UnstableSearch,
    #[serde(rename = "sequência de mate")]
    Mate,
    #[serde(rename = "promoção")]
    Promotion,
}
fn refinement_kind(m: &Critical) -> RefinementKind {
    let has = |reason| m.reasons.contains(&reason);
    if has(Reason::Mate) {
        RefinementKind::Mate
    } else if m.hard {
        // Promotion is tracked independently of captures and checks.
        if has(Reason::Promotion) {
            RefinementKind::Promotion
        } else {
            RefinementKind::Critical
        }
    } else if has(Reason::ClassificationBoundary) || has(Reason::UnstableSearch) {
        RefinementKind::Uncertain
    } else if has(Reason::Tactical) {
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
    pub reasons: Vec<Reason>,
}
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub position_index: usize,
    pub score: u32,
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
        .map(|m| {
            rank_move(
                m,
                &game.positions[m.ply - 1],
                &raw[m.ply - 1],
                &raw[m.ply],
                book,
            )
        })
        .collect()
}
pub fn rank_move(
    m: &PlayedMove,
    position: &Chess,
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
        .filter(|(a, b)| a.depth == b.depth)
        .map(|(a, b)| (a.cp as f64 - b.cp as f64).abs());
    let legal = position.legal_moves().len();
    let pieces = position
        .board()
        .iter()
        .filter(|(_, piece)| piece.role != Role::Pawn && piece.role != Role::King)
        .count();
    let in_check = position.is_check();
    let capture = m.san.contains('x');
    let check = m.san.contains('+') || m.san.contains('#');
    let promotion = m.san.contains('=');
    let mate = before.cp.abs() >= 90000 || after.cp.abs() >= 90000;
    let mut score =
        ((loss / 20.0) * 24.0).clamp(0.0, 24.0) + ((swing / 20.0) * 11.0).clamp(0.0, 11.0);
    let mut reasons = Vec::new();
    if loss >= 5.0 {
        reasons.push(Reason::Loss);
    }
    if swing >= 10.0 {
        reasons.push(Reason::Swing);
    }
    if capture {
        score += 4.0;
    }
    if check {
        score += 7.0;
    }
    if promotion {
        reasons.push(Reason::Promotion);
        score += 12.0;
    }
    if in_check {
        score += 7.0;
    }
    if capture || check || promotion {
        reasons.push(Reason::Tactical);
    }
    score += (((legal as f64 - 18.0) / 22.0) * 10.0).clamp(0.0, 10.0)
        + ((pieces as f64 / 10.0) * 8.0).clamp(0.0, 8.0);
    if legal >= 30 || pieces >= 8 {
        reasons.push(Reason::Complex);
    }
    if let Some(gap) = gap {
        score += ((gap / 180.0) * 12.0).clamp(0.0, 12.0);
        if gap >= 80.0 {
            reasons.push(Reason::BestMoveGap);
        }
    }
    if [2.0, 5.0, 10.0, 20.0]
        .iter()
        .any(|b| (loss - b).abs() <= 1.25)
    {
        score += 8.0;
        reasons.push(Reason::ClassificationBoundary);
    }
    if [before, after].iter().any(|raw| {
        raw.lines
            .iter()
            .any(|line| line.multipv == 1 && line.unstable)
    }) {
        score += 8.0;
        reasons.push(Reason::UnstableSearch);
    }
    if mate {
        score += 20.0;
        reasons.push(Reason::Mate);
    }
    Critical {
        ply: m.ply,
        score: score.round().min(100.0) as u32,
        hard: loss >= 10.0 || swing >= 15.0 || mate || promotion,
        reasons,
    }
}

pub fn targets(critical: &[Critical], count: usize, profile: Profile) -> Vec<Target> {
    let mut ranked: Vec<_> = critical
        .iter()
        .filter(|m| m.score >= 32 || m.hard || m.reasons.contains(&Reason::UnstableSearch))
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
        let kind = refinement_kind(m);
        let pair = [m.ply - 1, m.ply];
        let indexes = if kind == RefinementKind::Complex {
            &pair[..1]
        } else {
            &pair[..]
        };
        if !m.hard && indexes.iter().any(|i| !selected.contains_key(i)) && selected.len() >= limit {
            continue;
        }
        for &i in indexes {
            if i >= count {
                continue;
            }
            let search = profile.budget(kind);
            let target = selected.entry(i).or_insert_with(|| {
                order.push(i);
                Target {
                    position_index: i,
                    score: m.score,
                    hard: m.hard,
                    kind,
                    search,
                }
            });
            target.score = target.score.max(m.score);
            target.hard |= m.hard;
            target.kind = target.kind.max(kind);
            target.search.ms = target.search.ms.max(search.ms);
            target.search.multipv = target.search.multipv.max(search.multipv);
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
    let positions = &game.positions;
    let forcing: Vec<_> = game
        .moves
        .iter()
        .enumerate()
        .map(|(i, m)| {
            m.san.contains('x')
                || m.san.contains('+')
                || m.san.contains('#')
                || m.san.contains('=')
                || (positions[i].is_check() || positions[i].legal_moves().len() == 1)
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
            let before = &positions[i];
            let after = &positions[i + 2];
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
            candidate.reasons.push(Reason::Tactical);
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
                && (forcing[i] || sacrifices[i] || m.reasons.contains(&Reason::Mate))
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

#[cfg(test)]
mod profile_tests {
    use super::*;

    #[test]
    fn invalid_profiles_return_contextual_errors() {
        for json in ["invalid", "{}", r#"{"fast":{},"deep":{}}"#] {
            let error = parse_profiles(json).err().unwrap();
            assert_eq!(error.code, ReviewErrorCode::Session);
            assert_eq!(error.operation, "review.profiles");
            assert!(std::error::Error::source(&error).is_some());
        }
        assert!(profile(AnalysisKind::Manual).unwrap().is_none());
        assert!(profile(AnalysisKind::Fast).unwrap().is_some());
        assert!(profile(AnalysisKind::Deep).unwrap().is_some());
    }
}
