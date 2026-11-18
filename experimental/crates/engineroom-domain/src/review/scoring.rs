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
const MAX_WINDOW_SIZE: usize = 8;

fn window_size(count: usize) -> usize {
    (count / 10).clamp(2, MAX_WINDOW_SIZE)
}

fn window_weight(window: &[f64]) -> f64 {
    let mean = window.iter().sum::<f64>() / window.len() as f64;
    (window.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / window.len() as f64)
        .sqrt()
        .clamp(0.5, 12.0)
}

pub fn weights(values: &[f64], count: usize) -> Vec<f64> {
    let size = window_size(count);
    let mut result = Vec::with_capacity(count);
    for _ in 0..size.min(values.len()).saturating_sub(2) {
        result.push(window_weight(&values[..size.min(values.len())]));
    }
    for window in values.windows(size) {
        result.push(window_weight(window));
    }
    result.resize(count, 0.5);
    result
}

struct AccuracySamples {
    count: usize,
    weight_sum: f64,
    weighted_sum: f64,
    harmonic_inv_sum: f64,
}

impl AccuracySamples {
    fn new() -> Self {
        // Match the starting values used by f64::sum.
        Self {
            count: 0,
            weight_sum: -0.0,
            weighted_sum: -0.0,
            harmonic_inv_sum: -0.0,
        }
    }

    fn add(&mut self, loss: f64, weight: f64) {
        let accuracy = move_accuracy(loss.max(0.0));
        self.count += 1;
        self.weight_sum += weight;
        self.weighted_sum += accuracy * weight;
        self.harmonic_inv_sum += 1.0 / accuracy.max(1.0);
    }

    fn finish(self) -> f64 {
        if self.count == 0 {
            return 100.0;
        }
        let weighted = self.weighted_sum / self.weight_sum;
        let harmonic = self.count as f64 / self.harmonic_inv_sum;
        (weighted + harmonic) / 2.0
    }
}

struct ColorSamples {
    white: AccuracySamples,
    black: AccuracySamples,
}

impl ColorSamples {
    fn new() -> Self {
        Self {
            white: AccuracySamples::new(),
            black: AccuracySamples::new(),
        }
    }

    fn add(&mut self, color: &str, before: f64, after: f64, weight: f64) {
        match color {
            "w" => self.white.add(before - after, weight),
            "b" => self.black.add(after - before, weight),
            _ => {}
        }
    }

    fn finish(self) -> Accuracy {
        Accuracy {
            white: self.white.finish(),
            black: self.black.finish(),
        }
    }
}

pub fn accuracy(colors: &[&str], position_values: &[f64]) -> Accuracy {
    let weights = weights(position_values, colors.len());
    let mut samples = ColorSamples::new();
    for (i, (&color, weight)) in colors.iter().zip(weights).enumerate() {
        if matches!(color, "w" | "b") {
            samples.add(color, position_values[i], position_values[i + 1], weight);
        }
    }
    samples.finish()
}

pub fn phase_accuracy(moves: &[MoveAnalysis], phases: &[Phase], values: &[f64]) -> PhaseAccuracy {
    let for_phase = |phase| {
        let selected = moves.iter().filter(|m| phases[m.played.ply - 1] == phase);
        let mut samples = ColorSamples::new();
        let Some(first) = selected.clone().next() else {
            return samples.finish();
        };

        let size = window_size(selected.clone().count());
        let mut before = values[first.played.ply - 1];
        // Each phase starts at its first ply's input, then uses the output of
        // each matching ply even when the phase skips intervening plies.
        let mut future = selected.clone().map(|m| values[m.played.ply]);
        let mut buffer = [0.0; MAX_WINDOW_SIZE];
        let window = &mut buffer[..size];
        window[0] = before;
        for value in &mut window[1..] {
            *value = future
                .next()
                .expect("phase has enough values for its window");
        }
        let mut weight = window_weight(window);
        for (i, m) in selected.enumerate() {
            if i > size - 2 {
                window.copy_within(1.., 0);
                window[size - 1] = future
                    .next()
                    .expect("phase has one window per ply after padding");
                weight = window_weight(window);
            }
            let after = values[m.played.ply];
            samples.add(&m.played.color, before, after, weight);
            before = after;
        }
        samples.finish()
    };
    PhaseAccuracy {
        opening: for_phase(Phase::Opening),
        middlegame: for_phase(Phase::Middlegame),
        endgame: for_phase(Phase::Endgame),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_accuracy(colors: &[&str], values: &[f64]) -> Accuracy {
        let weights = weights(values, colors.len());
        let aggregate = |color| {
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

    fn assert_bits(actual: Accuracy, expected: Accuracy) {
        assert_eq!(actual.white.to_bits(), expected.white.to_bits());
        assert_eq!(actual.black.to_bits(), expected.black.to_bits());
    }

    #[test]
    fn accuracy_matches_reference_for_empty_and_single_side_samples() {
        let values = [50.0, 44.2, 89.13, 20.7, 6.3];
        assert_bits(accuracy(&[], &[]), reference_accuracy(&[], &[]));
        for colors in [["w"; 4], ["b"; 4], ["w", "unknown", "b", "w"]] {
            assert_bits(
                accuracy(&colors, &values),
                reference_accuracy(&colors, &values),
            );
        }
    }

    #[test]
    fn phase_accuracy_matches_reference_for_noncontiguous_windows() {
        let plies = 125;
        let phases: Vec<_> = (0..plies)
            .map(|i| {
                if i % 4 == 0 {
                    Phase::Middlegame
                } else {
                    Phase::Opening
                }
            })
            .collect();
        let values: Vec<_> = (0..=plies)
            .map(|i| ((i * i * 17) % 997) as f64 / 10.0)
            .collect();
        let moves: Vec<_> = (1..=plies)
            .map(|ply| MoveAnalysis {
                played: PlayedMove {
                    ply,
                    color: if ply % 2 == 1 { "w" } else { "b" }.into(),
                    san: String::new(),
                    uci: String::new(),
                    fen_before: String::new(),
                },
                classification: Classification::Melhor,
                win_pct_before: 0.0,
                win_pct_after: 0.0,
                win_pct_loss: 0.0,
                cp_loss: 0.0,
                best_uci: None,
                is_book: false,
                eco: None,
            })
            .collect();

        for moves in [&moves[..], &moves[..1], &moves[..0]] {
            let actual = phase_accuracy(moves, &phases, &values);
            for (phase, actual) in [
                (Phase::Opening, actual.opening),
                (Phase::Middlegame, actual.middlegame),
                (Phase::Endgame, actual.endgame),
            ] {
                let selected: Vec<_> = moves
                    .iter()
                    .map(|m| &m.played)
                    .filter(|m| phases[m.ply - 1] == phase)
                    .collect();
                let colors: Vec<_> = selected.iter().map(|m| m.color.as_str()).collect();
                let phase_values: Vec<_> = selected
                    .first()
                    .map(|m| values[m.ply - 1])
                    .into_iter()
                    .chain(selected.iter().map(|m| values[m.ply]))
                    .collect();
                assert_bits(actual, reference_accuracy(&colors, &phase_values));
            }
        }
    }
}
