//! Opt-in micro-benchmark for the analysis lookup path used by review frames.
use super::*;
use std::hint::black_box;
use std::time::Instant;

const BENCH_GAME: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../perf/data/immortal.pgn"
));

fn bench_time<F: FnMut()>(label: &str, n: u32, mut f: F) {
    let start = Instant::now();
    for _ in 0..n {
        f();
    }
    let elapsed = start.elapsed();
    println!(
        "{label}: {n} iters, {elapsed:?} total, {:?}/iter",
        elapsed / n
    );
}

#[test]
#[ignore = "opt-in analysis lookup benchmark"]
fn analysis_lookup_bench() {
    use engineroom_backend::review::{
        core,
        types::{RawLine, RawPosition},
    };
    use shakmaty::{CastlingMode, Position, san::SanPlus};

    const N: u32 = 20_000;

    let core_game = core::extract(BENCH_GAME).expect("legal bench PGN");
    let raw: Vec<RawPosition> = core_game
        .positions
        .iter()
        .enumerate()
        .map(|(ply, board)| {
            let ply_cp = i32::try_from(ply).expect("bench ply fits in i32");
            let first = board
                .legal_moves()
                .into_iter()
                .next()
                .map(|m| m.to_uci(CastlingMode::Standard).to_string());
            let lines = (0_u32..3)
                .map(|i| RawLine {
                    unstable: false,
                    multipv: i + 1,
                    cp: ply_cp - i.cast_signed() * 10,
                    pv: first.clone().into_iter().collect(),
                    san: None,
                    depth: Some(18),
                })
                .collect();
            RawPosition {
                fen: core_game.fens[ply].clone(),
                cp: ply_cp,
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
    let mut variation = crate::variation::Variation::new(0, state.game.positions()[0].clone());
    for san in &state.game.moves {
        let mv = san
            .parse::<SanPlus>()
            .expect("legal SAN")
            .san
            .to_move(variation.position())
            .expect("legal move");
        variation.append(mv);
    }
    state.variations.push(variation);
    state.active_variation = Some(0);

    for _ in 0..200 {
        black_box(state.position_analysis().map(|a| a.cp));
        black_box(state.candidate_roots().len());
        black_box(state.candidate_previews().len());
    }

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
