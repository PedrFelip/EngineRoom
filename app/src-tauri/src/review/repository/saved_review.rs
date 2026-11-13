//! Validation and compatibility normalization of saved review payloads.
use crate::review::{core, scoring::*, types::*};

// Legacy data is normalized only at the Rust boundary, using persisted scores.
pub fn normalize(mut value: serde_json::Value) -> Result<ReviewResult> {
    let fail = || {
        ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "games.normalize",
            "Revisão salva inválida.",
        )
    };
    let old_positions = value
        .get("positions")
        .and_then(|v| v.as_array())
        .ok_or_else(fail)?;
    let fens: Vec<_> = old_positions
        .iter()
        .map(|v| v.get("fen").and_then(|v| v.as_str()).ok_or_else(fail))
        .collect::<Result<_>>()?;
    let phases = core::phases(fens);
    let current = value.get("accuracyModel").and_then(|v| v.as_str()) == Some(ACCURACY_MODEL)
        && value.get("accuracyByPhase").is_some()
        && old_positions.iter().all(|p| p.get("phase").is_some())
        && value
            .get("moves")
            .and_then(|v| v.as_array())
            .is_some_and(|ms| {
                ms.iter().all(|m| {
                    m.get("cpLoss").is_some()
                        && !matches!(
                            m.get("classification").and_then(|v| v.as_str()),
                            Some("brilhante" | "otimo")
                        )
                })
            });
    for (i, p) in value["positions"]
        .as_array_mut()
        .ok_or_else(fail)?
        .iter_mut()
        .enumerate()
    {
        if p.get("phase").is_none() {
            p["phase"] = serde_json::to_value(phases[i]).map_err(|_| fail())?;
        }
    }
    let cps: Vec<_> = value["positions"]
        .as_array()
        .ok_or_else(fail)?
        .iter()
        .map(|p| p["cp"].as_f64().ok_or_else(fail))
        .collect::<Result<_>>()?;
    for m in value["moves"].as_array_mut().ok_or_else(fail)? {
        let ply = m["ply"].as_u64().filter(|p| *p > 0).ok_or_else(fail)? as usize;
        if matches!(m["classification"].as_str(), Some("brilhante" | "otimo")) {
            m["classification"] = serde_json::to_value(classify(
                m["winPctLoss"].as_f64().ok_or_else(fail)?,
                m["isBook"].as_bool().ok_or_else(fail)?,
            ))
            .map_err(|_| fail())?;
        }
        if m.get("cpLoss").is_none() {
            m["cpLoss"] = serde_json::json!(cps
                .get(ply - 1)
                .zip(cps.get(ply))
                .map_or(0.0, |(a, b)| (a + b).max(0.0)));
        }
    }
    value["accuracyModel"] = serde_json::json!(ACCURACY_MODEL);
    if !current {
        value["accuracyByPhase"] = serde_json::to_value(PhaseAccuracy {
            opening: accuracy(&[], &[50.0]),
            middlegame: accuracy(&[], &[50.0]),
            endgame: accuracy(&[], &[50.0]),
        })
        .map_err(|_| fail())?;
    }
    let mut review: ReviewResult = serde_json::from_value(value).map_err(|_| fail())?;
    validate_review(&review)?;
    if !current {
        let values: Vec<_> = review.positions.iter().map(|p| p.win_pct).collect();
        review.accuracy = accuracy(
            &review
                .moves
                .iter()
                .map(|m| m.played.color.as_str())
                .collect::<Vec<_>>(),
            &values,
        );
        review.accuracy_by_phase = phase_accuracy(&review.moves, &phases, &values);
    }
    Ok(review)
}
pub fn validate_review(review: &ReviewResult) -> Result<()> {
    let valid = review.positions.len() == review.moves.len() + 1
        && review.positions.iter().enumerate().all(|(i, p)| {
            p.ply == i
                && p.win_pct.is_finite()
                && p.lines
                    .iter()
                    .chain(p.triage_lines.iter().flatten())
                    .all(|l| l.multipv > 0 && l.win_pct.is_finite())
        })
        && review.moves.iter().enumerate().all(|(i, m)| {
            m.played.ply == i + 1
                && [m.win_pct_before, m.win_pct_after, m.win_pct_loss, m.cp_loss]
                    .iter()
                    .all(|v| v.is_finite())
        });
    if valid {
        Ok(())
    } else {
        Err(ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "review.validate",
            "Revisão inválida.",
        ))
    }
}

/// Restored reviews must describe the PGN mainline. Legacy JSON may omit newer
/// analysis fields, but chess positions and moves must still match the game.
pub fn validate_game(review: &ReviewResult, game: &core::Game) -> Result<()> {
    validate_review(review)?;
    let matches_game = review.positions.len() == game.fens.len()
        && review
            .positions
            .iter()
            .zip(&game.fens)
            .all(|(saved, expected)| {
                core::position(&saved.fen).is_ok_and(|position| core::fen(&position) == *expected)
            })
        && review
            .moves
            .iter()
            .zip(&game.moves)
            .all(|(saved, expected)| {
                let played = &saved.played;
                played.color == expected.color
                    && played.san == expected.san
                    && played.uci == expected.uci
                    && core::position(&played.fen_before)
                        .is_ok_and(|position| core::fen(&position) == expected.fen_before)
            });
    if matches_game {
        Ok(())
    } else {
        Err(ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "review.restore",
            "Revisão salva não corresponde ao PGN.",
        ))
    }
}
