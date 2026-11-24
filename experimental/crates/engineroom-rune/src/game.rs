//! Local PGN preview. Analysis and persistence remain the backend's responsibility.
use engineroom_backend::review::core;
use pgn_reader::{RawTag, Reader, SanPlus, Visitor};
use shakmaty::{Chess, Position, Square};
use std::{collections::HashMap, io::Cursor, ops::ControlFlow};

// Share the strict token check with core::extract without replaying its mainline.
#[path = "../../engineroom-application/src/review/core/pgn_tokens.rs"]
mod pgn_tokens;

pub const DEMO_PGN: &str = "[Event \"Estudo de abertura\"]\n[White \"Jogador de brancas\"]\n[Black \"Jogador de pretas\"]\n[Result \"*\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 6. Re1 b5 7. Bb3 d6 8. c3 O-O *";
#[derive(Clone)]
pub struct Game {
    pub white: String,
    pub black: String,
    pub event: String,
    pub avatar_key: Option<String>,
    pub ratings: [Option<String>; 2],
    positions: Vec<Chess>,
    /// FEN for each position, parallel to `positions`. Analysis lookups reuse
    /// these instead of re-serializing the same board several times per frame.
    fens: Vec<String>,
    pub moves: Vec<String>,
}
impl Game {
    pub fn empty() -> Self {
        let position = Chess::default();
        Self {
            white: "Brancas".into(),
            black: "Pretas".into(),
            event: "Nova partida".into(),
            avatar_key: None,
            ratings: [None, None],
            fens: vec![core::fen(&position)],
            positions: vec![position],
            moves: Vec::new(),
        }
    }

    pub fn positions(&self) -> &[Chess] {
        &self.positions
    }

    /// Cached FEN for the position at `ply`.
    pub fn fen(&self, ply: usize) -> &str {
        &self.fens[ply.min(self.fens.len() - 1)]
    }

    #[cfg(test)]
    pub(crate) fn replace_position(&mut self, ply: usize, position: Chess) {
        self.fens[ply] = core::fen(&position);
        self.positions[ply] = position;
    }

    pub fn board(&self, ply: usize) -> [char; 64] {
        let position = &self.positions[ply.min(self.moves.len())];
        std::array::from_fn(|index| {
            let square = Square::new((index ^ 56) as u32);
            position
                .board()
                .piece_at(square)
                .map_or('.', |piece| piece.char())
        })
    }
}
const MAX_HALF_MOVES: usize = 2000;

