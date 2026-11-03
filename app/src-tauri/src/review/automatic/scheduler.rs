//! Indexed, pure scheduling decisions for the currently selected targets.
use super::{SearchBudget, Target};

/// Per-position scheduler transitions, independent of engine and repository I/O.
pub(super) struct Positions {
    refining: Vec<bool>,
    cache_checked: Vec<Option<SearchBudget>>,
    active: usize,
}

impl Positions {
    pub fn new(count: usize) -> Self {
        Self {
            refining: vec![false; count],
            cache_checked: vec![None; count],
            active: 0,
        }
    }

    pub fn plan(&self, targets: &[Target], budgets: &[Option<SearchBudget>]) -> Plan {
        Plan::new(targets, budgets, &self.refining, &self.cache_checked)
    }

    pub fn checked(&mut self, index: usize, budget: SearchBudget) {
        self.cache_checked[index] = Some(budget);
    }

    pub fn start(&mut self, index: usize) {
        debug_assert!(!self.refining[index]);
        self.refining[index] = true;
        self.active += 1;
    }

    pub fn finish(&mut self, index: usize) {
        debug_assert!(self.refining[index]);
        self.refining[index] = false;
        self.active -= 1;
    }

    pub fn active(&self) -> bool {
        self.active != 0
    }
    pub fn is_refining(&self, index: usize) -> bool {
        self.refining[index]
    }
}

pub(super) struct Plan {
    pub groups: Vec<(SearchBudget, Vec<usize>)>,
    pub candidates: Vec<(usize, bool, SearchBudget)>,
}

impl Plan {
    pub fn new(
        targets: &[Target],
        budgets: &[Option<SearchBudget>],
        refining: &[bool],
        cache_checked: &[Option<SearchBudget>],
    ) -> Self {
        let mut plan = Self {
            groups: Vec::new(),
            candidates: Vec::new(),
        };
        for target in targets {
            let i = target.position_index;
            if budgets[i]
                .is_some_and(|b| b.ms >= target.search.ms && b.multipv >= target.search.multipv)
                || refining[i]
            {
                continue;
            }
            plan.candidates.push((i, target.hard, target.search));
            if cache_checked[i] != Some(target.search) {
                let slot = plan
                    .groups
                    .iter()
                    .position(|(budget, _)| *budget == target.search)
                    .unwrap_or_else(|| {
                        plan.groups.push((target.search, Vec::new()));
                        plan.groups.len() - 1
                    });
                plan.groups[slot].1.push(i);
            }
        }
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::adaptive::RefinementKind;

    fn target(index: usize, hard: bool, budget: SearchBudget) -> Target {
        Target {
            position_index: index,
            score: 40,
            hard,
            kind: RefinementKind::Critical,
            search: budget,
        }
    }

    #[test]
    fn transitions_recheck_raised_budgets_and_preserve_candidate_order() {
        let low = SearchBudget {
            ms: 100,
            multipv: 2,
        };
        let high = SearchBudget {
            ms: 200,
            multipv: 2,
        };
        let mut positions = Positions::new(4);
        let budgets = vec![None; 4];
        let targets = vec![
            target(3, true, low),
            target(1, false, low),
            target(2, true, high),
        ];
        let plan = positions.plan(&targets, &budgets);
        assert_eq!(
            plan.candidates,
            vec![(3, true, low), (1, false, low), (2, true, high)]
        );
        assert_eq!(plan.groups, vec![(low, vec![3, 1]), (high, vec![2])]);
        positions.checked(3, low);
        positions.start(1);
        assert!(positions.active());
        let plan = positions.plan(&targets, &budgets);
        assert_eq!(plan.groups, vec![(high, vec![2])]);
        positions.finish(1);
        assert!(!positions.active());
        let raised = vec![target(3, true, high)];
        assert_eq!(
            positions.plan(&raised, &budgets).groups,
            vec![(high, vec![3])]
        );
        let mut covered = budgets;
        covered[3] = Some(high);
        assert!(positions.plan(&raised, &covered).candidates.is_empty());
    }
}
