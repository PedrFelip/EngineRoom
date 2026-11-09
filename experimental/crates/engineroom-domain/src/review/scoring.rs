use super::types::*;
pub const ACCURACY_MODEL: &str = "lichess-2026-10-initial-position";
pub fn win_pct(cp: i32) -> f64 {
    50.0 + 50.0 * (2.0 / (1.0 + (-0.00368208 * cp.clamp(-1000, 1000) as f64).exp()) - 1.0)
}
pub fn white_win_pct(cp: i32, white: bool) -> f64 {
    if white {
        win_pct(cp)
    } else {
        100.0 - win_pct(cp)
    }
}
pub fn classify(loss: f64, book: bool) -> Classification {
    if book {
        Classification::Livro
    } else if loss <= 0.0 {
        Classification::Melhor
    } else if loss <= 2.0 {
        Classification::Excelente
    } else if loss <= 5.0 {
        Classification::Bom
    } else if loss <= 10.0 {
        Classification::Imprecisao
    } else if loss <= 20.0 {
        Classification::Erro
    } else {
        Classification::Blunder
    }
}
pub fn move_accuracy(loss: f64) -> f64 {
    if loss <= 0.0 {
        return 100.0;
    }
    (103.1668100711649 * (-0.04354415386753951 * loss).exp() - 3.166924740191411 + 1.0)
        .clamp(0.0, 100.0)
}
pub fn weights(values: &[f64], count: usize) -> Vec<f64> {
    let size = (count / 10).clamp(2, 8);
    let stddev = |window: &[f64]| {
        let mean = window.iter().sum::<f64>() / window.len() as f64;
        (window.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / window.len() as f64)
            .sqrt()
            .clamp(0.5, 12.0)
    };
    let mut result = Vec::new();
    for _ in 0..size.min(values.len()).saturating_sub(2) {
        result.push(stddev(&values[..size.min(values.len())]));
    }
    for window in values.windows(size) {
        result.push(stddev(window));
    }
    result.resize(count, 0.5);
    result
}
pub fn accuracy(colors: &[&str], position_values: &[f64]) -> Accuracy {
    let values = position_values;
    let weights = weights(values, colors.len());
    let aggregate = |color: &str| {
        let samples: Vec<_> = colors
            .iter()
            .enumerate()
            .filter(|(_, c)| **c == color)
            .map(|(i, _)| {
                let loss = if color == "w" {
                    values[i] - values[i + 1]
                } else {
                    values[i + 1] - values[i]
                };
                (move_accuracy(loss.max(0.0)), weights[i])
            })
            .collect();
        if samples.is_empty() {
            return 100.0;
        }
        let weighted = samples.iter().map(|(a, w)| a * w).sum::<f64>()
            / samples.iter().map(|(_, w)| w).sum::<f64>();
        let harmonic =
            samples.len() as f64 / samples.iter().map(|(a, _)| 1.0 / a.max(1.0)).sum::<f64>();
        (weighted + harmonic) / 2.0
    };
    Accuracy {
        white: aggregate("w"),
        black: aggregate("b"),
    }
}
pub fn phase_accuracy(moves: &[MoveAnalysis], phases: &[Phase], values: &[f64]) -> PhaseAccuracy {
    let for_phase = |phase| {
        let selected: Vec<_> = moves
            .iter()
            .filter(|m| phases[m.played.ply - 1] == phase)
            .collect();
        let Some(first) = selected.first() else {
            return accuracy(&[], &[50.0]);
        };
        let colors: Vec<_> = selected.iter().map(|m| m.played.color.as_str()).collect();
        let mut v = vec![values[first.played.ply - 1]];
        v.extend(selected.iter().map(|m| values[m.played.ply]));
        accuracy(&colors, &v)
    };
    PhaseAccuracy {
        opening: for_phase(Phase::Opening),
        middlegame: for_phase(Phase::Middlegame),
        endgame: for_phase(Phase::Endgame),
    }
}
