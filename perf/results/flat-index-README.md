# Flat-indexed review pipeline — A/B benchmark report

Date: 2026-11-17. Baseline commit: `46f804a53e6ebf6a5fe39a827993f57e71f3dd19`
(`feat/session-colors-groups`). Shared checkout; no PR. All benchmark commands ran
under `flock /tmp/engineroom-benchmark.lock` to avoid overlap with the Rune UI agent.

## Change under test

- `engineroom-domain`: `adaptive::targets` now uses a flat `Vec<Option<Target>>`
  indexed by position instead of `BTreeMap<usize, Target>`. The separate
  `order` vector is retained so the stable final sort keeps the original
  first-selection tie-breaking. Position indexes are dense in `0..count`, so
  lookup is O(1) and the tree nodes disappear.
- `engineroom-application`: `engine::evaluate` accumulates UCI MultiPV reports
  in a flat `Vec<Option<RawLine>>` indexed by `multipv - 1` instead of
  `BTreeMap<u32, RawLine>`. Ranks are validated to `1..=256` (Stockfish's MultiPV
  cap); rank 0 was already rejected by `parse_info`. Ascending-rank iteration,
  sparse gaps, deepest-report-wins and instability propagation are preserved.
- The engine/repository seams (`EnginePort`, `EngineFactory`, `Repository`) are
  untouched. No changes outside the application/domain crates; the pre-existing
  Rune sidebar edits were left alone.

## Harness and method

`crates/engineroom-application/examples/bench_flat.rs` is a release-mode example
that uses only public APIs present in both revisions, so the same source was
built against baseline and candidate. It measures:

1. `uci` — 1500 calls to `engine::evaluate` against an in-memory fake port that
   emits 4 ranks x 24 depths per search (96 `info` lines + `bestmove`).
2. `targets` — 8000 calls to `adaptive::targets` over 220 deterministic
   `Critical` candidates, `count = 240`, `AnalysisKind::Fast`.

A wrapping `System` global allocator counts `alloc`/`realloc` calls and bytes;
counters are snapshotted around each workload (setup excluded). Each data point
is a separate process. The baseline binary was built first and preserved at
`/tmp/engineroom-bench-baseline`; the candidate at
`/tmp/engineroom-bench-candidate`. Candidate and baseline runs were interleaved
five times each under the same lock. Times are wall clock on a current-thread
tokio runtime.

Environment: rustc/cargo 1.98.0, x86-64 Linux, AMD Ryzen 7 5700U (16 logical
CPUs, `lscpu` reports scaling at 60%), release profile from the workspace
`Cargo.toml`. `cargo fmt --check`, `cargo clippy --all-targets --all-features --
-D warnings`, `cargo test --workspace --locked` and the real native Stockfish
test all pass on the candidate.

## Raw data

`perf/results/flat-index-baseline.txt` — 5 baseline processes (median UCI
79.3 µs/op, targets 74.8 µs/op).

`perf/results/flat-index-candidate-abba.txt` — 5 interleaved candidate/baseline
pairs. Summary of those 10 matched processes (median of ns/op):

| Workload | Baseline | Candidate | Delta | Allocs/op (base → cand) | Bytes/op (base → cand) |
| --- | ---: | ---: | ---: | ---: | ---: |
| UCI MultiPV accumulate | 79,572 ns | 76,096 ns | **-4.4%** | 688 → 688 | 19,303 → 18,743 (-2.9%) |
| Dense adaptive targets | 75,543 ns | 60,971 ns | **-19.3%** | 24 → 18 (-25%) | 10,704 → 13,792 (+28.8%) |

Individual interleaved samples (ns/op):

- UCI baseline: 79,660 / 79,636 / 79,258 / 79,573 / 79,197
- UCI candidate: 76,096 / 75,610 / 77,639 / 76,513 / 75,425
- Targets baseline: 75,193 / 75,543 / 74,779 / 75,765 / 76,622
- Targets candidate: 63,102 / 60,971 / 60,823 / 60,767 / 61,128

Allocation counters were bit-identical across every run of a given binary.

## Interpretation and limits

- The targets win comes from removing one tree-node allocation per selected
  position (24 → 18 allocs/op) and O(1) slot updates. It pays for a
  `count`-sized slot vector up front, so **bytes allocated per call rise ~29%**
  in this synthetic case (240 slots vs ~18 selected positions). In real games
  `count` equals the position count (~40-90), so the vector is ~1-5 KB and
  freed with the call; the trade-off is deliberate: fewer allocator calls and
  better locality for dense indexes. A compact sorted `Vec<(usize, Target)>`
  would avoid the byte increase but is not a directly indexed equivalent.
- The UCI win is modest. The dominant allocation cost in `evaluate` is the PV
  `String`s produced by `parse_info`, which is unchanged; the flat slots only
  replace four tree nodes with one vector plus growth reallocations, which is
  why the allocation *count* is unchanged while bytes drop slightly.
- The harness uses a fake engine. It excludes Stockfish, the shell adapter,
  SQLite, Tauri IPC and rendering. It is not an end-to-end game speedup.
  The real native Stockfish UCI test passes but no matched A/B timing was run
  with a real engine (engine time dominates and CPU scaling is unstable).
- CPU scaling was reported at 60% and the samples are small (5 per binary),
  interleaved but not a full A/B/B/A run. Treat the percentages as directional
  for Rust-side overhead, not confidence intervals.
- The allocator counter treats `realloc` as a fresh allocation of the new size;
  it is not retained memory or peak RSS, and it observes other threads.

## Packed position/job index — considered, not applied

I evaluated packing the hot metadata into the upper bits of a position/job
index (the `Job { index: usize, refinement: bool }` pair in
`automatic.rs`). On this 64-bit target `Job` is 32 bytes; packing the flag into
a `u32` index would make it 24 bytes, i.e. at most 8 bytes per in-flight job
with 1-3 jobs alive. That is not meaningful storage, and it would hide the
refinement flag behind bit manipulation in the queue/scheduling code. The
dominant per-position storage is the `RawPosition`/`RawLine` payloads from the
engine, not the indexes. Per the "where it removes meaningful storage and stays
maintainable" condition, I left this out.

## Verification

- `cargo fmt --check` — clean
- `cargo clippy --all-targets --all-features -- -D warnings` — clean
- `cargo test --workspace --locked` — all suites pass (incl. 4 new `LatestLines`
  tests and a new randomized equivalence test that compares `targets` against
  a reference copy of the old map algorithm across profiles, counts, sparse and
  out-of-range inputs)
- `ENGINE_ROOM_STOCKFISH=... cargo test -p engineroom-uci real_native_stockfish
  -- --ignored` — passes with the bundled Stockfish binary
