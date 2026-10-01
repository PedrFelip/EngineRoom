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
    static DATA: OnceLock<Vec<Entry>> = OnceLock::new();
    let data = DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../src/data/eco.json")).unwrap_or_default()
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
