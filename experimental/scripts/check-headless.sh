#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p artifacts
cargo build --locked
binary="target/debug/engineroom-rune"
"$binary" --snap artifacts/home.png --tree artifacts/home.json --scale 1
for scene in configure manual review opening summary history settings settings-light settings-analysis settings-storage; do
  "$binary" --scene "$scene" --snap "artifacts/$scene.png" --tree "artifacts/$scene.json" --scale 1
done
"$binary" --script scripts/review.json --at 3.8 --snap artifacts/interaction.png --tree artifacts/interaction.json --scale 1
"$binary" --script scripts/settings.json --at 4 --snap artifacts/settings-interaction.png --tree artifacts/settings-interaction.json --scale 1
"$binary" --script scripts/import.json --at 3.8 --snap artifacts/import.png --tree artifacts/import.json --scale 1
"$binary" --scene opening --size 1100x860 --snap artifacts/compact.png --tree artifacts/compact.json --scale 1
python3 scripts/check-tree.py
