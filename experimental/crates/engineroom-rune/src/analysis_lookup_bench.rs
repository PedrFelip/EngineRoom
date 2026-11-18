//! Opt-in micro-benchmark for the analysis lookup path used by review frames.
use super::*;
use std::hint::black_box;
use std::time::Instant;

const BENCH_GAME: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../perf/data/immortal.pgn"
));

fn bench_time<F: FnMut()>(label: &str, n: usize, mut f: F) {
    let start = Instant::now();
    for _ in 0..n {
        f();
    }
    let elapsed = start.elapsed();
    println!(
        "{label}: {n} iters, {elapsed:?} total, {:?}/iter",
        elapsed / n as u32
    );
}

#[test]
#[ignore = "opt-in analysis lookup benchmark"]
fn analysis_lookup_bench() {
    use engineroom_backend::review::{
        core,
        types::{RawLine, RawPosition},
    };
    use shakmaty::{CastlingMode, Position};

    let core_game = core::extract(BENCH_GAME).expect("legal bench PGN");
    let raw: Vec<RawPosition> = core_game
        .positions
        .iter()
        .enumerate()
        .map(|(ply, board)| {
            let first = board
                .legal_moves()
                .into_iter()
                .next()
                .map(|m| m.to_uci(CastlingMode::Standard).to_string());
            let lines = (0..3)
                .map(|i| RawLine {
                    unstable: false,
                    multipv: i + 1,
                    cp: ply as i32 - i as i32 * 10,
                    pv: first.clone().into_iter().collect(),
                    san: None,
                    depth: Some(18),
                })
                .collect();
            RawPosition {
                fen: core_game.fens[ply].clone(),
                cp: ply as i32,
                depth: 18,
                pv: first.into_iter().collect(),
                lines,
            }
        })
        .collect();
    let review = core::build(&core_game, &raw).expect("bench review builds");

    let mut state = State::fixture();
    state.game = crate::game::parse(BENCH_GAME).expect("legal bench PGN");
    state.ply = state.game.moves.len();
    state.real_result = Some(review);

    // A variation with cursor > 0 makes current_mainline_ply() return None so the
    // FEN lookup scans the whole positions vector, as it does for branch moves.
    let mut variation = crate::variation::Variation::new(0, state.game.positions[0].clone());
    variation.positions = state.game.positions.clone();
    variation.fens = state.game.fens.clone();
    variation.cursor = state.game.moves.len();
    state.variations.push(variation);
    state.active_variation = Some(0);

    for _ in 0..200 {
        black_box(state.position_analysis().map(|a| a.cp));
        black_box(state.candidate_roots().len());
        black_box(state.candidate_previews().len());
    }

    const N: usize = 20_000;
    bench_time("bench:current_fen", N, || {
        black_box(state.current_fen());
    });
    bench_time("bench:position_analysis", N, || {
        black_box(state.position_analysis().map(|a| a.cp));
    });
    bench_time("bench:candidate_roots", N, || {
        black_box(state.candidate_roots().len());
    });
    bench_time("bench:candidate_previews", N, || {
        black_box(state.candidate_previews().len());
    });
    bench_time("analysis_lookup_bench", N, || {
        black_box(state.position_analysis().map(|a| a.cp));
        black_box(state.candidate_roots().len());
        black_box(state.candidate_previews().len());
    });
}
