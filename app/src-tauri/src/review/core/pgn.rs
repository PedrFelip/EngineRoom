//! Mainline PGN validation and metadata extraction.
use super::{fen, invalid, position};
use crate::review::types::*;
use pgn_reader::{Outcome, RawTag, Reader, SanPlus, Visitor};
use shakmaty::{san, CastlingMode, Chess, Position};
use std::{collections::HashMap, io::Cursor, ops::ControlFlow};

pub struct Game {
    pub fens: Vec<String>,
    pub moves: Vec<PlayedMove>,
    pub meta: PgnMeta,
}
struct Reading {
    pos: Chess,
    game: Game,
}
struct PgnVisitor;
impl Visitor for PgnVisitor {
    type Tags = HashMap<String, String>;
    type Movetext = Reading;
    type Output = Result<Game>;
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
    fn begin_movetext(&mut self, tags: Self::Tags) -> ControlFlow<Self::Output, Reading> {
        let pos = if let Some(start) = tags.get("FEN") {
            match position(start) {
                Ok(p) => p,
                Err(e) => return ControlFlow::Break(Err(e)),
            }
        } else {
            Chess::default()
        };
        let player = |key: &str| {
            tags.get(key)
                .filter(|s| !s.is_empty() && *s != "?")
                .map(|s| s.trim().to_owned())
                .unwrap_or_else(|| "Jogador".into())
        };
        let meta = PgnMeta {
            white: player("White"),
            black: player("Black"),
            white_elo: tags.get("WhiteElo").cloned(),
            black_elo: tags.get("BlackElo").cloned(),
            result: tags
                .get("Result")
                .filter(|s| *s != "?")
                .cloned()
                .unwrap_or_else(|| "*".into()),
            event: tags.get("Event").filter(|s| *s != "?").cloned(),
            plies: 0,
        };
        let start = fen(&pos);
        ControlFlow::Continue(Reading {
            pos,
            game: Game {
                fens: vec![start],
                moves: vec![],
                meta,
            },
        })
    }
    fn san(&mut self, state: &mut Reading, supplied: SanPlus) -> ControlFlow<Self::Output> {
        let m = match supplied.san.to_move(&state.pos) {
            Ok(m) => m,
            Err(e) => return ControlFlow::Break(Err(invalid(format!("Lance inválido: {e}")))),
        };
        let color = if state.pos.turn().is_white() {
            "w"
        } else {
            "b"
        }
        .into();
        let before = fen(&state.pos);
        let uci = m.to_uci(CastlingMode::Standard).to_string();
        let san = san::SanPlus::from_move_and_play_unchecked(&mut state.pos, m).to_string();
        state.game.moves.push(PlayedMove {
            ply: state.game.moves.len() + 1,
            color,
            san,
            uci,
            fen_before: before,
        });
        state.game.fens.push(fen(&state.pos));
        ControlFlow::Continue(())
    }
    fn outcome(&mut self, state: &mut Reading, outcome: Outcome) -> ControlFlow<Self::Output> {
        state.game.meta.result = outcome.to_string();
        ControlFlow::Continue(())
    }
    fn end_game(&mut self, mut state: Reading) -> Self::Output {
        if state.game.moves.is_empty() {
            return Err(invalid("O PGN não contém lances."));
        }
        state.game.meta.plies = state.game.moves.len();
        Ok(state.game)
    }
}
pub fn extract(pgn: &str) -> Result<Game> {
    let mut reader = Reader::new(Cursor::new(pgn.as_bytes()));
    let game = reader
        .read_game(&mut PgnVisitor)
        .map_err(invalid)?
        .ok_or_else(|| invalid("PGN inválido."))??;
    if reader
        .read_game(&mut PgnVisitor)
        .map_err(invalid)?
        .is_some()
    {
        return Err(invalid("Importe apenas uma partida por revisão."));
    }
    Ok(game)
}
