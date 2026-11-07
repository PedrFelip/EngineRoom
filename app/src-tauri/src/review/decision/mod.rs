//! Experimental decision model. This is not wired into the production pipeline.
pub mod driver;
pub mod planner;
pub mod policy;
mod resource_metrics;
pub mod runner;

use crate::review::{core, types::Result};
use serde::{Deserialize, Serialize};
use shakmaty::{uci::UciMove, Position};

pub const POLICY: &str = "b-experimental-2";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum History {
    Complete,
    BeforeRootUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub root_fen: String,
    pub prefix: Vec<String>,
    pub history: History,
    pub policy: String,
    pub fen: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub ply: usize,
    pub before: Context,
    pub played: String,
    pub after: Context,
}

/// Conservatively treats any explicit FEN tag as having unknown earlier history.
pub fn decisions(pgn: &str) -> Result<Vec<Decision>> {
    let game = core::extract(pgn)?;
    let history = if game.has_fen_tag {
        History::BeforeRootUnknown
    } else {
        History::Complete
    };
    let context = |i: usize| Context {
        root_fen: game.fens[0].clone(),
        prefix: game.moves[..i].iter().map(|m| m.uci.clone()).collect(),
        history: history.clone(),
        policy: POLICY.into(),
        fen: game.fens[i].clone(),
    };
    Ok(game
        .moves
        .iter()
        .enumerate()
        .map(|(i, m)| Decision {
            ply: i + 1,
            before: context(i),
            played: m.uci.clone(),
            after: context(i + 1),
        })
        .collect())
}

/// Claims are reported but do not end the analysis. Only forced endings suppress a search.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionStatus {
    Checkmate,
    Stalemate,
    InsufficientMaterial,
    SeventyFiveMoves,
    FivefoldRepetition,
    Claimable { fifty_moves: bool, threefold: bool },
    Ongoing,
}

