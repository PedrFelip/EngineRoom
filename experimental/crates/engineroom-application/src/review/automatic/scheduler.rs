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
    fn covering(&self, demand: Demand) -> Option<RawPosition> {
        if demand.multipv == 0 || demand.multipv > self.multipv {
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
    // Exact UCI FEN, including the move clocks. Mainline keys borrow the game.
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

    fn mainline(game: &'game Game) -> Self {
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
        for (fen, position) in game.fens.iter().zip(&game.positions) {
            let id = index.intern(Cow::Borrowed(fen), || position.legal_moves().len());
            index.ply_ids.push(id);
        }
        for (ply, id) in index.ply_ids.iter().enumerate().rev() {
            index.ply_next[ply] = std::mem::replace(&mut index.ply_head[id.index()], link(ply));
        }
        index
    }
}

pub(super) struct Scheduler<'game> {
    game: &'game Game,
    index: Option<FenIndex<'game>>,
}

impl<'game> Scheduler<'game> {
    pub fn new(game: &'game Game) -> Self {
        Self { game, index: None }
    }

    fn mainline_index(&mut self) -> &mut FenIndex<'game> {
        self.index
            .get_or_insert_with(|| FenIndex::mainline(self.game))
    }

    pub fn prepare(&mut self) {
        self.mainline_index();
    }

    pub fn intern_ply(&mut self, ply: usize) -> FenId {
        self.mainline_index().ply_ids[ply]
    }

    pub fn intern_live(&mut self, fen: &str, legal_moves: impl FnOnce() -> usize) -> FenId {
        let index = self.mainline_index();
        index
            .ids
            .get(fen)
            .copied()
            .unwrap_or_else(|| index.intern(Cow::Owned(fen.to_owned()), legal_moves))
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

    pub fn complete(&mut self, id: FenId, demand: Demand, raw: &RawPosition) {
        let index = self.index.as_mut().expect("interned FEN");
        let slot = id.index();
        index.active.remove(slot);
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
        let mut multipv = covered as u32;
        if multipv < demand.multipv && covered >= usize::from(index.legal_moves[slot]) {
            multipv = demand.multipv;
        }
        let Some(reached_depth) = raw
            .lines
            .iter()
            .map(|line| line.depth.unwrap_or(raw.depth))
            .min()
        else {
            return;
        };
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
    }

    pub fn lookup(&self, id: FenId, demand: Demand) -> Option<RawPosition> {
        let index = self.index.as_ref().expect("interned FEN");
        let mut link = index.evidence_head[id.index()];
        while let Some(slot) = link.checked_sub(1) {
            let evidence = &index.evidence[slot as usize];
            if let Some(raw) = evidence.covering(demand) {
                return Some(raw);
            }
            link = evidence.next;
        }
        None
    }

    pub fn take_triage(
        &self,
        worker: usize,
        queue: &mut VecDeque<usize>,
        blocks: &mut [VecDeque<usize>],
    ) -> Option<usize> {
        let available = |ply: &usize| self.ply_id(*ply).is_none_or(|id| self.available(id));
        if let Some(slot) = blocks[worker].iter().position(available) {
            return blocks[worker].remove(slot);
        }
        if let Some(slot) = queue.iter().position(available) {
            let index = queue.remove(slot)?;
            // Keep the usual consecutive batch limit. Blocked entries stay
            // pending and never prevent a worker from claiming another run.
            for next in (index + 1..index + 4).take(if blocks[worker].is_empty() { 3 } else { 0 }) {
                if queue.get(slot) != Some(&next) || !available(&next) {
                    break;
                }
                blocks[worker].extend(queue.remove(slot));
            }
            return Some(index);
        }
        for block in blocks.iter_mut() {
            if let Some(slot) = block.iter().position(available) {
                return block.remove(slot);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
