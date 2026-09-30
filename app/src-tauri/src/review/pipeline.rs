use super::{
    adaptive, core,
    engine::{self, Cancellation, EngineFactory, EnginePort},
    repository::Repository,
    scoring,
    types::*,
};
use crate::db::mode::Mode;
use std::sync::Arc;

pub struct Pipeline {
    factory: Arc<dyn EngineFactory>,
    repository: Arc<dyn Repository>,
    port: Option<Box<dyn EnginePort>>,
    settings: Option<(u32, u32, u32)>,
    detected: Option<(u32, u32)>,
}
impl Pipeline {
    pub fn new(factory: Arc<dyn EngineFactory>, repository: Arc<dyn Repository>) -> Self {
        Self {
            factory,
            repository,
            port: None,
            settings: None,
            detected: None,
        }
    }
    pub async fn discard(&mut self) {
        if let Some(mut port) = self.port.take() {
            port.shutdown().await;
        }
        self.settings = None;
    }
    pub async fn close(&mut self) {
        self.discard().await;
        self.repository.drain().await;
    }
    async fn ensure(
        &mut self,
        threads: Option<u32>,
        memory: Option<u32>,
        multipv: u32,
        cancel: &Cancellation,
    ) -> Result<()> {
        if let Some(port) = self.port.as_mut() {
            if self.settings != Some((threads.unwrap_or(1), memory.unwrap_or(16), multipv)) {
                if let Some(n) = threads {
                    port.send(&format!("setoption name Threads value {n}"))?;
                }
                if let Some(n) = memory {
                    port.send(&format!("setoption name Hash value {n}"))?;
                }
                port.send(&format!("setoption name MultiPV value {multipv}"))?;
                engine::ask(port.as_mut(), "isready", "readyok", 10000, cancel).await?;
            }
        } else {
            let port = self.port.insert(self.factory.acquire(cancel).await?);
            engine::configure(port.as_mut(), threads, memory, multipv, cancel).await?;
        }
        self.settings = Some((threads.unwrap_or(1), memory.unwrap_or(16), multipv));
        Ok(())
    }
    pub async fn review(
        &mut self,
        config: &ReviewConfig,
        sizing: Option<(u32, u32)>,
        cancel: &Cancellation,
        emit: &mut (dyn FnMut(Event) + Send),
    ) -> Result<ReviewResult> {
        let pgn = config.pgn.clone();
        let game = tauri::async_runtime::spawn_blocking(move || core::extract(&pgn))
            .await
            .map_err(|e| ReviewError::new("invalidPgn", "pgn", e))??;
        cancel.check()?;
        let phases = core::phases(&game.fens);
        let terminals: Vec<_> = game
            .fens
            .iter()
            .map(|f| core::terminal(f))
            .collect::<Result<_>>()?;
        let profile = adaptive::profile(config.analysis_kind);
        let (mode, value, multipv, stage) = profile
            .map(|p| (Mode::Time, p.triage_ms, p.triage_multipv, "triage"))
            .unwrap_or((
                config.mode,
                if config.mode == Mode::Time {
                    config.movetime_ms.unwrap_or(5000)
                } else {
                    config.engine.depth
                },
                config.lines,
                "analyzing",
            ));
        let mut pending: Vec<(Mode, u32, u32, Vec<RawPosition>)> = Vec::new();
        let run = async {
            let (threads, memory) = sizing.map_or((None, None), |(t, m)| (Some(t), Some(m)));
            self.ensure(threads, memory, multipv, cancel).await?;
            let hits = cancellable(
                self.repository.lookup(&game.fens, mode, value, multipv),
                cancel,
            )
            .await?;
            cancel.check()?;
            if hits.len() != game.fens.len() {
                return Err(ReviewError::new(
                    "cache",
                    "cache.lookup",
                    "Quantidade de avaliações inválida.",
                ));
            }
            pending.push((mode, value, multipv, vec![]));
            let mut raw = Vec::new();
            let mut cached = 0;
            let mut searched = 0;
            let mut remaining = terminals
                .iter()
                .zip(&hits)
                .filter(|(t, h)| t.is_none() && h.is_none())
                .count();
            for (i, fen) in game.fens.iter().enumerate() {
                cancel.check()?;
                if terminals[i].is_none() && hits[i].is_none() {
                    remaining -= 1;
                }
                let pos = if let Some(cp) = terminals[i] {
                    core::terminal_raw(fen, cp)
                } else if let Some(hit) = &hits[i] {
                    cached += 1;
                    hit.clone()
                } else {
                    let mut pos = engine::evaluate(
                        self.port.as_mut().unwrap().as_mut(),
                        fen,
                        mode,
                        value,
                        timeout(mode, value),
                        cancel,
                    )
                    .await?;
                    core::add_san(&mut pos);
                    searched += 1;
                    pending[0].3.push(pos.clone());
                    if pending[0].3.len() >= 8 {
                        self.repository
                            .put(&pending[0].3, mode, value, multipv)
                            .await?;
                        pending[0].3.clear();
                    }
                    pos
                };
                emit(Event::Progress {
                    progress: Progress {
                        stage: stage.into(),
                        completed: i + 1,
                        total: game.fens.len(),
                        current_ply: i,
                        phase: Some(phases[i]),
                        cached_positions: cached,
                        engine_positions: searched,
                        remaining_budget_ms: if mode == Mode::Time {
                            Some(remaining as u64 * value as u64)
                        } else {
                            None
                        },
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
            }
            if !pending[0].3.is_empty() {
                self.repository
                    .put(&pending[0].3, mode, value, multipv)
                    .await?;
                pending[0].3.clear();
            }
            if let Some(profile) = profile {
                let targets =
                    adaptive::targets(&adaptive::rank(&game, &raw), game.fens.len(), profile);
                let targets: Vec<_> = targets
                    .into_iter()
                    .filter(|t| terminals[t.position_index].is_none())
                    .collect();
                if !targets.is_empty() {
                    self.ensure(threads, memory, profile.refinement_multipv, cancel)
                        .await?;
                    let budget = |t: &adaptive::Target| {
                        if t.budget == "high" {
                            profile.high_ms
                        } else {
                            profile.medium_ms
                        }
                    };
                    let mut remaining: u64 = targets.iter().map(|t| budget(t) as u64).sum();
                    let mut refined = 0;
                    emit(Event::Progress {
                        progress: Progress {
                            stage: "refinement".into(),
                            completed: 0,
                            total: targets.len(),
                            current_ply: targets[0].position_index,
                            phase: Some(phases[targets[0].position_index]),
                            cached_positions: cached,
                            engine_positions: searched,
                            remaining_budget_ms: Some(remaining),
                            update: None,
                        },
                    });
                    for kind in ["high", "medium"] {
                        let group: Vec<_> = targets.iter().filter(|t| t.budget == kind).collect();
                        if group.is_empty() {
                            continue;
                        }
                        let value = budget(group[0]);
                        let multipv = profile.refinement_multipv;
                        let fens: Vec<_> = group
                            .iter()
                            .map(|t| game.fens[t.position_index].clone())
                            .collect();
                        let hits = cancellable(
                            self.repository.lookup(&fens, Mode::Time, value, multipv),
                            cancel,
                        )
                        .await?;
                        if hits.len() != fens.len() {
                            return Err(ReviewError::new(
                                "cache",
                                "cache.lookup",
                                "Quantidade de avaliações inválida.",
                            ));
                        }
                        pending.push((Mode::Time, value, multipv, vec![]));
                        let buffer = pending.len() - 1;
                        for (t, hit) in group.iter().zip(hits) {
                            cancel.check()?;
                            let i = t.position_index;
                            let fen = &game.fens[i];
                            let pos = if let Some(hit) = hit {
                                cached += 1;
                                hit
                            } else {
                                let mut pos = engine::evaluate(
                                    self.port.as_mut().unwrap().as_mut(),
                                    fen,
                                    Mode::Time,
                                    value,
                                    timeout(Mode::Time, value),
                                    cancel,
                                )
                                .await?;
                                core::add_san(&mut pos);
                                searched += 1;
                                pending[buffer].3.push(pos.clone());
                                if pending[buffer].3.len() >= 8 {
                                    self.repository
                                        .put(&pending[buffer].3, Mode::Time, value, multipv)
                                        .await?;
                                    pending[buffer].3.clear();
                                }
                                pos
                            };
                            refined += 1;
                            remaining -= value as u64;
                            emit(Event::Progress {
                                progress: Progress {
                                    stage: "refinement".into(),
                                    completed: refined,
                                    total: targets.len(),
                                    current_ply: i,
                                    phase: Some(phases[i]),
                                    cached_positions: cached,
                                    engine_positions: searched,
                                    remaining_budget_ms: Some(remaining),
                                    update: Some(WinPctUpdate {
                                        index: i,
                                        win_pct: scoring::white_win_pct(
                                            pos.cp,
                                            fen.split_whitespace().nth(1) != Some("b"),
                                        ),
                                    }),
                                },
                            });
                            raw[i] = pos;
                        }
                        if !pending[buffer].3.is_empty() {
                            self.repository
                                .put(&pending[buffer].3, Mode::Time, value, multipv)
                                .await?;
                            pending[buffer].3.clear();
                        }
                    }
                }
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
            core::build(&game, &raw)
        }
        .await;
        self.discard().await;
        if run.is_err() {
            for (mode, value, multipv, entries) in &pending {
                if !entries.is_empty() {
                    if let Err(error) = self.repository.put(entries, *mode, *value, *multipv).await
                    {
                        emit(Event::Warning { error });
                    }
                }
            }
        }
        run
    }
    async fn analyze_fen(
        &mut self,
        fen: &str,
        settings: &LiveSettings,
        cancel: &Cancellation,
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
        } else if let Some(cp) = core::terminal(fen)? {
            core::terminal_raw(fen, cp)
        } else {
            let (threads, memory) = if settings.threads_auto {
                self.detected
                    .unwrap_or((settings.threads, settings.memory_mb))
            } else {
                (settings.threads, settings.memory_mb)
            };
            self.ensure(Some(threads), Some(memory), multipv, cancel)
                .await?;
            let port = self.port.as_mut().unwrap().as_mut();
            let mut result =
                engine::evaluate(port, fen, Mode::Time, value, value as u64 + 10000, cancel).await;
            if result
                .as_ref()
                .is_err_and(|e| e.code == "missingEvaluation")
            {
                engine::ask(port, "isready", "readyok", 10000, cancel).await?;
                result =
                    engine::evaluate(port, fen, Mode::Time, value, value as u64 + 10000, cancel)
                        .await;
            }
            let mut raw = result?;
            core::add_san(&mut raw);
            self.repository
                .put(&[raw.clone()], Mode::Time, value, multipv)
                .await?;
            raw
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
        let run = async {
            cancel.check()?;
            emit(Event::LiveStarted {
                fen: request.fen.clone(),
            });
            let after = self.analyze_fen(&request.fen, settings, cancel).await?;
            emit(Event::LiveCompleted {
                fen: request.fen.clone(),
                analysis: after.clone(),
            });
            if settings.move_feedback_enabled {
                if let (Some(id), Some(source_fen)) =
                    (&request.variation_node_id, &request.source_fen)
                {
                    let before = if let Some(source) = request
                        .source_analysis
                        .as_ref()
                        .filter(|s| s.fen == *source_fen)
                    {
                        Some(source.clone())
                    } else if *source_fen == request.fen {
                        Some(after.clone())
                    } else if !settings.fast_pass {
                        Some(self.analyze_fen(source_fen, settings, cancel).await?)
                    } else {
                        None
                    };
                    cancel.check()?;
                    if let Some(before) = before {
                        emit(Event::Classification {
                            node_id: id.clone(),
                            classification: scoring::classify(
                                (scoring::win_pct(before.cp)
                                    - (100.0 - scoring::win_pct(after.cp)))
                                .max(0.0),
                                false,
                            ),
                        });
                    }
                }
            }
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
fn timeout(mode: Mode, value: u32) -> u64 {
    if mode == Mode::Depth {
        180000
    } else {
        value as u64 * 3 + 10000
    }
}
async fn cancellable<T>(
    task: super::engine::Task<'_, Result<T>>,
    cancel: &Cancellation,
) -> Result<T> {
    tokio::select! { _ = cancel.cancelled() => Err(ReviewError::cancelled()), result = task => result }
}
