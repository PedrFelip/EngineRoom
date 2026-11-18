//! Session-local reservations and coverage for unrestricted engine searches.

use super::{Mode, RawPosition, core};
use shakmaty::Position;
use std::collections::{HashMap, HashSet, VecDeque};

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

struct Evidence {
    demand: Demand,
    raw: RawPosition,
    reached_depth: u32,
    multipv: u32,
}

impl Evidence {
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

/// One automatic run owns the engine configuration and this index. Keys retain
/// every FEN field, including the move clocks, exactly as passed to UCI.
#[derive(Default)]
pub(super) struct Scheduler {
    active: HashSet<String>,
    evidence: HashMap<String, Vec<Evidence>>,
}

impl Scheduler {
    pub fn available(&self, fen: &str) -> bool {
        !self.active.contains(fen)
    }

    pub fn reserve(&mut self, fen: &str) {
        assert!(
            self.active.insert(fen.to_owned()),
            "duplicate search for {fen}"
        );
    }

    pub fn complete(&mut self, demand: Demand, raw: &RawPosition) {
        self.active.remove(&raw.fen);
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
        if multipv < demand.multipv
            && core::position(&raw.fen)
                .is_ok_and(|position| covered >= position.legal_moves().len())
        {
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
        self.evidence
            .entry(raw.fen.clone())
            .or_default()
            .push(Evidence {
                demand,
                raw,
                reached_depth,
                multipv,
            });
    }

    pub fn lookup(&self, fen: &str, demand: Demand) -> Option<RawPosition> {
        self.evidence
            .get(fen)?
            .iter()
            .find_map(|evidence| evidence.covering(demand))
    }

    pub fn take_triage(
        &self,
        worker: usize,
        queue: &mut VecDeque<usize>,
        blocks: &mut [VecDeque<usize>],
        fens: &[String],
    ) -> Option<usize> {
        let available = |index: &usize| self.available(&fens[*index]);
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
