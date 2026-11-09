//! Display formatting for backend classifications, evaluations, and legal candidate moves.
use rune::{Color, oklch};
use shakmaty::{CastlingMode, Move};
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
            Self::Best => "",
            Self::Excellent => "!!",
            Self::Good => "",
            Self::Inaccuracy => "?!",
            Self::Mistake => "?",
            Self::Blunder => "??",
        }
    }
    pub fn color(self) -> Color {
        match self {
            Self::Book => oklch(0.70, 0.075, 295.0),
            Self::Best => oklch(0.70, 0.08, 225.0),
            Self::Excellent => oklch(0.70, 0.10, 160.0),
            Self::Good => oklch(0.70, 0.075, 145.0),
            Self::Inaccuracy => oklch(0.80, 0.105, 85.0),
            Self::Mistake => oklch(0.74, 0.115, 55.0),
            Self::Blunder => oklch(0.70, 0.13, 28.0),
        }
    }
    pub fn critical(self) -> bool {
        matches!(self, Self::Mistake | Self::Blunder)
    }
}
pub fn eval(cp: i32) -> String {
    format!("{:+.2}", cp as f32 / 100.0)
}
#[derive(Clone)]
pub struct Candidate {
    pub moves: Vec<Move>,
    pub san: Vec<String>,
    pub cp: i32,
    pub win_pct: f64,
    pub depth: usize,
}
pub fn squares(m: Move) -> (usize, usize) {
    let uci = m.to_uci(CastlingMode::Standard).to_string();
    let bytes = uci.as_bytes();
    let square = |offset: usize| {
        (8 - (bytes[offset + 1] - b'0') as usize) * 8 + (bytes[offset] - b'a') as usize
    };
    (square(0), square(2))
}
