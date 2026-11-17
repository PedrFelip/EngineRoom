// Strobe cases for the deterministic parts of the review pipeline.
//   bunx strobe bench perf/pipeline.bench.ts --head . --check
//   bunx strobe bench perf/pipeline.bench.ts --aa --head . --pairs 8
// The harnesses use the production Pipeline / ReviewSessions with the fake
// engine and isolated SQLite caches; see perf/README.md before trusting a
// timed comparison. Every case here excludes Stockfish from cpu.work.
import { bench, cli } from "@hona/strobe"

// Variants are resolved from the git root, so paths are root-relative.
const exe =
  process.platform === "win32"
    ? "experimental\\target\\release\\examples\\pipeline_strobe.exe"
    : "./experimental/target/release/examples/pipeline_strobe"
const cacheExe =
  process.platform === "win32"
    ? "experimental\\target\\release\\examples\\pipeline_strobe_cache.exe"
    : "./experimental/target/release/examples/pipeline_strobe_cache"
const pgn = "experimental/perf/data/immortal.pgn"
const command = (scenario: string) => `${exe} ${scenario} ${pgn}`

// The cache harness owns its fixtures and resolves them from an explicit path;
// `short` interrupts after 2 searches, `long` after 15 and `fen` after 2.
const cacheCmd = (scenario: string) =>
  `${cacheExe} ${scenario} experimental/perf/fixtures/${scenario}.pgn`
// Every counter printed by pipeline_strobe_cache is declared so --check reads
// it. All cases use the fake engine: cpu.work excludes any Stockfish child.
const cacheMetrics = [
  "run.duration",
  "metric.fixture_positions",
  "metric.fixture_moves",
  "metric.fixture_terminals",
  "metric.fixture_search_positions",
  "metric.interrupt_after",
  "metric.interrupt_go",
  "metric.interrupt_completed",
  "metric.interrupt_progress_searched",
  "metric.interrupt_progress_cached",
  "metric.partial_cache_entries",
  "metric.resume_cache_hits",
  "metric.resume_searched",
  "metric.resume_go",
  "metric.result_positions",
  "metric.result_moves",
  "metric.deterministic_ok",
  "metric.ports_acquired",
  "metric.ports_terminated",
  "metric.ports_active",
  "metric.ports_peak_active",
  "metric.teardown_ok",
  "metric.fake_engine",
]

export default bench({
  targets: {
    pipeline: cli(command("pgn"), {
      build:
        "cargo build --release --locked --manifest-path experimental/Cargo.toml -p engineroom-backend --example pipeline_strobe",
    }),
    cache: cli(cacheCmd("short"), {
      build:
        "cargo build --release --locked --manifest-path experimental/Cargo.toml -p engineroom-backend --example pipeline_strobe_cache",
    }),
  },
  cases: {
    "pgn-extract": {
      targets: ["pipeline"],
      primary: "cpu.work",
      metrics: ["run.duration", "metric.positions", "metric.moves"],
      run: (ctx) => ctx.exec(command("pgn")),
    },
    "manual-cold": {
      targets: ["pipeline"],
      primary: "cpu.work",
      metrics: ["run.duration", "metric.positions", "metric.searched", "metric.searches"],
      run: (ctx) => ctx.exec(command("manual")),
    },
    "manual-warm": {
      targets: ["pipeline"],
      primary: "cpu.work",
      metrics: ["run.duration", "metric.cached", "metric.searched", "metric.searches"],
      run: (ctx) => ctx.exec(command("manual-warm")),
    },
    "live-feedback": {
      targets: ["pipeline"],
      primary: "cpu.work",
      metrics: ["run.duration", "metric.live", "metric.classifications", "metric.searches"],
      run: (ctx) => ctx.exec(command("live")),
    },
    "automatic-fast": {
      targets: ["pipeline"],
      primary: "cpu.work",
      metrics: ["run.duration", "metric.positions", "metric.searched"],
      run: (ctx) => ctx.exec(command("automatic")),
    },
    "cache-partial-short": {
      targets: ["cache"],
      primary: "cpu.work",
      metrics: cacheMetrics,
      run: (ctx) => ctx.exec(cacheCmd("short")),
    },
    "cache-partial-long": {
      targets: ["cache"],
      primary: "cpu.work",
      metrics: cacheMetrics,
      run: (ctx) => ctx.exec(cacheCmd("long")),
    },
    "cache-partial-fen": {
      targets: ["cache"],
      primary: "cpu.work",
      metrics: cacheMetrics,
      run: (ctx) => ctx.exec(cacheCmd("fen")),
    },
  },
})
