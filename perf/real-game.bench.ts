// Real Stockfish scenarios for the production manual review pipeline.
// Reuses the existing example; requires ENGINE_ROOM_STOCKFISH.
//   ENGINE_ROOM_STOCKFISH=/path/to/stockfish \
//     bunx strobe bench perf/real-game.bench.ts --head . --check
// Timed verdicts need a quiet machine and a git base with the same bench file.
// `cpu.work` here is the sum over the whole tree, including every Stockfish
// child; it is NOT comparable to the fake-engine cases in pipeline.bench.ts.
// See perf/measurement-contract.md for units and scopes.
import { bench, cli } from "@hona/strobe"

// The example resolves its PGN fixture from the repository root.
const exe =
  process.platform === "win32"
    ? ".\\target\\release\\examples\\strobe_review.exe"
    : "./target/release/examples/strobe_review"
const command = (scenario: string) => `${exe} ${scenario}`
const env: Record<string, string> = {}
for (const key of [
  "ENGINE_ROOM_STOCKFISH",
  "STROBE_REVIEW_PROFILE",
  "STROBE_REVIEW_THREADS",
  "STROBE_REVIEW_PGN",
]) {
  const value = process.env[key]
  if (value !== undefined) env[key] = value
}

export default bench({
  targets: {
    real: cli(command("fresh"), {
      build:
        "cargo build --release --locked --manifest-path Cargo.toml -p engineroom-backend --example strobe_review",
      env,
    }),
  },
  cases: {
    "manual-fresh": {
      primary: "cpu.work",
      metrics: ["run.duration", "memory.peak"],
      run: (ctx) => ctx.exec(command("fresh")),
    },
    "manual-warm": {
      primary: "cpu.work",
      metrics: ["run.duration", "memory.peak"],
      run: (ctx) => ctx.exec(command("warm")),
    },
    "auto-pipeline": {
      primary: "cpu.work",
      metrics: [
        "run.duration",
        "memory.peak",
        "metric.cached",
        "metric.searched",
        "metric.live_before_completed",
        "metric.live_latency_ms",
        "metric.first_pair_ms",
        "metric.review_ms",
      ],
      run: (ctx) => ctx.exec(command("pipeline-auto")),
    },
    "auto-native": {
      primary: "cpu.work",
      metrics: [
        "run.duration",
        "memory.peak",
        "metric.cached",
        "metric.searched",
        "metric.live_before_completed",
        "metric.live_latency_ms",
        "metric.first_pair_ms",
        "metric.review_ms",
      ],
      run: (ctx) => ctx.exec(command("native-auto")),
    },
  },
})
