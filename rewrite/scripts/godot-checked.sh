#!/usr/bin/env bash
# Common library: Godot may report SCRIPT ERROR and return zero.
run_godot_checked() {
    local log status=0
    log="$(mktemp)" || return 1
    "$GODOT" "$@" >"$log" 2>&1 || status=$?
    cat "$log"
    if [ "$status" -eq 0 ] && grep -Eq 'SCRIPT ERROR:|(^|[[:space:]])ERROR:' "$log"; then
        status=1
    fi
    rm -f "$log"
    if [ "$status" -ne 0 ]; then
        echo "Godot failed (code $status): $*" >&2
    fi
    return "$status"
}
