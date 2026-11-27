//! Session-local reservations and coverage for unrestricted engine searches.

use super::{Game, Mode, RawPosition};
use shakmaty::Position;
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque, hash_map::Entry},
};

#[derive(Clone, Copy, Debug)]
pub(super) struct Demand {
    pub mode: Mode,
    pub value: u32,
    pub multipv: u32,
}

impl Demand {
    pub fn time(value: u32, multipv: u32) -> Self {
        Self {
            mode: Mode::Time,
            value,
            multipv,
        }
    }
}

struct EvidenceNode {
    next: u32,
    demand: Demand,
    raw: RawPosition,
    reached_depth: u32,
    multipv: u32,
}

impl EvidenceNode {
    fn covering(&self, demand: Demand, legal_moves: u8) -> Option<RawPosition> {
        let required_multipv = demand.multipv.min(u32::from(legal_moves).max(1));
        if demand.multipv == 0 || required_multipv > self.multipv {
            return None;
        }
        let covers_budget = match demand.mode {
            Mode::Depth => self.reached_depth >= demand.value,
            Mode::Time => self.demand.mode == Mode::Time && self.demand.value >= demand.value,
        };
        if !covers_budget {
            return None;
        }
        let mut raw = self.raw.clone();
        raw.lines.truncate(demand.multipv as usize);
        Some(raw)
    }
}

pub(in crate::review) fn fen_key(fen: &str) -> &str {
    fen.rsplit_once(' ').map_or(fen, |(prefix, _)| prefix)
}

pub(super) fn covered_multipv(
    raw: &RawPosition,
    requested_multipv: u32,
    legal_moves: usize,
) -> u32 {
    let covered = if raw.lines.is_sorted_by_key(|line| line.multipv) {
        raw.lines
            .iter()
            .enumerate()
            .take_while(|(i, line)| line.multipv as usize == i + 1)
            .count()
    } else {
        let mut ranks: Vec<u32> = raw.lines.iter().map(|line| line.multipv).collect();
        ranks.sort_unstable();
        ranks
            .into_iter()
            .enumerate()
            .take_while(|(i, multipv)| *multipv as usize == i + 1)
            .count()
    }
    .min(requested_multipv as usize);
    if covered == 0 {
        return 0;
    }
    let mut multipv = covered as u32;
    if multipv < requested_multipv && covered >= legal_moves {
        multipv = requested_multipv;
    }
    multipv
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub(super) struct FenId(u32);

impl FenId {
    fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Default)]
pub(super) struct BitSet(Vec<u64>);

impl BitSet {
    pub fn contains(&self, index: usize) -> bool {
        self.0
            .get(index / 64)
            .is_some_and(|word| word & (1 << (index % 64)) != 0)
    }

    pub fn insert(&mut self, index: usize) {
        let word = index / 64;
        if word >= self.0.len() {
            self.0.resize(word + 1, 0);
        }
        self.0[word] |= 1 << (index % 64);
    }

    pub fn remove(&mut self, index: usize) {
        self.0[index / 64] &= !(1 << (index % 64));
    }
}

// Links are one-based so zero can represent the end of either chain.
fn link(index: usize) -> u32 {
    u32::try_from(index + 1).expect("scheduler index exceeds u32")
}

struct FenIndex<'game> {
    // Canonical five-field FEN prefix, retaining the halfmove clock and omitting
    // the fullmove counter. Mainline keys borrow the game.
    ids: HashMap<Cow<'game, str>, FenId>,
    ply_ids: Vec<FenId>,
    ply_head: Vec<u32>,
    ply_next: Vec<u32>,
    legal_moves: Vec<u8>,
    active: BitSet,
    evidence_head: Vec<u32>,
    evidence_tail: Vec<u32>,
    evidence: Vec<EvidenceNode>,
}

impl<'game> FenIndex<'game> {
    fn intern(&mut self, fen: Cow<'game, str>, legal_moves: impl FnOnce() -> usize) -> FenId {
        let slot = match self.ids.entry(fen) {
            Entry::Occupied(slot) => return *slot.get(),
            Entry::Vacant(slot) => slot,
        };
        let id = FenId(u32::try_from(self.legal_moves.len()).expect("FEN index exceeds u32"));
        let count = u8::try_from(legal_moves()).expect("legal move count exceeds u8");
        slot.insert(id);
        self.legal_moves.push(count);
        self.ply_head.push(0);
        self.evidence_head.push(0);
        self.evidence_tail.push(0);
        id
    }

    fn mainline(game: &'game Game, contextual: bool) -> Self {
        let count = game.fens.len();
        let mut index = Self {
            ids: HashMap::with_capacity(count),
            ply_ids: Vec::with_capacity(count),
            ply_head: Vec::with_capacity(count),
            ply_next: vec![0; count],
            legal_moves: Vec::with_capacity(count),
            active: BitSet(vec![0; count.div_ceil(64)]),
            evidence_head: Vec::with_capacity(count),
            evidence_tail: Vec::with_capacity(count),
            evidence: Vec::new(),
        };
        for (ply, (fen, position)) in game.fens.iter().zip(&game.positions).enumerate() {
            let key = if contextual {
                Cow::Owned(format!("mainline:{ply}"))
            } else {
                Cow::Borrowed(fen_key(fen))
            };
            let id = index.intern(key, || position.legal_moves().len());
            index.ply_ids.push(id);
        }
        for (ply, id) in index.ply_ids.iter().enumerate().rev() {
            index.ply_next[ply] = std::mem::replace(&mut index.ply_head[id.index()], link(ply));
        }
        index
    }
}

