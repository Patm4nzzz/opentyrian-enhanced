#!/usr/bin/env bash
# Screenshots of the Godot client (phase 7) -> docs/screenshots/godot-*.png
#
# Prerequisites: scripts/build-linux.sh already run (bin/, assets/imported/,
# levels/), xvfb-run, ImageMagick (import, convert), Mesa (llvmpipe is enough).
# Usage: rewrite/scripts/capture_godot.sh [output_folder]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rewrite="$(dirname "$here")"
repo="$(dirname "$rewrite")"
godot_dir="$rewrite/godot"
out="${1:-$repo/docs/screenshots}"
GODOT="${GODOT:-godot}"
export GODOT_SILENCE_ROOT_WARNING=1
mkdir -p "$out"
: >"$out/godot-captures.log"


capture() {
    local spec="$1"; shift
    xvfb-run -a -s "-screen 0 1280x800x24" \
        "$GODOT" --rendering-driver opengl3 --path "$godot_dir" -- --capture="$spec" "$@" 2>&1 \
        | grep -E "^capture|ERROR|SCRIPT" | tee -a "$out/godot-captures.log" || true
}

# Game scenes: replay of the golden master that capture_c.sh freezes on the C side.
# The C version freezes the image of tick T (line T of the .gm); on the Godot side it takes
# T + 1 simulation steps (tyrian-sim: Sim::new plays the top of tick 0).
gm="--replay=$rewrite/golden/e1-tyrian-autopilot.gm"
capture "menu:$out/godot-menu.png:10"
capture "level:$out/godot-level1-start.png:61" "$gm"
capture "level:$out/godot-combat.png:4221" "$gm"
capture "level:$out/godot-boss.png:6801" "$gm"
capture "shop:$out/godot-shop.png:10" --shop-loadout=1,1,11,0,1,4,2,0,0,0,15267 --keys=Down,Down,Enter,Enter
capture "level:$out/godot-crt.png:4221" "$gm" --crt
capture "level:$out/godot-bullet-time.png:4221" "$gm" --bullet-time

# Godot editor opened on a generated level scene, capture of the X screen.
editor_png="$out/godot-editor.png"
xvfb-run -a -s "-screen 0 1600x900x24" bash -c "
    TYRIAN_EDITOR_OPEN=res://levels/ep1_09.tscn \"$GODOT\" --rendering-driver opengl3 -e --path \"$godot_dir\" \\
        --resolution 1600x900 --position 0,0 >/tmp/godot-editor.log 2>&1 &
    pid=\$!
    sleep \"\${EDITOR_SETTLE:-35}\"
    import -window root \"$editor_png\"
    kill \$pid 2>/dev/null || true
    wait \$pid 2>/dev/null || true
"
echo "capture: editor -> $editor_png"

# Check: no black image (mean > 0.02).
status=0
for f in "$out"/godot-*.png; do
    mean=$(convert "$f" -format '%[fx:mean]' info:)
    printf '%-28s mean %s\n' "$(basename "$f")" "$mean"
    awk -v m="$mean" 'BEGIN { exit !(m > 0.02) }' || { echo "  -> image too dark"; status=1; }
done
exit $status
