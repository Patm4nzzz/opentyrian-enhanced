#!/usr/bin/env bash
# "Player" check of the Linux package: to be run locally before any push to
# master (GitHub Actions CI is unavailable: billing).
#
#   rewrite/scripts/player-smoke.sh [--package DIR] [--build] [--update-refs] [--out DIR]
#
# What the script does, on the exported package (not on the Godot project):
#   1. package content: demo.1-5, cubetxt1-5.dat, levels, SoundFont, MIDI,
#      icons, native library, locales;
#   2. strict replay of 2 golden masters (ticks identical to the C, 18 columns including
#      the seven state fingerprints), in --headless;
#   3. game played under Xvfb 1280x684 (letterboxed window), Dummy audio,
#      --max-fps 70, real X inputs (xdotool): title, new game,
#      shop (hover + click on 3 entries), level 1 (keyboard then mouse),
#      pause, Quit box (hover Yes/No), back to title, demo, Load,
#      Setup (hover on each row), Jukebox, exit through the game; then arcade
#      mode Game Menu with the mouse (hover Options / Quit, click = Quit box);
#      startup: first frame in under 1.5 s (C: 0.1 s);
#   4. at each step: 320x200 game image compared to rewrite/golden/player-smoke
#      (perceptual tolerance, see player_smoke_compare.py);
#   5. failure on SCRIPT ERROR / ERROR in the Godot log, crash, unexpected exit;
#   6. linters (lint.sh): formatting and warnings, all at zero;
#   7. requires an all-PASS run of player-journeys.sh on the same head (docs/testing.md):
#      the images above do not prove that an action has its effect.
#
# Output: OUT/report.md (+ screenshots ref | obtained | gap), code 0 (OK) or 1.
#   --package DIR   already built package (default: <repo>/build/linux/OpenTyrian2000-Enhanced)
#   --build         (re)builds the package first (package.sh --only=linux, ~5 min)
#   --update-refs   replaces the references with the obtained screenshots (review them!)
#   --out DIR       results folder (default: <repo>/build/player-smoke)
# SMOKE_DISPLAY_FIRST=N: first Xvfb display number tried (default: 400 + script PID modulo 500,
# specific to each run); only the PIDs launched by the script are killed, never by name.
# Only one game lives at a time; Xvfb and the game are killed when the script exits.
set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rewrite="$(dirname "$here")"
repo="$(dirname "$rewrite")"
pkg="$repo/build/linux/OpenTyrian2000-Enhanced"
OUT="$repo/build/player-smoke"
do_build=0
update_refs=0
while [ $# -gt 0 ]; do
    case "$1" in
        --package) pkg="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        --build) do_build=1; shift ;;
        --update-refs) update_refs=1; shift ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
refs="$rewrite/golden/player-smoke"
cmp_py="$here/player_smoke_compare.py"
exe_name=OpenTyrian2000-Enhanced.x86_64
export GODOT_SILENCE_ROOT_WARNING=1
started=$(date +%s)

for tool in Xvfb xdotool import python3; do
    command -v "$tool" >/dev/null || { echo "tool missing: $tool (apt install xvfb xdotool imagemagick python3-pil python3-numpy)" >&2; exit 2; }
done

if [ "$do_build" = 1 ] || [ ! -x "$pkg/$exe_name" ]; then
    echo "== building the Linux package"
    bash "$here/package.sh" --only=linux >"$OUT.build.log" 2>&1 || { echo "build failed: see $OUT.build.log" >&2; exit 1; }
fi
[ -x "$pkg/$exe_name" ] || { echo "package missing: $pkg/$exe_name" >&2; exit 2; }

rm -rf "$OUT"
mkdir -p "$OUT"/{got,ref,diff,raw,home} "$refs"
results="$OUT/results.tsv"
: >"$results"
fails=0
record() {  # name status detail
    printf '%s\t%s\t%s\n' "$1" "$2" "$3" >>"$results"
    [ "$2" = FAIL ] && fails=$((fails + 1))
    printf '  %-28s %-4s %s\n' "$1" "$2" "$3"
}

