//! Opening lookup in the bundled ECO dataset.
use crate::review::types::{Eco, PlayedMove};
use std::sync::OnceLock;

#[derive(Clone, serde::Deserialize)]
struct Entry {
    code: String,
    name: String,
    moves: Vec<String>,
}
pub fn opening(moves: &[PlayedMove]) -> Option<(usize, Eco)> {
    // ECO sequences assume the standard starting position, even when supplied
    // through an explicit PGN FEN tag.
    if moves.first()?.fen_before != super::fen(&shakmaty::Chess::default()) {
        return None;
    }
    static DATA: OnceLock<Vec<Entry>> = OnceLock::new();
    let data = DATA.get_or_init(|| {
        let entries: Vec<Entry> =
            serde_json::from_str(include_str!("../../../../src/data/eco.json"))
                .expect("bundled ECO dataset must contain valid opening JSON");
        assert!(
            !entries.is_empty()
                && entries.iter().all(|entry| !entry.code.is_empty()
                    && !entry.name.is_empty()
                    && !entry.moves.is_empty()),
            "bundled ECO dataset must contain complete opening entries"
        );
        entries
    });
    let mut best: Option<&Entry> = None;
    for entry in data {
        if entry.moves.is_empty() || entry.moves.len() > moves.len() {
            continue;
        }
        if entry.moves.iter().zip(moves).all(|(s, m)| *s == m.san)
            && best.is_none_or(|b| entry.moves.len() > b.moves.len())
        {
            best = Some(entry);
        }
    }
    best.map(|b| {
        (
            b.moves.len(),
            Eco {
                code: b.code.clone(),
                name: b.name.clone(),
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_dataset_recognizes_common_opening() {
        let game = crate::review::core::extract("1. e4 e5 2. Nf3 Nc6 3. Bb5").unwrap();
        let (ply, eco) =
            opening(&game.moves).expect("Ruy Lopez must be in the bundled ECO dataset");
        assert!(ply >= 5);
        assert!(eco.code.starts_with('C'));
        assert!(!eco.name.is_empty());
    }
}
