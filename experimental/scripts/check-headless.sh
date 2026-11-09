#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p artifacts
python3 scripts/test-headless-report.py
cargo build --locked
binary="target/debug/engineroom-rune"
"$binary" --snap artifacts/home.png --tree artifacts/home.json --scale 1
for scene in sessions home-file home-chesscom home-valid home-invalid home-light configure configure-deep manual review opening summary history settings settings-light settings-analysis settings-storage; do
  "$binary" --scene "$scene" --snap "artifacts/$scene.png" --tree "artifacts/$scene.json" --scale 1
done
"$binary" --scene review --script scripts/review.json --at 3.8 --snap artifacts/interaction.png --tree artifacts/interaction.json --scale 1
"$binary" --script scripts/settings.json --at 4 --snap artifacts/settings-interaction.png --tree artifacts/settings-interaction.json --scale 1
"$binary" --script scripts/import.json --at 3.8 --snap artifacts/import.png --tree artifacts/import.json --scale 1
"$binary" --scene opening --size 1100x860 --snap artifacts/compact.png --tree artifacts/compact.json --scale 1
for scene in home-file home-chesscom home-valid home-invalid configure configure-deep manual; do
  for size in 720x560 1440x1080; do
    "$binary" --scene "$scene" --size "$size" --snap "artifacts/$scene-$size.png" --tree "artifacts/$scene-$size.json" --scale 1
  done
done
# Sidebar states in both themes and at the minimum and roomy window sizes.
for scene in home-light review-light configure history settings settings-light demo-collapsed; do
  for size in 720x560 1440x1080; do
    "$binary" --scene "$scene" --size "$size" --snap "artifacts/sidebar-$scene-$size.png" --tree "artifacts/sidebar-$scene-$size.json" --scale 1
  done
done
python3 scripts/check-sidebar.py
python3 scripts/check-tree.py
for scene in review review-light; do
  for checkpoint in 1.2 1.6 2.2; do
    "$binary" --scene "$scene" --size 1440x1080 --script scripts/right-sidebar.json --at "$checkpoint" --snap "artifacts/right-$scene-$checkpoint.png" --tree "artifacts/right-$scene-$checkpoint.json" --scale 1
  done
done
python3 scripts/check-right-sidebar.py
python3 scripts/check-review-matrix.py
