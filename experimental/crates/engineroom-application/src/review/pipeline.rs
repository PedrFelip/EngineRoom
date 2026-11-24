use super::{
    adaptive,
    automatic::{LiveDemand, scheduler::fen_key},
    contextual::ContextualReview,
    core,
    engine::{self, Cancellation, EngineFactory},
    repository::Repository,
    scoring,
    types::*,
};
use crate::db::mode::Mode;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::{mpsc, oneshot, watch};

mod owned_engine;
use owned_engine::{EngineSettings, OwnedEngine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativePhase {
    Running,
    Ready,
    Failed,
}

#[derive(Clone, Copy)]
pub(super) struct NativeLiveClient<'a> {
    pub demands: &'a mpsc::Sender<LiveDemand>,
    pub phase: &'a watch::Receiver<NativePhase>,
}

enum NativeOutcome {
    Evaluated(RawPosition),
    FallbackToOwnedEngine,
}

pub struct Pipeline {
    engine: OwnedEngine,
    repository: Arc<dyn Repository>,
    detected: Option<(u32, u32)>,
}
impl Pipeline {
    pub fn new(factory: Arc<dyn EngineFactory>, repository: Arc<dyn Repository>) -> Self {
        Self {
            engine: OwnedEngine::new(factory),
            repository,
            detected: None,
        }
    }
    pub(super) fn factory_arc(&self) -> Arc<dyn EngineFactory> {
        self.engine.factory_arc()
    }
    pub(super) fn repository_arc(&self) -> Arc<dyn Repository> {
        self.repository.clone()
    }
    /// Awaits engine teardown without draining repository work.
    pub async fn discard(&mut self) {
        self.engine.discard().await;
    }
    /// Awaits engine teardown and repository drain before session close acknowledgment.
    pub async fn close(&mut self) {
        self.discard().await;
        self.repository.drain().await;
    }
    pub async fn review(
        &mut self,
        config: &ReviewConfig,
        sizing: Option<(u32, u32)>,
        cancel: &Cancellation,
        emit: &mut (dyn FnMut(Event) + Send),
    ) -> Result<ReviewResult> {
        let pgn = config.pgn.clone();
        let game = tokio::task::spawn_blocking(move || core::extract(&pgn))
            .await
            .map_err(|e| ReviewError::with_source(ReviewErrorCode::InvalidPgn, "pgn", e))??;
        cancel.check()?;
        if let Some(profile) = adaptive::profile(config.analysis_kind)? {
            self.discard().await;
            return super::automatic::review(
                self.engine.factory(),
                self.repository.as_ref(),
                &game,
                profile,
                sizing,
                cancel,
                emit,
            )
            .await;
        }
        let phases = core::phases(&game.fens);
        let mode = config.mode;
        let value = if mode == Mode::Time {
            config.movetime_ms.unwrap_or(DEFAULT_MOVETIME_MS)
        } else {
            config.engine.depth
        };
        let multipv = config.lines;
        let context = self
            .engine
            .factory()
            .artifact_signature()
            .await?
            .map(|signature| ContextualReview::new(&game, &signature, sizing));
        cancel.check()?;
        let mut contextual_pending = Vec::new();
        let mut pending = Vec::new();
        let mut evidence = vec![None; game.fens.len()];
        let run = async {
            let hits = if let Some(context) = &context {
                let indexes: Vec<_> = (0..game.fens.len()).collect();
                cancellable(
                    Box::pin(context.lookup(
                        self.repository.as_ref(),
                        &indexes,
                        mode,
                        value,
                        multipv,
                    )),
                    cancel,
                )
                .await?
                .into_iter()
                .enumerate()
                .map(|(i, hit)| {
                    hit.map(|mut hit| {
                        hit.evidence.origin = EvidenceOrigin::Cache;
                        evidence[i] = Some(hit.evidence);
                        hit.raw
                    })
                })
                .collect()
            } else {
                cancellable(
                    self.repository.lookup(&game.fens, mode, value, multipv),
                    cancel,
                )
                .await?
            };
            if hits.len() != game.fens.len() {
                return Err(ReviewError::new(
                    ReviewErrorCode::Cache,
                    "cache.lookup",
                    "Quantidade de avaliações inválida.",
                ));
            }
            let terminals = game
                .positions
                .iter()
                .map(core::terminal_position)
                .collect::<Vec<_>>();
            let position_key = |i: usize| (context.as_ref().map(|_| i), fen_key(&game.fens[i]));
            let mut evaluations: HashMap<_, _> = game
                .fens
                .iter()
                .zip(&terminals)
                .zip(hits)
                .enumerate()
                .filter_map(|(i, ((_, terminal), hit))| {
                    terminal
                        .is_none()
                        .then_some(hit)
                        .flatten()
                        .map(|raw| (position_key(i), raw))
                })
                .collect();
            let mut remaining = game
                .fens
                .iter()
                .zip(&terminals)
                .enumerate()
                .filter_map(|(i, (_, terminal))| terminal.is_none().then_some(position_key(i)))
                .filter(|key| !evaluations.contains_key(key))
                .collect::<HashSet<_>>()
                .len();
            let mut raw = Vec::new();
            let mut cached = 0;
            let mut searched = 0;
            for (i, fen) in game.fens.iter().enumerate() {
                cancel.check()?;
                let pos = if let Some(cp) = terminals[i] {
                    let raw = core::terminal_raw(fen, cp);
                    if context.is_some() {
                        evidence[i] = Some(PositionEvidence::from_raw(
                            &raw,
                            Some(i),
                            EvidenceOrigin::Terminal,
                            None,
                            None,
                        ));
                    }
                    raw
                } else if let Some(hit) = evaluations.get(&position_key(i)) {
                    cached += 1;
                    let mut pos = hit.clone();
                    pos.fen.clone_from(fen);
                    pos
                } else {
                    let (threads, memory_mb) = sizing.unwrap_or((1, 16));
                    let port = self
                        .engine
                        .ensure(
                            EngineSettings {
                                threads,
                                memory_mb,
                                multipv,
                            },
                            cancel,
                        )
                        .await?;
                    let mut pos = if context.is_some() {
                        let context = ContextualReview::position(&game, i)?;
                        engine::evaluate_position(
                            port,
                            Some(&context.initial_fen),
                            &context.uci_path,
                            fen,
                            mode,
                            value,
                            timeout(mode, value),
                            cancel,
                        )
                        .await?
                    } else {
                        engine::evaluate(port, fen, mode, value, timeout(mode, value), cancel)
                            .await?
                    };
                    core::add_san(&mut pos);
                    evaluations.insert(position_key(i), pos.clone());
                    if let Some(context) = &context {
                        let entry = context.entry(
                            i,
                            &pos,
                            self.engine.identity(),
                            EvidenceBudget {
                                mode,
                                value,
                                multipv,
                            },
                        );
                        evidence[i] = Some(entry.evidence.clone());
                        contextual_pending.push(entry);
                    } else {
                        pending.push(pos.clone());
                    }
                    searched += 1;
                    remaining -= 1;
                    pos
                };
                emit(Event::Progress {
                    progress: Progress {
                        stage: "analyzing".into(),
                        completed: i + 1,
                        total: game.fens.len(),
                        current_ply: i,
                        phase: Some(phases[i]),
                        cached_positions: cached,
                        engine_positions: searched,
                        remaining_budget_ms: (mode == Mode::Time)
                            .then_some(remaining as u64 * value as u64),
                        update: Some(WinPctUpdate {
                            index: i,
                            win_pct: scoring::white_win_pct(
                                pos.cp,
                                fen.split_whitespace().nth(1) != Some("b"),
                            ),
                        }),
                    },
                });
                raw.push(pos);
                if contextual_pending.len() >= 8 {
                    self.repository
                        .put_context(&contextual_pending, mode, value)
                        .await?;
                    contextual_pending.clear();
                }
                if pending.len() >= 8 {
                    self.repository.put(&pending, mode, value, multipv).await?;
                    pending.clear();
                }
            }
            if !contextual_pending.is_empty() {
                self.repository
                    .put_context(&contextual_pending, mode, value)
                    .await?;
                contextual_pending.clear();
            }
            if !pending.is_empty() {
                self.repository.put(&pending, mode, value, multipv).await?;
                pending.clear();
            }
            cancel.check()?;
            emit(Event::Progress {
                progress: Progress {
                    stage: "finalizing".into(),
                    completed: raw.len(),
                    total: raw.len(),
                    current_ply: raw.len() - 1,
                    phase: phases.last().copied(),
                    cached_positions: cached,
                    engine_positions: searched,
                    remaining_budget_ms: None,
                    update: None,
                },
            });
            let mut result = core::build(&game, &raw)?;
            for (position, evidence) in result.positions.iter_mut().zip(evidence) {
                position.evidence = evidence;
            }
            Ok(result)
        }
        .await;
        self.discard().await;
        if run.is_err()
            && !pending.is_empty()
            && let Err(error) = self.repository.put(&pending, mode, value, multipv).await
        {
            emit(Event::Warning { error });
        }
        if run.is_err()
            && !contextual_pending.is_empty()
            && let Err(error) = self
                .repository
                .put_context(&contextual_pending, mode, value)
                .await
        {
            emit(Event::Warning { error });
        }
        run
    }
    async fn evaluate_owned_live(
        &mut self,
        fen: &str,
        value: u32,
        multipv: u32,
        settings: &LiveSettings,
        cancel: &Cancellation,
    ) -> Result<RawPosition> {
        let (threads, memory) = if settings.threads_auto {
            self.detected
                .unwrap_or((settings.threads, settings.memory_mb))
        } else {
            (settings.threads, settings.memory_mb)
        };
        let port = self
            .engine
            .ensure(
                EngineSettings {
                    threads,
                    memory_mb: memory,
                    multipv,
                },
                cancel,
            )
            .await?;
        let mut result =
            engine::evaluate(port, fen, Mode::Time, value, value as u64 + 10000, cancel).await;
        if result
            .as_ref()
            .is_err_and(|e| e.code == ReviewErrorCode::MissingEvaluation)
        {
            engine::ask(port, "isready", "readyok", 10000, cancel).await?;
            result =
                engine::evaluate(port, fen, Mode::Time, value, value as u64 + 10000, cancel).await;
        }
        let mut raw = result?;
        core::add_san(&mut raw);
        self.repository
            .put(std::slice::from_ref(&raw), Mode::Time, value, multipv)
            .await?;
        Ok(raw)
    }
    async fn analyze_fen(
        &mut self,
        fen: &str,
        settings: &LiveSettings,
        cancel: &Cancellation,
        native: Option<NativeLiveClient<'_>>,
    ) -> Result<PositionAnalysis> {
        cancel.check()?;
        let value = if settings.fast_pass {
            500
        } else {
            settings.search_seconds * 1000
        };
        let multipv = if settings.fast_pass {
            1
        } else {
            settings.lines
        };
        let hits = cancellable(
            self.repository
                .lookup(&[fen.into()], Mode::Time, value, multipv),
            cancel,
        )
        .await?;
        cancel.check()?;
        let raw = if let Some(hit) = hits.into_iter().next().flatten() {
            hit
        } else if fen.contains(['\r', '\n']) {
            return Err(ReviewError::new(
                ReviewErrorCode::InvalidPgn,
                "review.live",
                "FEN inválido.",
            ));
        } else if let Some(cp) = core::terminal(fen)? {
            core::terminal_raw(fen, cp)
        } else if let Some(client) = native {
            match request_native_live(client, fen, value, multipv, cancel).await? {
                NativeOutcome::Evaluated(mut raw) => {
                    if raw
                        .lines
                        .iter()
                        .any(|line| line.san.is_none() && !line.pv.is_empty())
                    {
                        core::add_san(&mut raw);
                    }
                    raw
                }
                NativeOutcome::FallbackToOwnedEngine => {
                    let flushed = cancellable(
                        self.repository
                            .lookup(&[fen.into()], Mode::Time, value, multipv),
                        cancel,
                    )
                    .await?;
                    cancel.check()?;
                    if let Some(hit) = flushed.into_iter().next().flatten() {
                        hit
                    } else {
                        self.evaluate_owned_live(fen, value, multipv, settings, cancel)
                            .await?
                    }
                }
            }
        } else {
            self.evaluate_owned_live(fen, value, multipv, settings, cancel)
                .await?
        };
        cancel.check()?;
        let mut analysis = core::position_analysis(&raw, 0, core::phase(fen));
        analysis.search = Some(Search {
            purpose: if settings.fast_pass {
                "playback"
            } else {
                "refinement"
            }
            .into(),
            movetime_ms: value,
            multipv,
        });
        Ok(analysis)
    }
    pub fn set_detected_resources(&mut self, threads: u32, memory_mb: u32) {
        self.detected = Some((threads.div_ceil(3).max(1), hash_mb(memory_mb)));
    }
    pub async fn live(
        &mut self,
        request: &LiveRequest,
        settings: &LiveSettings,
        cancel: &Cancellation,
        emit: &mut (dyn FnMut(Event) + Send),
    ) -> Result<()> {
        self.live_with_native(request, settings, cancel, None, emit)
            .await
    }
    pub(super) async fn live_with_native(
        &mut self,
        request: &LiveRequest,
        settings: &LiveSettings,
        cancel: &Cancellation,
        native: Option<NativeLiveClient<'_>>,
        emit: &mut (dyn FnMut(Event) + Send),
    ) -> Result<()> {
        let run = async {
            cancel.check()?;
            emit(Event::LiveStarted {
                fen: request.fen.clone(),
            });
            let after = self
                .analyze_fen(&request.fen, settings, cancel, native)
                .await?;
            let after_cp = after.cp;
            if settings.move_feedback_enabled
                && let (Some(id), Some(source_fen)) =
                    (&request.variation_node_id, &request.source_fen)
            {
                let before_cp = if let Some(source) = request
                    .source_analysis
                    .as_ref()
                    .filter(|s| s.fen == *source_fen)
                {
                    Some(source.cp)
                } else if *source_fen == request.fen {
                    Some(after_cp)
                } else if !settings.fast_pass {
                    Some(
                        self.analyze_fen(source_fen, settings, cancel, native)
                            .await?
                            .cp,
                    )
                } else {
                    None
                };
                cancel.check()?;
                if let Some(before_cp) = before_cp {
                    emit(Event::Classification {
                        node_id: id.clone(),
                        classification: scoring::classify(
                            (scoring::win_pct(before_cp) - (100.0 - scoring::win_pct(after_cp)))
                                .max(0.0),
                            false,
                        ),
                    });
                }
            }
            emit(Event::LiveCompleted {
                fen: request.fen.clone(),
                analysis: after,
            });
            Ok(())
        }
        .await;
        if run.is_err() {
            self.discard().await;
        }
        run
    }
    pub async fn save(&mut self, config: &ReviewConfig, result: &ReviewResult) -> Result<()> {
        self.repository.save(config, result).await
    }
}
pub fn hash_mb(memory: u32) -> u32 {
    ((memory as f64 * 0.2).floor() as u32).clamp(512, 4096)
}
pub(super) fn timeout(mode: Mode, value: u32) -> u64 {
    if mode == Mode::Depth {
        180000
    } else {
        value as u64 * 3 + 10000
    }
}
pub(super) async fn cancellable<T>(
    task: super::engine::Task<'_, Result<T>>,
    cancel: &Cancellation,
) -> Result<T> {
    tokio::select! { _ = cancel.cancelled() => Err(ReviewError::cancelled()), result = task => result }
}