pub(super) struct Scheduler<'game> {
    contextual: bool,
    game: &'game Game,
    index: Option<FenIndex<'game>>,
}

impl<'game> Scheduler<'game> {
    pub fn new(game: &'game Game) -> Self {
        Self {
            game,
            index: None,
            contextual: false,
        }
    }

    pub fn enable_context(&mut self) {
        self.contextual = true;
    }

    fn mainline_index(&mut self) -> &mut FenIndex<'game> {
        self.index
            .get_or_insert_with(|| FenIndex::mainline(self.game, self.contextual))
    }

    pub fn prepare(&mut self) {
        self.mainline_index();
    }

    pub fn intern_ply(&mut self, ply: usize) -> FenId {
        self.mainline_index().ply_ids[ply]
    }

    pub fn intern_live(&mut self, fen: &str, legal_moves: impl FnOnce() -> usize) -> FenId {
        let key = fen_key(fen);
        let index = self.mainline_index();
        index
            .ids
            .get(key)
            .copied()
            .unwrap_or_else(|| index.intern(Cow::Owned(key.to_owned()), legal_moves))
    }

    pub fn ply_id(&self, ply: usize) -> Option<FenId> {
        Some(self.index.as_ref()?.ply_ids[ply])
    }

    pub fn plies(&self, id: FenId) -> impl Iterator<Item = usize> + '_ {
        let index = self.index.as_ref().expect("interned FEN");
        let mut link = index.ply_head[id.index()];
        std::iter::from_fn(move || {
            let ply = link.checked_sub(1)? as usize;
            link = index.ply_next[ply];
            Some(ply)
        })
    }

    pub fn available(&self, id: FenId) -> bool {
        !self
            .index
            .as_ref()
            .expect("interned FEN")
            .active
            .contains(id.index())
    }

    pub fn reserve(&mut self, id: FenId) {
        assert!(self.available(id), "duplicate search for {id:?}");
        self.index
            .as_mut()
            .expect("interned FEN")
            .active
            .insert(id.index());
    }

    pub fn complete(&mut self, id: FenId, demand: Demand, raw: &RawPosition) -> u32 {
        self.index
            .as_mut()
            .expect("interned FEN")
            .active
            .remove(id.index());
        self.record(id, demand, raw)
    }

    pub fn record(&mut self, id: FenId, demand: Demand, raw: &RawPosition) -> u32 {
        let index = self.index.as_mut().expect("interned FEN");
        let slot = id.index();
        let mut raw = raw.clone();
        raw.lines.sort_by_key(|line| line.multipv);
        let covered = raw
            .lines
            .iter()
            .enumerate()
            .take_while(|(i, line)| line.multipv as usize == i + 1)
            .count()
            .min(demand.multipv as usize);
        raw.lines.truncate(covered);
        let Some(reached_depth) = raw
            .lines
            .iter()
            .map(|line| line.depth.unwrap_or(raw.depth))
            .min()
        else {
            return 0;
        };
        let mut multipv = covered as u32;
        if multipv < demand.multipv && covered >= usize::from(index.legal_moves[slot]) {
            multipv = demand.multipv;
        }
        let new_link = link(index.evidence.len());
        index.evidence.push(EvidenceNode {
            next: 0,
            demand,
            raw,
            reached_depth,
            multipv,
        });
        let tail = std::mem::replace(&mut index.evidence_tail[slot], new_link);
        if let Some(previous) = tail.checked_sub(1) {
            index.evidence[previous as usize].next = new_link;
        } else {
            index.evidence_head[slot] = new_link;
        }
        multipv
    }

    pub fn lookup(&self, id: FenId, demand: Demand) -> Option<RawPosition> {
        let index = self.index.as_ref().expect("interned FEN");
        let legal_moves = index.legal_moves[id.index()];
        let mut link = index.evidence_head[id.index()];
        while let Some(slot) = link.checked_sub(1) {
            let evidence = &index.evidence[slot as usize];
            if let Some(raw) = evidence.covering(demand, legal_moves) {
                return Some(raw);
            }
            link = evidence.next;
        }
        None
    }

    /// Takes the oldest queued position without a search in flight.
    ///
    /// Blocked entries stay queued and never prevent a worker from taking the
    /// next ready position.
    pub fn take_triage(&self, queue: &mut VecDeque<usize>) -> Option<usize> {
        let available = |ply: &usize| self.ply_id(*ply).is_none_or(|id| self.available(id));
        let slot = queue.iter().position(available)?;
        queue.remove(slot)
    }
}

#[cfg(test)]
mod tests;
