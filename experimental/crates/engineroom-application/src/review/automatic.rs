//! Mixed automatic queue. Baseline scores remain immutable for adaptive ranking.
use super::{
    adaptive::{self, Profile, SearchBudget, Target},
    contextual::ContextualReview,
    core::{self, Game},
    engine::{Cancellation, EngineFactory, SearchCheckpoint},
    pipeline::cancellable,
    pool::{Completed, Pool},
    repository::{ContextualEvaluation, Repository},
    scoring,
    types::*,
};
use crate::db::mode::Mode;
use shakmaty::Position;
use std::collections::{HashMap, VecDeque};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinSet,
};

pub(super) mod scheduler;
use scheduler::{BitSet, Demand, FenId, Scheduler};

pub(super) struct LiveDemand {
    pub fen: String,
    pub value: u32,
    pub multipv: u32,
    pub reply: oneshot::Sender<Result<RawPosition>>,
}

pub(super) struct NativeLive<'a> {
    pub cancel: &'a Cancellation,
    pub demands: &'a mut mpsc::Receiver<LiveDemand>,
}

struct PendingLive {
    id: FenId,
    request: LiveDemand,
}

struct Native<'game, 'a> {
    scheduler: Scheduler<'game>,
    demands: &'a mut mpsc::Receiver<LiveDemand>,
    latest: Option<PendingLive>,
    receiving: bool,
    // Replacing a subscriber leaves its unrestricted search and reservation
    // in place. Its completed evidence can still serve review or navigation.
    live_running: bool,
    latest_dispatched: bool,
}

impl Native<'_, '_> {
    fn refresh(&mut self, mut latest: Option<LiveDemand>) {
        while let Ok(request) = self.demands.try_recv() {
            latest = Some(request);
        }
        if let Some(request) = latest {
            self.accept(request);
        }
        if self
            .latest
            .as_ref()
            .is_some_and(|live| live.request.reply.is_closed())
        {
            self.latest = None;
            self.latest_dispatched = false;
        }
    }

    fn accept(&mut self, mut request: LiveDemand) {
        self.latest = None;
        self.latest_dispatched = false;
        if request.reply.is_closed() {
            return;
        }
        let position = if request.fen.contains(['\r', '\n']) {
            Err(ReviewError::new(
                ReviewErrorCode::InvalidPgn,
                "review.live",
                "FEN inválido.",
            ))
        } else {
            core::position(&request.fen)
        };
        match position {
            Ok(position) => {
                let canonical = core::fen(&position);
                if let Some(cp) = core::terminal_position(&position) {
                    let _ = request.reply.send(Ok(core::terminal_raw(&canonical, cp)));
                } else {
                    let id = self
                        .scheduler
                        .intern_live(&canonical, || position.legal_moves().len());
                    request.fen = canonical;
                    self.latest = Some(PendingLive { id, request });
                }
            }
            Err(error) => {
                let _ = request.reply.send(Err(error));
            }
        }
    }

    fn resolve_live(&mut self) {
        let Some(live) = self.latest.as_ref().filter(|_| !self.latest_dispatched) else {
            return;
        };
        let demand = Demand::time(live.request.value, live.request.multipv);
        if let Some(mut raw) = self.scheduler.lookup(live.id, demand)
            && let Some(live) = self.latest.take()
        {
            raw.fen = live.request.fen.clone();
            let _ = live.request.reply.send(Ok(raw));
        }
    }

    fn next_live(&mut self, worker: usize) -> Option<Job> {
        let live = self.latest.as_ref()?;
        if self.live_running || !self.scheduler.available(live.id) {
            return None;
        }
        let demand = Demand::time(live.request.value, live.request.multipv);
        self.live_running = true;
        self.latest_dispatched = true;
        Some(Job {
            worker,
            purpose: Purpose::Live,
            demand,
        })
    }

    fn complete_live(&mut self, raw: RawPosition) {
        self.refresh(None);
        self.live_running = false;
        // The original consumer can use its own result even if the engine
        // returned partial MultiPV. Replacement consumers need full coverage.
        if std::mem::take(&mut self.latest_dispatched)
            && let Some(live) = self.latest.take()
        {
            let _ = live.request.reply.send(Ok(raw));
        }
    }
}

