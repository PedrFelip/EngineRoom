#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Fixed input and optimized code make comparisons independent of debug overhead.
cargo build --release --locked
mkdir -p artifacts/sidebar-profile
cat > artifacts/sidebar-profile/resize.json <<'JSON'
{"events":[{"t":0.3,"type":"drag","key":"review-sidebar-grip","dx":40,"dy":0,"dur":1.2},{"t":1.8,"type":"drag","key":"review-sidebar-grip","dx":-80,"dy":0,"dur":1.2}]}
JSON
for scene in review review-long demo-progress; do
  for run in 1 2 3; do
    target/release/engineroom-rune --scene "$scene" \
      --script artifacts/sidebar-profile/resize.json --at 3.2 \
      --snap "artifacts/sidebar-profile/$scene-$run.png" --scale 1 \
      > "artifacts/sidebar-profile/$scene-$run.log" 2>&1
    cat "artifacts/sidebar-profile/$scene-$run.log"
  done
done
