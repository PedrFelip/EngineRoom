use super::chess_com::*;
use serde_json::{json, Value};
use std::{collections::VecDeque, sync::Mutex, time::Duration};
use tokio::sync::watch;

const PGN: &str = "[White \"PGN White\"]\n[Black \"PGN Black\"]\n[WhiteElo \"1200\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 Nc6 1-0";

struct FakeHttp {
    responses: Mutex<VecDeque<Result<Vec<u8>>>>,
    requests: Mutex<Vec<String>>,
}
impl FakeHttp {
    fn new(responses: Vec<Result<Vec<u8>>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(vec![]),
        }
    }
}
impl Http for FakeHttp {
    fn get<'a>(&'a self, url: &'a str, _max_bytes: usize) -> Task<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            self.requests.lock().unwrap().push(url.into());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected HTTP request")
        })
    }
}
fn bytes(value: Value) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(&value).unwrap())
}
fn archive(month: &str) -> String {
    format!("https://api.chess.com/pub/player/test-player/games/2026/{month}")
}
fn index(months: &[&str]) -> Result<Vec<u8>> {
    bytes(json!({"archives": months.iter().map(|m| archive(m)).collect::<Vec<_>>()}))
}
fn game(id: u64) -> Value {
    json!({
        "url": format!("https://www.chess.com/game/live/{id}"),
        "end_time": 1_700_000_000 + id,
        "rules": "chess", "time_class": "blitz", "pgn": PGN,
        "white": {"username": "API White", "rating": 1300},
        "black": {"username": "API Black", "rating": 1400}
    })
}
async fn run(http: &dyn Http, cursor: Option<Cursor>, limits: Limits) -> Result<SearchResult> {
    let (_tx, mut cancel) = watch::channel(false);
    search(http, " Test-Player ", cursor, limits, &mut cancel).await
}

#[tokio::test]
async fn searches_newest_first_and_cursor_keeps_the_rest_of_the_same_month() {
    let limits = Limits {
        page_size: 2,
        ..Limits::default()
    };
    let http = FakeHttp::new(vec![
        index(&["08", "09"]),
        bytes(json!({"games": [game(1), game(3), game(2)]})),
    ]);
    let first = run(&http, None, limits.clone()).await.unwrap();
    assert_eq!(
        first.items.iter().map(|g| g.end_time).collect::<Vec<_>>(),
        vec![1_700_000_003, 1_700_000_002]
    );
    assert_eq!(http.requests.lock().unwrap()[1], archive("09"));
    let http = FakeHttp::new(vec![
        index(&["08", "09"]),
        bytes(json!({"games": [game(3), game(1), game(2)]})),
        bytes(json!({"games": []})),
    ]);
    let next = run(&http, first.next_cursor, limits).await.unwrap();
    assert_eq!(next.items.len(), 1);
    assert_eq!(next.items[0].url, game(1)["url"].as_str().unwrap());
    assert!(next.next_cursor.is_none());
}

#[tokio::test]
async fn filters_variants_invalid_incomplete_and_multiple_games_but_keeps_pgn_metadata() {
    let mut variant = game(10);
    variant["rules"] = json!("chess960");
    let mut invalid = game(9);
    invalid["pgn"] = json!("not a game");
    let mut unfinished = game(8);
    unfinished["pgn"] = json!("[Result \"*\"]\n\n1. e4 e5 *");
    let mut multiple = game(7);
    multiple["pgn"] = json!(format!("{PGN}\n\n{PGN}"));
    let mut valid = game(6);
    valid.as_object_mut().unwrap().remove("time_class");
    let http = FakeHttp::new(vec![
        index(&["09"]),
        bytes(json!({"games": [variant, invalid, unfinished, multiple, valid.clone(), valid]})),
    ]);
    let result = run(&http, None, Limits::default()).await.unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].white.username, "API White");
    assert_eq!(result.items[0].pgn, PGN);
    assert_eq!(
        crate::review::core::extract(&result.items[0].pgn)
            .unwrap()
            .meta
            .white_elo
            .as_deref(),
        Some("1200")
    );
    assert!(result.items[0].time_class.is_none());
    assert_eq!(result.skipped_unsupported, 1);
    assert_eq!(result.skipped_invalid, 3);
}