enum Purpose {
    Triage(usize),
    Refinement(usize),
    Live,
}
struct Job {
    worker: usize,
    purpose: Purpose,
    demand: Demand,
}
impl Job {
    fn triage(worker: usize, index: usize, profile: Profile) -> Self {
        Self {
            worker,
            purpose: Purpose::Triage(index),
            demand: Demand::time(profile.triage_ms, profile.triage_multipv),
        }
    }
}
struct ActiveSearch {
    job: Job,
    fen_id: Option<FenId>,
    checkpoint: watch::Receiver<Option<SearchCheckpoint>>,
}

struct State<'a> {
    context: Option<ContextualReview>,
    baseline_evidence: Vec<Option<PositionEvidence>>,
    refined_evidence: Vec<Option<PositionEvidence>>,
    contextual_pending: Vec<ContextualEvaluation>,
    game: &'a Game,
    profile: Profile,
    sizing: Option<(u32, u32)>,
    phases: Vec<Phase>,
    terminals: Vec<Option<i32>>,
    baseline: Vec<Option<RawPosition>>,
    refined: Vec<Option<RawPosition>>,
    budgets: Vec<Option<SearchBudget>>,
    attempted: Vec<Option<SearchBudget>>,
    critical: Vec<Option<adaptive::Critical>>,
    book: usize,
    cached: usize,
    searched: usize,
    refinement_stage: bool,
    final_targets: Option<Vec<Target>>,
    ready_targets: Vec<Target>,
}
impl State<'_> {
    async fn lookup(
        &self,
        repository: &dyn Repository,
        indexes: &[usize],
        budget: SearchBudget,
        cancel: &Cancellation,
    ) -> Result<Vec<(Option<RawPosition>, Option<PositionEvidence>)>> {
        if let Some(context) = &self.context {
            let hits = cancellable(
                Box::pin(context.lookup(
                    repository,
                    indexes,
                    Mode::Time,
                    budget.ms,
                    budget.multipv,
                )),
                cancel,
            )
            .await?;
            return Ok(hits
                .into_iter()
                .map(|hit| {
                    hit.map_or((None, None), |mut hit| {
                        hit.evidence.origin = EvidenceOrigin::Cache;
                        (Some(hit.raw), Some(hit.evidence))
                    })
                })
                .collect());
        }
        let fens: Vec<_> = indexes.iter().map(|&i| self.game.fens[i].clone()).collect();
        let hits = cancellable(
            repository.lookup(&fens, Mode::Time, budget.ms, budget.multipv),
            cancel,
        )
        .await?;
        if hits.len() != indexes.len() {
            return Err(invalid_cache_count());
        }
        Ok(hits.into_iter().map(|raw| (raw, None)).collect())
    }

    async fn flush_context(&mut self, repository: &dyn Repository) -> Result<()> {
        while let Some(entry) = self.contextual_pending.last() {
            let budget = entry
                .evidence
                .requested
                .expect("completed search has a budget");
            let start = self
                .contextual_pending
                .iter()
                .rposition(|entry| entry.evidence.requested != Some(budget))
                .map_or(0, |i| i + 1);
            repository
                .put_context(&self.contextual_pending[start..], budget.mode, budget.value)
                .await?;
            self.contextual_pending.truncate(start);
        }
        Ok(())
    }

    fn complete(&self) -> bool {
        self.baseline.iter().all(Option::is_some)
    }
    fn targets(&self) -> &[Target] {
        self.final_targets.as_deref().unwrap_or(&self.ready_targets)
    }
    fn satisfied(&self, target: &Target) -> bool {
        let index = target.position_index;
        self.budgets[index].is_some_and(|b| covers(b, target.search))
            || self.attempted[index].is_some_and(|b| covers(b, target.search))
    }
    fn baseline(&self, index: usize) -> Result<&RawPosition> {
        self.baseline
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                ReviewError::new(
                    ReviewErrorCode::MissingEvaluation,
                    "review.triage",
                    "Avaliação de triagem ausente.",
                )
            })
    }
    fn refinement(&self, index: usize) -> Result<Option<&RawPosition>> {
        let baseline = self.baseline(index)?;
        Ok(self
            .refined
            .get(index)
            .and_then(Option::as_ref)
            .filter(|refined| refined.depth >= baseline.depth))
    }
    fn evaluation(&self, index: usize) -> Result<&RawPosition> {
        Ok(self.refinement(index)?.unwrap_or(self.baseline(index)?))
    }
    fn discover(&mut self, index: usize) {
        for ply in [index, index + 1] {
            if ply == 0 || ply > self.game.moves.len() || self.critical[ply - 1].is_some() {
                continue;
            }
            if let (Some(before), Some(after)) = (&self.baseline[ply - 1], &self.baseline[ply]) {
                self.critical[ply - 1] = Some(adaptive::rank_move(
                    &self.game.moves[ply - 1],
                    &self.game.positions[ply - 1],
                    before,
                    after,
                    self.book,
                ));
                if self.critical[ply - 1].as_ref().is_some_and(|c| c.hard) {
                    let critical: Vec<_> = self
                        .critical
                        .iter()
                        .flatten()
                        .filter(|c| c.hard)
                        .cloned()
                        .collect();
                    self.ready_targets =
                        adaptive::targets(&critical, self.baseline.len(), self.profile)
                            .into_iter()
                            .filter(|t| self.terminals[t.position_index].is_none())
                            .collect();
                }
            }
        }
    }
    fn reuse_refinements(
        &mut self,
        scheduler: &Scheduler<'_>,
        refining: &BitSet,
        emit: &mut (dyn FnMut(Event) + Send),
    ) -> Result<()> {
        if self.context.is_some() {
            return Ok(());
        }
        for slot in 0..self.targets().len() {
            let target = &self.targets()[slot];
            let i = target.position_index;
            let budget = target.search;
            if !self.satisfied(target)
                && !refining.contains(i)
                && let Some(id) = scheduler.ply_id(i)
                && let Some(mut raw) = scheduler.lookup(id, Demand::time(budget.ms, budget.multipv))
            {
                raw.fen.clone_from(&self.game.fens[i]);
                self.budgets[i] = Some(budget);
                self.refined[i] = Some(raw);
                self.cached += 1;
                self.progress(i, Some(self.evaluation(i)?), emit);
            }
        }
        Ok(())
    }

    fn share_triage(
        &mut self,
        id: FenId,
        scheduler: &Scheduler<'_>,
        emit: &mut (dyn FnMut(Event) + Send),
    ) -> Result<()> {
        if self.context.is_some() {
            return Ok(());
        }
        let demand = Demand::time(self.profile.triage_ms, self.profile.triage_multipv);
        let mut plies = scheduler.plies(id);
        if let Some(index) = plies.find(|&index| self.baseline[index].is_none())
            && let Some(raw) = scheduler.lookup(id, demand)
        {
            for index in std::iter::once(index).chain(plies) {
                if self.baseline[index].is_none() {
                    let mut ply_raw = raw.clone();
                    ply_raw.fen = self.game.fens[index].clone();
                    self.baseline[index] = Some(ply_raw);
                    self.cached += 1;
                    self.discover(index);
                    self.progress(index, Some(self.baseline(index)?), emit);
                }
            }
        }
        Ok(())
    }

    fn progress(
        &self,
        index: usize,
        update: Option<&RawPosition>,
        emit: &mut (dyn FnMut(Event) + Send),
    ) {
        let targets = self.targets();
        let remaining = self.baseline.iter().filter(|p| p.is_none()).count() as u64
            * self.profile.triage_ms as u64
            + targets
                .iter()
                .filter(|t| !self.satisfied(t))
                .map(|t| t.search.ms as u64)
                .sum::<u64>();
        let (completed, total) = if self.refinement_stage {
            (
                targets.iter().filter(|t| self.satisfied(t)).count(),
                targets.len(),
            )
        } else {
            (self.baseline.iter().flatten().count(), self.baseline.len())
        };
        emit(Event::Progress {
            progress: Progress {
                stage: if self.refinement_stage {
                    "refinement"
                } else {
                    "triage"
                }
                .into(),
                completed,
                total,
                current_ply: index,
                phase: Some(self.phases[index]),
                cached_positions: self.cached,
                engine_positions: self.searched,
                remaining_budget_ms: Some(remaining),
                update: update.map(|raw| WinPctUpdate {
                    index,
                    win_pct: scoring::white_win_pct(
                        raw.cp,
                        self.game.fens[index].split_whitespace().nth(1) != Some("b"),
                    ),
                }),
            },
        });
    }
}

