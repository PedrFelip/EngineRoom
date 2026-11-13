//! Legal board exploration and candidate playback; no analysis is performed here.
use shakmaty::{Chess, Move, Position, san::SanPlus};
#[derive(Clone)]
pub struct Variation {
    pub root_ply: usize,
    pub branch_from: Option<(usize, usize)>,
    pub positions: Vec<Chess>,
    pub moves: Vec<Move>,
    pub san: Vec<String>,
    pub classifications: Vec<Option<engineroom_backend::review::types::Classification>>,
    pub cursor: usize,
}
impl Variation {
    pub fn new(root_ply: usize, position: Chess) -> Self {
        Self {
            root_ply,
            branch_from: None,
            positions: vec![position],
            moves: Vec::new(),
            san: Vec::new(),
            classifications: Vec::new(),
            cursor: 0,
        }
    }
    pub fn append(&mut self, m: Move) {
        self.positions.truncate(self.cursor + 1);
        self.moves.truncate(self.cursor);
        self.san.truncate(self.cursor);
        self.classifications.truncate(self.cursor);
        let mut position = self.positions[self.cursor].clone();
        self.san
            .push(SanPlus::from_move_and_play_unchecked(&mut position, m).to_string());
        self.moves.push(m);
        self.classifications.push(None);
        self.positions.push(position);
        self.cursor += 1;
    }
    pub fn position(&self) -> &Chess {
        &self.positions[self.cursor]
    }
}
pub fn board(position: &Chess) -> [char; 64] {
    std::array::from_fn(|index| {
        position
            .board()
            .piece_at(shakmaty::Square::new((index ^ 56) as u32))
            .map_or('.', |p| p.char())
    })
}
