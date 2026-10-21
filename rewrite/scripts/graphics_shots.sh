#!/usr/bin/env bash
# Before/after sheets of the Setup graphics options (graphics_menu.c:
# Scaler, Scaling Mode, CRT, Sprites, Visual FX). Each variant is captured
# under Xvfb (never a visible window) -- `--capture=...` writes the FINAL render of the
# root viewport, scaler and CRT filter included, not the raw 320x200 image.
#
# Usage: bash rewrite/scripts/graphics_shots.sh [OUTPUT=${TMPDIR:-/tmp}/ot-par-gfx-shots]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
godot_dir="$(dirname "$here")/godot"
out="${1:-${TMPDIR:-/tmp}/ot-par-gfx-shots}"
GODOT="${GODOT:-godot}"
mkdir -p "$out"
export GODOT_SILENCE_ROOT_WARNING=1

shot() {  # shot NAME [capture options...]
    local name="$1"; shift
    local root; root="$(mktemp -d /tmp/gfxshot.XXXXXX)"
    XDG_DATA_HOME="$root/data" XDG_CACHE_HOME="$root/cache" XDG_CONFIG_HOME="$root/config" \
        xvfb-run -a -s "-screen 0 1280x800x24" timeout 300 \
        "$GODOT" --rendering-driver opengl3 --audio-driver Dummy --path "$godot_dir" -- \
        --capture="title:$out/$name.png:1" "$@" >"$out/$name.log" 2>&1 || true
    rm -r -f -- "$root"
    [ -s "$out/$name.png" ] && echo "ok  $name" || echo "FAILED $name (see $out/$name.log)"
}

# Reference (all defaults).
shot base
# Scaler (index video_scale.c:scalers[]).
for i in 1 2 4 5 8; do shot "scaler-$i" "--scaler=$i"; done
# Scaling Mode (video.c:scaling_mode_names[]).
for i in 0 1 2 3 4 5; do shot "scaling-$i" "--scaling-mode=$i"; done
# The six CRT modes (video.h:CrtMode).
for i in 0 1 2 3 4 5; do shot "crt-$i" "--crt-mode=$i"; done
# Sprites (art_mode) : Original / HD (IA) / Pixel art net.
shot sprites-sharp --xbrz
# Effects levels (config.c:processorType): the scenery changes from
# level 1 on (layer 2 blocked by the water) -- comparison sheet of the report.
for i in 1 3 4; do shot "detail-$i" "--detail=$i" "--crt-mode=0"; done
