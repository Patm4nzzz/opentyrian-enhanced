#!/usr/bin/env bash
# Full build on the Linux / WSL side:
#   1. GDExtension tyrian-godot (cargo, release) -> godot/bin/libtyrian_godot.so
#   2. (option --windows) cross-compiled Windows DLL (x86_64-pc-windows-gnu, mingw-w64)
#   3. Resources: tyrian-import data/ -> godot/assets/imported/
#      then music: tools/music-render/render.sh -> assets/imported/music/
#      (DOSBox, Classic) and assets/imported/music_nuked/ (Nuked OPL3, Authentic)
#      (41 OGGs rendered offline by the C OPL code, ADR 0004 and 0010;
#      skipped if music.json already exists, except with --music)
#   4. Godot import (textures, sounds), then level scenes (editor/build_levels.gd)
#
# --no-gdextension: step 1 builds tyrian-import only, the GDExtension already in
# godot/bin/ is kept (scripts/build-macos.sh: universal .dylib built before;
# steps 3 and 4 are plain POSIX and also run on a macOS host).
#
# Variables: GODOT (default: godot), CARGO_TARGET_DIR (default cargo).
# Usage: rewrite/scripts/build-linux.sh [--windows] [--skip-import] [--music] [--no-gdextension]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rewrite="$(dirname "$here")"
repo="$(dirname "$rewrite")"
godot_dir="$rewrite/godot"
GODOT="${GODOT:-godot}"
# shellcheck source-path=SCRIPTDIR
source "$here/godot-checked.sh"
target_dir="${CARGO_TARGET_DIR:-$rewrite/target}"

with_windows=0
skip_import=0
force_music=0
gdextension=1
for arg in "$@"; do
    case "$arg" in
        --windows) with_windows=1 ;;
        --skip-import) skip_import=1 ;;
        --music) force_music=1 ;;
        --no-gdextension) gdextension=0 ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

cd "$rewrite"

if [ "$gdextension" = 1 ]; then
    echo "== 1. GDExtension (linux)"
    cargo build --release -p tyrian-godot -p tyrian-import
    mkdir -p "$godot_dir/bin"
    cp "$target_dir/release/libtyrian_godot.so" "$godot_dir/bin/"
else
    echo "== 1. tyrian-import (GDExtension kept)"
    cargo build --release -p tyrian-import
fi

if [ "$with_windows" = 1 ]; then
    echo "== 2. GDExtension (windows, mingw cross-compilation)"
    rustup target add x86_64-pc-windows-gnu >/dev/null
    cargo build --release -p tyrian-godot --target x86_64-pc-windows-gnu
    cp "$target_dir/x86_64-pc-windows-gnu/release/tyrian_godot.dll" "$godot_dir/bin/"
fi

if [ "$skip_import" = 0 ]; then
    echo "== 3. tyrian-import"
    "$target_dir/release/tyrian-import" --data "$repo/data" --out "$godot_dir/assets/imported"
    if [ "$force_music" = 1 ] || [ ! -f "$godot_dir/assets/imported/music/music.json" ]; then
        echo "== 3b. music (offline OPL rendering)"
        bash "$rewrite/tools/music-render/render.sh" "$repo/data" "$godot_dir/assets/imported/music"
    fi
    # Authentic engine (Setup > Sound, sound_menu.c:eng_set case 1): same
    # tracks rendered by the C's Nuked OPL3 emulator.
    if [ "$force_music" = 1 ] || [ ! -f "$godot_dir/assets/imported/music_nuked/music.json" ]; then
        echo "== 3c. Authentic music (offline Nuked OPL3 rendering)"
        MUSIC_BACKEND=nuked bash "$rewrite/tools/music-render/render.sh" "$repo/data" "$godot_dir/assets/imported/music_nuked"
    fi
fi

echo "== 4. Godot import + level scenes"
# First pass: import of the PNG/WAV files (creates .godot/ and the .import files).
run_godot_checked --headless --path "$godot_dir" --import
run_godot_checked --headless --path "$godot_dir" res://editor/build_levels.tscn
# Second pass: import of the generated scenes and resources.
run_godot_checked --headless --path "$godot_dir" --import

echo "OK: run \"$GODOT --path $godot_dir\""