struct Resources<'a> {
    factory: &'a dyn EngineFactory,
    repository: &'a dyn Repository,
    cancel: &'a Cancellation,
}

pub(super) async fn review(
    factory: &dyn EngineFactory,
    repository: &dyn Repository,
    game: &Game,
    profile: Profile,
    sizing: Option<(u32, u32)>,
    cancel: &Cancellation,
    emit: &mut (dyn FnMut(Event) + Send),
) -> Result<ReviewResult> {
    run_review(
        Resources {
            factory,
            repository,
            cancel,
        },
        game,
        profile,
        sizing,
        emit,
        None,
    )
    .await
}

pub(super) async fn review_native(
    factory: &dyn EngineFactory,
    repository: &dyn Repository,
    game: &Game,
    profile: Profile,
    sizing: Option<(u32, u32)>,
    live: NativeLive<'_>,
    emit: &mut (dyn FnMut(Event) + Send),
) -> Result<ReviewResult> {
    let mut native = Native {
        scheduler: Scheduler::new(game),
        demands: live.demands,
        latest: None,
        receiving: true,
        live_running: false,
        latest_dispatched: false,
    };
    run_review(
        Resources {
            factory,
            repository,
            cancel: live.cancel,
        },
        game,
        profile,
        sizing,
        emit,
        Some(&mut native),
    )
    .await
}

