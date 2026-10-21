#!/usr/bin/env bash
# Godot image sequence (TyrianScreen, --frames) for visual parity with
# rewrite/scripts/visual_capture_c.sh.
#
# Usage: rewrite/scripts/visual_capture_godot.sh SCENE OUTPUT_DIR [keys] [Godot options...]
#   SCENE   : menu | setup | ... (tools/capture.gd:SCENES)
#   keys    : list separated by "," of Godot names (Down, Up, Enter, Escape...)
#   e.g.: visual_capture_godot.sh menu /tmp/g-menu Down,Down,Enter
# Prerequisites: scripts/build-linux.sh run (bin/libtyrian_godot.so).
#
# Each capture starts from a blank user folder and cache (like the
# temporary HOME of visual_capture_c.sh): captures launched in parallel
# no longer fight over the shader cache or the settings (errors
# "shader_file_header" / "variant_size" otherwise). Dummy audio driver: the
# Xvfb session has no sound card, ALSA would fail (ERR_CANT_OPEN); Music and
# Sfx still run and go through Game.quit_cleanly on shutdown.
# Any remaining ERROR / SCRIPT ERROR makes the capture fail.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
godot_dir="$(dirname "$here")/godot"
scene="${1:?scene}"; out="${2:?output folder}"; keys="${3:-}"
shift 2; [ $# -gt 0 ] && shift
GODOT="${GODOT:-godot}"
export GODOT_SILENCE_ROOT_WARNING=1

mkdir -p "$out"
rm -f "$out"/*.otfr
log="$(mktemp)"
user_root="$(mktemp -d)"
trap 'rm -f "$log"; rm -rf "$user_root"' EXIT
status=0
XDG_DATA_HOME="$user_root/data" XDG_CACHE_HOME="$user_root/cache" XDG_CONFIG_HOME="$user_root/config" \
xvfb-run -a -s "-screen 0 1280x800x24" timeout "${CAPTURE_TIMEOUT:-600}" \
    "$GODOT" --rendering-driver opengl3 --audio-driver Dummy --path "$godot_dir" -- \
    --capture="$scene:$out/last.png:1" --frames="$out" --frames-dedup ${keys:+--keys=$keys} "$@" >"$log" 2>&1 || status=$?
grep -E "^capture|ERROR|SCRIPT|leaked|still in use" "$log" || true
if [ "$status" -ne 0 ] || grep -Eq 'SCRIPT ERROR:|(^|[[:space:]])ERROR:|leaked at exit|still in use at exit' "$log"; then
    cat "$log" >&2
    exit 1
fi
echo "godot: $(find "$out" -maxdepth 1 -name '*.otfr' ! -name '.*' -printf . | wc -c) images -> $out"
