#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p artifacts
cargo build --locked
binary="target/debug/engineroom-rune"
for size in 720x560 900x700 1024x768 1440x1080; do
  "$binary" --size "$size" --snap "artifacts/home-$size.png" --tree "artifacts/home-$size.json" --scale 1
  for scene in review settings settings-analysis manual; do
    "$binary" --scene "$scene" --size "$size" --snap "artifacts/$scene-$size.png" --tree "artifacts/$scene-$size.json" --scale 1
  done
done
"$binary" --scene settings-light --size 720x560 --snap artifacts/settings-light-720x560.png --tree artifacts/settings-light-720x560.json --scale 1
"$binary" --scene review --size 720x560 --script scripts/narrow-review.json --at 2 --snap artifacts/review-narrow-scrolled.png --tree artifacts/review-narrow-scrolled.json --scale 1
"$binary" --scene settings --size 720x560 --script scripts/narrow-settings.json --at 1.3 --snap artifacts/settings-narrow-scrolled.png --tree artifacts/settings-narrow-scrolled.json --scale 1
"$binary" --scene review --size 720x560 --script scripts/sidebar-responsive.json --at 1 --snap artifacts/review-collapsed-720x560.png --tree artifacts/review-collapsed-720x560.json --scale 1
"$binary" --scene settings --size 720x560 --script scripts/sidebar-wide.json --at 1 --snap artifacts/settings-sidebar-wide-720x560.png --tree artifacts/settings-sidebar-wide-720x560.json --scale 1
python3 scripts/check-responsive.py
