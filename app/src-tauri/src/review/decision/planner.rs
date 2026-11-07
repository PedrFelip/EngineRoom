//! Deterministic experimental planner with bounded concurrent reservations.
use super::policy::{conflict, critical, Value};
use super::{
    decisions, position_status, satisfies, Context, Decision, EngineSignature, Evidence,
    PositionStatus, Request,
};
use crate::review::types::{Result, ReviewError, ReviewErrorCode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub generation: u64,
    pub engine: EngineSignature,
    pub screening_depth: u32,
    pub multipv: usize,
    pub verification_depth: u32,
    pub critical_cp: i32,
    pub max_attempts: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dispatch {
    pub generation: u64,
    pub execution_id: u64,
    pub request: Request,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub execution_id: u64,
    #[serde(default)]
    pub cause: String,
    pub authorized_depth: u32,
    pub reserved: bool,
    pub started_ms: u64,
    pub wall_ms: Option<u64>,
    pub cpu_ms: Option<u64>,
    pub memory_bytes: Option<u64>,
    pub outcome: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conclusion {
    pub ply: usize,
    pub version: u32,
    pub best: Value,
    pub played: Value,
    pub best_evidence: Option<u64>,
    pub played_evidence: Option<u64>,
    pub critical: bool,
    #[serde(default)]
    pub conflict: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pending {
    pub ply: usize,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub config: Config,
    pub decisions: Vec<Decision>,
    pub evidence: Vec<Evidence>,
    pub screening: Vec<Conclusion>,
    pub revisions: Vec<Conclusion>,
    pub unresolved: Vec<Pending>,
    pub ledger: Vec<LedgerEntry>,
    pub complete: bool,
    pub verified_plies: Vec<usize>,
    #[serde(default)]
    pub metrics: Option<super::runner::RunMetrics>,
}
#[derive(Clone)]
struct Slot {
    context: Context,
    terminal: Option<PositionStatus>,
    screening: Option<usize>,
    latest: Option<usize>,
    failed: bool,
}
#[derive(Clone)]
struct Active {
    dispatch: Dispatch,
    slot: usize,
    verification: bool,
}
pub struct Planner {
    config: Config,
    decisions: Vec<Decision>,
    slots: Vec<Slot>,
    evidence: Vec<Evidence>,
    screening: Vec<Conclusion>,
    revisions: Vec<Conclusion>,
    unresolved: Vec<Pending>,
    ledger: Vec<LedgerEntry>,
    active: Vec<Active>,
    next_id: u64,
    verify_index: usize,
    attempts: Vec<u32>,
    verification: [Option<usize>; 2],
    complete: bool,
    verified_plies: Vec<usize>,
    metrics: Option<super::runner::RunMetrics>,
}
fn forced(s: &PositionStatus) -> bool {
    !matches!(
        s,
        PositionStatus::Ongoing | PositionStatus::Claimable { .. }
    )
}
impl Planner {
    pub fn new(pgn: &str, config: Config) -> Result<Self> {
        if config.screening_depth == 0
            || config.multipv == 0
            || config.verification_depth <= config.screening_depth
            || config.critical_cp <= 0
            || config.max_attempts == 0
        {
            return Err(ReviewError::new(
                ReviewErrorCode::InvalidPgn,
                "decision.config",
                "Configuração experimental inválida",
            ));
        }
        let decisions = decisions(pgn)?;
        let mut contexts: Vec<Context> = decisions.iter().map(|d| d.before.clone()).collect();
        if let Some(last) = decisions.last() {
            contexts.push(last.after.clone());
        }
        let slots = contexts
            .into_iter()
            .map(|context| {
                let status = position_status(&context)?;
                Ok(Slot {
                    context,
                    terminal: forced(&status).then_some(status),
                    screening: None,
                    latest: None,
                    failed: false,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            config,
            decisions: decisions.clone(),
            slots,
            evidence: vec![],
            screening: vec![],
            revisions: vec![],
            unresolved: vec![],
            ledger: vec![],
            active: vec![],
            next_id: 1,
            verify_index: 0,
            attempts: vec![0; decisions.len()],
            verification: [None, None],
            complete: false,
            verified_plies: vec![],
            metrics: None,
        })
    }
    fn value(
        &self,
        slot: usize,
        index: Option<usize>,
        invert: bool,
    ) -> Option<(Value, Option<u64>)> {
        if let Some(status) = &self.slots[slot].terminal {
            return Some((Value::Terminal(status.clone()), None));
        }
        let e = &self.evidence[index?];
        let score = &e.lines.first()?.score;
        Some((
            Value::Engine(if invert {
                score.invert()?
            } else {
                score.clone()
            }),
            Some(e.id),
        ))
    }
    fn conclusion(
        &self,
        i: usize,
        indexes: [Option<usize>; 2],
        version: u32,
    ) -> Option<Conclusion> {
        let (best, best_evidence) = self.value(i, indexes[0], false)?;
        let (played, played_evidence) = self.value(i + 1, indexes[1], true)?;
        let critical = critical(&best, &played, self.config.critical_cp);
        let conflict = conflict(&best, &played, self.config.critical_cp);
        Some(Conclusion {
            ply: i + 1,
            version,
            best,
            played,
            best_evidence,
            played_evidence,
            critical,
            conflict,
        })
    }
    fn publish_screening(&mut self) {
        for i in 0..self.decisions.len() {
            if !self.screening.iter().any(|c| c.ply == i + 1) {
                if let Some(c) =
                    self.conclusion(i, [self.slots[i].screening, self.slots[i + 1].screening], 1)
                {
                    self.screening.push(c);
                }
            }
        }
    }
    fn dispatch(&mut self, slot: usize, verification: bool, now_ms: u64) -> Dispatch {
        let id = self.next_id;
        self.next_id += 1;
        let request = Request {
            context: self.slots[slot].context.clone(),
            engine: self.config.engine.clone(),
            depth: if verification {
                self.config.verification_depth
            } else {
                self.config.screening_depth
            },
            multipv: self.config.multipv,
        };
        let d = Dispatch {
            generation: self.config.generation,
            execution_id: id,
            request,
        };
        self.ledger.push(LedgerEntry {
            execution_id: id,
            cause: if verification {
                let ply = self.verify_index + 1;
                let latest = self
                    .revisions
                    .iter()
                    .rev()
                    .find(|c| c.ply == ply)
                    .or_else(|| self.screening.iter().find(|c| c.ply == ply));
                if latest.is_some_and(|c| c.conflict) {
                    "conflict_verification"
                } else {
                    "critical_verification"
                }
            } else {
                "screening"
            }
            .into(),
            authorized_depth: d.request.depth,
            reserved: true,
            started_ms: now_ms,
            wall_ms: None,
            cpu_ms: None,
            memory_bytes: None,
            outcome: None,
        });
        self.active.push(Active {
            dispatch: d.clone(),
            slot,
            verification,
        });
        d
    }
    pub fn next_request(&mut self, now_ms: u64) -> Option<Dispatch> {
        if self.complete {
            return None;
        }
        self.publish_screening();
        if let Some(i) = self
            .slots
            .iter()
            .enumerate()
            .find(|(i, s)| {
                s.terminal.is_none()
                    && s.screening.is_none()
                    && !s.failed
                    && !self.active.iter().any(|a| a.slot == *i)
            })
            .map(|(i, _)| i)
        {
            return Some(self.dispatch(i, false, now_ms));
        }
        if !self.active.is_empty() {
            return None;
        }
        if self.verification == [None, None] {
            let candidate = (0..self.decisions.len()).find(|&i| {
                let latest = self
                    .revisions
                    .iter()
                    .rev()
                    .find(|c| c.ply == i + 1)
                    .or_else(|| self.screening.iter().find(|c| c.ply == i + 1));
                latest.is_some_and(|c| c.critical || c.conflict)
                    && self.attempts[i] < self.config.max_attempts
            });
            let Some(i) = candidate else {
                self.unresolved.retain(|pending| {
                    self.revisions
                        .iter()
                        .rev()
                        .find(|c| c.ply == pending.ply)
                        .or_else(|| self.screening.iter().find(|c| c.ply == pending.ply))
                        .is_none_or(|c| c.critical || c.conflict)
                });
                for i in 0..self.decisions.len() {
                    let latest = self
                        .revisions
                        .iter()
                        .rev()
                        .find(|c| c.ply == i + 1)
                        .or_else(|| self.screening.iter().find(|c| c.ply == i + 1));
                    let reason = match latest {
                        None => Some("screening evidence unavailable"),
                        Some(c) if c.critical => {
                            Some("critical result persists after verification budget")
                        }
                        Some(c) if c.conflict => {
                            Some("conflicting estimates persist after verification budget")
                        }
                        _ => None,
                    };
                    if let Some(reason) = reason {
                        if !self.unresolved.iter().any(|p| p.ply == i + 1) {
                            self.unresolved.push(Pending {
                                ply: i + 1,
                                reason: reason.into(),
                            });
                        }
                    }
                }
                self.complete = true;
                return None;
            };
            self.verify_index = i;
        }
        let i = self.verify_index;
        for side in 0..2 {
            if self.slots[i + side].terminal.is_none() && self.verification[side].is_none() {
                return Some(self.dispatch(i + side, true, now_ms));
            }
        }
        self.attempts[i] += 1;
        if !self.verified_plies.contains(&(i + 1)) {
            self.verified_plies.push(i + 1);
        }
        self.verification = [None, None];
        self.publish_revisions();
        self.next_request(now_ms)
    }
    fn publish_revisions(&mut self) {
        for i in 0..self.decisions.len() {
            let Some(previous) = self
                .revisions
                .iter()
                .rev()
                .find(|c| c.ply == i + 1)
                .or_else(|| self.screening.iter().find(|c| c.ply == i + 1))
            else {
                continue;
            };
            let pair = [self.slots[i].latest, self.slots[i + 1].latest];
            if let Some(c) = self.conclusion(i, pair, previous.version + 1) {
                if (c.best_evidence, c.played_evidence)
                    != (previous.best_evidence, previous.played_evidence)
                {
                    self.revisions.push(c);
                }
            }
        }
    }
    fn finish(&mut self, execution_id: u64, now_ms: u64, outcome: &str) -> bool {
        let Some(e) = self
            .ledger
            .iter_mut()
            .find(|e| e.execution_id == execution_id && e.reserved)
        else {
            return false;
        };
        e.reserved = false;
        e.wall_ms = now_ms.checked_sub(e.started_ms);
        e.outcome = Some(outcome.into());
        true
    }
    pub fn accept(
        &mut self,
        generation: u64,
        execution_id: u64,
        evidence: Evidence,
        now_ms: u64,
    ) -> bool {
        let Some(active) = self
            .active
            .iter()
            .find(|a| a.dispatch.execution_id == execution_id)
            .cloned()
        else {
            return false;
        };
        if generation != active.dispatch.generation
            || execution_id != active.dispatch.execution_id
            || evidence.execution_id != execution_id
            || self.evidence.iter().any(|e| e.id == evidence.id)
            || !satisfies(&evidence, &active.dispatch.request)
        {
            return false;
        }
        if !self.finish(execution_id, now_ms, "accepted") {
            return false;
        }
        self.active
            .retain(|a| a.dispatch.execution_id != execution_id);
        let index = self.evidence.len();
        self.evidence.push(evidence);
        if active.verification {
            self.verification[active.slot - self.verify_index] = Some(index);
            self.slots[active.slot].latest = Some(index);
        } else {
            self.slots[active.slot].screening = Some(index);
            self.slots[active.slot].latest = Some(index);
            self.publish_screening();
        }
        true
    }
    pub fn reject(
        &mut self,
        generation: u64,
        execution_id: u64,
        now_ms: u64,
        reason: &str,
    ) -> bool {
        let Some(active) = self
            .active
            .iter()
            .find(|a| a.dispatch.execution_id == execution_id)
            .cloned()
        else {
            return false;
        };
        if generation != active.dispatch.generation || execution_id != active.dispatch.execution_id
        {
            return false;
        }
        if !self.finish(execution_id, now_ms, reason) {
            return false;
        }
        self.active
            .retain(|a| a.dispatch.execution_id != execution_id);
        if active.verification {
            self.unresolved.push(Pending {
                ply: self.verify_index + 1,
                reason: format!("verification unavailable: {reason}"),
            });
            self.attempts[self.verify_index] = self.config.max_attempts;
            self.verification = [None, None];
            self.publish_revisions();
        } else {
            self.slots[active.slot].failed = true;
        }
        true
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: 1,
            config: self.config.clone(),
            decisions: self.decisions.clone(),
            evidence: self.evidence.clone(),
            screening: self.screening.clone(),
            revisions: self.revisions.clone(),
            unresolved: self.unresolved.clone(),
            ledger: self.ledger.clone(),
            complete: self.complete,
            verified_plies: self.verified_plies.clone(),
            metrics: self.metrics.clone(),
        }
    }
}

impl super::runner::Plan for Planner {
    type Snapshot = Snapshot;
    fn record_metrics(&mut self, metrics: super::runner::RunMetrics) {
        self.metrics = Some(metrics);
    }
    fn next(&mut self, now_ms: u64) -> Result<super::runner::Next> {
        match self.next_request(now_ms) {
            Some(d) => Ok(super::runner::Next::Dispatch(
                d.generation,
                d.execution_id,
                d.request,
            )),
            None if self.complete => Ok(super::runner::Next::Complete),
            None if !self.active.is_empty() => Ok(super::runner::Next::Wait),
            None => Err(ReviewError::new(
                ReviewErrorCode::EngineProtocol,
                "decision.planner",
                "planner has no dispatch and is incomplete",
            )),
        }
    }
    fn accept(
        &mut self,
        generation: u64,
        execution: u64,
        evidence: Evidence,
        now_ms: u64,
    ) -> Result<()> {
        if Planner::accept(self, generation, execution, evidence, now_ms) {
            Ok(())
        } else {
            Err(ReviewError::new(
                ReviewErrorCode::EngineProtocol,
                "decision.planner",
                "evidence rejected",
            ))
        }
    }
    fn reject(&mut self, generation: u64, execution: u64, now_ms: u64, reason: &str) -> Result<()> {
        if Planner::reject(self, generation, execution, now_ms, reason) {
            Ok(())
        } else {
            Err(ReviewError::new(
                ReviewErrorCode::EngineProtocol,
                "decision.planner",
                "reservation rejection failed",
            ))
        }
    }
    fn snapshot(&self) -> Snapshot {
        Planner::snapshot(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::decision::{Bound, Line, Score};
    use shakmaty::{uci::UciMove, CastlingMode, Position};
    fn config() -> Config {
        Config {
            generation: 7,
            engine: EngineSignature {
                executable_digest: "a".repeat(64),
                options: vec![],
            },
            screening_depth: 8,
            multipv: 1,
            verification_depth: 12,
            critical_cp: 100,
            max_attempts: 2,
        }
    }
    fn evidence(d: &Dispatch, score: i32) -> Evidence {
        let p = crate::review::core::position(&d.request.context.fen).unwrap();
        let m = *p.legal_moves().first().unwrap();
        Evidence {
            id: d.execution_id,
            execution_id: d.execution_id,
            context: d.request.context.clone(),
            engine: d.request.engine.clone(),
            unrestricted: true,
            depth: d.request.depth,
            lines: vec![Line {
                depth: d.request.depth,
                execution_id: d.execution_id,
                score: Score::Cp(score),
                bound: Bound::Exact,
                pv: vec![UciMove::from_move(m, CastlingMode::Standard).to_string()],
            }],
        }
    }
    #[test]
    fn entire_game_shares_positions_and_rejects_stale_events() {
        let mut p = Planner::new("1. e4 e5 2. Nf3 Nc6 *", config()).unwrap();
        let mut count = 0;
        while let Some(d) = p.next_request(count) {
            let e = evidence(&d, 0);
            assert!(!p.accept(6, d.execution_id, e.clone(), count + 1));
            assert!(!p.accept(7, d.execution_id + 1, e.clone(), count + 1));
            assert!(p.accept(7, d.execution_id, e, count + 1));
            count += 1;
            if count == 2 {
                assert_eq!(p.snapshot().screening.len(), 1);
            }
            assert!(count < 20);
        }
        let s = p.snapshot();
        assert!(s.complete);
        assert_eq!(count, 5);
        assert_eq!(s.screening.len(), 4);
        assert!(s.unresolved.is_empty());
        assert!(s
            .ledger
            .iter()
            .all(|e| !e.reserved && e.cpu_ms.is_none() && e.memory_bytes.is_none()));
        assert!(s.ledger.iter().all(|e| e.cause == "screening"));
    }
    #[test]
    fn concurrent_screening_accepts_reverse_order_without_duplicates() {
        let mut p = Planner::new("1. e4 e5 2. Nf3 Nc6 *", config()).unwrap();
        let requests: Vec<_> = (0..5).map(|_| p.next_request(0).unwrap()).collect();
        assert!(p.next_request(0).is_none());
        for d in requests.iter().rev() {
            assert!(p.accept(7, d.execution_id, evidence(d, 0), 10));
        }
        assert!(p.next_request(11).is_none());
        let s = p.snapshot();
        assert!(s.complete);
        assert_eq!(s.screening.len(), 4);
        assert_eq!(s.ledger.len(), 5);
        assert!(s
            .ledger
            .iter()
            .all(|e| !e.reserved && e.wall_ms == Some(10)));
    }
    #[test]
    fn critical_verification_is_finite_and_screening_immutable() {
        let mut p = Planner::new("1. e4 *", config()).unwrap();
        let mut n = 0;
        while let Some(d) = p.next_request(n) {
            let e = evidence(&d, 100);
            assert!(p.accept(7, d.execution_id, e, n + 1));
            n += 1;
            assert!(n < 10);
        }
        let s = p.snapshot();
        assert_eq!(n, 6);
        assert_eq!(s.screening.len(), 1);
        assert_eq!(s.revisions.len(), 2);
        assert_eq!(s.screening[0].version, 1);
        assert_eq!(s.unresolved.len(), 1);
        assert!(s.ledger.iter().any(|e| e.cause == "critical_verification"));
    }
    #[test]
    fn inverse_score_conflict_is_verified_and_only_persistent_conflict_is_pending() {
        for persists in [true, false] {
            let mut p = Planner::new("1. e4 *", config()).unwrap();
            let mut count = 0;
            while let Some(d) = p.next_request(count) {
                let is_child = !d.request.context.prefix.is_empty();
                let score = if d.request.depth == 8 || persists {
                    if is_child {
                        -200
                    } else {
                        0
                    }
                } else if is_child {
                    -100
                } else {
                    100
                };
                assert!(p.accept(7, d.execution_id, evidence(&d, score), count + 1));
                count += 1;
                assert!(count <= 6, "verification must respect max_attempts");
            }
            let s = p.snapshot();
            assert!(s.screening[0].conflict);
            assert!(!s.screening[0].critical);
            assert!(s.ledger.iter().any(|e| e.cause == "conflict_verification"));
            assert_eq!(s.unresolved.len(), usize::from(persists));
            if persists {
                assert_eq!(
                    s.unresolved[0].reason,
                    "conflicting estimates persist after verification budget"
                );
                assert_eq!(count, 6);
            } else {
                assert_eq!(count, 4);
                assert!(!s.revisions.last().unwrap().conflict);
            }
        }
    }
    #[test]
    fn neighbor_discovery_opens_finite_obligation() {
        let mut p = Planner::new("1. e4 e5 *", config()).unwrap();
        let mut n = 0;
        while let Some(d) = p.next_request(n) {
            // Screening: only the first decision is critical. Its verification
            // changes the shared middle position, making the neighbor critical.
            let score = if d.request.depth == 8 {
                if d.request.context.prefix.is_empty() {
                    200
                } else {
                    0
                }
            } else if d.request.context.prefix.len() == 1 {
                200
            } else {
                0
            };
            assert!(p.accept(7, d.execution_id, evidence(&d, score), n + 1));
            n += 1;
            assert!(n < 20);
        }
        let s = p.snapshot();
        assert!(!s.screening[1].critical);
        assert!(s.revisions.iter().any(|c| c.ply == 2 && c.critical));
        assert!(s.ledger.iter().any(|e| e.authorized_depth == 12));
        assert!(s.complete);
    }

    #[test]
    fn neighbor_verification_can_discover_inverse_conflict() {
        let mut p = Planner::new("1. e4 e5 *", config()).unwrap();
        let mut count = 0;
        while let Some(d) = p.next_request(count) {
            let prefix_len = d.request.context.prefix.len();
            let score = if d.request.depth == 8 && prefix_len == 0 {
                200
            } else if d.request.depth == 12 && prefix_len == 1 {
                -200
            } else {
                0
            };
            assert!(p.accept(7, d.execution_id, evidence(&d, score), count + 1));
            count += 1;
            assert!(count < 20);
        }
        let s = p.snapshot();
        assert!(!s.screening[1].conflict);
        assert!(s
            .revisions
            .iter()
            .any(|c| c.ply == 2 && c.conflict && !c.critical));
        assert!(s.ledger.iter().any(|e| e.cause == "conflict_verification"));
        assert!(s.complete);
    }

    #[test]
    fn checkmate_child_needs_no_search() {
        let mut p = Planner::new("1. f3 e5 2. g4 Qh4# *", config()).unwrap();
        let mut count = 0;
        while let Some(d) = p.next_request(count) {
            let e = evidence(&d, 0);
            assert!(p.accept(7, d.execution_id, e, count + 1));
            count += 1;
            assert!(count < 20);
        }
        let s = p.snapshot();
        assert_eq!(count, 4);
        assert!(matches!(
            s.screening.last().unwrap().played,
            Value::Terminal(PositionStatus::Checkmate)
        ));
        assert_eq!(s.screening.last().unwrap().played_evidence, None);
    }

    #[test]
    fn rejected_search_terminates_as_unresolved() {
        let mut p = Planner::new("1. e4 *", config()).unwrap();
        while let Some(d) = p.next_request(0) {
            assert!(p.reject(7, d.execution_id, 1, "no result"));
        }
        assert_eq!(p.snapshot().unresolved.len(), 1);
    }
}
