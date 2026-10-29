use super::*;
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn cache_advertises_only_produced_lines_and_their_common_depth() {
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage(DbState(std::sync::Mutex::new(
        crate::db::open_memory().unwrap(),
    )));
    let repo = SqliteRepository::new(app.handle().clone());
    let fen = core::fen(&shakmaty::Chess::default());
    let mut raw = core::terminal_raw(&fen, 15);
    raw.depth = 20;
    raw.lines[0].depth = Some(20);
    raw.lines[0].pv = vec!["e2e4".into()];
    repo.put(&[raw.clone()], Mode::Time, 1000, 3).await.unwrap();
    assert!(repo
        .lookup(std::slice::from_ref(&fen), Mode::Time, 500, 3)
        .await
        .unwrap()[0]
        .is_none());
    assert!(repo
        .lookup(std::slice::from_ref(&fen), Mode::Depth, 20, 1)
        .await
        .unwrap()[0]
        .is_some());
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
    assert!(repo
        .lookup(std::slice::from_ref(&fen), Mode::Depth, 14, 3)
        .await
        .unwrap()[0]
        .is_none());
    assert!(repo
        .lookup(std::slice::from_ref(&fen), Mode::Depth, 12, 3)
        .await
        .unwrap()[0]
        .is_some());
    // Old rows advertised PV1 depth for every alternative. Validate the payload
    // even when the existing SQL returns one of those rows.
    {
        let state = app.state::<DbState>();
        let conn = state.0.lock().unwrap();
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
    assert!(repo
        .lookup(std::slice::from_ref(&fen), Mode::Depth, 21, 3)
        .await
        .unwrap()[0]
        .is_none());
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
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage(DbState(std::sync::Mutex::new(
        crate::db::open_memory().unwrap(),
    )));
    let repo = Arc::new(SqliteRepository::new(app.handle().clone()));
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
#[tokio::test]
async fn dropped_cpu_waiter_is_tracked_without_locking_sqlite() {
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage(DbState(std::sync::Mutex::new(
        crate::db::open_memory().unwrap(),
    )));
    let repo = Arc::new(SqliteRepository::new(app.handle().clone()));
    let started = Arc::new(tokio::sync::Notify::new());
    let (release, wait) = std::sync::mpsc::channel();
    let owned = repo.clone();
    let entered = started.clone();
    let worker = tokio::spawn(async move {
        owned
            .blocking("test", ReviewErrorCode::InvalidPayload, move || {
                entered.notify_one();
                wait.recv().unwrap();
                Ok(())
            })
            .await
    });
    started.notified().await;
    assert!(app.state::<DbState>().0.try_lock().is_ok());
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
    let config = crate::review::tests::config("[White \"Alice\"]\n1. e4", AnalysisKind::Manual);
    let game = core::extract(&config.pgn).unwrap();
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
    let current = review_config(stored(config.pgn, json.clone())).unwrap();
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

fn raw_line(multipv: u32, cp: i32, depth: u32) -> crate::review::types::RawLine {
    crate::review::types::RawLine {
        unstable: false,
        multipv,
        cp,
        pv: vec!["e2e4".into()],
        san: None,
        depth: Some(depth),
    }
}

fn store_candidate<R: tauri::Runtime>(
    app: &tauri::App<R>,
    fen: &str,
    mode: Mode,
    value: u32,
    reached_depth: u32,
    multipv: u32,
    cp: i32,
    lines: &[crate::review::types::RawLine],
) {
    let state = app.state::<DbState>();
    let conn = state.0.lock().unwrap();
    Cache::new(&conn)
        .store(
            fen,
            mode,
            value,
            multipv,
            reached_depth,
            cp,
            &serde_json::to_string(lines).unwrap(),
        )
        .unwrap();
}

#[tokio::test]
async fn lookup_selects_first_valid_covering_candidate_and_rejects_all_invalid() {
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage(DbState(std::sync::Mutex::new(
        crate::db::open_memory().unwrap(),
    )));
    let repo = SqliteRepository::new(app.handle().clone());
    let lines = |cp, depth, multipv| {
        (1..=multipv)
            .map(|slot| raw_line(slot, cp, depth))
            .collect::<Vec<_>>()
    };

    // Corrupt shallower row, valid deeper row: the second candidate wins.
    let corrupt = core::fen(&shakmaty::Chess::default());
    store_candidate(
        &app,
        &corrupt,
        Mode::Depth,
        20,
        20,
        2,
        40,
        &lines(40, 20, 2),
    );
    {
        let state = app.state::<DbState>();
        let conn = state.0.lock().unwrap();
        conn.execute(
            "UPDATE position_cache SET lines_json = 'broken' WHERE reached_depth = 20",
            [],
        )
        .unwrap();
    }
    store_candidate(
        &app,
        &corrupt,
        Mode::Depth,
        20,
        30,
        2,
        60,
        &lines(60, 30, 2),
    );
    let hit = repo
        .lookup(std::slice::from_ref(&corrupt), Mode::Depth, 20, 2)
        .await
        .unwrap()[0]
        .clone()
        .unwrap();
    assert_eq!(hit.cp, 60);
    assert_eq!(hit.depth, 30);

    // Incomplete shallower row (advertises more lines than it stores).
    let incomplete = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
    store_candidate(
        &app,
        incomplete,
        Mode::Depth,
        20,
        20,
        3,
        40,
        &lines(40, 20, 1),
    );
    store_candidate(
        &app,
        incomplete,
        Mode::Depth,
        20,
        30,
        2,
        60,
        &lines(60, 30, 2),
    );
    let hit = repo
        .lookup(&[incomplete.into()], Mode::Depth, 20, 2)
        .await
        .unwrap()[0]
        .clone()
        .unwrap();
    assert_eq!(hit.cp, 60, "candidato incompleto é ignorado");

    // Insufficient per-line depth in the shallower row.
    let shallow = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
    store_candidate(&app, shallow, Mode::Depth, 20, 20, 2, 40, &lines(40, 10, 2));
    store_candidate(&app, shallow, Mode::Depth, 20, 30, 2, 60, &lines(60, 30, 2));
    let hit = repo
        .lookup(&[shallow.into()], Mode::Depth, 20, 2)
        .await
        .unwrap()[0]
        .clone()
        .unwrap();
    assert_eq!(hit.cp, 60, "profundidade insuficiente é ignorada");

    // Time ordering follows the smaller covering budget first, then falls back.
    let timed = "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq - 0 1";
    store_candidate(&app, timed, Mode::Time, 1000, 20, 2, 40, &lines(40, 20, 1));
    store_candidate(&app, timed, Mode::Time, 2000, 25, 2, 60, &lines(60, 25, 2));
    let hit = repo
        .lookup(&[timed.into()], Mode::Time, 1000, 2)
        .await
        .unwrap()[0]
        .clone()
        .unwrap();
    assert_eq!(hit.cp, 60, "candidato de tempo incompleto é ignorado");

    // All candidates invalid: the position is a miss, not an error.
    let invalid = "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR w KQkq - 0 2";
    store_candidate(&app, invalid, Mode::Depth, 20, 20, 2, 40, &lines(40, 20, 1));
    store_candidate(&app, invalid, Mode::Depth, 20, 30, 2, 60, &lines(60, 10, 2));
    assert!(repo
        .lookup(&[invalid.into()], Mode::Depth, 20, 2)
        .await
        .unwrap()[0]
        .is_none());
}
