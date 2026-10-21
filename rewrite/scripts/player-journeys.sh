#!/usr/bin/env bash
# Player journeys with verified effects: the mandatory gate before any push to
# master and before any version handed to the maintainer (docs/testing.md).
#
#   rewrite/scripts/player-journeys.sh [--core] [--package DIR] [--build] [--out DIR]
#                                      [--only a,b] [--profile DIR]
#
# Plays the exported Linux package like a player: Xvfb (1280x684 letterbox x3;
# full batch: the core also at 1920x1080), two games at a time at most, Dummy
# audio, --max-fps 70, real X keys and clicks (xdotool), each journey
# with keyboard only then mouse only. After each action, the EFFECT is read back from the game (autoload
# JourneyState, --journey-state=<file>) or from the files it writes
# (saves, setup.cfg, ranking): money debited by the price, item mounted
# and marked on screen, option changed then kept, save read back
# identically. An image alone is never enough for a PASS.
#
# Journeys (rewrite/scripts/player_journeys.py, JOURNEYS): shop (purchase in
# each category, sale, Ship Specs, data cube), pause (each option,
# each value, F2-F12, Quit box No / Yes), end of level (equipment and
# money kept), save / load, title (each entry, Setup: value
# changed, kept when reopening and after restart), arcade 1P / 2P, Endless,
# The maintainer's real profile (--profile: copy of his user folder).
# --core: shop, pause, save, title, profile, lightened, 1280x684 (about 2 min 40 s).
#
# Output: OUT/report.md (PASS/FAIL table, each FAIL with its screenshot),
# OUT/steps.jsonl, OUT/shots/. Code 0 if everything passes. A full run without
# FAIL writes OUT/../player-journeys.pass (git head + fingerprint of the
# modified files), which player-smoke.sh requires for the same head.
#   --package DIR  already built package (default: <repo>/build/linux/OpenTyrian2000-Enhanced)
#   --build        (re)builds the package first (package.sh --only=linux)
#   --out DIR      results folder (default: <repo>/build/player-journeys)
#   --jobs N       simultaneous games (default 2; 1 when WSL is loaded)
#   --profile DIR  the maintainer's Godot user folder (opentyrian.dat, saves/, setup.cfg...)
# Only the PIDs launched by the script are killed (process groups), never by name.
set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rewrite="$(dirname "$here")"
repo="$(dirname "$rewrite")"
pkg="$repo/build/linux/OpenTyrian2000-Enhanced"
OUT="$repo/build/player-journeys"
profile=""
do_build=0
args=()
while [ $# -gt 0 ]; do
    case "$1" in
        --package) pkg="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        --profile) profile="$2"; shift 2 ;;
        --build) do_build=1; shift ;;
        --core) args+=(--core); shift ;;
        --only) args+=(--only "$2"); shift 2 ;;
        --modes) args+=(--modes "$2"); shift 2 ;;
        --sizes) args+=(--sizes "$2"); shift 2 ;;
        --jobs) args+=(--jobs "$2"); shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
for tool in Xvfb xdotool import python3; do
    command -v "$tool" >/dev/null || { echo "tool missing: $tool (apt install xvfb xdotool imagemagick python3-pil)" >&2; exit 2; }
done
if [ "$do_build" = 1 ] || [ ! -x "$pkg/OpenTyrian2000-Enhanced.x86_64" ]; then
    echo "== building the Linux package"
    mkdir -p "$(dirname "$OUT")"
    bash "$here/package.sh" --only=linux >"$OUT.build.log" 2>&1 || { echo "build failed: see $OUT.build.log" >&2; exit 1; }
fi
# The maintainer's profile: default = his Windows folder seen from WSL, if it exists (copied, never modified).
if [ -z "$profile" ]; then
    for p in /mnt/c/Users/*/AppData/Roaming/Godot/app_userdata/"OpenTyrian2000 Enhanced"; do
        [ -f "$p/opentyrian.dat" ] && profile="$p"
    done
fi
if [ -n "$profile" ]; then
    copy="$(dirname "$OUT")/player-journeys-profile"
    rm -rf "$copy"; mkdir -p "$copy"
    for f in opentyrian.dat c_profile_imported display.cfg setup.cfg highscores.json saves mods; do
        [ -e "$profile/$f" ] && cp -r "$profile/$f" "$copy/"
    done
    args+=(--profile "$copy")
fi
head_id() {  # git head + fingerprint of the uncommitted changes
    printf '%s %s\n' "$(git -C "$repo" rev-parse HEAD)" "$(git -C "$repo" diff HEAD -- . ':!build' | sha1sum | cut -c1-12)"
}
rev="$(git -C "$repo" rev-parse --short HEAD)"
rm -rf "$OUT"
started=$(date +%s)
python3 "$here/player_journeys.py" --package "$pkg" --out "$OUT" --rev "$rev" "${args[@]}"
rc=$?
stamp="$(dirname "$OUT")/player-journeys.pass"
core=0; partial=0
for a in "${args[@]}"; do
    [ "$a" = --core ] && core=1
    case "$a" in --only|--modes|--sizes) partial=1 ;; esac
done
if [ "$rc" -eq 0 ] && [ "$partial" = 0 ]; then
    { head_id; echo "core=$core"; echo "duration=$(( $(date +%s) - started ))s"; } >"$stamp"
    echo "== player journeys: all PASS ($stamp)"
elif [ "$rc" -eq 0 ]; then
    echo "== player journeys: all PASS (partial batch: does not count for the gate)"
else
    rm -f "$stamp"
    echo "== player journeys: FAIL (see $OUT/report.md)"
fi
exit "$rc"
