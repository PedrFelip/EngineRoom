//! SQLite and Tauri adapter for review persistence and tracked blocking operations.
use super::{normalize, shape, validate_game, Repository};
use crate::db::{
    cache::{Cache, CachedPositionPut},
    games::{NewGame, Store, StoredGame},
    mode::Mode,
    DbState,
};
use crate::review::{core, engine::Task, types::*};
use shakmaty::Position;
use tauri::Manager;

pub struct SqliteRepository<R: tauri::Runtime = tauri::Wry> {
    app: tauri::AppHandle<R>,
    active: std::sync::Arc<tokio::sync::watch::Sender<usize>>,
}
struct Operation(std::sync::Arc<tokio::sync::watch::Sender<usize>>);

#[cfg(test)]
#[path = "operation_tests.rs"]
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
        self.blocking(operation, code, move || {
            let state = app.state::<DbState>();
            let conn = state
                .0
                .lock()
                .map_err(|e| ReviewError::new(code, operation, e))?;
            run(&conn).map_err(|e| ReviewError::with_source(code, operation, e))
        })
        .await
    }
    async fn blocking<T: Send + 'static>(
        &self,
        operation: &'static str,
        code: ReviewErrorCode,
        run: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.active.send_modify(|n| *n += 1);
        let operation_guard = Operation(self.active.clone());
        tauri::async_runtime::spawn_blocking(move || {
            let _operation = operation_guard;
            run()
        })
        .await
        .map_err(|e| ReviewError::with_source(code, operation, e))?
    }
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
            let candidates = self
                .db("cache.lookup", ReviewErrorCode::Cache, move |c| {
                    Cache::new(c).lookup_bulk_candidates(&owned, mode, value, multipv)
                })
                .await?;
            Ok(candidates
                .into_iter()
                .zip(fens)
                .map(|(hits, f)| {
                    hits.into_iter().find_map(|hit| {
                        shape(hit, f, multipv).filter(|raw| {
                            mode != Mode::Depth
                                || raw
                                    .lines
                                    .iter()
                                    .all(|line| line.depth.unwrap_or(raw.depth) >= value)
                        })
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
                let mut lines: Vec<_> = position.lines.iter().collect();
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
                let Some(first) = lines.first() else { continue };
                let reached_depth = lines
                    .iter()
                    .skip(1)
                    .fold(first.depth.unwrap_or(position.depth), |depth, line| {
                        depth.min(line.depth.unwrap_or(position.depth))
                    });
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
                        cp: first.cp,
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
            let profile = crate::review::adaptive::profile(config.analysis_kind)?;
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
        repo.blocking("games.decode", ReviewErrorCode::InvalidPayload, move || {
            review_config(stored).map(Some)
        })
        .await
    }
    .await
    .map_err(|error: ReviewError| ReviewErrorPayload::from(error))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnginePolicy {
    min_depth: u32,
    max_depth: u32,
    default_depth: u32,
    custom_label: String,
    custom_hint_prefix: String,
    custom_hint_suffix: String,
    tiers: Vec<EngineTier>,
}

fn engine_policy() -> &'static EnginePolicy {
    static POLICY: std::sync::OnceLock<EnginePolicy> = std::sync::OnceLock::new();
    POLICY.get_or_init(|| {
        let policy: EnginePolicy = serde_json::from_str(include_str!("../engine-policy.json"))
            .expect("bundled engine policy must be valid JSON");
        assert!(
            policy.min_depth <= policy.default_depth
                && policy.default_depth <= policy.max_depth
                && !policy.tiers.is_empty()
                && policy
                    .tiers
                    .iter()
                    .all(|tier| (policy.min_depth..=policy.max_depth).contains(&tier.depth)),
            "bundled engine policy must have valid depth bounds and tiers"
        );
        policy
    })
}

fn restored_engine(mode: Mode, stored_depth: u32) -> EngineTier {
    let policy = engine_policy();
    let depth = if mode == Mode::Depth {
        stored_depth.clamp(policy.min_depth, policy.max_depth)
    } else {
        policy.default_depth
    };
    policy
        .tiers
        .iter()
        .find(|tier| tier.depth == depth)
        .cloned()
        .unwrap_or_else(|| EngineTier {
            id: "custom".into(),
            depth,
            label: policy.custom_label.clone(),
            hint: format!(
                "{}{depth}{}",
                policy.custom_hint_prefix, policy.custom_hint_suffix
            ),
        })
}

fn review_config(stored: StoredGame) -> Result<ReviewConfig> {
    let review = normalize(serde_json::from_str(&stored.review_json).map_err(|e| {
        ReviewError::with_source(ReviewErrorCode::InvalidPayload, "games.decode", e)
    })?)?;
    let kind = match stored.summary.analysis_kind.as_str() {
        "auto-fast" => AnalysisKind::Fast,
        "auto-deep" => AnalysisKind::Deep,
        _ => AnalysisKind::Manual,
    };
    let game = core::extract(&stored.pgn)?;
    validate_game(&review, &game)?;
    let meta = game.meta;
    Ok(ReviewConfig {
        pgn: stored.pgn,
        meta,
        engine: restored_engine(stored.summary.mode, stored.summary.depth),
        mode: stored.summary.mode,
        analysis_kind: kind,
        movetime_ms: if stored.summary.mode == Mode::Time && kind == AnalysisKind::Manual {
            Some(stored.summary.depth)
        } else {
            None
        },
        lines: stored.summary.multipv,
        initial_result: Some(review),
    })
}
