//! Turns a finished review into a new PGN with annotations for mistakes and blunders.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    io::{self, Write as _},
    ops::ControlFlow,
    path::Path,
};

use engineroom_backend::review::{
    core,
    types::{Classification, PlayedMove, PositionAnalysis, ReviewResult},
};
use pgn_reader::{RawTag, Reader, Visitor};
use shakmaty::{Chess, Position, san::SanPlus, uci::UciMove};

struct Tags;

impl Visitor for Tags {
    type Tags = BTreeMap<String, String>;
    type Movetext = ();
    type Output = Self::Tags;

    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(BTreeMap::new())
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

    fn begin_movetext(&mut self, tags: Self::Tags) -> ControlFlow<Self::Output, ()> {
        ControlFlow::Break(tags)
    }

    fn end_game(&mut self, (): ()) -> Self::Output {
        BTreeMap::new()
    }
}

pub fn annotated_pgn(source: &str, review: &ReviewResult) -> Result<String, String> {
    let game = core::extract(source).map_err(|error| error.to_string())?;
    if review.moves.len() != game.moves.len()
        || review
            .moves
            .iter()
            .zip(&game.moves)
            .any(|(reviewed, played)| {
                reviewed.played.ply != played.ply
                    || reviewed.played.uci != played.uci
                    || reviewed.played.fen_before != played.fen_before
            })
    {
        return Err("A análise não corresponde à partida.".into());
    }

    let positions: BTreeMap<_, _> = review.positions.iter().map(|p| (p.ply, p)).collect();
    let mut tags = Reader::new(source.as_bytes())
        .read_game(&mut Tags)
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let result = match game.meta.result.as_str() {
        "1-0" | "0-1" | "1/2-1/2" => game.meta.result.as_str(),
        _ => "*",
    };
    tags.insert("Result".into(), result.into());
    if tags.contains_key("FEN") {
        tags.insert("SetUp".into(), "1".into());
    }
    let mut pgn = String::new();
    for name in ["Event", "Site", "Date", "Round", "White", "Black", "Result"] {
        let value = tags.remove(name).unwrap_or_else(|| "?".into());
        write_tag(&mut pgn, name, &value);
    }
    for (name, value) in tags {
        write_tag(&mut pgn, &name, &value);
    }
    pgn.push('\n');
    for ((played, before), reviewed) in game.moves.iter().zip(&game.positions).zip(&review.moves) {
        pgn.push_str(&move_number(before));
        pgn.push_str(&played.san);
        if let Some((nag, comment)) = annotation(reviewed.classification) {
            write!(pgn, " ${nag} {{{comment}}}").expect("writing to String is infallible");
            if let Some(variation) = positions
                .get(&(played.ply - 1))
                .and_then(|candidate| alternative(played, before, candidate))
            {
                write!(pgn, " ({variation})").expect("writing to String is infallible");
            }
        }
        pgn.push('\n');
    }
    pgn.push_str(result);
    pgn.push('\n');
    Ok(pgn)
}

pub fn save_pgn(path: &Path, pgn: &str) -> io::Result<()> {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(pgn.as_bytes())?;
    file.persist(path)?;
    Ok(())
}

fn annotation(classification: Classification) -> Option<(u8, &'static str)> {
    match classification {
        Classification::Erro => Some((2, "Erro.")),
        Classification::Blunder => Some((4, "Erro grave.")),
        _ => None,
    }
}

fn alternative(
    played: &PlayedMove,
    before: &Chess,
    candidate: &PositionAnalysis,
) -> Option<String> {
    if candidate.fen != played.fen_before {
        return None;
    }
    let played_move = played.uci.parse::<UciMove>().ok()?.to_move(before).ok()?;
    let mut position = before.clone();
    let mut variation = String::new();
    for (index, uci) in candidate.pv.iter().take(4).enumerate() {
        let m = uci.parse::<UciMove>().ok()?.to_move(&position).ok()?;
        if index == 0 && m == played_move {
            return None;
        }
        if index != 0 {
            variation.push(' ');
        }
        variation.push_str(&move_number(&position));
        variation.push_str(&SanPlus::from_move_and_play_unchecked(&mut position, m).to_string());
    }
    (!variation.is_empty()).then_some(variation)
}

fn move_number(position: &impl Position) -> String {
    let dots = if position.turn().is_white() {
        "."
    } else {
        "..."
    };
    format!("{}{dots} ", position.fullmoves())
}

fn write_tag(pgn: &mut String, name: &str, value: &str) {
    let value = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], " ");
    writeln!(pgn, "[{name} \"{value}\"]").expect("writing to String is infallible");
}

#[cfg(test)]
mod tests {
    use engineroom_backend::review::types::RawPosition;

    use super::*;

    #[test]
    fn exports_black_to_move_from_fen_with_a_legal_alternative() {
        let source = concat!(
            "[Event \"Training\"]\n[WhiteElo \"1800\"]\n[SetUp \"1\"]\n",
            "[FEN \"rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 7\"]\n\n",
            "7... d5 8. exd5 *"
        );
        let game = core::extract(source).unwrap();
        let positions: Vec<_> = game
            .fens
            .iter()
            .enumerate()
            .map(|(index, fen)| RawPosition {
                fen: fen.clone(),
                depth: 12,
                cp: 0,
                pv: if index == 0 {
                    vec!["e7e5".into(), "g1f3".into()]
                } else {
                    Vec::new()
                },
                lines: Vec::new(),
            })
            .collect();
        let mut review = core::build(&game, &positions).unwrap();
        review.moves[0].classification = Classification::Blunder;
        review.moves[1].classification = Classification::Bom;

        let exported = annotated_pgn(source, &review).unwrap();
        assert!(exported.contains("[WhiteElo \"1800\"]"));
        assert!(exported.contains("7... d5 $4 {Erro grave.} (7... e5 8. Nf3)"));
        assert_eq!(core::extract(&exported).unwrap().fens, game.fens);
        assert!(annotated_pgn("1. d4 d5 *", &review).is_err());

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("review.pgn");
        std::fs::write(&path, "old export").unwrap();
        save_pgn(&path, &exported).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), exported);
    }
}
