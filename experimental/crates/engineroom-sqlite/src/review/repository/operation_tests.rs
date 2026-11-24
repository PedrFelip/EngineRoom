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
    assert!(
        std::error::Error::source(&error).is_some(),
        "decode failure must preserve its serde_json source"
    );
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
    let error = failing
        .lookup(&["position".into()], Mode::Depth, 20, 1)
        .await
        .unwrap_err();
    assert_eq!(error.code, ReviewErrorCode::Cache);
    assert_eq!(error.operation, "cache.lookup");
    assert!(
        std::error::Error::source(&error).is_some(),
        "database open failure must preserve its rusqlite source"
    );
}

fn manual_time_config(pgn: String, movetime_ms: Option<u32>) -> ReviewConfig {
    ReviewConfig {
        meta: core::extract(&pgn).unwrap().meta,
        pgn,
        engine: EngineTier {
            id: "time".into(),
            depth: 20,
            label: String::new(),
            hint: String::new(),
        },
        mode: Mode::Time,
        analysis_kind: AnalysisKind::Manual,
        movetime_ms,
        lines: 3,
        initial_result: None,
    }
}

async fn save_and_reopen(config: &ReviewConfig) -> ReviewConfig {
    let repo = SqliteRepository::new(temp_path());
    let game = core::extract(&config.pgn).unwrap();
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let review = core::build(&game, &raw).unwrap();
    repo.save(config, &review).await.unwrap();
    let id = repo.list_games(10, None).await.unwrap().games[0].id;
    repo.review_config(id).await.unwrap().unwrap()
}

#[tokio::test]
async fn manual_time_without_movetime_round_trips_to_the_shared_default() {
    let reopened =
        save_and_reopen(&manual_time_config("[White \"Alice\"]\n1. e4".into(), None)).await;
    assert_eq!(reopened.movetime_ms, Some(DEFAULT_MOVETIME_MS));
}

#[tokio::test]
async fn manual_time_explicit_small_budgets_survive_round_trip() {
    for budget in [100, 1000, 4999] {
        let reopened = save_and_reopen(&manual_time_config(
            "[White \"Alice\"]\n1. e4".into(),
            Some(budget),
        ))
        .await;
        assert_eq!(
            reopened.movetime_ms,
            Some(budget),
            "explicit budget {budget} must not be clamped to the default"
        );
    }
}

#[tokio::test]
async fn legacy_manual_time_zero_budget_is_normalized_on_reopen() {
    let pgn = "[White \"Alice\"]\n1. e4".to_string();
    let game = core::extract(&pgn).unwrap();
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let review = core::build(&game, &raw).unwrap();
    let path = temp_path();
    let conn = crate::db::open_file(&path).unwrap();
    // Reproduce the legacy save that recorded 0 for an unspecified budget.
    let id = Store::new(&conn)
        .save(&NewGame {
            pgn,
            white: game.meta.white,
            black: game.meta.black,
            result: game.meta.result,
            plies: game.moves.len() as u32,
            engine_tier: "time".into(),
            mode: Mode::Time,
            analysis_kind: "manual".into(),
            depth: 0,
            multipv: 3,
            accuracy_white: review.accuracy.white,
            accuracy_black: review.accuracy.black,
            review_json: serde_json::to_string(&review).unwrap(),
        })
        .unwrap();
    drop(conn);
    let reopened = SqliteRepository::new(path)
        .review_config(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reopened.movetime_ms, None);
}

#[tokio::test]
async fn contextual_cache_roundtrip_preserves_evidence_and_validates_key() {
    let repo = SqliteRepository::new(temp_path());
    let fen = core::fen(&shakmaty::Chess::default());
    let key = "mainline-unrestricted-v1|stockfish-19|e2e4".to_string();
    let mut raw = core::terminal_raw(&fen, 35);
    raw.depth = 20;
    raw.pv = vec!["e2e4".into()];
    raw.lines[0].depth = Some(20);
    raw.lines[0].pv = vec!["e2e4".into()];
    for (multipv, root, depth) in [(2, "d2d4", 20), (3, "g1f3", 19)] {
        raw.lines.push(RawLine {
            multipv,
            cp: 30,
            pv: vec![root.into()],
            san: None,
            depth: Some(depth),
            unstable: false,
        });
    }
    let evidence = PositionEvidence {
        history_ply: Some(0),
        scope: EvidenceScope::Mainline,
        origin: EvidenceOrigin::Engine,
        completion: EvidenceCompletion::BestMove,
        engine: Some("Stockfish 19".into()),
        engine_build: Some("stockfish-19".into()),
        requested: Some(EvidenceBudget {
            mode: Mode::Time,
            value: 1000,
            multipv: 3,
        }),
        reached_depth: 19,
        observed_lines: 3,
    };
    let entry = ContextualEvaluation {
        key: key.clone(),
        raw,
        evidence,
    };
    repo.put_context(std::slice::from_ref(&entry), Mode::Time, 1000)
        .await
        .unwrap();

    let hits = repo
        .lookup_context(std::slice::from_ref(&key), Mode::Time, 1000, 3)
        .await
        .unwrap();
    let hit = hits[0].as_ref().expect("contextual hit");
    assert_eq!(hit.key, key);
    assert_eq!(hit.evidence.engine_build.as_deref(), Some("stockfish-19"));
    assert_eq!(hit.evidence.reached_depth, 19);
    assert_eq!(hit.raw.lines.len(), 3);

    let smaller = repo
        .lookup_context(std::slice::from_ref(&key), Mode::Time, 1000, 1)
        .await
        .unwrap();
    assert_eq!(smaller[0].as_ref().unwrap().raw.lines.len(), 1);

    // Um orçamento maior ou uma chave diferente não podem reusar a evidência.
    assert!(
        repo.lookup_context(std::slice::from_ref(&key), Mode::Time, 2000, 3)
            .await
            .unwrap()[0]
            .is_none()
    );
    assert!(
        repo.lookup_context(&["outra-chave".into()], Mode::Time, 1000, 3)
            .await
            .unwrap()[0]
            .is_none()
    );
}

#[tokio::test]
async fn contextual_cache_ignora_payload_corrompido_e_clear_cache_limpa_ambos() {
    let path = temp_path();
    let repo = SqliteRepository::new(path.clone());
    let fen = core::fen(&shakmaty::Chess::default());
    // O cache legado e o contextual coexistem no mesmo arquivo.
    repo.put(&[core::terminal_raw(&fen, 15)], Mode::Depth, 20, 1)
        .await
        .unwrap();
    {
        let conn = crate::db::open_file(&path).unwrap();
        crate::db::evidence::EvidenceCache::new(&conn)
            .store(
                "chave",
                Mode::Depth,
                20,
                1,
                20,
                35,
                "[]",
                "{payload inválido",
            )
            .unwrap();
    }
    // Payload corrompido é miss, não erro fatal.
    assert!(
        repo.lookup_context(&["chave".into()], Mode::Depth, 20, 1)
            .await
            .unwrap()[0]
            .is_none()
    );
    // Limpar o cache remove os dois namespaces.
    repo.clear_cache().await.unwrap();
    assert!(
        repo.lookup(std::slice::from_ref(&fen), Mode::Depth, 20, 1)
            .await
            .unwrap()[0]
            .is_none()
    );
    {
        let conn = crate::db::open_file(&path).unwrap();
        assert!(
            crate::db::evidence::EvidenceCache::new(&conn)
                .lookup("chave", Mode::Depth, 20, 1)
                .unwrap()
                .is_none()
        );
    }
}