async fn run_review(
    resources: Resources<'_>,
    game: &Game,
    profile: Profile,
    sizing: Option<(u32, u32)>,
    emit: &mut (dyn FnMut(Event) + Send),
    mut native: Option<&mut Native<'_, '_>>,
) -> Result<ReviewResult> {
    let count = game.fens.len();
    let signature = resources.factory.artifact_signature().await?;
    resources.cancel.check()?;
    let context = signature.map(|signature| {
        if let Some(native) = &mut native {
            native.scheduler.enable_context();
        }
        ContextualReview::automatic(game, &signature, sizing)
    });
    let mut state = State {
        context,
        baseline_evidence: vec![None; count],
        refined_evidence: vec![None; count],
        contextual_pending: Vec::new(),
        game,
        profile,
        sizing,
        phases: core::phases(&game.fens),
        terminals: game.positions.iter().map(core::terminal_position).collect(),
        baseline: vec![None; count],
        refined: vec![None; count],
        budgets: vec![None; count],
        attempted: vec![None; count],
        critical: vec![None; game.moves.len()],
        book: core::opening(&game.moves).map_or(0, |(n, _)| n),
        cached: 0,
        searched: 0,
        refinement_stage: false,
        final_targets: None,
        ready_targets: vec![],
    };
    let mut pool: Option<Pool> = None;
    let mut pending: Vec<(u32, u32, Vec<RawPosition>)> = vec![];
    let mut run = execute(
        &resources,
        emit,
        &mut state,
        &mut pool,
        &mut pending,
        native,
    )
    .await;
    if let Some(pool) = &mut pool
        && let Err(error) = pool.close().await
    {
        if run.is_err() {
            emit(Event::Warning { error });
        } else {
            run = Err(error);
        }
    }
    if run.is_err()
        && let Err(error) = flush(resources.repository, &mut pending).await
    {
        emit(Event::Warning { error });
    }
    if run.is_err()
        && let Err(error) = state.flush_context(resources.repository).await
    {
        emit(Event::Warning { error });
    }
    run
}
async fn execute(
    &Resources {
        factory,
        repository,
        cancel,
    }: &Resources<'_>,
    emit: &mut (dyn FnMut(Event) + Send),
    state: &mut State<'_>,
    pool: &mut Option<Pool>,
    pending: &mut Vec<(u32, u32, Vec<RawPosition>)>,
    mut native: Option<&mut Native<'_, '_>>,
) -> Result<ReviewResult> {
    let game = state.game;
    let profile = state.profile;
    let count = game.fens.len();

    let indexes: Vec<_> = (0..count).collect();
    let hits = state
        .lookup(
            repository,
            &indexes,
            SearchBudget {
                ms: profile.triage_ms,
                multipv: profile.triage_multipv,
            },
            cancel,
        )
        .await?;
    let mut queue = VecDeque::new();
    for (i, (hit, evidence)) in hits.into_iter().enumerate() {
        cancel.check()?;
        state.baseline[i] = if let Some(cp) = state.terminals[i] {
            Some(core::terminal_raw(&game.fens[i], cp))
        } else if let Some(hit) = hit {
            state.baseline_evidence[i] = evidence;
            state.cached += 1;
            Some(hit)
        } else {
            queue.push_back(i);
            None
        };
        if let Some(raw) = &state.baseline[i] {
            state.progress(i, Some(raw), emit);
        }
        state.discover(i);
    }
    if !queue.is_empty()
        && let Some(native) = &mut native
    {
        native.scheduler.prepare();
        let demand = Demand::time(profile.triage_ms, profile.triage_multipv);
        for (i, raw) in state.baseline.iter().enumerate() {
            if let Some(raw) = raw.as_ref().filter(|_| state.terminals[i].is_none()) {
                let id = native.scheduler.intern_ply(i);
                if native.scheduler.lookup(id, demand).is_none() {
                    native.scheduler.record(id, demand, raw);
                }
            }
        }
        for &i in &queue {
            if state.baseline[i].is_none() {
                let id = native.scheduler.intern_ply(i);
                state.share_triage(id, &native.scheduler, emit)?;
            }
        }
        queue.retain(|&i| state.baseline[i].is_none());
    }
    let mut waiters = JoinSet::new();
    let mut blocks: Vec<VecDeque<usize>> = vec![];
    // The watch receiver belongs to this dispatch and retains its latest
    // complete evidence while the worker is busy, independently of completion.
    let mut active_searches = HashMap::new();
    let mut refining = BitSet::default();
    let mut cache_misses = CacheMisses::default();
    loop {
        cancel.check()?;
        if let Some(native) = &mut native {
            native.refresh(None);
            native.resolve_live();
        }
        if state.complete() && state.final_targets.is_none() {
            let raw: Vec<_> = state.baseline.iter().flatten().cloned().collect();
            state.final_targets = Some(
                adaptive::review_targets(game, &raw, profile)
                    .into_iter()
                    .filter(|t| state.terminals[t.position_index].is_none())
                    .collect(),
            );
        }
        if state.complete() && !state.refinement_stage {
            state.refinement_stage = true;
            if let Some(target) = state.targets().first() {
                state.progress(target.position_index, None, emit);
            }
        }
        if let Some(native) = &native {
            state.reuse_refinements(&native.scheduler, &refining, emit)?;
        }
        // Query selected positions in budget groups, skipping requests
        // already ruled out by a miss at an equal or broader lookup.
        let mut groups: Vec<(SearchBudget, Vec<usize>)> = vec![];
        for slot in 0..state.targets().len() {
            let target = &state.targets()[slot];
            let i = target.position_index;
            if state.satisfied(target) || refining.contains(i) {
                continue;
            }
            let search = target.search;
            if cache_misses.rules_out(i, search) {
                continue;
            }
            let slot = groups
                .iter()
                .position(|(budget, _)| *budget == search)
                .unwrap_or_else(|| {
                    groups.push((search, vec![]));
                    groups.len() - 1
                });
            groups[slot].1.push(i);
        }
        let mut cache_evidence = Vec::new();
        for (budget, indexes) in groups {
            let hits = state.lookup(repository, &indexes, budget, cancel).await?;
            for (i, (hit, evidence)) in indexes.into_iter().zip(hits) {
                if let Some(raw) = hit {
                    state.refined_evidence[i] = evidence;
                    if let Some(native) = &mut native {
                        let id = native.scheduler.intern_ply(i);
                        native
                            .scheduler
                            .record(id, Demand::time(budget.ms, budget.multipv), &raw);
                        cache_evidence.push(id);
                    }
                    state.budgets[i] = Some(budget);
                    state.refined[i] = Some(raw);
                    state.cached += 1;
                    state.progress(i, Some(state.evaluation(i)?), emit);
                } else {
                    cache_misses.record(i, budget, count);
                }
            }
        }
        if let Some(native) = &mut native
            && !cache_evidence.is_empty()
        {
            for id in cache_evidence {
                // An active triage still owns its immutable baseline.
                if native.scheduler.available(id) {
                    state.share_triage(id, &native.scheduler, emit)?;
                }
            }
            // Reconcile aliases against the new cache evidence before dispatch.
            native.resolve_live();
            state.reuse_refinements(&native.scheduler, &refining, emit)?;
        }
        let needs_refinement = state
            .targets()
            .iter()
            .any(|t| !state.satisfied(t) && !refining.contains(t.position_index));
        let needs_live = native
            .as_ref()
            .is_some_and(|native| native.latest.is_some());
        if pool.is_none() && (!queue.is_empty() || needs_refinement || needs_live) {
            let acquired = Pool::acquire(factory, state.sizing, count, cancel).await?;
            let workers = acquired.len();
            *pool = Some(acquired);
            blocks = (0..workers).map(|_| VecDeque::new()).collect();
        }
        if let Some(pool) = pool.as_ref() {
            for worker in 0..pool.len() {
                if active_searches.contains_key(&worker) {
                    continue;
                }
                let prefer_refinement =
                    state.complete() || (pool.len() > 1 && worker == pool.len() - 1);
                let target = if prefer_refinement {
                    state.targets().iter().find(|t| {
                        let i = t.position_index;
                        !state.satisfied(t)
                            && !refining.contains(i)
                            && native
                                .as_ref()
                                .and_then(|n| {
                                    n.scheduler.ply_id(i).map(|id| n.scheduler.available(id))
                                })
                                .unwrap_or(true)
                    })
                } else {
                    None
                };
                let live = native.as_mut().and_then(|native| native.next_live(worker));
                let job = if live.is_some() {
                    live
                } else if let Some(target) = target {
                    Some(Job {
                        worker,
                        purpose: Purpose::Refinement(target.position_index),
                        demand: Demand::time(target.search.ms, target.search.multipv),
                    })
                } else if let Some(native) = &native {
                    std::iter::from_fn(|| {
                        native
                            .scheduler
                            .take_triage(worker, &mut queue, &mut blocks)
                    })
                    .find(|&index| state.baseline[index].is_none())
                    .map(|index| Job::triage(worker, index, profile))
                } else {
                    if blocks[worker].is_empty() {
                        // Claim at most four consecutive positions. Missing cache
                        // entries can have gaps, so do not join separate runs.
                        while blocks[worker].len() < 4 {
                            let Some(&next) = queue.front() else {
                                break;
                            };
                            if blocks[worker].back().is_some_and(|last| next != last + 1) {
                                break;
                            }
                            if let Some(index) = queue.pop_front() {
                                blocks[worker].push_back(index);
                            }
                        }
                    }
                    // A refiner may have paused its block. Let a free triage
                    // worker take those remaining positions rather than idle.
                    let preferred = pool.len() - 1;
                    if blocks[worker].is_empty() && worker != preferred {
                        blocks[worker] = std::mem::take(&mut blocks[preferred]);
                    }
                    blocks[worker]
                        .pop_front()
                        .map(|index| Job::triage(worker, index, profile))
                };
                if let Some(job) = job {
                    let worker = job.worker;
                    let (fen, fen_id) = match job.purpose {
                        Purpose::Triage(index) | Purpose::Refinement(index) => {
                            let id = native
                                .as_mut()
                                .map(|native| native.scheduler.intern_ply(index));
                            (&game.fens[index], id)
                        }
                        Purpose::Live => {
                            let live = native
                                .as_ref()
                                .and_then(|native| native.latest.as_ref())
                                .expect("live job has a pending request");
                            (&live.request.fen, Some(live.id))
                        }
                    };
                    let fen = fen.clone();
                    if let (Some(native), Some(id)) = (&mut native, fen_id) {
                        native.scheduler.reserve(id);
                    }
                    let history = match job.purpose {
                        Purpose::Triage(index) | Purpose::Refinement(index)
                            if state.context.is_some() =>
                        {
                            Some(ContextualReview::position(game, index)?)
                        }
                        _ => None,
                    };
                    let evaluation = pool
                        .submit_position(worker, fen, history, job.demand.value, job.demand.multipv)
                        .await?;
                    if let Purpose::Refinement(index) = job.purpose {
                        refining.insert(index);
                    }
                    active_searches.insert(
                        worker,
                        ActiveSearch {
                            job,
                            fen_id,
                            checkpoint: evaluation.checkpoint,
                        },
                    );
                    waiters.spawn(async move { (worker, evaluation.completed.await) });
                }
            }
        }
        // Start the next searches before flushing so engines keep working
        // during database writes. Each finished position is buffered immediately.
        if pending
            .iter()
            .map(|(_, _, entries)| entries.len())
            .sum::<usize>()
            >= 8
        {
            flush(repository, pending).await?;
        }
        if state.contextual_pending.len() >= 8 {
            state.flush_context(repository).await?;
        }
        if waiters.is_empty() {
            if !state.complete() || state.targets().iter().any(|t| !state.satisfied(t)) {
                return Err(ReviewError::new(
                    ReviewErrorCode::EngineExited,
                    "review.queue",
                    "Fila sem worker disponível.",
                ));
            }
            break;
        }
        let incoming = native.as_mut().filter(|native| native.receiving);
        let result = tokio::select! {
            _ = cancel.cancelled() => return Err(ReviewError::cancelled()),
            (native, request) = async {
                let Some(native) = incoming else {
                    return std::future::pending().await;
                };
                let request = native.demands.recv().await;
                (native, request)
            } => {
                match request {
                    Some(request) => native.refresh(Some(request)),
                    None => native.receiving = false,
                }
                continue;
            },
            result = waiters.join_next() => result.ok_or_else(|| queue_error("Fila encerrou sem resultado."))?.map_err(|e| ReviewError::with_source(ReviewErrorCode::EngineExited, "review.queue", e))?,
        };
        let (worker, reply) = result;
        let ActiveSearch {
            job,
            fen_id,
            checkpoint,
        } = active_searches
            .remove(&worker)
            .ok_or_else(|| queue_error("Resultado sem busca ativa."))?;
        drop(checkpoint);
        let pool = pool
            .as_ref()
            .ok_or_else(|| queue_error("Pool indisponível."))?;
        let failure = pool.failure()?;
        let completed: Completed = reply
            .map_err(|_| disconnected_worker_error(failure.clone(), cancel))?
            .map_err(|e| failure.unwrap_or(e))?;
        let raw = completed.raw;
        let covered_multipv = if let (Some(native), Some(id)) = (&mut native, fen_id) {
            Some(native.scheduler.complete(
                id,
                Demand::time(completed.value, job.demand.multipv),
                &raw,
            ))
        } else {
            None
        };
        // Both stages use unrestricted searches and can populate the FEN cache.
        if let Some(context) = &state.context {
            let budget = EvidenceBudget {
                mode: Mode::Time,
                value: completed.value,
                multipv: job.demand.multipv,
            };
            let slot = match job.purpose {
                Purpose::Triage(index) => Some((index, &mut state.baseline_evidence[index])),
                Purpose::Refinement(index) => Some((index, &mut state.refined_evidence[index])),
                Purpose::Live => None,
            };
            if let Some((index, slot)) = slot {
                let entry = context.entry(index, &raw, completed.engine, budget);
                *slot = Some(entry.evidence.clone());
                state.contextual_pending.push(entry);
            }
        } else {
            let buffer = pending
                .iter()
                .position(|(value, multipv, _)| {
                    *value == completed.value && *multipv == job.demand.multipv
                })
                .unwrap_or_else(|| {
                    pending.push((completed.value, job.demand.multipv, vec![]));
                    pending.len() - 1
                });
            pending[buffer].2.push(raw.clone());
        }
        match job.purpose {
            Purpose::Refinement(index) => {
                let covered_multipv = covered_multipv.unwrap_or_else(|| {
                    scheduler::covered_multipv(
                        &raw,
                        job.demand.multipv,
                        game.positions[index].legal_moves().len(),
                    )
                });
                let covered_multipv = if state.context.is_some() {
                    core::observed_coverage(&raw, job.demand.multipv as usize).map_or(
                        0,
                        |(lines, _, legal)| {
                            if lines == legal {
                                job.demand.multipv
                            } else {
                                lines as u32
                            }
                        },
                    )
                } else {
                    covered_multipv
                };
                state.searched += 1;
                refining.remove(index);
                state.budgets[index] = (covered_multipv > 0).then_some(SearchBudget {
                    ms: completed.value,
                    multipv: covered_multipv,
                });
                state.attempted[index] = Some(SearchBudget {
                    ms: completed.value,
                    multipv: job.demand.multipv,
                });
                state.refined[index] = Some(raw);
                state.progress(index, Some(state.evaluation(index)?), emit);
            }
            Purpose::Triage(index) => {
                state.searched += 1;
                state.baseline[index] = Some(raw);
                state.discover(index);
                state.progress(index, Some(state.baseline(index)?), emit);
            }
            Purpose::Live => {
                cancel.check()?;
                if let Some(native) = &mut native {
                    native.complete_live(raw);
                }
            }
        }
        if let (Some(native), Some(id)) = (&native, fen_id) {
            state.share_triage(id, &native.scheduler, emit)?;
        }
    }
    flush(repository, pending).await?;
    state.flush_context(repository).await?;
    cancel.check()?;
    emit(Event::Progress {
        progress: Progress {
            stage: "finalizing".into(),
            completed: count,
            total: count,
            current_ply: count - 1,
            phase: state.phases.last().copied(),
            cached_positions: state.cached,
            engine_positions: state.searched,
            remaining_budget_ms: None,
            update: None,
        },
    });
    let raw: Vec<_> = (0..count)
        .map(|i| state.evaluation(i).cloned())
        .collect::<Result<_>>()?;
    let mut result = core::build(game, &raw)?;
    for (i, position) in result.positions.iter_mut().enumerate() {
        if state.context.is_some() {
            position.evidence = if state.terminals[i].is_some() {
                Some(PositionEvidence::from_raw(
                    &raw[i],
                    Some(i),
                    EvidenceOrigin::Terminal,
                    None,
                    None,
                ))
            } else if state.refinement(i)?.is_some() {
                state.refined_evidence[i].clone()
            } else {
                state.baseline_evidence[i].clone()
            };
        }
        if let (Some(budget), Some(_)) = (state.budgets[i], state.refinement(i)?) {
            position.triage_lines =
                Some(core::position_analysis(state.baseline(i)?, i, state.phases[i]).lines);
            position.search = Some(Search {
                purpose: "refinement".into(),
                movetime_ms: budget.ms,
                multipv: budget.multipv,
            });
        }
    }
    if state.context.is_some() {
        for (i, decision) in result.moves.iter_mut().enumerate() {
            if let Some(critical) = state.critical[i].as_ref().filter(|critical| critical.hard) {
                let requested = profile.budget(adaptive::refinement_kind(critical));
                let pending_plies = [i, i + 1]
                    .into_iter()
                    .filter(|&ply| {
                        state.terminals[ply].is_none()
                            && !state.budgets[ply].is_some_and(|actual| covers(actual, requested))
                    })
                    .collect();
                let before = state.baseline(i)?.cp;
                let after = state.baseline(i + 1)?.cp;
                let screening_loss =
                    (scoring::win_pct(before) + scoring::win_pct(after) - 100.0).max(0.0);
                decision.critical_verification = Some(CriticalVerification {
                    screening_classification: scoring::classify(screening_loss, decision.is_book),
                    screening_before_cp: before,
                    screening_after_cp: after,
                    reasons: critical.reasons.clone(),
                    pending_plies,
                });
            }
        }
    }
    Ok(result)
}
#[derive(Default)]
struct CacheMisses(Vec<Vec<SearchBudget>>);

