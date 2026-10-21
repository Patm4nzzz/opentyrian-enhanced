#!/usr/bin/env bash
# Godot tests (gdUnit4, rewrite/godot/test) in headless mode.
#   GODOT=<binary> bash rewrite/scripts/godot-tests.sh [report-folder]
# Prerequisites: extension and resources built (scripts/build-linux.sh).
# Fails if a test fails, if no test is run, if Godot reports
# SCRIPT ERROR / ERROR (run_godot_checked) or if nodes remain orphaned
# (gdUnit4 exit code 101).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
godot_dir="$(dirname "$here")/godot"
GODOT="${GODOT:-godot}"
# shellcheck source-path=SCRIPTDIR
source "$here/godot-checked.sh"
report="${1:-$(mktemp -d)}"
mkdir -p "$report"
report="$(cd "$report" && pwd)"

# Clean user folder: the player's saves, controls and mods must
# neither influence the tests nor be modified by them.
user_dir="$(mktemp -d)"
export XDG_DATA_HOME="$user_dir"
export GODOT_SILENCE_ROOT_WARNING=1
trap 'rm -rf "$user_dir"' EXIT

# --ignoreHeadlessMode: the tests inject their inputs through
# Input.parse_input_event, which works in headless mode; none depends on the
# interface events that Godot does not carry without a display.
# The report path goes through res://: gdUnit4 prefixes "res:/" to an
# absolute path, hence the project-relative path.
log="$report/godot-tests.log"
rel="$(realpath --relative-to="$godot_dir" "$report")"
status=0
run_godot_checked --headless --path "$godot_dir" -s res://addons/gdUnit4/bin/GdUnitCmdTool.gd \
    -a res://test --ignoreHeadlessMode -rd "res://$rel" </dev/null >"$log" 2>&1 || status=$?
cat "$log"
# A green with no test run is not a green.
if ! grep -Eq 'Executed test cases : \(([1-9][0-9]*)/\1\)' "$log"; then
    echo "gdUnit4: no test run, or not all of them" >&2
    [ "$status" -ne 0 ] || status=1
fi
exit "$status"
