use super::*;

#[test]
fn legacy_review_normalization_and_current_data_preservation() {
    for case in cases() {
        let mut legacy = case["expected"].clone();
        assert_parity(
            &legacy,
            &serde_json::to_value(repository::normalize(legacy.clone()).unwrap()).unwrap(),
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
        assert_parity(
            &case["expected"],
            &serde_json::to_value(repository::normalize(legacy).unwrap()).unwrap(),
        );
    }
    assert!(repository::normalize(serde_json::json!({"moves":[],"positions":[]})).is_err());
}
#[test]
fn cache_shape_corruption_and_covering_queries_are_preserved() {
    let conn = db::open_memory().unwrap();
    let cache = Cache::new(&conn);
    let raw: RawPosition = serde_json::from_value(cases()[0]["raw"][0].clone()).unwrap();
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
