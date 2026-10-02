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
        .lookup(&[fen.clone()], Mode::Time, 500, 3)
        .await
        .unwrap()[0]
        .is_none());
    assert!(repo
        .lookup(&[fen.clone()], Mode::Depth, 20, 1)
        .await
        .unwrap()[0]
        .is_some());
    for (multipv, depth, root) in [(2, 12, "d2d4"), (3, 15, "g1f3")] {
        raw.lines.push(super::super::types::RawLine {
            multipv,
            depth: Some(depth),
            cp: 10,
            pv: vec![root.into()],
            san: None,
        });
    }
    repo.put(&[raw.clone()], Mode::Time, 1000, 3).await.unwrap();
    assert!(repo
        .lookup(&[fen.clone()], Mode::Depth, 14, 3)
        .await
        .unwrap()[0]
        .is_none());
    assert!(repo
        .lookup(&[fen.clone()], Mode::Depth, 12, 3)
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
        .lookup(&[fen.clone()], Mode::Depth, 21, 3)
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
            .db("test", "cache", move |_| {
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
