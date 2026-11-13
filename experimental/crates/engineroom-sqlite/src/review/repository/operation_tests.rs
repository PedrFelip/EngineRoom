use super::*;
use std::{sync::Arc, time::Duration};
fn temp_path() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "engineroom-{}-{}.db",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

#[tokio::test]
async fn cache_advertises_only_produced_lines_and_their_common_depth() {
    let path = temp_path();
    let repo = SqliteRepository::new(path.clone());
    let fen = core::fen(&shakmaty::Chess::default());
    let mut raw = core::terminal_raw(&fen, 15);
    raw.depth = 20;
    raw.lines[0].depth = Some(20);
    raw.lines[0].pv = vec!["e2e4".into()];
    repo.put(&[raw.clone()], Mode::Time, 1000, 3).await.unwrap();
    assert!(
        repo.lookup(std::slice::from_ref(&fen), Mode::Time, 500, 3)
            .await
            .unwrap()[0]
            .is_none()
    );
    assert!(
        repo.lookup(std::slice::from_ref(&fen), Mode::Depth, 20, 1)
            .await
            .unwrap()[0]
            .is_some()
    );
    for (multipv, depth, root) in [(2, 12, "d2d4"), (3, 15, "g1f3")] {
        raw.lines.push(crate::review::types::RawLine {
            multipv,
            depth: Some(depth),
            cp: 10,
            pv: vec![root.into()],
            san: None,
            unstable: false,
        });
    }
    repo.put(&[raw.clone()], Mode::Time, 1000, 3).await.unwrap();
    assert!(
        repo.lookup(std::slice::from_ref(&fen), Mode::Depth, 14, 3)
            .await
            .unwrap()[0]
            .is_none()
    );
    assert!(
        repo.lookup(std::slice::from_ref(&fen), Mode::Depth, 12, 3)
            .await
            .unwrap()[0]
            .is_some()
    );
    // Old rows advertised PV1 depth for every alternative. Validate the payload
    // even when the existing SQL returns one of those rows.
    {
        let conn = crate::db::open_file(&path).unwrap();
        Cache::new(&conn)
            .store(
                &fen,
                Mode::Time,
                2000,
                3,
                25,
                raw.cp,
                &serde_json::to_string(&raw.lines).unwrap(),
            )
            .unwrap();
    }
    assert!(
        repo.lookup(std::slice::from_ref(&fen), Mode::Depth, 21, 3)
            .await
            .unwrap()[0]
            .is_none()
    );
    assert!(repo.lookup(&[fen], Mode::Time, 2000, 3).await.unwrap()[0].is_some());

    // A position with one legal move fully covers a MultiPV 5 request.
    let forced = "7k/8/5KR1/8/8/8/8/8 b - - 0 1";
    assert_eq!(core::position(forced).unwrap().legal_moves().len(), 1);
    let mut raw = core::terminal_raw(forced, -500);
    raw.depth = 10;
    raw.lines[0].depth = Some(10);
    raw.lines[0].pv = vec!["h8h7".into()];
    repo.put(&[raw], Mode::Time, 500, 5).await.unwrap();
    let hit = repo
        .lookup(&[forced.into()], Mode::Time, 500, 5)
        .await
        .unwrap()[0]
        .clone()
        .unwrap();
    assert_eq!(hit.lines.len(), 1);
}

#[tokio::test]
async fn dropped_waiter_keeps_blocking_operation_owned_until_drain() {
    let repo = Arc::new(SqliteRepository::new(temp_path()));
    let started = Arc::new(tokio::sync::Notify::new());
    let (release, wait) = std::sync::mpsc::channel();
    let owned = repo.clone();
    let entered = started.clone();
    let worker = tokio::spawn(async move {
        owned
            .db("test", ReviewErrorCode::Cache, move |_| {
                entered.notify_one();
                wait.recv().unwrap();
                Ok(())
            })
            .await
    });
    started.notified().await;
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    assert_eq!(*repo.active.borrow(), 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), repo.drain())
            .await
            .is_err()
    );
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(1), repo.drain())
        .await
        .unwrap();
    assert_eq!(*repo.active.borrow(), 0);
}
#[test]
fn history_conversion_preserves_metadata_fallback_and_decode_errors() {
    let pgn = "[White \"Alice\"]\n1. e4".to_string();
    let game = core::extract(&pgn).unwrap();
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let review = core::build(&game, &raw).unwrap();
    let stored = |pgn: String, review_json: String| StoredGame {
        summary: crate::db::games::GameSummary {
            id: 1,
            white: "Legacy white".into(),
            black: "Legacy black".into(),
            result: "*".into(),
            plies: 1,
            engine_tier: "balanced".into(),
            mode: Mode::Depth,
            analysis_kind: "manual".into(),
            depth: 20,
            multipv: 1,
            accuracy_white: 100.0,
            accuracy_black: 100.0,
            created_at: String::new(),
        },
        pgn,
        review_json,
    };
    let json = serde_json::to_string(&review).unwrap();
    let current = review_config(stored(pgn, json.clone())).unwrap();
    assert_eq!(current.meta.white, "Alice");
    assert_eq!(
        serde_json::to_value(current.initial_result.unwrap()).unwrap(),
        serde_json::to_value(&review).unwrap()
    );
    let fallback = review_config(stored("1. e5".into(), json)).unwrap();
    assert_eq!(fallback.meta.white, "Legacy white");
    assert_eq!(fallback.meta.black, "Legacy black");
    let error = review_config(stored("1. e4".into(), "{".into())).unwrap_err();
    assert_eq!(error.code, ReviewErrorCode::InvalidPayload);
    assert_eq!(error.operation, "games.decode");
}

#[tokio::test]
async fn corrupt_cache_is_a_miss_but_database_open_failure_is_fatal() {
    let path = temp_path();
    let conn = crate::db::open_file(&path).unwrap();
    let fen = core::fen(&shakmaty::Chess::default());
    Cache::new(&conn)
        .store(&fen, Mode::Depth, 20, 1, 20, 0, "{")
        .unwrap();
    drop(conn);
    let repo = SqliteRepository::new(path);
    assert!(repo.lookup(&[fen], Mode::Depth, 20, 1).await.unwrap()[0].is_none());
    let invalid_path = temp_path().join("missing").join("database.db");
    let failing = SqliteRepository::new(invalid_path);
    assert!(
        failing
            .lookup(&["position".into()], Mode::Depth, 20, 1)
            .await
            .is_err()
    );
}
