#!/usr/bin/env bash
# Reproducible Strobe verification for the review pipeline harnesses.
# Optional: --timed runs an A/A block (8 pairs) to sanity-check this machine.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$here/.." && pwd)"
strobe="$here/node_modules/.bin/strobe"

if [[ ! -x "$strobe" ]]; then
  (cd "$here" && bun install --frozen-lockfile)
fi

(cd "$here" && cargo build --release --locked -p engineroom-backend \
  --example pipeline_strobe --example pipeline_strobe_cache --example strobe_review)

(cd "$here" && python3 scripts/test-strobe-harness.py)

# Strobe resolves variants from the git root; the bench file is root-relative.
cd "$root"
"$strobe" bench experimental/perf/pipeline.bench.ts --head . --check

if [[ -n "${ENGINE_ROOM_STOCKFISH:-}" ]]; then
  "$strobe" bench experimental/perf/real-game.bench.ts --head . --check
else
  echo "skip: real-game bench (defina ENGINE_ROOM_STOCKFISH)" >&2
fi

if [[ "${1:-}" == "--timed" ]]; then
  "$strobe" bench experimental/perf/pipeline.bench.ts --aa --head . --pairs 8
fi
