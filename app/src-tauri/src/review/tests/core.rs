use super::*;

#[test]
fn frozen_typescript_core_and_adaptive_parity() {
    for case in cases() {
        let game = core::extract(case["pgn"].as_str().unwrap()).unwrap();
        assert_parity(&case["moves"], &serde_json::to_value(&game.moves).unwrap());
        let raw: Vec<RawPosition> = serde_json::from_value(case["raw"].clone()).unwrap();
        assert_eq!(
            game.fens,
            raw.iter().map(|r| r.fen.clone()).collect::<Vec<_>>()
        );
        let review = core::build(&game, &raw).unwrap();
        assert_parity(&case["expected"], &serde_json::to_value(&review).unwrap());
        let critical = adaptive::rank(&game, &raw);
        assert_parity(&case["critical"], &serde_json::to_value(&critical).unwrap());
        assert_parity(
            &case["targets"],
            &serde_json::to_value(adaptive::targets(
                &critical,
                raw.len(),
                adaptive::profile(AnalysisKind::Fast).unwrap(),
            ))
            .unwrap(),
        );
        let mut sans = raw.clone();
        for p in &mut sans {
            core::add_san(p);
        }
        assert_parity(&case["raw"], &serde_json::to_value(sans).unwrap());
    }
}
#[test]
fn pgn_comments_variations_fen_promotion_and_invalid_moves() {
    let g = core::extract("[White \"Alice\"]\n1. e4 {hello} e5 (1... c5) 2. Nf3 $1 Nc6 *").unwrap();
    assert_eq!(g.moves.len(), 4);
    assert_eq!(g.meta.white, "Alice");
    assert_eq!(
        core::extract("1. e4 e5 1/2-1/2").unwrap().meta.result,
        "1/2-1/2"
    );
    let g =
        core::extract("[SetUp \"1\"]\n[FEN \"7k/P7/8/8/8/8/8/7K w - - 0 1\"]\n1. a8=Q+").unwrap();
    assert_eq!(g.moves[0].san, "a8=Q+");
    assert_eq!(g.moves[0].uci, "a7a8q");
    assert!(g.fens[0].starts_with("7k/P7"));
    assert!(core::extract("1. e5").is_err());
    assert!(core::extract("").is_err());
    assert!(core::extract("1. e4 e5 1-0\n\n1. d4 d5 0-1").is_err());
    assert!(core::extract("[FEN \"bad\"]\n1. e4").is_err());
    let g = core::extract("1. f3 e5 2. g4 Qh4#").unwrap();
    assert_eq!(
        core::terminal(g.fens.last().unwrap()).unwrap(),
        Some(-100000)
    );
    assert_eq!(
        core::terminal("7k/8/8/8/8/8/8/7K w - - 0 1").unwrap(),
        Some(0)
    );
    assert_eq!(
        core::terminal("7k/8/8/8/8/8/8/NN5K w - - 0 1").unwrap(),
        None
    );
    assert_eq!(
        core::terminal("7k/8/8/8/8/8/8/R6K w - - 100 1").unwrap(),
        Some(0)
    );
}
