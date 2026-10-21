#!/usr/bin/env bash
# Performance benchmark of the Godot client (docs/performance.md).
#
# Prerequisites: scripts/build-linux.sh already run. Under WSL/Xvfb rendering
# goes through Mesa llvmpipe (software): the measured FPS do not represent
# a graphics card; sim_us, refresh_us and shots_us remain meaningful.
# Usage: rewrite/scripts/bench_godot.sh <output.json> [counts] [seconds] [bench.gd options...]
#   e.g.: scripts/bench_godot.sh /tmp/bench.json 0,500,2000,5000 6 --bench-legacy-shots
# Variables: GODOT (default godot), BENCH_DISPLAY (existing X display;
# default: xvfb-run -a).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
godot_dir="$(dirname "$here")/godot"
out="${1:?output.json}"
counts="${2:-0,60,500,1000,2000,5000,10000}"
seconds="${3:-6}"
shift $(( $# < 3 ? $# : 3 ))
GODOT="${GODOT:-godot}"
export GODOT_SILENCE_ROOT_WARNING=1

args=(--rendering-driver opengl3 --path "$godot_dir" -- --bench="$out" --bench-counts="$counts" --bench-seconds="$seconds" "$@")
if [ -n "${BENCH_DISPLAY:-}" ]; then
    DISPLAY="$BENCH_DISPLAY" "$GODOT" "${args[@]}" 2>&1 | grep -E "^bench|ERROR|SCRIPT"
else
    xvfb-run -a -s "-screen 0 1280x800x24" "$GODOT" "${args[@]}" 2>&1 | grep -E "^bench|ERROR|SCRIPT"
fi
