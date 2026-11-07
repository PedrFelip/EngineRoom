//! Deterministic presentation fixtures. Scores are fictional; candidate moves are legal.
use rune::{Color, oklch};
use shakmaty::{CastlingMode, Chess, Move, Position, san::SanPlus};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classification {
    Book,
    Best,
    Excellent,
    Good,
    Inaccuracy,
    Mistake,
    Blunder,
}
pub const CLASSES: [Classification; 7] = [
    Classification::Book,
    Classification::Best,
    Classification::Excellent,
    Classification::Good,
    Classification::Inaccuracy,
    Classification::Mistake,
    Classification::Blunder,
];
impl Classification {
    pub fn label(self) -> &'static str {
        match self {
            Self::Book => "Livro",
            Self::Best => "Melhor",
            Self::Excellent => "Excelente",
            Self::Good => "Bom",
            Self::Inaccuracy => "Imprecisão",
            Self::Mistake => "Erro",
            Self::Blunder => "Blunder",
        }
    }
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Book => "B",
            Self::Best => "★",
            Self::Excellent => "!!",
            Self::Good => "✓",
            Self::Inaccuracy => "?!",
            Self::Mistake => "?",
            Self::Blunder => "??",
        }
    }
    pub fn color(self) -> Color {
        match self {
            Self::Book => oklch(0.66, 0.12, 295.0),
            Self::Best => oklch(0.7, 0.11, 225.0),
            Self::Excellent => oklch(0.7, 0.13, 160.0),
            Self::Good => oklch(0.7, 0.1, 145.0),
            Self::Inaccuracy => oklch(0.8, 0.12, 85.0),
            Self::Mistake => oklch(0.74, 0.15, 55.0),
            Self::Blunder => oklch(0.68, 0.18, 28.0),
        }
    }
    pub fn critical(self) -> bool {
        matches!(self, Self::Mistake | Self::Blunder)
    }
}
pub fn classification(ply: usize) -> Classification {
    const PATTERN: [Classification; 16] = [
        Classification::Book,
        Classification::Book,
        Classification::Best,
        Classification::Good,
        Classification::Excellent,
        Classification::Inaccuracy,
        Classification::Best,
        Classification::Mistake,
        Classification::Good,
        Classification::Best,
        Classification::Blunder,
        Classification::Good,
        Classification::Best,
        Classification::Inaccuracy,
        Classification::Excellent,
        Classification::Mistake,
    ];
    PATTERN[ply.saturating_sub(1) % PATTERN.len()]
}
pub fn cp(ply: usize) -> i32 {
    const SCORES: [i32; 17] = [
        18, 24, 12, 38, 20, 64, 48, 82, 164, 126, 102, -142, -88, -64, 34, 76, 152,
    ];
    SCORES[ply % SCORES.len()]
}
pub fn win_pct(ply: usize) -> f32 {
    50.0 + (cp(ply) as f32 / 5.0).clamp(-45.0, 45.0)
}
pub fn eval(cp: i32) -> String {
    format!("{:+.2}", cp as f32 / 100.0)
}
pub fn accuracy(plies: impl Iterator<Item = usize>) -> Option<f32> {
    let scores: Vec<f32> = plies
        .map(|ply| match classification(ply) {
            Classification::Book | Classification::Best => 100.0,
            Classification::Excellent => 98.0,
            Classification::Good => 92.0,
            Classification::Inaccuracy => 75.0,
            Classification::Mistake => 48.0,
            Classification::Blunder => 18.0,
        })
        .collect();
    if scores.is_empty() {
        None
    } else {
        Some(scores.iter().sum::<f32>() / scores.len() as f32)
    }
}
#[derive(Clone)]
pub struct Candidate {
    pub moves: Vec<Move>,
    pub san: Vec<String>,
    pub cp: i32,
    pub depth: usize,
}
pub fn candidates(position: &Chess, ply: usize) -> Vec<Candidate> {
    let mut legal: Vec<Move> = position.legal_moves().into_iter().collect();
    legal.sort_by_key(|m| m.to_uci(CastlingMode::Standard).to_string());
    legal
        .into_iter()
        .take(3)
        .enumerate()
        .map(|(index, first)| {
            let mut p = position.clone();
            let mut moves = Vec::new();
            let mut san = Vec::new();
            let mut next = Some(first);
            for step in 0..5 {
                let Some(m) = next else {
                    break;
                };
                san.push(SanPlus::from_move_and_play_unchecked(&mut p, m).to_string());
                moves.push(m);
                let choices = p.legal_moves();
                next = choices
                    .get((ply + index + step) % choices.len().max(1))
                    .copied();
            }
            Candidate {
                moves,
                san,
                cp: cp(ply) - index as i32 * 36,
                depth: 22 - index * 2,
            }
        })
        .collect()
}
pub fn squares(m: Move) -> (usize, usize) {
    let uci = m.to_uci(CastlingMode::Standard).to_string();
    let bytes = uci.as_bytes();
    let square = |offset: usize| {
        (8 - (bytes[offset + 1] - b'0') as usize) * 8 + (bytes[offset] - b'a') as usize
    };
    (square(0), square(2))
}
