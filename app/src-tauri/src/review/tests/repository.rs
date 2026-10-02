use super::*;

#[test]
fn cache_rejects_incomplete_duplicate_or_missing_principal_slots() {
    let fen = core::fen(&shakmaty::Chess::default());
    for slots in [vec![1], vec![1, 1, 2], vec![2, 3], vec![1, 3]] {
        let lines: Vec<_> = slots
            .into_iter()
            .map(|multipv| RawLine {
                multipv,
                cp: 15,
                pv: vec!["e2e4".into()],
                san: None,
                depth: Some(10),
                unstable: false,
            })
            .collect();
        let hit = crate::db::cache::CachedPosition {
            cp: 15,
            reached_depth: 10,
            lines_json: serde_json::to_string(&lines).unwrap(),
        };
        assert!(repository::shape(hit, &fen, 2).is_none());
    }
}

#[test]
fn previous_accuracy_model_is_recalculated_from_saved_initial_evaluation() {
    let game = core::extract("[FEN \"7k/8/8/8/8/8/8/R6K w - - 0 1\"]\n1. Ra2 Kg7").unwrap();
    let raw: Vec<_> = game
        .fens
        .iter()
        .enumerate()
        .map(|(i, fen)| core::terminal_raw(fen, if i % 2 == 0 { 700 } else { -700 }))
        .collect();
    let expected = core::build(&game, &raw).unwrap();
    let mut old = serde_json::to_value(&expected).unwrap();
    old["accuracyModel"] = serde_json::json!("lichess-2026-08");
    old["accuracy"] = serde_json::json!({"white": 50.0, "black": 50.0});
    old["accuracyByPhase"]["endgame"] = serde_json::json!({"white": 50.0, "black": 50.0});
    let normalized = repository::normalize(old).unwrap();
    assert_eq!(normalized.accuracy.white, 100.0);
    assert_eq!(normalized.accuracy_by_phase.endgame.white, 100.0);
    assert_eq!(
        normalized.accuracy_model,
        crate::review::scoring::ACCURACY_MODEL
    );
}

#[test]
fn legacy_review_normalization_and_current_data_preservation() {
    let game = core::extract("1. e4 e5 2. Nf3 Nc6").unwrap();
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let expected = serde_json::to_value(core::build(&game, &raw).unwrap()).unwrap();
    let mut legacy = expected.clone();
    assert_eq!(
        expected,
        serde_json::to_value(repository::normalize(legacy.clone()).unwrap()).unwrap()
    );
    legacy.as_object_mut().unwrap().remove("accuracyModel");
    legacy.as_object_mut().unwrap().remove("accuracyByPhase");
    for p in legacy["positions"].as_array_mut().unwrap() {
        p.as_object_mut().unwrap().remove("phase");
    }
    for m in legacy["moves"].as_array_mut().unwrap() {
        m.as_object_mut().unwrap().remove("cpLoss");
        m["classification"] = serde_json::json!("brilhante");
    }
    assert_eq!(
        expected,
        serde_json::to_value(repository::normalize(legacy).unwrap()).unwrap()
    );
    assert!(repository::normalize(serde_json::json!({"moves":[],"positions":[]})).is_err());
}
#[test]
fn cache_shape_corruption_and_covering_queries_are_preserved() {
    let conn = db::open_memory().unwrap();
    let cache = Cache::new(&conn);
    let mut raw = core::terminal_raw(&core::fen(&shakmaty::Chess::default()), 15);
    raw.depth = 14;
    raw.lines = (1..=3)
        .map(|multipv| RawLine {
            unstable: false,
            multipv,
            cp: 15,
            depth: Some(14),
            pv: vec!["e2e4".into()],
            san: None,
        })
        .collect();
    let entry = CachedPositionPut {
        fen: raw.fen.clone(),
        reached_depth: 20,
        cp: raw.cp,
        lines_json: serde_json::to_string(&raw.lines).unwrap(),
    };
    cache.store_many(&[entry], Mode::Time, 1000, 3).unwrap();
    assert!(cache
        .lookup(&raw.fen, Mode::Depth, 19, 2)
        .unwrap()
        .is_some());
    assert!(cache
        .lookup(&raw.fen, Mode::Time, 500, 2)
        .unwrap()
        .is_some());
    assert!(cache
        .lookup(&raw.fen, Mode::Time, 1500, 2)
        .unwrap()
        .is_none());
    let shaped = repository::shape(
        cache.lookup(&raw.fen, Mode::Depth, 20, 2).unwrap().unwrap(),
        &raw.fen,
        2,
    )
    .unwrap();
    assert_eq!(shaped.lines.len(), 2);
    assert_eq!(shaped.depth, 14);
    conn.execute("UPDATE position_cache SET lines_json='broken'", [])
        .unwrap();
    assert!(repository::shape(
        cache.lookup(&raw.fen, Mode::Depth, 20, 2).unwrap().unwrap(),
        &raw.fen,
        2
    )
    .is_none());
}
