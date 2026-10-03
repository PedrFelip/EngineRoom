use super::{core, engine::Task, scoring::*, types::*};
use crate::db::{
    cache::{Cache, CachedPosition, CachedPositionPut},
    games::{NewGame, Store},
    mode::Mode,
    DbState,
};
use shakmaty::Position;
use tauri::Manager;

pub trait Repository: Send + Sync {
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Task<'a, Result<Vec<Option<RawPosition>>>>;
    fn put<'a>(
        &'a self,
        entries: &'a [RawPosition],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Task<'a, Result<()>>;
    fn save<'a>(
        &'a self,
        config: &'a ReviewConfig,
        review: &'a ReviewResult,
    ) -> Task<'a, Result<()>>;
    fn drain(&self) -> Task<'_, ()> {
        Box::pin(async {})
    }
}
pub struct SqliteRepository<R: tauri::Runtime = tauri::Wry> {
    app: tauri::AppHandle<R>,
    active: std::sync::Arc<tokio::sync::watch::Sender<usize>>,
}
struct Operation(std::sync::Arc<tokio::sync::watch::Sender<usize>>);

#[cfg(test)]
mod operation_tests;

impl Drop for Operation {
    fn drop(&mut self) {
        self.0.send_modify(|n| *n -= 1);
    }
}
impl<R: tauri::Runtime> SqliteRepository<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        let (active, _) = tokio::sync::watch::channel(0);
        Self {
            app,
            active: std::sync::Arc::new(active),
        }
    }
    async fn db<T: Send + 'static>(
        &self,
        operation: &'static str,
        code: ReviewErrorCode,
        run: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static,
    ) -> Result<T> {
        let app = self.app.clone();
        self.active.send_modify(|n| *n += 1);
        let operation_guard = Operation(self.active.clone());
        tauri::async_runtime::spawn_blocking(move || {
            let _operation = operation_guard;
            let state = app.state::<DbState>();
            let conn = state
                .0
                .lock()
                .map_err(|e| ReviewError::new(code, operation, e))?;
            run(&conn).map_err(|e| ReviewError::with_source(code, operation, e))
        })
        .await
        .map_err(|e| ReviewError::with_source(code, operation, e))?
    }
}
pub fn shape(hit: CachedPosition, fen: &str, multipv: u32) -> Option<RawPosition> {
    let mut lines: Vec<RawLine> = serde_json::from_str(&hit.lines_json).ok()?;
    let required = multipv.min(core::position(fen).ok()?.legal_moves().len() as u32);
    lines.sort_by_key(|line| line.multipv);
    if required == 0
        || lines.len() < required as usize
        || lines
            .iter()
            .enumerate()
            .any(|(i, line)| line.multipv != i as u32 + 1)
    {
        return None;
    }
    lines.truncate(required as usize);
    let principal = lines.first()?;
    if principal.cp != hit.cp {
        return None;
    }
    Some(RawPosition {
        fen: fen.into(),
        cp: hit.cp,
        depth: principal.depth.unwrap_or(hit.reached_depth),
        pv: principal.pv.clone(),
        lines,
    })
}
impl<R: tauri::Runtime> Repository for SqliteRepository<R> {
    fn drain(&self) -> Task<'_, ()> {
        Box::pin(async move {
            let mut active = self.active.subscribe();
            let _ = active.wait_for(|n| *n == 0).await;
        })
    }
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Task<'a, Result<Vec<Option<RawPosition>>>> {
        Box::pin(async move {
            let owned = fens.to_vec();
            let hits = self
                .db("cache.lookup", ReviewErrorCode::Cache, move |c| {
                    Cache::new(c).lookup_bulk(&owned, mode, value, multipv)
                })
                .await?;
            Ok(hits
                .into_iter()
                .zip(fens)
                .map(|(hit, f)| {
                    hit.and_then(|h| shape(h, f, multipv)).filter(|raw| {
                        mode != Mode::Depth
                            || raw
                                .lines
                                .iter()
                                .all(|line| line.depth.unwrap_or(raw.depth) >= value)
                    })
                })
                .collect())
        })
    }
    fn put<'a>(
        &'a self,
        entries: &'a [RawPosition],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Task<'a, Result<()>> {
        Box::pin(async move {
            // A short search may not publish every requested PV. Store only
            // contiguous, scored lines, at the minimum depth they all reached.
            let mut writes = Vec::with_capacity(entries.len());
            for position in entries {
                let legal = core::position(&position.fen)?.legal_moves().len();
                let mut lines = position.lines.clone();
                lines.sort_by_key(|line| line.multipv);
                let coverage = lines
                    .iter()
                    .enumerate()
                    .take_while(|(i, line)| line.multipv == *i as u32 + 1)
                    .count()
                    .min(multipv as usize);
                if coverage == 0 {
                    continue;
                }
                lines.truncate(coverage);
                let reached_depth = lines
                    .iter()
                    .map(|line| line.depth.unwrap_or(position.depth))
                    .min()
                    .unwrap();
                // MultiPV beyond the legal move count is fully covered once
                // every legal move has a line.
                let advertised = if coverage >= legal {
                    multipv
                } else {
                    coverage as u32
                };
                writes.push((
                    CachedPositionPut {
                        fen: position.fen.clone(),
                        reached_depth,
                        cp: lines[0].cp,
                        lines_json: serde_json::to_string(&lines).map_err(|e| {
                            ReviewError::with_source(ReviewErrorCode::Cache, "cache.encode", e)
                        })?,
                    },
                    advertised,
                ));
            }
            self.db("cache.put", ReviewErrorCode::Cache, move |c| {
                Cache::new(c).store_many_covered(&writes, mode, value)
            })
            .await
        })
    }
    fn save<'a>(
        &'a self,
        config: &'a ReviewConfig,
        review: &'a ReviewResult,
    ) -> Task<'a, Result<()>> {
        Box::pin(async move {
            let game = core::extract(&config.pgn)?;
            let profile = super::adaptive::profile(config.analysis_kind);
            let value = profile.map(|p| p.high_ms).unwrap_or_else(|| {
                if config.mode == Mode::Time {
                    config.movetime_ms.unwrap_or(0)
                } else {
                    config.engine.depth
                }
            });
            let entry = NewGame {
                pgn: config.pgn.clone(),
                white: game.meta.white,
                black: game.meta.black,
                result: game.meta.result,
                plies: game.moves.len() as u32,
                engine_tier: if config.analysis_kind == AnalysisKind::Manual {
                    config.engine.id.clone()
                } else {
                    config.analysis_kind.as_str().into()
                },
                mode: config.mode,
                analysis_kind: config.analysis_kind.as_str().into(),
                depth: value,
                multipv: config.lines,
                accuracy_white: review.accuracy.white,
                accuracy_black: review.accuracy.black,
                review_json: serde_json::to_string(review).map_err(|e| {
                    ReviewError::with_source(ReviewErrorCode::Persistence, "games.encode", e)
                })?,
            };
            self.db("games.save", ReviewErrorCode::Persistence, move |c| {
                Store::new(c).save(&entry).map(|_| ())
            })
            .await
        })
    }
}
// Legacy data is normalized only at the Rust boundary, using persisted scores.
pub fn normalize(mut value: serde_json::Value) -> Result<ReviewResult> {
    let fail = || {
        ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "games.normalize",
            "Revisão salva inválida.",
        )
    };
    let old_positions = value
        .get("positions")
        .and_then(|v| v.as_array())
        .ok_or_else(fail)?;
    let fens: Vec<_> = old_positions
        .iter()
        .map(|v| v.get("fen").and_then(|v| v.as_str()).ok_or_else(fail))
        .collect::<Result<_>>()?;
    let phases = core::phases(fens);
    let current = value.get("accuracyModel").and_then(|v| v.as_str()) == Some(ACCURACY_MODEL)
        && value.get("accuracyByPhase").is_some()
        && old_positions.iter().all(|p| p.get("phase").is_some())
        && value
            .get("moves")
            .and_then(|v| v.as_array())
            .is_some_and(|ms| {
                ms.iter().all(|m| {
                    m.get("cpLoss").is_some()
                        && !matches!(
                            m.get("classification").and_then(|v| v.as_str()),
                            Some("brilhante" | "otimo")
                        )
                })
            });
    for (i, p) in value["positions"]
        .as_array_mut()
        .ok_or_else(fail)?
        .iter_mut()
        .enumerate()
    {
        if p.get("phase").is_none() {
            p["phase"] = serde_json::to_value(phases[i]).map_err(|_| fail())?;
        }
    }
    let cps: Vec<_> = value["positions"]
        .as_array()
        .ok_or_else(fail)?
        .iter()
        .map(|p| p["cp"].as_f64().ok_or_else(fail))
        .collect::<Result<_>>()?;
    for m in value["moves"].as_array_mut().ok_or_else(fail)? {
        let ply = m["ply"].as_u64().filter(|p| *p > 0).ok_or_else(fail)? as usize;
        if matches!(m["classification"].as_str(), Some("brilhante" | "otimo")) {
            m["classification"] = serde_json::to_value(classify(
                m["winPctLoss"].as_f64().ok_or_else(fail)?,
                m["isBook"].as_bool().ok_or_else(fail)?,
            ))
            .map_err(|_| fail())?;
        }
        if m.get("cpLoss").is_none() {
            m["cpLoss"] = serde_json::json!(cps
                .get(ply - 1)
                .zip(cps.get(ply))
                .map_or(0.0, |(a, b)| (a + b).max(0.0)));
        }
    }
    value["accuracyModel"] = serde_json::json!(ACCURACY_MODEL);
    if !current {
        value["accuracyByPhase"] = serde_json::to_value(PhaseAccuracy {
            opening: accuracy(&[], &[50.0]),
            middlegame: accuracy(&[], &[50.0]),
            endgame: accuracy(&[], &[50.0]),
        })
        .map_err(|_| fail())?;
    }
    let mut review: ReviewResult = serde_json::from_value(value).map_err(|_| fail())?;
    validate_review(&review)?;
    if !current {
        let values: Vec<_> = review.positions.iter().map(|p| p.win_pct).collect();
        review.accuracy = accuracy(
            &review
                .moves
                .iter()
                .map(|m| m.played.color.as_str())
                .collect::<Vec<_>>(),
            &values,
        );
        review.accuracy_by_phase = phase_accuracy(&review.moves, &phases, &values);
    }
    Ok(review)
}
pub fn validate_review(review: &ReviewResult) -> Result<()> {
    let valid = review.positions.len() == review.moves.len() + 1
        && review.positions.iter().enumerate().all(|(i, p)| {
            p.ply == i
                && p.win_pct.is_finite()
                && p.lines
                    .iter()
                    .chain(p.triage_lines.iter().flatten())
                    .all(|l| l.multipv > 0 && l.win_pct.is_finite())
        })
        && review.moves.iter().enumerate().all(|(i, m)| {
            m.played.ply == i + 1
                && matches!(m.played.color.as_str(), "w" | "b")
                && [m.win_pct_before, m.win_pct_after, m.win_pct_loss, m.cp_loss]
                    .iter()
                    .all(|v| v.is_finite())
        });
    if valid {
        Ok(())
    } else {
        Err(ReviewError::new(
            ReviewErrorCode::InvalidPayload,
            "review.validate",
            "Revisão inválida.",
        ))
    }
}

