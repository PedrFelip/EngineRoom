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
            let next = (threads.unwrap_or(1), memory.unwrap_or(16), multipv);
            let previous = self.settings;
            if previous != Some(next) {
                if previous.map(|s| s.0) != Some(next.0) {
                    port.send(&format!("setoption name Threads value {}", next.0))?;
                }
                if previous.map(|s| s.1) != Some(next.1) {
                    port.send(&format!("setoption name Hash value {}", next.1))?;
                }
                if previous.map(|s| s.2) != Some(next.2) {
                    port.send(&format!("setoption name MultiPV value {multipv}"))?;
                }
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
        let game = tokio::task::spawn_blocking(move || core::extract(&pgn))
            .await
            .map_err(|e| ReviewError::new(ReviewErrorCode::InvalidPgn, "pgn", e))??;
        cancel.check()?;
        if let Some(profile) = adaptive::profile(config.analysis_kind) {
            self.discard().await;
            return super::automatic::review(
                self.factory.as_ref(),
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
            config.movetime_ms.unwrap_or(5000)
        } else {
            config.engine.depth
        };
        let multipv = config.lines;
        let mut pending = Vec::new();
        let run = async {
            let hits = cancellable(
                self.repository.lookup(&game.fens, mode, value, multipv),
                cancel,
            )
            .await?;
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
            let mut remaining = hits
                .iter()
                .zip(&terminals)
                .filter(|(h, t)| h.is_none() && t.is_none())
                .count();
            let mut raw = Vec::new();
            let mut cached = 0;
            let mut searched = 0;
            for (i, fen) in game.fens.iter().enumerate() {
                cancel.check()?;
                let pos = if let Some(cp) = terminals[i] {
                    core::terminal_raw(fen, cp)
                } else if let Some(hit) = &hits[i] {
                    cached += 1;
                    hit.clone()
                } else {
                    let (threads, memory) = sizing.unwrap_or((1, 16));
                    self.ensure(Some(threads), Some(memory), multipv, cancel)
                        .await?;
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
                    pending.push(pos.clone());
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
                if pending.len() >= 8 {
                    self.repository.put(&pending, mode, value, multipv).await?;
                    pending.clear();
                }
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
            core::build(&game, &raw)
        }
        .await;
        self.discard().await;
        if run.is_err() && !pending.is_empty() {
            if let Err(error) = self.repository.put(&pending, mode, value, multipv).await {
                emit(Event::Warning { error });
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
                .is_err_and(|e| e.code == ReviewErrorCode::MissingEvaluation)
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
