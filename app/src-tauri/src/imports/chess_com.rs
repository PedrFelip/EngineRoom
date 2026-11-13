//! Bounded, sequential Chess.com archive search.
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, future::Future, pin::Pin, time::Duration};
use tokio::{sync::watch, time::Instant};

pub type Task<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type Result<T> = std::result::Result<T, ImportError>;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    InvalidInput,
    Unavailable,
    RateLimited,
    Network,
    Timeout,
    InvalidResponse,
    ResponseTooLarge,
    Cancelled,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportError {
    pub code: ErrorCode,
    pub message: String,
    pub retry_after_seconds: Option<u64>,
}
impl ImportError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retry_after_seconds: None,
        }
    }
}
impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ImportError {}
pub trait Http: Send + Sync {
    /// Implementations must bound the body while reading, not just Content-Length.
    fn get<'a>(&'a self, url: &'a str, max_bytes: usize) -> Task<'a, Result<Vec<u8>>>;
}
#[derive(Clone)]
pub struct Limits {
    pub page_size: usize,
    pub max_months: usize,
    pub max_candidates: usize,
    pub index_bytes: usize,
    pub month_bytes: usize,
    pub total_bytes: usize,
    pub pgn_bytes: usize,
    pub total_time: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            page_size: 20,
            max_months: 3,
            max_candidates: 200,
            index_bytes: 256 * 1024,
            month_bytes: 10 * 1024 * 1024,
            total_bytes: 20 * 1024 * 1024,
            pgn_bytes: 256 * 1024,
            total_time: Duration::from_secs(30),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct Before {
    pub end_time: u64,
    pub url: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cursor {
    pub username: String,
    pub year: u16,
    pub month: u8,
    pub before: Option<Before>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Player {
    pub username: String,
    pub rating: Option<u32>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteGame {
    pub url: String,
    pub end_time: u64,
    pub time_class: Option<String>,
    pub white: Player,
    pub black: Player,
    pub result: String,
    pub pgn: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    Exhausted,
    Page,
    Limit,
    Error,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub items: Vec<RemoteGame>,
    pub next_cursor: Option<Cursor>,
    pub archives_read: usize,
    pub skipped_invalid: usize,
    pub skipped_unsupported: usize,
    pub stop_reason: StopReason,
    pub warning: Option<ImportError>,
}
#[derive(Deserialize)]
struct Index {
    archives: Vec<String>,
}
#[derive(Deserialize)]
struct Month {
    games: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
struct ApiGame {
    url: String,
    end_time: u64,
    rules: String,
    time_class: Option<String>,
    white: Player,
    black: Player,
    pgn: String,
}
impl ApiGame {
    fn key(&self) -> Before {
        Before {
            end_time: self.end_time,
            url: self.url.clone(),
        }
    }
}
pub fn username(input: &str) -> Result<String> {
    let name = input.trim().to_ascii_lowercase();
    if name.is_empty()
        || name.len() > 50
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(ImportError::new(
            ErrorCode::InvalidInput,
            "Informe um nome de usuário válido do Chess.com.",
        ));
    }
    Ok(name)
}
fn archive_url(name: &str, year: u16, month: u8) -> String {
    format!("https://api.chess.com/pub/player/{name}/games/{year:04}/{month:02}")
}
fn invalid_response() -> ImportError {
    ImportError::new(
        ErrorCode::InvalidResponse,
        "O Chess.com retornou dados inválidos.",
    )
}
fn too_large() -> ImportError {
    ImportError::new(
        ErrorCode::ResponseTooLarge,
        "O arquivo do Chess.com excedeu o limite da busca.",
    )
}
fn check(cancel: &watch::Receiver<bool>, deadline: Instant) -> Result<()> {
    if *cancel.borrow() {
        Err(ImportError::new(ErrorCode::Cancelled, "Busca cancelada."))
    } else if Instant::now() >= deadline {
        Err(ImportError::new(
            ErrorCode::Timeout,
            "A busca no Chess.com demorou demais. Tente novamente.",
        ))
    } else {
        Ok(())
    }
}
async fn download(
    http: &dyn Http,
    url: &str,
    max_bytes: usize,
    cancel: &mut watch::Receiver<bool>,
    deadline: Instant,
) -> Result<Vec<u8>> {
    check(cancel, deadline)?;
    let result = tokio::select! {
        biased;
        _ = cancel.wait_for(|v| *v) => Err(ImportError::new(ErrorCode::Cancelled, "Busca cancelada.")),
        result = tokio::time::timeout_at(deadline, http.get(url, max_bytes)) => result.map_err(|_| ImportError::new(ErrorCode::Timeout, "A busca no Chess.com demorou demais."))?,
    }?;
    if result.len() > max_bytes {
        return Err(too_large());
    }
    Ok(result)
}
/// Blocking parsing is always awaited, including on cancellation: no detached CPU work.
async fn parse<T: serde::de::DeserializeOwned + Send + 'static>(bytes: Vec<u8>) -> Result<T> {
    tokio::task::spawn_blocking(move || {
        serde_json::from_slice(&bytes).map_err(|_| invalid_response())
    })
    .await
    .map_err(|_| invalid_response())?
}
fn months(index: Index, name: &str) -> Result<Vec<(u16, u8)>> {
    let prefix = format!("https://api.chess.com/pub/player/{name}/games/");
    let mut months = Vec::new();
    for url in index.archives {
        let suffix = url.strip_prefix(&prefix).ok_or_else(invalid_response)?;
        let (year, month) = suffix.split_once('/').ok_or_else(invalid_response)?;
        let year: u16 = year.parse().map_err(|_| invalid_response())?;
        let month: u8 = month.parse().map_err(|_| invalid_response())?;
        if !(2000..=9999).contains(&year)
            || !(1..=12).contains(&month)
            || url != archive_url(name, year, month)
        {
            return Err(invalid_response());
        }
        months.push((year, month));
    }
    months.sort_unstable_by(|a, b| b.cmp(a));
    months.dedup();
    Ok(months)
}
fn game_url(url: &str) -> bool {
    [
        "https://www.chess.com/game/live/",
        "https://www.chess.com/game/daily/",
        "https://www.chess.com/live/game/",
        "https://www.chess.com/daily/game/",
    ]
    .iter()
    .any(|prefix| {
        url.strip_prefix(prefix)
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
    })
}
fn stop(mut result: SearchResult, reason: StopReason, cursor: Cursor) -> SearchResult {
    result.stop_reason = reason;
    result.next_cursor = Some(cursor);
    result
}
fn failure(mut result: SearchResult, error: ImportError, cursor: Cursor) -> Result<SearchResult> {
    if error.code == ErrorCode::Cancelled || result.items.is_empty() {
        return Err(error);
    }
    result.warning = Some(error);
    Ok(stop(result, StopReason::Error, cursor))
}
/// Close a search after the archive index was fully traversed.
///
/// A deadline or cancellation seen only at the finish line must not discard the
/// page that was already collected. `resume` points at the last month that was
/// processed with `before` set to its oldest accepted key, so retrying from it
/// skips everything already emitted and cannot skip older games.
pub(super) fn complete(
    result: SearchResult,
    cancel: &watch::Receiver<bool>,
    deadline: Instant,
    resume: Option<Cursor>,
) -> Result<SearchResult> {
    match check(cancel, deadline) {
        Ok(()) => Ok(result),
        Err(error) => match resume {
            Some(cursor) => failure(result, error, cursor),
            None => Err(error),
        },
    }
}
pub async fn search(
    http: &dyn Http,
    input: &str,
    cursor: Option<Cursor>,
    limits: Limits,
    cancel: &mut watch::Receiver<bool>,
) -> Result<SearchResult> {
    let name = username(input)?;
    if let Some(c) = &cursor {
        if c.username != name
            || !(2000..=9999).contains(&c.year)
            || !(1..=12).contains(&c.month)
            || c.before
                .as_ref()
                .is_some_and(|b| !game_url(&b.url) || !(1..=253402300799).contains(&b.end_time))
        {
            return Err(ImportError::new(
                ErrorCode::InvalidInput,
                "Cursor da busca inválido.",
            ));
        }
    }
    let deadline = Instant::now() + limits.total_time;
    let bytes = download(
        http,
        &format!("https://api.chess.com/pub/player/{name}/games/archives"),
        limits.index_bytes.min(limits.total_bytes),
        cancel,
        deadline,
    )
    .await?;
    let mut total_bytes = bytes.len();
    let index = months(parse(bytes).await?, &name)?;
    let mut result = SearchResult {
        items: vec![],
        next_cursor: None,
        archives_read: 0,
        skipped_invalid: 0,
        skipped_unsupported: 0,
        stop_reason: StopReason::Exhausted,
        warning: None,
    };
    let mut seen = HashSet::new();
    let mut candidates = 0;
    let mut resume = None;
    for (year, month) in index {
        if cursor
            .as_ref()
            .is_some_and(|c| (year, month) > (c.year, c.month))
        {
            continue;
        }
        let mut at = Cursor {
            username: name.clone(),
            year,
            month,
            before: cursor
                .as_ref()
                .filter(|c| (year, month) == (c.year, c.month))
                .and_then(|c| c.before.clone()),
        };
        if result.archives_read >= limits.max_months || total_bytes >= limits.total_bytes {
            return Ok(stop(result, StopReason::Limit, at));
        }
        let bytes = match download(
            http,
            &archive_url(&name, year, month),
            limits.month_bytes.min(limits.total_bytes - total_bytes),
            cancel,
            deadline,
        )
        .await
        {
            Ok(bytes) => bytes,
            Err(e) => return failure(result, e, at),
        };
        total_bytes += bytes.len();
        let data: Month = match parse(bytes).await {
            Ok(data) => data,
            Err(e) => return failure(result, e, at),
        };
        if let Err(e) = check(cancel, deadline) {
            return failure(result, e, at);
        }
        result.archives_read += 1;
        let mut games = Vec::new();
        for value in data.games {
            match serde_json::from_value::<ApiGame>(value) {
                Ok(game) => games.push(game),
                Err(_) => result.skipped_invalid += 1,
            }
        }
        games.sort_unstable_by(|a, b| b.end_time.cmp(&a.end_time).then_with(|| b.url.cmp(&a.url)));
        for game in games {
            // Unusable ordering keys must never escape through the public cursor.
            if !game_url(&game.url) || !(1..=253402300799).contains(&game.end_time) {
                result.skipped_invalid += 1;
                continue;
            }
            let key = game.key();
            if at.before.as_ref().is_some_and(|before| &key >= before) {
                continue;
            }
            if let Err(e) = check(cancel, deadline) {
                return failure(result, e, at);
            }
            if candidates >= limits.max_candidates {
                return Ok(stop(result, StopReason::Limit, at));
            }
            candidates += 1;
            let previous = at.before.clone();
            at.before = Some(key);
            if !seen.insert(game.url.clone()) {
                continue;
            }
            if game.rules != "chess" {
                result.skipped_unsupported += 1;
                continue;
            }
            if game.pgn.len() > limits.pgn_bytes
                || game.white.username.is_empty()
                || game.black.username.is_empty()
            {
                result.skipped_invalid += 1;
                continue;
            }
            let validated = tokio::task::spawn_blocking(move || {
                let parsed = crate::review::core::extract(&game.pgn).ok()?;
                if !matches!(parsed.meta.result.as_str(), "1-0" | "0-1" | "1/2-1/2") {
                    return None;
                }
                Some(RemoteGame {
                    url: game.url,
                    end_time: game.end_time,
                    time_class: game.time_class,
                    white: game.white,
                    black: game.black,
                    result: parsed.meta.result,
                    pgn: game.pgn,
                })
            })
            .await
            .map_err(|_| invalid_response())?;
            // Always drain parsing before observing cancellation.
            if let Err(e) = check(cancel, deadline) {
                at.before = previous;
                return failure(result, e, at);
            }
            match validated {
                Some(game) => result.items.push(game),
                None => result.skipped_invalid += 1,
            }
            if result.items.len() >= limits.page_size {
                return Ok(stop(result, StopReason::Page, at));
            }
        }
        resume = Some(at);
    }
    complete(result, cancel, deadline, resume)
}