#[tauri::command]
pub async fn games_get_review_config(
    app: tauri::AppHandle,
    id: i64,
) -> IpcResult<Option<ReviewConfig>> {
    async {
        let repo = SqliteRepository::new(app);
        let Some(stored) = repo
            .db("games.get", ReviewErrorCode::Persistence, move |c| {
                Store::new(c).get(id)
            })
            .await?
        else {
            return Ok(None);
        };
        let review = normalize(serde_json::from_str(&stored.review_json).map_err(|e| {
            ReviewError::with_source(ReviewErrorCode::InvalidPayload, "games.decode", e)
        })?)?;
        let kind = match stored.summary.analysis_kind.as_str() {
            "auto-fast" => AnalysisKind::Fast,
            "auto-deep" => AnalysisKind::Deep,
            _ => AnalysisKind::Manual,
        };
        let game = core::extract(&stored.pgn);
        let meta = game.map(|g| g.meta).unwrap_or(PgnMeta {
            white: stored.summary.white,
            black: stored.summary.black,
            white_elo: None,
            black_elo: None,
            event: None,
            result: stored.summary.result,
            plies: stored.summary.plies as usize,
        });
        let depth = if stored.summary.mode == Mode::Depth {
            stored.summary.depth.clamp(15, 25)
        } else {
            20
        };
        let (tier, label, hint) = match depth {
            15 => (
                "fast",
                "Rápido",
                "Pré-visualização rápida dos lances críticos.",
            ),
            20 => (
                "balanced",
                "Equilibrado",
                "Bom equilíbrio entre qualidade e tempo.",
            ),
            25 => (
                "deep",
                "Profundo",
                "Análise profunda, mais lenta por lance.",
            ),
            _ => ("custom", "Personalizado", ""),
        };
        Ok(Some(ReviewConfig {
            pgn: stored.pgn,
            meta,
            engine: EngineTier {
                id: tier.into(),
                depth,
                label: label.into(),
                hint: if tier == "custom" {
                    format!("Profundidade fixa em d{depth}.")
                } else {
                    hint.into()
                },
            },
            mode: stored.summary.mode,
            analysis_kind: kind,
            movetime_ms: if stored.summary.mode == Mode::Time && kind == AnalysisKind::Manual {
                Some(stored.summary.depth)
            } else {
                None
            },
            lines: stored.summary.multipv,
            initial_result: Some(review),
        }))
    }
    .await
    .map_err(|error: ReviewError| ReviewErrorPayload::from(error))
}