impl CacheMisses {
    fn rules_out(&self, ply: usize, budget: SearchBudget) -> bool {
        self.0
            .get(ply)
            .is_some_and(|misses| misses.iter().any(|&miss| covers(budget, miss)))
    }

    fn record(&mut self, ply: usize, budget: SearchBudget, count: usize) {
        if self.0.is_empty() {
            self.0.resize_with(count, Vec::new);
        }
        let misses = &mut self.0[ply];
        // Keep only the least restrictive misses in the two-dimensional order.
        misses.retain(|&miss| !covers(miss, budget));
        misses.push(budget);
    }
}

fn covers(available: SearchBudget, requested: SearchBudget) -> bool {
    available.ms >= requested.ms && available.multipv >= requested.multipv
}

async fn flush(
    repository: &dyn Repository,
    pending: &mut [(u32, u32, Vec<RawPosition>)],
) -> Result<()> {
    for (value, multipv, entries) in pending {
        if !entries.is_empty() {
            repository
                .put(entries, Mode::Time, *value, *multipv)
                .await?;
            entries.clear();
        }
    }
    Ok(())
}

fn disconnected_worker_error(failure: Option<ReviewError>, cancel: &Cancellation) -> ReviewError {
    failure.unwrap_or_else(|| {
        cancel.check().err().unwrap_or_else(|| {
            ReviewError::new(
                ReviewErrorCode::EngineExited,
                "review.queue",
                "Worker encerrou sem resultado.",
            )
        })
    })
}

fn invalid_cache_count() -> ReviewError {
    ReviewError::new(
        ReviewErrorCode::Cache,
        "cache.lookup",
        "Quantidade de avaliações inválida.",
    )
}

#[cfg(test)]
mod tests;

fn queue_error(message: &str) -> ReviewError {
    ReviewError::new(ReviewErrorCode::EngineExited, "review.queue", message)
}
