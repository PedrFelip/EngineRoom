// Strobe cases for the deterministic parts of the review pipeline.
//   bunx strobe bench perf/pipeline.bench.ts --head . --check
//   bunx strobe bench perf/pipeline.bench.ts --aa --head . --pairs 8
// The harness uses the production Pipeline with the fake engine and an isolated
// SQLite cache; see perf/README.md before trusting a timed comparison.
import { bench, cli } from "@hona/strobe"

// Variants are resolved from the git root, so paths are root-relative.
const exe =
  process.platform === "win32"
    ? "experimental\\target\\release\\examples\\pipeline_strobe.exe"
    : "./experimental/target/release/examples/pipeline_strobe"
const pgn = "experimental/perf/data/immortal.pgn"
const command = (scenario: string) => `${exe} ${scenario} ${pgn}`

export default bench({
  targets: {
    pipeline: cli(command("pgn"), {
      build:
        "cargo build --release --locked --manifest-path experimental/Cargo.toml -p engineroom-backend --example pipeline_strobe",
    }),
  },
  cases: {
    "pgn-extract": {
      primary: "cpu.work",
      metrics: ["run.duration", "metric.positions", "metric.moves"],
      run: (ctx) => ctx.exec(command("pgn")),
    },
    "manual-cold": {
      primary: "cpu.work",
      metrics: ["run.duration", "metric.positions", "metric.searched", "metric.searches"],
      run: (ctx) => ctx.exec(command("manual")),
    },
    "manual-warm": {
      primary: "cpu.work",
      metrics: ["run.duration", "metric.cached", "metric.searched", "metric.searches"],
      run: (ctx) => ctx.exec(command("manual-warm")),
    },
    "live-feedback": {
      primary: "cpu.work",
      metrics: ["run.duration", "metric.live", "metric.classifications", "metric.searches"],
      run: (ctx) => ctx.exec(command("live")),
    },
    "automatic-fast": {
      primary: "cpu.work",
      metrics: ["run.duration", "metric.positions", "metric.searched"],
      run: (ctx) => ctx.exec(command("automatic")),
    },
  },
})
