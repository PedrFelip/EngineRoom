//! Mixed automatic queue. Baseline scores remain immutable for adaptive ranking.
use super::{
    adaptive::{self, Profile, SearchBudget, Target},
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
    ready_targets: Vec<Target>,
}
impl State<'_> {
    fn complete(&self) -> bool {
        self.baseline.iter().all(Option::is_some)
    }
    fn targets(&self) -> &[Target] {
        self.final_targets.as_deref().unwrap_or(&self.ready_targets)
    }
    fn satisfied(&self, target: &Target) -> bool {
        self.budgets[target.position_index]
            .is_some_and(|b| b.ms >= target.search.ms && b.multipv >= target.search.multipv)
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
        terminals: game.positions.iter().map(core::terminal_position).collect(),
        baseline: vec![None; count],
        refined: vec![None; count],
        budgets: vec![None; count],
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
        if let Err(error) = pool.close().await {
            if run.is_err() {
                emit(Event::Warning { error });
            } else {
                run = Err(error);
            }
        }
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

    let mut hits = cancellable(
        repository.lookup(
            &game.fens,
            Mode::Time,
            profile.triage_ms,
            profile.triage_multipv,
        ),
        cancel,
    )
    .await?;
    if hits.len() != count {
        return Err(invalid_cache_count());
    }
    let mut queue = VecDeque::new();
    for i in 0..count {
        cancel.check()?;
        state.baseline[i] = if let Some(cp) = state.terminals[i] {
            Some(core::terminal_raw(&game.fens[i], cp))
        } else if let Some(hit) = hits[i].take() {
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
    let mut cache_checked = vec![None; count];
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
        let targets = state.targets().to_vec();
        if state.complete() && !state.refinement_stage {
            state.refinement_stage = true;
            if let Some(target) = targets.first() {
                state.progress(target.position_index, None, emit);
            }
        }
        // Query only selected positions, grouped by budget. Recheck only if a
        // later overlapping decision raises the required coverage.
        let mut groups: Vec<(SearchBudget, Vec<usize>)> = vec![];
        for target in &targets {
            let i = target.position_index;
            if state.satisfied(target) || refining[i] || cache_checked[i] == Some(target.search) {
                continue;
            }
            let slot = groups
                .iter()
                .position(|(budget, _)| *budget == target.search)
                .unwrap_or_else(|| {
                    groups.push((target.search, vec![]));
                    groups.len() - 1
                });
            groups[slot].1.push(i);
        }
        for (budget, indexes) in groups {
            let fens: Vec<_> = indexes.iter().map(|&i| game.fens[i].clone()).collect();
            let hits = cancellable(
                repository.lookup(&fens, Mode::Time, budget.ms, budget.multipv),
                cancel,
            )
            .await?;
            if hits.len() != indexes.len() {
                return Err(invalid_cache_count());
            }
            for (i, hit) in indexes.into_iter().zip(hits) {
                cache_checked[i] = Some(budget);
                if let Some(raw) = hit {
                    state.budgets[i] = Some(budget);
                    state.refined[i] = Some(raw);
                    state.cached += 1;
                    state.progress(i, Some(state.evaluation(i)?), emit);
                }
            }
        }
        let needs_refinement = targets
            .iter()
            .any(|t| !state.satisfied(t) && !refining[t.position_index]);
        if pool.is_none() && (!queue.is_empty() || needs_refinement) {
            let acquired = Pool::acquire(factory, sizing, count, cancel).await?;
            let workers = acquired.len();
            *pool = Some(acquired);
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
                        value: target.search.ms,
                        multipv: target.search.multipv,
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
                    blocks[worker].pop_front().map(|index| Job {
                        worker,
                        index,
                        refinement: false,
                        value: profile.triage_ms,
                        multipv: profile.triage_multipv,
                    })
                };
                if let Some(job) = job {
                    let receiver = pool
                        .submit(worker, game.fens[job.index].clone(), job.value, job.multipv)
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
                    ReviewErrorCode::EngineExited,
                    "review.queue",
                    "Fila sem worker disponível.",
                ));
            }
            break;
        }
        let result = tokio::select! {
            _ = cancel.cancelled() => return Err(ReviewError::cancelled()),
            result = waiters.join_next() => result.ok_or_else(|| queue_error("Fila encerrou sem resultado."))?.map_err(|e| ReviewError::new(ReviewErrorCode::EngineExited, "review.queue", e))?,
        };
        let (job, reply) = result;
        let pool = pool
            .as_ref()
            .ok_or_else(|| queue_error("Pool indisponível."))?;
        let failure = pool.failure()?;
        let completed: Completed = reply
            .map_err(|_| disconnected_worker_error(failure.clone(), cancel))?
            .map_err(|e| failure.unwrap_or(e))?;
        busy[job.worker] = false;
        state.searched += 1;
        let raw = completed.raw;
        // Both stages use unrestricted searches and can populate the FEN cache.
        let buffer = pending
            .iter()
            .position(|(value, multipv, _)| *value == completed.value && *multipv == job.multipv)
            .unwrap_or_else(|| {
                pending.push((completed.value, job.multipv, vec![]));
                pending.len() - 1
            });
        pending[buffer].2.push(raw.clone());
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
        state.progress(job.index, Some(state.evaluation(job.index)?), emit);
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
    let raw: Vec<_> = (0..count)
        .map(|i| state.evaluation(i).cloned())
        .collect::<Result<_>>()?;
    let mut result = core::build(game, &raw)?;
    for (i, position) in result.positions.iter_mut().enumerate() {
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
    Ok(result)
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
