//! Versioned experimental decision policy, independent of production profiles.
use super::{PositionStatus, Score};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Engine(Score),
    Terminal(PositionStatus),
}

/// CP loss is only meaningful when both values are centipawns. Mates are
/// deliberately left incomparable across different roots.
pub fn critical(best: &Value, played: &Value, threshold: i32) -> bool {
    match (best, played) {
        (Value::Engine(Score::Cp(a)), Value::Engine(Score::Cp(b))) => {
            a.saturating_sub(*b) >= threshold
        }
        (Value::Engine(Score::Mate(_)), _) | (_, Value::Engine(Score::Mate(_))) => true,
        _ => false,
    }
}

/// An independently searched child should not substantially exceed the root's
/// best score. Keep this uncertainty distinct from a loss by the played move.
pub fn conflict(best: &Value, played: &Value, threshold: i32) -> bool {
    matches!((best, played), (Value::Engine(Score::Cp(a)), Value::Engine(Score::Cp(b))) if b.saturating_sub(*a) >= threshold)
}
