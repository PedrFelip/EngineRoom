//! Decode cached evaluations while checking candidate coverage and principal scores.
use crate::{
    db::cache::CachedPosition,
    review::{
        core,
        types::{RawLine, RawPosition},
    },
};
use shakmaty::Position;

pub fn shape(hit: CachedPosition, fen: &str, multipv: u32) -> Option<RawPosition> {
    let mut lines: Vec<RawLine> = serde_json::from_str(&hit.lines_json).ok()?;
    let required = multipv.min(core::position(fen).ok()?.legal_moves().len() as u32);
    lines.sort_by_key(|line| line.multipv);
    if required == 0
        || lines.len() < required as usize
        || lines
            .iter()
            .enumerate()
            .any(|(i, line)| line.multipv != i as u32 + 1)
    {
        return None;
    }
    lines.truncate(required as usize);
    let principal = lines.first()?;
    if principal.cp != hit.cp {
        return None;
    }
    Some(RawPosition {
        fen: fen.into(),
        cp: hit.cp,
        depth: principal.depth.unwrap_or(hit.reached_depth),
        pv: principal.pv.clone(),
        lines,
    })
}
