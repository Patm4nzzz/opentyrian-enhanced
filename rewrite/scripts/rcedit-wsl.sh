#!/usr/bin/env bash
# Replacement for "wine" for the Windows export from WSL: Godot launches
# "<wine> rcedit.exe <executable> --set-icon ..."; Wine crashes here, so
# rcedit.exe is run directly through WSL interoperability (Windows console
# process, no window). The Linux paths received are converted
# to Windows paths (wslpath -w).
set -euo pipefail
rcedit="$1"; shift
args=()
for a in "$@"; do
    if [ -e "$a" ]; then args+=("$(wslpath -w "$a")"); else args+=("$a"); fi
done
exec "$rcedit" "${args[@]}"