#[derive(Default)]
struct Preview {
    over_limit: bool,
}
impl Visitor for Preview {
    type Tags = HashMap<String, String>;
    type Movetext = Game;
    type Output = Result<Game, String>;
    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(HashMap::new())
    }
    fn tag(
        &mut self,
        tags: &mut Self::Tags,
        name: &[u8],
        value: RawTag<'_>,
    ) -> ControlFlow<Self::Output> {
        tags.insert(
            String::from_utf8_lossy(name).into_owned(),
            value.decode_utf8_lossy().into_owned(),
        );
        ControlFlow::Continue(())
    }
    fn begin_movetext(&mut self, mut tags: Self::Tags) -> ControlFlow<Self::Output, Game> {
        let position = match tags.remove("FEN") {
            Some(fen) => match core::position(&fen) {
                Ok(position) => position,
                Err(error) => return ControlFlow::Break(Err(error.to_string())),
            },
            None => Chess::default(),
        };
        ControlFlow::Continue(Game {
            fens: vec![core::fen(&position)],
            white: tags
                .get("White")
                .cloned()
                .unwrap_or_else(|| "Brancas".into()),
            black: tags
                .get("Black")
                .cloned()
                .unwrap_or_else(|| "Pretas".into()),
            event: tags
                .remove("Event")
                .unwrap_or_else(|| "Partida importada".into()),
            avatar_key: None,
            ratings: [tags.remove("WhiteElo"), tags.remove("BlackElo")],
            positions: vec![position],
            moves: Vec::new(),
        })
    }
    fn san(&mut self, game: &mut Game, san: SanPlus) -> ControlFlow<Self::Output> {
        let previous = game
            .positions
            .last_mut()
            .expect("preview always has a starting position");
        match san.san.to_move(previous) {
            Ok(m) if game.moves.len() == MAX_HALF_MOVES => {
                // Continue checking legality to preserve backend error precedence,
                // reusing the last position because this preview will be rejected.
                previous.play_unchecked(m);
                self.over_limit = true;
            }
            Ok(m) => {
                let mut position = previous.clone();
                position.play_unchecked(m);
                game.fens.push(core::fen(&position));
                game.positions.push(position);
                game.moves.push(san.to_string());
            }
            Err(error) => {
                return ControlFlow::Break(Err(format!("pgn: Lance inválido: {error}")));
            }
        }
        ControlFlow::Continue(())
    }
    fn end_game(&mut self, game: Game) -> Self::Output {
        if game.moves.is_empty() {
            Err("pgn: O PGN não contém lances.".into())
        } else {
            Ok(game)
        }
    }
}
pub fn parse(pgn: &str) -> Result<Game, String> {
    if pgn.len() > 1_000_000 {
        return Err("O PGN deve ter no máximo 1 MB.".into());
    }
    pgn_tokens::validate(pgn).map_err(|error| format!("pgn: {error}"))?;
    let mut reader = Reader::new(Cursor::new(pgn.as_bytes()));
    let mut preview = Preview::default();
    let game = reader
        .read_game(&mut preview)
        .map_err(|error| format!("pgn: {error}"))?
        .ok_or("pgn: PGN inválido.")??;
    if reader.has_more().map_err(|error| format!("pgn: {error}"))? {
        return Err("pgn: Importe apenas uma partida por revisão.".into());
    }
    if preview.over_limit {
        return Err("A partida ultrapassa o limite de 2000 meios-lances.".into());
    }
    Ok(game)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_moves_and_castling() {
        let game = parse(DEMO_PGN).expect("demo PGN is legal");
        let p = game.board(16);
        assert_eq!((p[62], p[60], p[6], p[5]), ('K', 'R', 'k', 'r'));
        assert!(parse("1. e4 e5 2. Ke5 *").is_err());
        assert!(parse("1. e4 e5 2. Invalid").is_err());
        assert!(parse("1. e4 { comment } e5 (1... c5) 2. Nf3!? *").is_ok());
        assert!(parse(&format!("{DEMO_PGN}\n{DEMO_PGN}")).is_err());
    }
    #[test]
    fn respects_setup_fen() {
        let game = parse("[SetUp \"1\"]\n[FEN \"8/8/8/8/8/8/4K3/7k w - - 0 1\"]\n\n1. Kf3 *")
            .expect("legal FEN game");
        assert_eq!(game.board(1)[45], 'K');
    }

    #[test]
    fn preserves_preview_metadata_and_supplied_san() {
        let pgn = "[White \" Alice \"]\n[Black \"?\"]\n[Event \"?\"]\n[WhiteElo \"1400\"]\n[BlackElo \"?\"]\n\n1. e4+ $1 { comment } e5 (1... c5 (1... d5)) 2. Nf3!? *";
        let game = parse(pgn).expect("legal mainline with annotations");
        let extracted = core::extract(pgn).expect("backend accepts the same mainline");
        assert_eq!(
            (&*game.white, &*game.black, &*game.event),
            (" Alice ", "?", "?")
        );
        assert_eq!(game.ratings, [Some("1400".into()), Some("?".into())]);
        assert_eq!(game.moves, ["e4+", "e5", "Nf3"]);
        assert_eq!(game.positions, extracted.positions);
    }

    #[test]
    fn preserves_backend_validation_errors() {
        for pgn in [
            "",
            "{ only a comment }",
            "[Event \"Empty\"]\n\n*",
            "[FEN \"bad\"]\n\n1. e4 *",
            "1. e4 e5 2. Ke5 *",
            "1. e4 e5 2. Invalid *",
            "1. e4 *\n\n[Event \"Second\"]\n\n*",
        ] {
            let expected = core::extract(pgn).err().expect("backend rejects PGN");
            assert_eq!(
                parse(pgn).err().as_deref(),
                Some(expected.to_string().as_str())
            );
        }
    }

    #[test]
    fn enforces_byte_and_half_move_limits() {
        let at_byte_limit = format!("1. e4 {{{}}} *", "é".repeat(499_995));
        assert_eq!(at_byte_limit.len(), 1_000_000);
        assert!(parse(&at_byte_limit).is_ok());
        assert_eq!(
            parse(&(at_byte_limit + " ")).err().as_deref(),
            Some("O PGN deve ter no máximo 1 MB.")
        );

        let at_ply_limit = "Nf3 Nf6 Ng1 Ng8 ".repeat(500);
        let game = parse(&(at_ply_limit.clone() + "*")).expect("2000 half-moves are allowed");
        assert_eq!((game.moves.len(), game.positions.len()), (2000, 2001));
        assert_eq!(
            parse(&(at_ply_limit + "Nf3 *")).err().as_deref(),
            Some("A partida ultrapassa o limite de 2000 meios-lances.")
        );
    }
}