GP=""
XV=""
# shellcheck disable=SC2317  # called by trap EXIT
cleanup() {
    [ -n "${ARMOR:-}" ] && kill "$ARMOR" 2>/dev/null
    [ -n "$GP" ] && kill "$GP" 2>/dev/null
    sleep 0.5
    [ -n "$XV" ] && kill "$XV" 2>/dev/null
    return 0
}
trap cleanup EXIT

# ---------------------------------------------------------------- 1. content
echo "== 1. package content"
missing=()
need() { [ -e "$pkg/$1" ] || missing+=("$1"); }
for n in 1 2 3 4 5; do need "data/demo.$n"; need "data/cubetxt$n.dat"; need "data/levels$n.dat"; need "data/tyrian$n.lvl"; done
for f in data/tyrian.hdt data/tyrian.shp data/tyrianc.shp data/tyrian.pic data/palette.dat data/soundfont.sf2 data/sprite_manifest.json \
    "$exe_name" OpenTyrian2000-Enhanced.pck libtyrian_godot.so COPYING \
    icons/tyrian2000-32.png icons/tyrian2000-128.png data/locales/locale_en.txt; do need "$f"; done
midi_count=$(find "$pkg/data/midi" -name '[0-9][0-9]-*.mid' 2>/dev/null | wc -l)
[ "$midi_count" -ge 30 ] || missing+=("data/midi/*.mid ($midi_count found)")
if [ ${#missing[@]} -eq 0 ]; then record "package-content" PASS "demo.*, cubetxt*, levels, music ($midi_count MIDI), icons"
else record "package-content" FAIL "missing: ${missing[*]}"; fi

# ------------------------------------------------- 2. strict replay (golden masters)
echo "== 2. strict replay of 2 golden masters (C = reference)"
for gm in e1-tyrian-autopilot e2-first-autopilot; do
    log="$OUT/replay-$gm.log"
    ( cd "$pkg" && HOME="$OUT/home" timeout 180 "./$exe_name" --headless --audio-driver Dummy --max-fps 70 -- \
        --user-dir="$OUT/home/userdata" \
        --capture="level:$OUT/raw/replay.png:1000000" --replay="$repo/rewrite/golden/$gm.gm" --replay-strict ) >"$log" 2>&1
    rc=$?
    if [ $rc -eq 0 ] && grep -q "strict replay: [0-9]* ticks identical to the C" "$log"; then
        record "replay:$gm" PASS "$(grep -o '[0-9]* ticks identical to the C' "$log")"
    else
        record "replay:$gm" FAIL "code $rc, $(grep -m1 -E 'DIVERGENT|tick [0-9]+|ERROR' "$log" | cut -c1-120)"
    fi
done

# ------------------------------------------------------------ 3. game played
echo "== 3. game played (Xvfb 1280x684, Dummy audio, 70 fps)"
load=$(cut -d' ' -f1 /proc/loadavg | cut -d. -f1)
[ "$load" -le 40 ] || { echo "load $load > 40: waiting"; while [ "$(cut -d. -f1 /proc/loadavg | cut -d' ' -f1)" -gt 40 ]; do sleep 10; done; }

user_home="$HOME"  # lint.sh (rustup, PowerShell modules) reads the real HOME
export HOME="$OUT/home"
rm -rf "$HOME"; mkdir -p "$HOME"
# No -displayfd: under WSL /tmp/.X11-unix is not 1777 and Xvfb loops over all the numbers.
dispnum=""
first_disp="${SMOKE_DISPLAY_FIRST:-$((400 + $$ % 500))}"
for n in $(seq "$first_disp" $((first_disp + 200))); do
    [ -e "/tmp/.X11-unix/X$n" ] || [ -e "/tmp/.X$n-lock" ] || { dispnum=$n; break; }
done
[ -n "$dispnum" ] || { record "xvfb" FAIL "no free display number"; exit 1; }
setsid Xvfb ":$dispnum" -screen 0 1280x684x24 -nolisten tcp >"$OUT/raw/xvfb.log" 2>&1 &
XV=$!
for _ in $(seq 1 20); do [ -e "/tmp/.X11-unix/X$dispnum" ] && break; sleep 0.5; done
kill -0 "$XV" 2>/dev/null || { record "xvfb" FAIL "Xvfb did not start: $(head -c 200 "$OUT/raw/xvfb.log")"; exit 1; }
export DISPLAY=":$dispnum"
glog="$OUT/godot.log"; : >"$glog"
check_exit() {  # $1: label; waits for the game to stop after Quit > Yes
    for _ in $(seq 1 20); do alive || break; sleep 0.5; done
    if alive; then record "exit" FAIL "$1: the game did not stop"; kill "$GP" 2>/dev/null
    else
        wait "$GP" 2>/dev/null; rc=$?
        if [ "$rc" -eq 0 ]; then record "exit" PASS "$1: code 0"; else record "exit" FAIL "$1: exit code $rc"; fi
    fi
    GP=""
}
launch_game() {  # [game args]: launches the game, waits for its window, fits it to 1280x684; sets $WID and $GP
    # Game arguments after "--": GAME_ARGS (--devmode of the first launch) then those of the call.
    local user_args="${GAME_ARGS#-- }"
    # shellcheck disable=SC2086  # $user_args: several intended arguments
    ( cd "$pkg" && exec setsid "./$exe_name" --audio-driver Dummy --max-fps 70 -- --windowed --user-dir="$HOME/userdata" $user_args "$@" ) >>"$glog" 2>&1 &
    GP=$!
    WID=""
    for _ in $(seq 1 60); do
        WID=$(xdotool search --pid "$GP" --name "OpenTyrian2000 Enhanced" 2>/dev/null | tail -1)
        [ -n "$WID" ] && break
        kill -0 "$GP" 2>/dev/null || break
        sleep 1
    done
    [ -n "$WID" ] || { record "launch" FAIL "game window not found"; exit 1; }
    sleep 4
    # Re-read after the wait: the window found may have been replaced.
    WID=$(xdotool search --pid "$GP" --name "OpenTyrian2000 Enhanced" 2>/dev/null | tail -1)
    [ -n "$WID" ] || { record "launch" FAIL "game window lost after launch"; exit 1; }
    # Letterboxed window: 1280x684 (the 320x200 game is shown at 960x600, black bars).
    xdotool windowsize "$WID" 1280 684; xdotool windowmove "$WID" 0 0; sleep 2; xdotool windowfocus "$WID"
}
# First launch in --devmode: Ctrl+Shift+H (armor restored) is sent every second during the
# level, so that the ship does not die before the pause (otherwise "Quit" would land on the
# Game Over screen). The second launch is without --devmode.
GAME_ARGS="-- --devmode"
launch_game
GAME_ARGS=""

# Geometry: game shown at (160,42) at factor 3.
gx() { echo $(( 160 + $1 * 3 + 1 )); }
gy() { echo $(( 42 + $1 * 3 + 1 )); }
sig() { import -window root -resize 64x40! gray:- 2>/dev/null | md5sum | cut -c1-12; }
settle() {  # waits for a stable image (two identical readouts), $1 s at most
    local max="${1:-5}" prev cur t=0
    prev=$(sig); sleep 0.5
    while [ "$t" -lt $((max * 2)) ]; do
        cur=$(sig); [ "$cur" = "$prev" ] && [ "$t" -ge 2 ] && return 0
        prev=$cur; sleep 0.5; t=$((t + 1))
    done
    return 0
}
alive() { kill -0 "$GP" 2>/dev/null; }
key() {  # key(s) then stable image
    local k
    for k in "$@"; do xdotool key --delay 80 "$k"; sleep 0.3; done
    settle "${SETTLE:-4}"
}
hover() { xdotool mousemove "$(gx "$1")" "$(gy "$2")"; sleep 0.3; settle 3; }
click() { hover "$1" "$2"; xdotool click 1; sleep 0.5; settle "${SETTLE:-4}"; }
# shot NAME TOLERANCE: capture, compare to the reference; tol "live" = non-black image only.
shot() {
    local name="$1" tol="$2" region="${3:-}"
    if ! alive; then record "shot:$name" FAIL "the game stopped before this step"; return; fi
    import -window root "$OUT/raw/$name-full.png" 2>/dev/null
    python3 "$cmp_py" crop "$OUT/raw/$name-full.png" "$OUT/got/$name.png" || { record "shot:$name" FAIL "unreadable capture"; return; }
    if ! python3 "$cmp_py" blank "$OUT/got/$name.png"; then record "shot:$name" FAIL "black/uniform image"; return; fi
    if [ "$update_refs" = 1 ]; then cp "$OUT/got/$name.png" "$refs/$name.png"; fi
    if [ "$tol" = live ]; then record "shot:$name" PASS "not black (no reference: animated screen)"; return; fi
    if [ ! -f "$refs/$name.png" ]; then record "shot:$name" FAIL "reference missing (--update-refs)"; return; fi
    cp "$refs/$name.png" "$OUT/ref/$name.png"
    local frac
    # shellcheck disable=SC2086  # $region: empty or "x y w h", several arguments
    if frac=$(python3 "$cmp_py" compare "$OUT/ref/$name.png" "$OUT/got/$name.png" "$OUT/diff/$name.png" "$tol" $region); then record "shot:$name" PASS "gap $frac (tolerance $tol)"; else record "shot:$name" FAIL "gap $frac > $tol"; fi
}
# Waits for the screen to look like the reference NAME ($3 s at most): under load, the title
# arrives after the intro logos, sometimes beyond a "stable" image of 15 s.
wait_like() {  # wait_like NAME TOLERANCE MAX_SECONDS
    local t=0 tmp="$OUT/raw/wait-$1"
    while [ "$t" -lt "$3" ]; do
        import -window root "$tmp-full.png" 2>/dev/null
        if python3 "$cmp_py" crop "$tmp-full.png" "$tmp.png" 2>/dev/null \
            && python3 "$cmp_py" compare "$refs/$1.png" "$tmp.png" "$tmp-diff.png" "$2" >/dev/null; then
            settle 4; return 0
        fi
        sleep 1; t=$((t + 1))
    done
    settle 4
}
# Mouse speed in a level (keyboard.c + video.c:scaleWindowDistanceToScreen):
# 180 raw window px, in steps of 1 px every 1 ms (slow mouse at 1000 Hz),
# must give at least 45 movement units to the simulation. The C, same x3 window
# and same X events: 55 to 58 units (local input-parity report, not versioned).
mouse_speed() {  # mouse_speed NAME TRACE [MINIMUM [EXPECTED]]
    local name="$1" trace="$2" min="${3:-45}" expect="${4:-C: 55-58}" cmd=() i before after units
    for i in $(seq 1 180); do cmd+=(mousemove_relative -- 1 0 sleep 0.001); done
    # The level may start later (HD sprites): wait for its first traced ticks.
    for i in $(seq 1 40); do [ -s "$trace" ] && break; sleep 0.5; done
    before=$(wc -l <"$trace" 2>/dev/null || echo 0)
    xdotool "${cmd[@]}"; sleep 0.5
    after=$(wc -l <"$trace" 2>/dev/null || echo 0)
    units=$(tail -n +"$((before + 1))" "$trace" 2>/dev/null | awk '{s += $2} END {print s + 0}')
    if [ "$after" -le "$before" ]; then record "mouse-speed:$name" FAIL "no traced tick (--input-trace)"
    elif [ "$units" -ge "$min" ]; then record "mouse-speed:$name" PASS "$units units for 180 px ($expect)"
    else record "mouse-speed:$name" FAIL "$units units for 180 px, $expect (ship too slow)"; fi
}
moved() {  # moved A B [ZONE]: two captures must differ (the game reacts to inputs)
    # shellcheck disable=SC2086  # ${3:-}: optional region, several arguments
    if python3 "$cmp_py" changed "$OUT/got/$1.png" "$OUT/got/$2.png" 0.002 ${3:-}; then record "react:$1>$2" PASS "the image changes"
    else record "react:$1>$2" FAIL "no reaction to inputs"; fi
}
same() {  # same A B ZONE: the zone must be identical (same cash box after resale, etc.)
    if python3 "$cmp_py" changed "$OUT/got/$1.png" "$OUT/got/$2.png" 0.002 "$3"; then record "same:$1=$2" FAIL "zone $3 differs"
    else record "same:$1=$2" PASS "zone $3 identical"; fi
}
owned() {  # owned CAPTURE ROW: the shop's "owned" mark (eye) is on ROW
    local got
    got=$(python3 "$cmp_py" owned_row "$OUT/got/$1.png")
    if [ "$got" = "$2" ]; then record "owned:$1" PASS "eye on row $2"
    else record "owned:$1" FAIL "eye on row $got, expected $2 (purchase not applied)"; fi
}
# Shop cash box (game pixels, under the power bar).
CASH_ZONE=55,168,100,184
# Quit box alone (sprite at 50,50, draw_quit_box): the scenery around depends on level progress.
QUIT_BOX_ZONE=55,50,240,152

wait_like title 0.02 40
shot title 0.02
# New game: mode, episode, difficulty then shop.
SETTLE=6 key Return; SETTLE=6 key Return; SETTLE=6 key Return; SETTLE=8 key Return
sleep 2; settle 8
shot shop 0.02
# Shop with the mouse: hover then click on 3 entries, back with Escape.
hover 160 59; shot shop-hover-specs 0.02
click 160 59; shot shop-specs 0.03; key Escape
click 200 75; shot shop-upgrade 0.03; key Escape
click 200 91; shot shop-options 0.03; key Escape
# Weapon purchase (player bug of 2026-10-10: Enter and click equipped nothing). Upgrade Ship >
# Front Gun; Enter on Multi-Cannon, click on Vulcan Cannon, then click on Pulse-Cannon
# (resale): the eye follows each purchase, the cash box drops then returns to its original value.
click 200 75; key Down; key Return
shot shop-frontgun live; owned shop-frontgun 0
key Down; key Return; shot shop-buy-key live; owned shop-buy-key 1
moved shop-frontgun shop-buy-key "$CASH_ZONE"
click 230 98; shot shop-buy-click live; owned shop-buy-click 2
moved shop-buy-key shop-buy-click "$CASH_ZONE"
click 230 46; shot shop-buy-back live; owned shop-buy-back 0
same shop-frontgun shop-buy-back "$CASH_ZONE"
key Escape; key Escape
# Data cubes with the mouse alone (game_menu.c:1262-1278, 1529-1532, 1597-1604): Data, first
# cube, down arrow pointer in the text, held button = scrolling, click on EXIT = back to the list.
click 200 45; shot shop-cubes 0.03
click 230 45; shot cube-read 0.03
hover 230 130; shot cube-pointer-down 0.03
xdotool mousedown 1; sleep 1.5; shot cube-scrolled live; xdotool mouseup 1
moved cube-pointer-down cube-scrolled
click 274 165; shot cube-after-exit 0.03
key Escape
# Level 1: Start Level, then destination.
click 200 107
SETTLE=10 key Return
( while true; do xdotool key --delay 80 ctrl+shift+h; sleep 1; done ) &
ARMOR=$!
sleep 6; settle 8
shot level-start live
# Keyboard: left + fire held.
xdotool keydown Left keydown space; sleep 1.2; xdotool keyup Left keyup space; sleep 0.3
shot level-keyboard live
moved level-start level-keyboard
# Mouse: ship movement with the button held down.
hover 160 150; xdotool mousedown 1; xdotool mousemove "$(gx 90)" "$(gy 120)"; sleep 1; shot level-mouse live; xdotool mouseup 1
moved level-keyboard level-mouse
# Pause (P), then resume.
key p; shot level-paused live; key p
# Escape: pause menu; Quit row; confirmation box.
key Escape; shot pause-menu live
# Chosen row visible (player bug of 2026-10-10: the 2x2 px "*" could not be seen);
# Enter then click confirm (Music Vol.: music cut then restored).
key Down; shot pause-sel-sound live; moved pause-menu pause-sel-sound 0,15,215,60
key Up; shot pause-sel-music live
key Return; shot pause-enter-mute live; moved pause-sel-music pause-enter-mute 115,15,215,40
click 40 25; shot pause-click-unmute live; moved pause-enter-mute pause-click-unmute 115,15,215,40
same pause-sel-music pause-click-unmute 115,15,215,40
key Down Down Down Down Down; key Return
shot quit-box 0.04 $QUIT_BOX_ZONE
hover 100 135; shot quit-box-hover-yes 0.04 $QUIT_BOX_ZONE
hover 195 135; shot quit-box-hover-no 0.04 $QUIT_BOX_ZONE
kill "$ARMOR" 2>/dev/null; ARMOR=""
key Left; key Return          # Yes: back to the shop (like the C)
settle 8
shot shop-after-quit 0.03
# Shop: Escape opens "Quit Game" (hover Yes); Yes returns to the title (like the C), where Escape then Yes closes the game.
key Escape; hover 100 135; shot shop-quit-hover-yes 0.04
key Left; key Return
settle 8
key Escape; key Left; key Return
check_exit "exit from the shop, via the title"
# Second launch: title, demo, Load, Setup.
launch_game
wait_like title-after-game 0.02 40
shot title-after-game 0.02
# Title rows (game pixels, step of 12 from "Start New Game" at y=94): the mouse
# aims at each row without depending on the current selection.
title_row() { click 160 $((94 + 12 * $1)); }
# Demo: "Demo" row (8).
title_row 7
sleep 5; shot demo live
key Escape; settle 8
# Load (1).
title_row 1; shot load 0.02; key Escape; settle 5
# Setup (4): hover on each row.
title_row 4
shot setup 0.02
i=0
for y in 41 56 71 86 101 116 131 146; do
    i=$((i + 1)); hover 90 "$y"; shot "setup-row-$i" 0.02
done
# Wheel: the C has none (no SDL_MOUSEWHEEL), the image stays that of the last row.
xdotool click 5; sleep 0.4; xdotool click 4; sleep 0.4; settle 4
shot setup-row-8-wheel 0.02
# Jukebox (row 7), animated image.
click 90 131; sleep 2; shot jukebox live
key Escape; key Escape; settle 6
# Exit through the game: Escape at the title, box, Yes.
key Escape; shot title-quit-box 0.03
xdotool key --delay 80 Left; sleep 0.3; xdotool key --delay 80 Return
check_exit "exit from the title"
GP=""

# Third launch: arcade mode Game Menu (1 player), with the mouse only.
# Regression: the menu had neither hover nor click (shop.gd sent everything back to the base screen).
launch_game
sleep 15; settle 15
SETTLE=6 key Return          # New Game
SETTLE=6 key Down; SETTLE=6 key Return   # 1 Player Arcade
SETTLE=6 key Return; SETTLE=8 key Return # episode, difficulty
sleep 2; settle 8
shot arcade-menu 0.02
hover 200 59; shot arcade-hover-options 0.02
hover 200 75; shot arcade-hover-quit 0.02
moved arcade-hover-options arcade-hover-quit
click 200 75; shot arcade-quit-box 0.04     # row already chosen: the click acts (Yes / No box)
key Left; key Return; settle 8              # Yes: back to the title
shot arcade-after-quit 0.03
key Escape; xdotool key --delay 80 Left; sleep 0.3; xdotool key --delay 80 Return
check_exit "exit from the title (arcade)"
GP=""

# C profile: a player coming from the C version must find their
# achievements (24/100 with the maintainer's real profile), the C file remaining untouched.
c_profile_home="$OUT/home-c"
rm -rf "$c_profile_home"; mkdir -p "$c_profile_home/.config/opentyrian2000"
cp "$repo/rewrite/crates/tyrian-formats/tests/saves/maintainer-opentyrian.dat" "$c_profile_home/.config/opentyrian2000/opentyrian.dat"
c_dat_sum=$(md5sum <"$c_profile_home/.config/opentyrian2000/opentyrian.dat")
HOME="$c_profile_home" launch_game
sleep 15; settle 15
title_row 5; sleep 2; settle 5
shot achievements-c-profile 0.02
key Escape; settle 5
key Escape; xdotool key --delay 80 Left; sleep 0.3; xdotool key --delay 80 Return
check_exit "exit after the Achievements screen (C profile)"
GP=""
if [ "$(md5sum <"$c_profile_home/.config/opentyrian2000/opentyrian.dat")" = "$c_dat_sum" ]; then record "c-profile-untouched" PASS "the C file did not change"
else record "c-profile-untouched" FAIL "the C file was modified"; fi

# Fourth and fifth launches: mouse speed in a level, original image
# then HD sprites (1280x800 viewport, the mode where the ship only followed the mouse
# by a quarter). Separate launches: the measurement does not shift the journey above.
level_mouse_speed() {  # level_mouse_speed NAME [game args]; MOUSE_MIN / MOUSE_EXPECT: threshold
    local name="$1"; shift
    launch_game "$@" --input-trace="$OUT/raw/input-$name.txt"
    sleep 15; settle 15
    SETTLE=6 key Return; SETTLE=6 key Return; SETTLE=6 key Return; SETTLE=8 key Return
    sleep 2; settle 8
    click 200 107
    SETTLE=10 key Return
    sleep 6; settle 8
    # Level not started yet (empty trace): a second validation of the destination.
    [ -s "$OUT/raw/input-$name.txt" ] || { SETTLE=10 key Return; sleep 6; settle 8; }
    mouse_speed "$name" "$OUT/raw/input-$name.txt" "${MOUSE_MIN:-45}" "${MOUSE_EXPECT:-C: 55-58}"
    kill "$GP" 2>/dev/null; wait "$GP" 2>/dev/null; GP=""
}
level_mouse_speed original
level_mouse_speed hd --hd-sprites
# Lite package (package.sh --lite, without upscaled/): --hd-sprites must neither
# crash nor silently ignore the absence of the HD pack (display_settings.gd).
if [ ! -d "$pkg/upscaled" ]; then
    if grep -q "HD pack not installed" "$glog"; then record "lite:hd-absent" PASS "--hd-sprites without pack: Original rendering, warning in the log"
    else record "lite:hd-absent" FAIL "no 'HD pack not installed' warning in the log"; fi
fi
# Windows at 125% (simulated: --screen-dpi=120). The C, not DPI aware, sees the
# 1280x684 window as 1024x547 logical and places a x2 image of 640 px in it (video.c:calc_dst_render_rect):
# 180 raw counts = 90 units (video.c:708). Godot at 96 dpi in the same window: ~57.
MOUSE_MIN=84 MOUSE_EXPECT="C at 125%: 88-90" level_mouse_speed dpi125 --screen-dpi=120

# Startup (fresh profile): delay until the first game frame (start of the fade to white of
# intro_logos, mean luminance of the game area >= 10). Same bench: C c-final 0.10 s,
# Godot 0.9 s (engine startup); failure beyond 1.5 s.
boot_time() {
    local home="$OUT/home-boot" t0 now luma ms=""
    rm -rf "$home"; mkdir -p "$home"
    t0=$(date +%s%N)
    ( cd "$pkg" && HOME="$home" exec setsid "./$exe_name" --audio-driver Dummy --max-fps 70 -- --windowed --user-dir="$home/userdata" ) >>"$glog" 2>&1 &
    GP=$!
    while :; do
        now=$(( ($(date +%s%N) - t0) / 1000000 ))
        [ "$now" -gt 6000 ] && break
        luma=$(import -window root -crop 960x600+160+42 +repage -resize 48x30! -colorspace gray -format "%[fx:int(mean*255)]" info: 2>/dev/null || echo 0)
        [ "${luma:-0}" -ge 10 ] && { ms=$now; break; }
        sleep 0.03
    done
    kill "$GP" 2>/dev/null; wait "$GP" 2>/dev/null; GP=""
    if [ -z "$ms" ]; then record "boot-time" FAIL "no image within 6 s"
    elif [ "$ms" -le 1500 ]; then record "boot-time" PASS "first image at $ms ms (C: 100 ms, limit 1500)"
    else record "boot-time" FAIL "first image at $ms ms > 1500 (C: 100 ms)"; fi
}
boot_time

# ---------------------------------------------------------- 4. Godot log
# Known harmless noise at shutdown: "BUG: Unreferenced static string" (engine 4.7.2).
bad=$(grep -E 'SCRIPT ERROR|Parse Error|^ERROR:|Failed (to )?load|Cannot (open|load)|Segmentation|signal 11|Aborted|Missing|No such file' "$glog" \
    | grep -v 'Unreferenced static string' | head -5)
if [ -z "$bad" ]; then record "godot-log" PASS "no error"
else record "godot-log" FAIL "$(echo "$bad" | head -2 | tr '\n' ' ' | cut -c1-200)"; fi

# ------------------------------------------------------------------- 5. linters
# rewrite/scripts/lint.sh: formatting and warnings of the four languages.
if HOME="$user_home" bash "$here/lint.sh" >"$OUT/lint.log" 2>&1; then record "lint" PASS "lint.sh without warnings"
else record "lint" FAIL "$(grep 'FAILED\|tool missing' "$OUT/lint.log" | tr '\n' ' ' | cut -c1-200) (see lint.log)"; fi

# -------------------------------------------- 6. player journeys (mandatory gate)
# docs/testing.md: a push also requires the player journeys with verified effects, all
# PASS, on this same head (player-journeys.sh writes player-journeys.pass). The
# image check above does not prove that an action has its effect.
journeys_stamp="$(dirname "$OUT")/player-journeys.pass"
journeys_id="$(git -C "$repo" rev-parse HEAD) $(git -C "$repo" diff HEAD -- . ':!build' | sha1sum | cut -c1-12)"
if [ "${SMOKE_SKIP_JOURNEYS:-0}" = 1 ]; then
    record "player-journeys" FAIL "SMOKE_SKIP_JOURNEYS=1: journeys not verified (forbidden before a push)"
elif [ ! -f "$journeys_stamp" ]; then
    record "player-journeys" FAIL "player journeys never passed: run rewrite/scripts/player-journeys.sh"
elif [ "$(head -1 "$journeys_stamp")" != "$journeys_id" ]; then
    record "player-journeys" FAIL "player journeys passed on another head ($(head -1 "$journeys_stamp" | cut -c1-8)): run them again"
else
    record "player-journeys" PASS "all PASS on this head ($(sed -n 2,3p "$journeys_stamp" | tr '\n' ' '))"
fi

# ------------------------------------------------------------------ report
python3 "$cmp_py" report "$OUT"
elapsed=$(( $(date +%s) - started ))
total=$(wc -l <"$results")
echo "== $((total - fails))/$total checks passed in ${elapsed}s; report: $OUT/report.md"
if [ "$fails" -eq 0 ]; then echo "player-smoke: OK"; exit 0; fi
echo "player-smoke: FAILED ($fails)"; exit 1
