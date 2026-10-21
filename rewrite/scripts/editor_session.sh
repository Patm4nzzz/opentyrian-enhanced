#!/usr/bin/env bash
# Driving the Godot editor under Xvfb for the "first level in
# 15 minutes" tutorial (docs/FIRST_LEVEL_15_MIN.md). Each subcommand acts like
# a human (xdotool) and can take a screenshot.
#
#   editor_session.sh start            starts Xvfb :88 and the editor
#   editor_session.sh shot <png>       captures the whole screen
#   editor_session.sh key <keys...>    xdotool key
#   editor_session.sh type <text>      xdotool type
#   editor_session.sh click <x> <y>    left click
#   editor_session.sh stop             stops the editor, the game and Xvfb
set -euo pipefail
DISPLAY_NUM="${DISPLAY_NUM:-:88}"
export DISPLAY="$DISPLAY_NUM"
export GODOT_SILENCE_ROOT_WARNING=1
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
godot_dir="$(dirname "$here")/godot"
GODOT="${GODOT:-godot}"
case "${1:-}" in
    start)
        # setsid: the editor must outlive the end of this shell (one
        # `wsl -e bash -lc` call per tutorial step).
        if ! xdotool getdisplaygeometry >/dev/null 2>&1; then
            setsid nohup Xvfb "$DISPLAY_NUM" -screen 0 1280x800x24 >/tmp/xvfb88.log 2>&1 < /dev/null &
            sleep 2
        fi
        # No sound under Xvfb: an unreachable PulseAudio server makes Godot fall
        # back to the dummy driver at once. With the WSLg server, the editor
        # sometimes hangs at startup and "Play from here" freezes on its first
        # frame (2026-10-12); the game launched by the editor inherits this.
        PULSE_SERVER=unix:/nonexistent setsid nohup "$GODOT" --rendering-driver opengl3 -e --path "$godot_dir" --resolution 1280x800 --position 0,0 \
            >/tmp/godot-editor88.log 2>&1 < /dev/null &
        sleep "${EDITOR_SETTLE:-30}"
        ;;
    shot) import -window root "$2" ;;
    key) shift; xdotool key --delay 120 "$@" ;;
    type) shift; xdotool type --delay 60 "$*" ;;
    click) xdotool mousemove "$2" "$3" sleep 0.3 click 1 ;;
    dclick) xdotool mousemove "$2" "$3" sleep 0.3 click --repeat 2 --delay 120 1 ;;
    stop)
        pkill -f "godot.*$godot_dir" 2>/dev/null || true
        pkill -f "^Xvfb $DISPLAY_NUM" 2>/dev/null || true
        ;;
    *) echo "usage: $0 start|shot|key|type|click|dclick|stop" >&2; exit 2 ;;
esac