pub fn position_status(context: &Context) -> Result<PositionStatus> {
    let mut position = core::position(&context.root_fen)?;
    if context.policy != POLICY
        || core::fen(&position) != context.root_fen
        || (context.history == History::Complete && position != shakmaty::Chess::default())
    {
        return Err(crate::review::types::ReviewError::new(
            crate::review::types::ReviewErrorCode::InvalidPgn,
            "decision.context",
            "Identidade de contexto inválida",
        ));
    }
    let repetition_key = |p: &shakmaty::Chess| {
        core::fen(p)
            .split_whitespace()
            .take(4)
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut keys = vec![repetition_key(&position)];
    for uci in &context.prefix {
        let m = uci
            .parse::<UciMove>()
            .ok()
            .and_then(|u| u.to_move(&position).ok())
            .ok_or_else(|| {
                crate::review::types::ReviewError::new(
                    crate::review::types::ReviewErrorCode::InvalidPgn,
                    "decision.context",
                    "Prefixo UCI inválido",
                )
            })?;
        position.play_unchecked(m);
        keys.push(repetition_key(&position));
    }
    if core::fen(&position) != context.fen {
        return Err(crate::review::types::ReviewError::new(
            crate::review::types::ReviewErrorCode::InvalidPgn,
            "decision.context",
            "FEN não corresponde à raiz e ao prefixo",
        ));
    }
    if position.is_checkmate() {
        return Ok(PositionStatus::Checkmate);
    }
    if position.is_stalemate() {
        return Ok(PositionStatus::Stalemate);
    }
    if position.is_insufficient_material() {
        return Ok(PositionStatus::InsufficientMaterial);
    }
    if position.halfmoves() >= 150 {
        return Ok(PositionStatus::SeventyFiveMoves);
    }
    let current = keys.last().expect("root exists");
    let occurrences = keys.iter().filter(|key| *key == current).count();
    if occurrences >= 5 {
        return Ok(PositionStatus::FivefoldRepetition);
    }
    let fifty_moves = position.halfmoves() >= 100;
    let threefold = occurrences >= 3;
    if fifty_moves || threefold {
        Ok(PositionStatus::Claimable {
            fifty_moves,
            threefold,
        })
    } else {
        Ok(PositionStatus::Ongoing)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Score {
    Cp(i32),
    Mate(i32),
}
impl Score {
    /// Convert a score from the side to move in the child to the mover's perspective.
    pub fn invert(&self) -> Option<Self> {
        match self {
            Self::Cp(n) => n.checked_neg().map(Self::Cp),
            Self::Mate(n) => n.checked_neg().map(Self::Mate),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bound {
    Exact,
    Lower,
    Upper,
}
impl Bound {
    pub fn invert(&self) -> Self {
        match self {
            Self::Exact => Self::Exact,
            Self::Lower => Self::Upper,
            Self::Upper => Self::Lower,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub depth: u32,
    pub execution_id: u64,
    pub score: Score,
    pub bound: Bound,
    pub pv: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineSignature {
    pub executable_digest: String,
    pub options: Vec<(String, String)>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub id: u64,
    pub execution_id: u64,
    pub context: Context,
    pub engine: EngineSignature,
    pub unrestricted: bool,
    pub depth: u32,
    pub lines: Vec<Line>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub context: Context,
    pub engine: EngineSignature,
    pub depth: u32,
    pub multipv: usize,
}

/// A single run must provide all requested coverage and legal, distinct PVs.
pub fn satisfies(e: &Evidence, r: &Request) -> bool {
    if e.context != r.context
        || e.engine != r.engine
        || !e.unrestricted
        || e.depth < r.depth
        || r.multipv == 0
        || e.engine.executable_digest.len() != 64
        || !e
            .engine
            .executable_digest
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || !e.engine.options.windows(2).all(|w| w[0].0 < w[1].0)
    {
        return false;
    }
    if position_status(&r.context).is_err() {
        return false;
    }
    let Ok(position) = core::position(&r.context.fen) else {
        return false;
    };
    let required = r.multipv.min(position.legal_moves().len());
    if required == 0 || e.lines.len() < required {
        return false;
    }
    let mut roots = std::collections::HashSet::new();
    for line in e.lines.iter().take(required) {
        if matches!(line.score, Score::Cp(i32::MIN) | Score::Mate(i32::MIN))
            || line.depth != e.depth
            || line.execution_id != e.execution_id
        {
            return false;
        }
        if line.bound != Bound::Exact || line.pv.is_empty() || !roots.insert(&line.pv[0]) {
            return false;
        }
        let mut p = position.clone();
        for uci in &line.pv {
            let Ok(m) = uci
                .parse::<UciMove>()
                .and_then(|u| u.to_move(&p).map_err(|_| shakmaty::uci::ParseUciMoveError))
            else {
                return false;
            };
            p.play_unchecked(m);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_and_perspective() {
        assert_eq!(Score::Cp(i32::MIN).invert(), None);
        assert_eq!(Score::Mate(i32::MIN).invert(), None);
        let d = decisions("1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 *").unwrap();
        assert_ne!(d[0].before, d[4].before);
        assert_eq!(Score::Cp(-20).invert(), Some(Score::Cp(20)));
        assert_eq!(Score::Mate(0).invert(), Some(Score::Mate(0)));
        assert_eq!(Bound::Lower.invert(), Bound::Upper);
        let mut inconsistent = d[0].before.clone();
        inconsistent.fen = d[0].after.fen.clone();
        assert!(position_status(&inconsistent).is_err());
        assert_eq!(d[4].before.prefix.len(), 4);
    }
    #[test]
    fn fen_history_is_unknown() {
        let d = decisions("[FEN \"8/P7/8/8/8/8/4k3/4K3 w - - 0 1\"]\n\n1. a8=Q *");
        // Use a legal promotion position, with kings separated.
        let d = d
            .or_else(|_| decisions("[FEN \"7k/P7/8/8/8/8/8/4K3 w - - 0 1\"]\n\n1. a8=Q+ *"))
            .unwrap();
        assert_eq!(d[0].before.history, History::BeforeRootUnknown);
    }
    #[test]
    fn claims_are_distinct_from_forced_draws() {
        let claim = decisions("[FEN \"7k/8/8/8/8/8/R7/4K3 w - - 100 1\"]\n\n1. Ra3 *").unwrap();
        assert!(matches!(
            position_status(&claim[0].before).unwrap(),
            PositionStatus::Claimable {
                fifty_moves: true,
                ..
            }
        ));
        let forced = decisions("[FEN \"7k/8/8/8/8/8/R7/4K3 w - - 150 1\"]\n\n1. Ra3 *").unwrap();
        assert_eq!(
            position_status(&forced[0].before).unwrap(),
            PositionStatus::SeventyFiveMoves
        );
        let repeat = decisions("1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 Nf6 4. Ng1 Ng8 5. Nf3 *").unwrap();
        assert!(matches!(
            position_status(&repeat[8].before).unwrap(),
            PositionStatus::Claimable {
                threefold: true,
                ..
            }
        ));
    }
    #[test]
    fn limits_repetition_and_promotion_context() {
        for (clock, expected) in [
            (99, PositionStatus::Ongoing),
            (
                100,
                PositionStatus::Claimable {
                    fifty_moves: true,
                    threefold: false,
                },
            ),
            (
                149,
                PositionStatus::Claimable {
                    fifty_moves: true,
                    threefold: false,
                },
            ),
            (150, PositionStatus::SeventyFiveMoves),
        ] {
            let pgn = format!("[FEN \"7k/8/8/8/8/8/R7/4K3 w - - {clock} 1\"]\n\n1. Ra3 *");
            assert_eq!(
                position_status(&decisions(&pgn).unwrap()[0].before).unwrap(),
                expected
            );
        }
        let repeated = decisions("1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 Nf6 4. Ng1 Ng8 5. Nf3 Nf6 6. Ng1 Ng8 7. Nf3 Nf6 8. Ng1 Ng8 9. Nf3 *").unwrap();
        assert_eq!(
            position_status(&repeated[16].before).unwrap(),
            PositionStatus::FivefoldRepetition
        );
        let promotion = decisions("[FEN \"7k/P7/8/8/8/8/8/4K3 w - - 0 1\"]\n\n1. a8=Q+ *").unwrap();
        assert_eq!(promotion[0].played, "a7a8q");
        assert_eq!(Score::Mate(3).invert(), Some(Score::Mate(-3)));
        assert_eq!(Score::Mate(-2).invert(), Some(Score::Mate(2)));
        assert_eq!(Bound::Upper.invert(), Bound::Lower);
    }

    #[test]
    fn evidence_requires_individual_exact_legal_coverage() {
        let c = decisions("1. e4 *").unwrap()[0].before.clone();
        let sig = EngineSignature {
            executable_digest: "a".repeat(64),
            options: vec![],
        };
        let r = Request {
            context: c.clone(),
            engine: sig.clone(),
            depth: 8,
            multipv: 2,
        };
        let mut e = Evidence {
            id: 1,
            execution_id: 1,
            context: c,
            engine: sig,
            unrestricted: true,
            depth: 8,
            lines: vec![
                Line {
                    depth: 8,
                    execution_id: 1,
                    score: Score::Cp(40),
                    bound: Bound::Exact,
                    pv: vec!["e2e4".into()],
                },
                Line {
                    depth: 8,
                    execution_id: 1,
                    score: Score::Mate(3),
                    bound: Bound::Exact,
                    pv: vec!["d2d4".into()],
                },
            ],
        };
        assert!(satisfies(&e, &r));
        for score in [Score::Cp(i32::MIN), Score::Mate(i32::MIN)] {
            e.lines[0].score = score;
            assert!(!satisfies(&e, &r));
        }
        e.lines[0].score = Score::Cp(40);
        e.lines[1].depth = 7;
        assert!(!satisfies(&e, &r));
        e.lines[1].depth = 8;
        e.lines[1].execution_id = 2;
        assert!(!satisfies(&e, &r));
        e.lines[1].execution_id = 1;
        e.lines[1].bound = Bound::Lower;
        assert!(!satisfies(&e, &r));
        e.lines[1].bound = Bound::Exact;
        e.lines[1].pv = vec!["e2e4".into()];
        assert!(!satisfies(&e, &r));
        e.lines[1].pv = vec!["e2e5".into()];
        assert!(!satisfies(&e, &r));
    }
}
