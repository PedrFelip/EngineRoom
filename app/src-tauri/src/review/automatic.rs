//! Mixed automatic queue. Baseline scores remain immutable for adaptive ranking.
use super::{
    adaptive::{self, Profile, RefinementKind, SearchBudget, Target},
    core::{self, Game},
    engine::{Cancellation, EngineFactory},
    pipeline::cancellable,
    pool::{Completed, Pool},
    repository::Repository,
    scoring,
    types::*,
};
use crate::db::mode::Mode;
use std::collections::VecDeque;
use tokio::task::JoinSet;

struct Job {
    worker: usize,
    index: usize,
    refinement: bool,
    value: u32,
    multipv: u32,
}
struct State<'a> {
    game: &'a Game,
    profile: Profile,
    phases: Vec<Phase>,
    terminals: Vec<Option<i32>>,
    baseline: Vec<Option<RawPosition>>,
    refined: Vec<Option<RawPosition>>,
    budgets: Vec<Option<SearchBudget>>,
    critical: Vec<Option<adaptive::Critical>>,
    book: usize,
    cached: usize,
    searched: usize,
    refinement_stage: bool,
    final_targets: Option<Vec<Target>>,
}
impl State<'_> {
    fn complete(&self) -> bool {
        self.baseline.iter().all(Option::is_some)
    }
    fn targets(&self) -> Vec<Target> {
        // Hard candidates bypass the global soft quota. Soft candidates wait
        // for the full ranking so early moves cannot consume later moves' quota.
        if let Some(targets) = &self.final_targets {
            return targets.clone();
        }
        let critical: Vec<_> = self
            .critical
            .iter()
            .flatten()
            .filter(|c| c.hard)
            .cloned()
            .collect();
        adaptive::targets(&critical, self.baseline.len(), self.profile)
            .into_iter()
            .filter(|t| t.hard && self.terminals[t.position_index].is_none())
            .collect()
    }
    fn value(&self, target: &Target) -> u32 {
        target.search.ms
    }
    fn multipv(&self, target: &Target) -> u32 {
        target.search.multipv
    }
    fn satisfied(&self, target: &Target) -> bool {
        self.budgets[target.position_index]
            .is_some_and(|b| b.ms >= self.value(target) && b.multipv >= self.multipv(target))
    }
    fn discover(&mut self, index: usize) {
        for ply in [index, index + 1] {
            if ply == 0 || ply > self.game.moves.len() || self.critical[ply - 1].is_some() {
                continue;
            }
            if let (Some(before), Some(after)) = (&self.baseline[ply - 1], &self.baseline[ply]) {
                self.critical[ply - 1] = Some(adaptive::rank_move(
                    &self.game.moves[ply - 1],
                    before,
                    after,
                    self.book,
                ));
            }
        }
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
                .map(|t| self.value(t) as u64)
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

pub(super) async fn review(
    factory: &dyn EngineFactory,
    repository: &dyn Repository,
    game: &Game,
    profile: Profile,
    sizing: Option<(u32, u32)>,
    cancel: &Cancellation,
    emit: &mut (dyn FnMut(Event) + Send),
) -> Result<ReviewResult> {
    let count = game.fens.len();
    let mut state = State {
        game,
        profile,
        phases: core::phases(&game.fens),
        terminals: game
            .fens
            .iter()
            .map(|f| core::terminal(f))
            .collect::<Result<_>>()?,
        baseline: vec![None; count],
        refined: vec![None; count],
        budgets: vec![None; count],
        critical: vec![None; game.moves.len()],
        book: core::opening(&game.moves).map_or(0, |(n, _)| n),
        cached: 0,
        searched: 0,
        refinement_stage: false,
        final_targets: None,
    };
    let mut pool: Option<Pool> = None;
    let mut pending: Vec<(u32, u32, Vec<RawPosition>)> = vec![];
    let run = execute(
        factory,
        repository,
        sizing,
        cancel,
        emit,
        &mut state,
        &mut pool,
        &mut pending,
    )
    .await;
    if let Some(pool) = &mut pool {
        pool.close().await;
    }
    if run.is_err() {
        if let Err(error) = flush(repository, &mut pending).await {
            emit(Event::Warning { error });
        }
    }
    run
}
async fn execute(
    factory: &dyn EngineFactory,
    repository: &dyn Repository,
    sizing: Option<(u32, u32)>,
    cancel: &Cancellation,
    emit: &mut (dyn FnMut(Event) + Send),
    state: &mut State<'_>,
    pool: &mut Option<Pool>,
    pending: &mut Vec<(u32, u32, Vec<RawPosition>)>,
) -> Result<ReviewResult> {
    let game = state.game;
    let profile = state.profile;
    let count = game.fens.len();

    let mut caches = vec![];
    let mut requests = vec![SearchBudget {
        ms: profile.triage_ms,
        multipv: profile.triage_multipv,
    }];
    for kind in [
        RefinementKind::Context,
        RefinementKind::Uncertain,
        RefinementKind::Tactical,
        RefinementKind::Complex,
        RefinementKind::Critical,
        RefinementKind::Promotion,
        RefinementKind::Mate,
    ] {
        let budget = profile.budget(kind);
        if !requests.contains(&budget) {
            requests.push(budget);
        }
    }
    for budget in &requests {
        let (value, multipv) = (budget.ms, budget.multipv);
        let hits = cancellable(
            repository.lookup(&game.fens, Mode::Time, value, multipv),
            cancel,
        )
        .await?;
        if hits.len() != count {
            return Err(ReviewError::new(
                "cache",
                "cache.lookup",
                "Quantidade de avaliações inválida.",
            ));
        }
        caches.push(hits);
    }
    let mut queue = VecDeque::new();
    for i in 0..count {
        cancel.check()?;
        state.baseline[i] = if let Some(cp) = state.terminals[i] {
            Some(core::terminal_raw(&game.fens[i], cp))
        } else if let Some(hit) = caches[0][i].take() {
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
    let mut waiters = JoinSet::new();
    let mut blocks: Vec<VecDeque<usize>> = vec![];
    let mut busy = vec![];
    let mut refining = vec![false; count];
    loop {
        cancel.check()?;
        if state.complete() && state.final_targets.is_none() {
            let raw: Vec<_> = state.baseline.iter().flatten().cloned().collect();
            state.final_targets = Some(
                adaptive::review_targets(game, &raw, profile)
                    .into_iter()
                    .filter(|t| state.terminals[t.position_index].is_none())
                    .collect(),
            );
        }
        let targets = state.targets();
        if state.complete() && !state.refinement_stage {
            state.refinement_stage = true;
            if let Some(target) = targets.first() {
                state.progress(target.position_index, None, emit);
            }
        }
        for target in &targets {
            let i = target.position_index;
            let value = state.value(target);
            if state.satisfied(target) || refining[i] {
                continue;
            }
            let hit = requests
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(_, b)| b.ms >= value && b.multipv >= state.multipv(target))
                .find_map(|(slot, b)| caches[slot][i].take().map(|raw| (raw, *b)));
            if let Some((raw, budget)) = hit {
                state.budgets[i] = Some(budget);
                state.refined[i] = Some(raw);
                state.cached += 1;
                state.progress(i, state.refined[i].as_ref(), emit);
            }
        }
        let todo: Vec<_> = targets
            .iter()
            .filter(|t| !state.satisfied(t) && !refining[t.position_index])
            .collect();
        if pool.is_none() && (!queue.is_empty() || !todo.is_empty()) {
            *pool = Some(Pool::acquire(factory, sizing, count, cancel).await?);
            let workers = pool.as_ref().unwrap().len();
            blocks = (0..workers).map(|_| VecDeque::new()).collect();
            busy = vec![false; workers];
        }
        if let Some(pool) = pool.as_ref() {
            for worker in 0..pool.len() {
                if busy[worker] {
                    continue;
                }
                let prefer_refinement =
                    state.complete() || (pool.len() > 1 && worker == pool.len() - 1);
                let target = if prefer_refinement {
                    targets
                        .iter()
                        .find(|t| !state.satisfied(t) && !refining[t.position_index])
                } else {
                    None
                };
                let job = if let Some(target) = target {
                    Some(Job {
                        worker,
                        index: target.position_index,
                        refinement: true,
                        value: state.value(target),
                        multipv: state.multipv(target),
                    })
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
                            blocks[worker].push_back(queue.pop_front().unwrap());
                        }
                    }
                    // A refiner may have paused its block. Let a free triage
                    // worker take those remaining positions rather than idle.
                    let preferred = pool.len() - 1;
                    if blocks[worker].is_empty() && worker != preferred {
                        blocks[worker] = std::mem::take(&mut blocks[preferred]);
                    }
                    blocks[worker].pop_front().map(|index| Job {
                        worker,
                        index,
                        refinement: false,
                        value: profile.triage_ms,
                        multipv: profile.triage_multipv,
                    })
                };
                if let Some(job) = job {
                    let candidates = if job.refinement {
                        refinement_candidates(
                            state.baseline[job.index].as_ref().unwrap(),
                            game.moves.get(job.index).map(|m| m.uci.as_str()),
                        )
                    } else {
                        vec![]
                    };
                    let receiver = pool
                        .submit(
                            worker,
                            game.fens[job.index].clone(),
                            job.value,
                            job.multipv,
                            candidates,
                        )
                        .await?;
                    busy[worker] = true;
                    if job.refinement {
                        refining[job.index] = true;
                    }
                    waiters.spawn(async move { (job, receiver.await) });
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
        if waiters.is_empty() {
            if !state.complete() || state.targets().iter().any(|t| !state.satisfied(t)) {
                return Err(ReviewError::new(
                    "engineExited",
                    "review.queue",
                    "Fila sem worker disponível.",
                ));
            }
            break;
        }
        let result = tokio::select! {
            _ = cancel.cancelled() => return Err(ReviewError::cancelled()),
            result = waiters.join_next() => result.unwrap().map_err(|e| ReviewError::new("engineExited", "review.queue", e))?,
        };
        let (job, reply) = result;
        let pool = pool.as_ref().unwrap();
        let completed: Completed = reply
            .map_err(|_| pool.failure().unwrap_or_else(ReviewError::cancelled))?
            .map_err(|e| pool.failure().unwrap_or(e))?
            .ok_or_else(|| {
                ReviewError::new("missingEvaluation", "review.queue", "Busca não concluída.")
            })?;
        busy[job.worker] = false;
        state.searched += 1;
        let raw = completed.raw;
        // Restricted candidate scores must never satisfy unrestricted cache requests.
        if !job.refinement {
            let buffer = pending
                .iter()
                .position(|(value, multipv, _)| {
                    *value == completed.value && *multipv == job.multipv
                })
                .unwrap_or_else(|| {
                    pending.push((completed.value, job.multipv, vec![]));
                    pending.len() - 1
                });
            pending[buffer].2.push(raw.clone());
        }
        if job.refinement {
            refining[job.index] = false;
            state.budgets[job.index] = Some(SearchBudget {
                ms: completed.value,
                multipv: job.multipv,
            });
            state.refined[job.index] = Some(raw.clone());
        } else {
            state.baseline[job.index] = Some(raw.clone());
            state.discover(job.index);
        }
        state.progress(
            job.index,
            state.refined[job.index].as_ref().or(Some(&raw)),
            emit,
        );
    }
    flush(repository, pending).await?;
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
    let raw: Vec<_> = state
        .refined
        .iter()
        .zip(&state.baseline)
        .map(|(refined, baseline)| refined.as_ref().or(baseline.as_ref()).unwrap().clone())
        .collect();
    core::build(game, &raw)
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

/// Keep all baseline roots and the played move, preserving baseline priority.
fn refinement_candidates(baseline: &RawPosition, played: Option<&str>) -> Vec<String> {
    let mut candidates = Vec::new();
    for candidate in baseline
        .lines
        .iter()
        .filter_map(|line| line.pv.first().map(String::as_str))
        .chain(baseline.pv.first().map(String::as_str))
        .chain(played)
    {
        if !candidates.iter().any(|existing| existing == candidate) {
            candidates.push(candidate.to_owned());
        }
    }
    candidates
}

#[cfg(test)]
mod tests;