async fn request_native_live(
    client: NativeLiveClient<'_>,
    fen: &str,
    value: u32,
    multipv: u32,
    cancel: &Cancellation,
) -> Result<NativeOutcome> {
    match *client.phase.borrow() {
        NativePhase::Ready => return Ok(NativeOutcome::FallbackToOwnedEngine),
        NativePhase::Failed => return Err(ReviewError::cancelled()),
        NativePhase::Running => {}
    }
    let (reply, rx) = oneshot::channel();
    let demand = LiveDemand {
        fen: fen.to_owned(),
        value,
        multipv,
        reply,
    };
    let sent = tokio::select! {
        _ = cancel.cancelled() => return Err(ReviewError::cancelled()),
        result = client.demands.send(demand) => result.is_ok(),
    };
    if sent {
        let received = tokio::select! {
            _ = cancel.cancelled() => return Err(ReviewError::cancelled()),
            result = rx => result,
        };
        if let Ok(result) = received {
            return result.map(NativeOutcome::Evaluated);
        }
    }
    cancel.check()?;
    let mut phase = client.phase.clone();
    let settled = tokio::select! {
        _ = cancel.cancelled() => return Err(ReviewError::cancelled()),
        result = phase.wait_for(|current| *current != NativePhase::Running) => {
            result.map(|current| *current).unwrap_or(NativePhase::Failed)
        }
    };
    match settled {
        NativePhase::Ready => Ok(NativeOutcome::FallbackToOwnedEngine),
        NativePhase::Failed | NativePhase::Running => Err(ReviewError::cancelled()),
    }
}
