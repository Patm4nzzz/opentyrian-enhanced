#!/usr/bin/env bash
# Linters for the maintained code, all check-only (nothing is rewritten):
#   Rust      cargo fmt --check, cargo clippy --all-targets -D warnings  (rewrite/rustfmt.toml)
#   GDScript  gdlint, gdformat --check (gdtoolkit 4)       (rewrite/godot/gdlintrc, gdformatrc)
#             Godot "unused" warnings (variable, local constant,
#             parameter, signal, private variable) treated as errors when loading
#             each script, if the extension is built (build-linux.sh)
#   Python    ruff check, ruff format --check                          (ruff.toml)
#   Shell     shellcheck
#   PowerShell PSScriptAnalyzer, if pwsh is there          (rewrite/scripts/PSScriptAnalyzerSettings.psd1)
# Exit code 1 as soon as a tool reports a problem.
#
#   rewrite/scripts/lint.sh            everything
#   rewrite/scripts/lint.sh --no-rust  without cargo
#
# Tools: `uv tool install ruff`, `uv tool install "gdtoolkit==4.*"` (or pip),
# `apt install shellcheck`; GODOT (default: godot). Fix before rerunning:
# `cargo fmt --all`, `ruff format .`, `gdformat <file>`.
set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rewrite="$(dirname "$here")"
repo="$(dirname "$rewrite")"
godot_dir="$rewrite/godot"
GODOT="${GODOT:-godot}"
with_rust=1
for arg in "$@"; do
    case "$arg" in
        --no-rust) with_rust=0 ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

fail=0
step() {  # name command...
    local name="$1"; shift
    if "$@"; then echo "lint: $name OK"; else echo "lint: $name FAILED" >&2; fail=1; fi
}
need() {
    command -v "$1" >/dev/null || { echo "lint: tool missing: $1" >&2; exit 2; }
}
need ruff; need gdlint; need gdformat; need shellcheck

# Loads every script and scene of the project in the game (autoloads active), with the
# "unused" warnings turned into errors by a temporary override.cfg.
# shellcheck disable=SC2317  # called by step
godot_unused_warnings() {
    local loader="$godot_dir/lint_load_all.gd" scene="$godot_dir/lint_load_all.tscn"
    local override="$godot_dir/override.cfg" log status=0
    log="$(mktemp)"
    printf '%s\n' '[debug]' '' 'gdscript/warnings/unused_variable=2' 'gdscript/warnings/unused_local_constant=2' \
        'gdscript/warnings/unused_private_class_variable=2' 'gdscript/warnings/unused_parameter=2' \
        'gdscript/warnings/unused_signal=2' >"$override"
    cat >"$loader" <<'EOF'
extends Node


func _ready() -> void:
	var failed := 0
	var dirs := ["res://"]
	while not dirs.is_empty():
		var dir: String = dirs.pop_back()
		for sub in DirAccess.get_directories_at(dir):
			if not sub.begins_with(".") and sub not in ["gdUnit4", "assets", "levels"]:
				dirs.append(dir.path_join(sub))
		for f in DirAccess.get_files_at(dir):
			if (f.ends_with(".gd") or f.ends_with(".tscn")) and not f.begins_with("lint_load_all"):
				if load(dir.path_join(f)) == null:
					failed += 1
	get_tree().quit(1 if failed > 0 else 0)
EOF
    printf '%s\n' '[gd_scene format=3]' '' '[ext_resource type="Script" path="res://lint_load_all.gd" id="1"]' '' \
        '[node name="LintLoadAll" type="Node"]' 'script = ExtResource("1")' >"$scene"
    GODOT_SILENCE_ROOT_WARNING=1 timeout 600 "$GODOT" --headless --audio-driver Dummy --path "$godot_dir" \
        res://lint_load_all.tscn >"$log" 2>&1 || status=$?
    if [ "$status" -eq 0 ] && grep -q 'SCRIPT ERROR' "$log"; then status=1; fi
    if [ "$status" -ne 0 ]; then grep -A1 'SCRIPT ERROR' "$log" | grep -v '^--' >&2; fi
    rm -f "$loader" "$loader.uid" "$scene" "$override" "$log"
    return "$status"
}

cd "$repo" || exit 2
mapfile -t sh_files < <(git ls-files '*.sh' | grep -v -e '^artifacts/' -e '/addons/gdUnit4/')
step "ruff check" ruff check --quiet .
step "ruff format" ruff format --check --quiet .
step "shellcheck" shellcheck -x "${sh_files[@]}"
# PowerShell: PSScriptAnalyzer if pwsh is installed (Windows workstation scripts).
# shellcheck disable=SC2317  # called by step
ps_analyzer() {
    local f out status=0
    for f in "$here"/*.ps1; do
        out="$(pwsh -NoProfile -Command \
            "Invoke-ScriptAnalyzer -Path '$f' -Settings '$here/PSScriptAnalyzerSettings.psd1' -ErrorAction Stop" 2>&1)" \
            || status=1
        if [ -n "$out" ]; then printf '%s\n' "$out" >&2; status=1; fi
    done
    return "$status"
}
if command -v pwsh >/dev/null; then
    step "PSScriptAnalyzer" ps_analyzer
else
    echo "lint: PSScriptAnalyzer skipped: pwsh missing" >&2
fi

cd "$godot_dir" || exit 2
mapfile -t gd_files < <(git ls-files '*.gd' | grep -v '^addons/gdUnit4/')
# Scripts that gdformat formats badly: formatted by hand, gdlint only.
mapfile -t gd_formatted < <(printf '%s\n' "${gd_files[@]}" | grep -vxF -f <(grep -v '^#' gdformat-skip.txt))
step "gdlint" gdlint "${gd_files[@]}"
step "gdformat" gdformat --check "${gd_formatted[@]}"
if command -v "$GODOT" >/dev/null && [ -f bin/libtyrian_godot.so ]; then
    step "Godot (unused)" godot_unused_warnings
else
    echo "lint: Godot (unused) skipped: $GODOT missing or extension not built (build-linux.sh)" >&2
fi

if [ "$with_rust" = 1 ]; then
    need cargo
    cd "$rewrite" || exit 2
    step "cargo fmt" cargo fmt --all --check
    step "cargo clippy" cargo clippy --workspace --all-targets --release --locked --quiet -- -D warnings
fi
exit "$fail"
