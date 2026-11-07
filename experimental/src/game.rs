//! Local PGN preview only; analysis and persistence remain the Tauri backend's responsibility.
use pgn_reader::{RawTag, Reader, SanPlus, Visitor};
use shakmaty::{CastlingMode, Chess, Position, Square, fen::Fen};
use std::{collections::HashMap, io::Cursor, ops::ControlFlow};

pub const DEMO_PGN: &str = "[Event \"Estudo de abertura\"]\n[White \"Jogador de brancas\"]\n[Black \"Jogador de pretas\"]\n[Result \"*\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 6. Re1 b5 7. Bb3 d6 8. c3 O-O *";
#[derive(Clone)]
pub struct Game {
    pub white: String,
    pub black: String,
    pub event: String,
    pub positions: Vec<Chess>,
    pub moves: Vec<String>,
}
impl Game {
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
struct Preview;
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
    fn begin_movetext(&mut self, tags: Self::Tags) -> ControlFlow<Self::Output, Game> {
        let position = match tags.get("FEN") {
            Some(fen) => match fen.parse::<Fen>().and_then(|fen| {
                fen.into_position::<Chess>(CastlingMode::Standard)
                    .map_err(|_| shakmaty::fen::ParseFenError::InvalidBoard)
            }) {
                Ok(position) => position,
                Err(_) => return ControlFlow::Break(Err("A posição FEN é inválida.".into())),
            },
            None => Chess::default(),
        };
        ControlFlow::Continue(Game {
            white: tags
                .get("White")
                .cloned()
                .unwrap_or_else(|| "Brancas".into()),
            black: tags
                .get("Black")
                .cloned()
                .unwrap_or_else(|| "Pretas".into()),
            event: tags
                .get("Event")
                .cloned()
                .unwrap_or_else(|| "Partida importada".into()),
            positions: vec![position],
            moves: Vec::new(),
        })
    }
    fn san(&mut self, game: &mut Game, san: SanPlus) -> ControlFlow<Self::Output> {
        let mut position = game
            .positions
            .last()
            .expect("preview always has a starting position")
            .clone();
        match san.san.to_move(&position) {
            Ok(m) => {
                position.play_unchecked(m);
                game.positions.push(position);
                game.moves.push(san.to_string());
            }
            Err(_) => return ControlFlow::Break(Err(format!("Lance inválido: {san}."))),
        }
        if game.moves.len() > 2000 {
            return ControlFlow::Break(Err(
                "A partida ultrapassa o limite de 2000 meios-lances.".into()
            ));
        }
        ControlFlow::Continue(())
    }
    fn end_game(&mut self, game: Game) -> Self::Output {
        if game.moves.is_empty() {
            Err("O PGN não contém lances.".into())
        } else {
            Ok(game)
        }
    }
}
pub fn parse(pgn: &str) -> Result<Game, String> {
    if pgn.len() > 1_000_000 {
        return Err("O PGN deve ter no máximo 1 MB.".into());
    }
    let mut reader = Reader::new(Cursor::new(pgn.as_bytes()));
    let game = reader
        .read_game(&mut Preview)
        .map_err(|e| e.to_string())?
        .ok_or("Cole um PGN para continuar.")??;
    if reader
        .read_game(&mut Preview)
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Err("Importe uma partida por vez.".into());
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
        assert!(parse(&format!("{DEMO_PGN}\n{DEMO_PGN}")).is_err());
    }
    #[test]
    fn respects_setup_fen() {
        let game = parse("[SetUp \"1\"]\n[FEN \"8/8/8/8/8/8/4K3/7k w - - 0 1\"]\n\n1. Kf3 *")
            .expect("legal FEN game");
        assert_eq!(game.board(1)[45], 'K');
    }
}
