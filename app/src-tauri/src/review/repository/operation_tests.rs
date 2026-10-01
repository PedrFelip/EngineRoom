use super::*;
use std::{sync::Arc, time::Duration};

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