#[tokio::test]
async fn bounds_months_candidates_and_bytes_without_claiming_exhaustion() {
    let http = FakeHttp::new(vec![
        index(&["07", "08", "09"]),
        bytes(json!({"games": []})),
    ]);
    let result = run(
        &http,
        None,
        Limits {
            max_months: 1,
            ..Limits::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(result.archives_read, 1);
    assert_eq!(result.stop_reason, StopReason::Limit);
    assert!(result.next_cursor.is_some());
    let http = FakeHttp::new(vec![
        index(&["09"]),
        bytes(json!({"games": [game(1), game(2)]})),
    ]);
    let result = run(
        &http,
        None,
        Limits {
            max_candidates: 1,
            ..Limits::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.stop_reason, StopReason::Limit);
    let http = FakeHttp::new(vec![index(&["09"]), bytes(json!({"games": [game(1)]}))]);
    let error = run(
        &http,
        None,
        Limits {
            month_bytes: 32,
            ..Limits::default()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResponseTooLarge);
}

#[tokio::test]
async fn preserves_partial_results_and_retries_the_failed_month() {
    let http = FakeHttp::new(vec![
        index(&["08", "09"]),
        bytes(json!({"games": [game(1)]})),
        Err(ImportError::new(ErrorCode::RateLimited, "Aguarde.")),
    ]);
    let result = run(&http, None, Limits::default()).await.unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.stop_reason, StopReason::Error);
    assert_eq!(result.warning.unwrap().code, ErrorCode::RateLimited);
    let cursor = result.next_cursor.unwrap();
    assert_eq!(cursor.month, 8);
    assert!(cursor.before.is_none());
}

#[tokio::test]
async fn rejects_untrusted_archive_urls_and_invalid_usernames_before_download() {
    for url in [
        "https://evil.test/pub/player/test-player/games/2026/09",
        "https://api.chess.com/pub/player/other/games/2026/09",
        "https://api.chess.com/pub/player/test-player/games/2026/09?url=evil",
    ] {
        let http = FakeHttp::new(vec![bytes(json!({"archives": [url]}))]);
        assert_eq!(
            run(&http, None, Limits::default()).await.unwrap_err().code,
            ErrorCode::InvalidResponse
        );
        assert_eq!(http.requests.lock().unwrap().len(), 1);
    }
    let http = FakeHttp::new(vec![]);
    let (_tx, mut cancel) = watch::channel(false);
    for username in ["", "../admin", "test/name", "https://evil.test"] {
        assert_eq!(
            search(&http, username, None, Limits::default(), &mut cancel)
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
    }
    assert!(http.requests.lock().unwrap().is_empty());
}

pub(super) struct PendingHttp;
impl Http for PendingHttp {
    fn get<'a>(&'a self, _url: &'a str, _max_bytes: usize) -> Task<'a, Result<Vec<u8>>> {
        Box::pin(std::future::pending())
    }
}
#[tokio::test]
async fn cancels_pending_http_and_enforces_total_deadline() {
    let (tx, mut cancel) = watch::channel(false);
    let task = tokio::spawn(async move {
        search(
            &PendingHttp,
            "test-player",
            None,
            Limits::default(),
            &mut cancel,
        )
        .await
    });
    tokio::task::yield_now().await;
    tx.send_replace(true);
    let result = tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.unwrap_err().code, ErrorCode::Cancelled);
    let error = run(
        &PendingHttp,
        None,
        Limits {
            total_time: Duration::from_millis(5),
            ..Limits::default()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
}

#[tokio::test]
async fn skips_invalid_sort_keys_without_producing_an_unusable_cursor() {
    let mut invalid = game(100);
    invalid["url"] = json!("https://evil.test/100");
    let mut invalid_time = game(99);
    invalid_time["end_time"] = json!(u64::MAX);
    let mut invalid_pgn = game(2);
    invalid_pgn["pgn"] = json!("invalid");
    let http = FakeHttp::new(vec![
        index(&["09"]),
        bytes(json!({"games": [invalid, invalid_time, invalid_pgn, game(1)]})),
    ]);
    let result = run(
        &http,
        None,
        Limits {
            max_candidates: 1,
            ..Limits::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(result.skipped_invalid, 3);
    assert!(result.next_cursor.is_some());
    let http = FakeHttp::new(vec![index(&["09"]), bytes(json!({"games": [game(1)]}))]);
    let next = run(&http, result.next_cursor, Limits::default())
        .await
        .unwrap();
    assert_eq!(next.items.len(), 1);
}
