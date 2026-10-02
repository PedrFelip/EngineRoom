use super::*;

#[test]
fn accuracy_uses_fen_start_and_actual_phase_entry_evaluation() {
    let game = core::extract("[FEN \"7k/8/8/8/8/8/8/R6K w - - 0 1\"]\n1. Ra2 Kg7 2. Ra3").unwrap();
    let raw: Vec<_> = game
        .fens
        .iter()
        .enumerate()
        .map(|(i, fen)| core::terminal_raw(fen, if i % 2 == 0 { 700 } else { -700 }))
        .collect();
    let review = core::build(&game, &raw).unwrap();
    assert_eq!(review.accuracy.white, 100.0);
    assert_eq!(review.accuracy.black, 100.0);
    assert_eq!(review.accuracy_by_phase.endgame.white, 100.0);
    assert_eq!(review.accuracy_by_phase.endgame.black, 100.0);

    let opening = core::extract("1. a3 a6 2. h3 h6").unwrap();
    let raw: Vec<_> = opening
        .fens
        .iter()
        .enumerate()
        .map(|(i, fen)| {
            core::terminal_raw(
                fen,
                if i == 0 {
                    0
                } else if i % 2 == 0 {
                    700
                } else {
                    -700
                },
            )
        })
        .collect();
    let mut review = core::build(&opening, &raw).unwrap();
    let phases = [
        Phase::Opening,
        Phase::Opening,
        Phase::Endgame,
        Phase::Endgame,
        Phase::Endgame,
    ];
    let values: Vec<_> = review.positions.iter().map(|p| p.win_pct).collect();
    review.accuracy_by_phase =
        crate::review::scoring::phase_accuracy(&review.moves, &phases, &values);
    assert_eq!(review.accuracy_by_phase.endgame.white, 100.0);
    assert_eq!(review.accuracy_by_phase.endgame.black, 100.0);
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

#[test]
fn situation_priorities_context_quotas_and_overlapping_requirements() {
    use adaptive::{Critical, RefinementKind as K};
    let candidate = |ply, score, hard, reason: &str| Critical {
        ply,
        score,
        hard,
        reasons: vec![reason.into()],
        promotion: false,
    };
    let profile = adaptive::profile(AnalysisKind::Deep).unwrap();
    let candidates = vec![
        candidate(5, 45, true, "perda de avaliação"),
        candidate(6, 90, true, "sequência de mate"),
        candidate(12, 90, false, "posição complexa"),
        candidate(16, 35, false, "classificação incerta"),
    ];
    let targets = adaptive::targets(&candidates, 40, profile);
    assert_eq!(targets[0].kind, K::Mate);
    // A shared position keeps the strongest time AND alternative requirements.
    let shared = targets.iter().find(|t| t.position_index == 5).unwrap();
    assert_eq!(shared.search.ms, 6000);
    assert_eq!(shared.search.multipv, 2);
    let uncertain = targets.iter().position(|t| t.kind == K::Uncertain).unwrap();
    let complex = targets.iter().position(|t| t.kind == K::Complex).unwrap();
    assert!(uncertain < complex);
    assert!(targets
        .iter()
        .filter(|t| t.kind == K::Context)
        .all(|t| !t.hard));
    assert!(targets.len() <= 14);
    assert!(targets.iter().all(|t| t.kind != K::Context));
    let capped = adaptive::Profile {
        minimum: 2,
        fraction: 0.0,
        ..profile
    };
    let targets = adaptive::targets(&candidates, 40, capped);
    assert_eq!(targets.len(), 3); // Required overlapping pairs bypass the quota.
    assert!(targets.iter().all(|t| t.hard));
}

#[test]
fn adaptive_parameters_use_wide_baseline_and_focused_refinement() {
    use adaptive::RefinementKind as K;
    for kind in [AnalysisKind::Fast, AnalysisKind::Deep] {
        let profile = adaptive::profile(kind).unwrap();
        assert_eq!(
            profile.triage_multipv,
            if kind == AnalysisKind::Fast { 3 } else { 5 }
        );
        for situation in [
            K::Context,
            K::Complex,
            K::Tactical,
            K::Uncertain,
            K::Critical,
            K::Promotion,
            K::Mate,
        ] {
            assert!(profile.budget(situation).multipv < profile.triage_multipv);
        }
        assert!(profile.budget(K::Critical).multipv > profile.budget(K::Context).multipv);
        assert!(profile.triage_ms < profile.budget(K::Context).ms);
        assert!(profile.budget(K::Context).ms < profile.budget(K::Uncertain).ms);
        assert!(profile.budget(K::Uncertain).ms < profile.budget(K::Tactical).ms);
        assert!(profile.budget(K::Tactical).ms < profile.budget(K::Critical).ms);
    }
}

#[test]
fn quiet_losses_and_uncertain_classification_do_not_expand_neighbors() {
    let game = core::extract("1. a3 a6 2. h3 h6 3. f3 f6 4. g3 g6 5. Kf2 Kf7").unwrap();
    let profile = adaptive::profile(AnalysisKind::Deep).unwrap();
    for cp in [500, 100] {
        let raw: Vec<_> = game
            .fens
            .iter()
            .enumerate()
            .map(|(i, fen)| core::terminal_raw(fen, if i == 1 { cp } else { 0 }))
            .collect();
        let targets = adaptive::review_targets(&game, &raw, profile);
        assert!(!targets.is_empty());
        assert!(targets
            .iter()
            .all(|t| t.kind != adaptive::RefinementKind::Context));
    }
}

#[test]
fn sacrifice_context_follows_reply_and_stops_at_calm_stable_move() {
    let game = core::extract(
        "[SetUp \"1\"]\n[FEN \"7k/8/5p2/8/8/8/8/R1B4K w - - 0 1\"]\n1. Bg5 fxg5 2. Ra2 Kg7 3. Ra3",
    )
    .unwrap();
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let profile = adaptive::profile(AnalysisKind::Fast).unwrap();
    let targets = adaptive::review_targets(&game, &raw, profile);
    assert!(targets
        .iter()
        .any(|t| t.position_index == 0 && t.kind == adaptive::RefinementKind::Tactical));
    assert!(targets
        .iter()
        .any(|t| t.position_index == 2 && t.kind == adaptive::RefinementKind::Context));
    assert!(!targets.iter().any(|t| t.position_index >= 3));
    // A quiet move still belongs to the context while the baseline changes.
    let mut unstable = raw.clone();
    for (i, value) in unstable.iter_mut().enumerate().skip(3) {
        value.cp = if i % 2 == 0 { 30 } else { -30 };
    }
    let focused = adaptive::review_targets(&game, &unstable, profile);
    assert!(!focused.iter().any(|t| t.position_index >= 3));
    let expanded = adaptive::review_targets(
        &game,
        &unstable,
        adaptive::Profile {
            context_plies: 4,
            ..profile
        },
    );
    assert!(expanded
        .iter()
        .any(|t| t.position_index == 3 && t.kind == adaptive::RefinementKind::Context));
    assert!(!expanded.iter().any(|t| t.position_index >= 4));
    // Surrendering material without evaluation compensation is not a sacrifice seed.
    let mut losing = raw.clone();
    for (i, value) in losing.iter_mut().enumerate().skip(2) {
        value.cp = if i % 2 == 0 { -60 } else { 60 };
    }
    assert!(!adaptive::review_targets(&game, &losing, profile)
        .iter()
        .any(|t| t.position_index == 0 && t.kind == adaptive::RefinementKind::Tactical));
    let capped = adaptive::Profile {
        minimum: 2,
        fraction: 0.0,
        ..profile
    };
    assert!(adaptive::review_targets(&game, &raw, capped)
        .iter()
        .all(|t| t.kind != adaptive::RefinementKind::Context));
}

#[test]
fn context_stays_inside_forcing_sequence_and_profile_limit() {
    let game = core::extract("[SetUp \"1\"]\n[FEN \"4k1r1/5p2/8/8/2B5/8/8/R6K w - - 0 1\"]\n1. Bxf7+ Kxf7 2. Ra2 Rg7 3. Ra3").unwrap();
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let profile = adaptive::profile(AnalysisKind::Deep).unwrap();
    let targets = adaptive::review_targets(&game, &raw, profile);
    assert!(targets.iter().any(|t| t.position_index == 2));
    assert!(!targets.iter().any(|t| t.position_index >= 3));
    let bounded = adaptive::Profile {
        context_plies: 0,
        ..profile
    };
    assert!(adaptive::review_targets(&game, &raw, bounded)
        .iter()
        .all(|t| t.kind != adaptive::RefinementKind::Context));
}

#[test]
fn equal_piece_exchange_is_not_a_sacrifice_context_seed() {
    let game = core::extract("[SetUp \"1\"]\n[FEN \"7k/8/7p/6b1/8/8/8/R1B4K w - - 0 1\"]\n1. Bxg5 hxg5 2. Ra2 Kg7 3. Ra3").unwrap();
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let targets =
        adaptive::review_targets(&game, &raw, adaptive::profile(AnalysisKind::Deep).unwrap());
    assert!(targets.is_empty());
}
