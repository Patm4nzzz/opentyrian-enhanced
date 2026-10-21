#!/usr/bin/env bash
# Full build on a macOS host (experimental macOS version, CI job "macos"):
#   1. GDExtension tyrian-godot (cargo, release) for aarch64-apple-darwin and
#      x86_64-apple-darwin, merged by lipo into ONE universal library
#      -> godot/bin/libtyrian_godot.dylib (declared in tyrian.gdextension);
#   2. then the same steps as on Linux (scripts/build-linux.sh --no-gdextension):
#      tyrian-import, offline music rendering, Godot import and level scenes,
#      run by the macOS Godot editor with the universal library.
#
# Needs the Xcode command line tools (linker, lipo), the two Rust targets
# (rustup target add aarch64-apple-darwin x86_64-apple-darwin) and, for the
# music rendering, sdl2-config and ffmpeg (Homebrew: brew install sdl2 ffmpeg).
# Cannot run under Linux/WSL: the Apple linker and SDK are required
# (`cargo check --target ...-apple-darwin` works there, without linking).
#
# Variables: GODOT (default: godot), CARGO_TARGET_DIR (default cargo).
# Usage: rewrite/scripts/build-macos.sh [--skip-import] [--music]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rewrite="$(dirname "$here")"
godot_dir="$rewrite/godot"
target_dir="${CARGO_TARGET_DIR:-$rewrite/target}"
targets=(aarch64-apple-darwin x86_64-apple-darwin)

[ "$(uname -s)" = Darwin ] || { echo "build-macos.sh needs a macOS host (Apple linker, lipo)" >&2; exit 1; }
for arg in "$@"; do
    case "$arg" in
        --skip-import | --music) ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

cd "$rewrite"
echo "== 1. GDExtension (macOS, universal: ${targets[*]})"
libs=()
for target in "${targets[@]}"; do
    cargo build --release --locked -p tyrian-godot --target "$target"
    libs+=("$target_dir/$target/release/libtyrian_godot.dylib")
done
mkdir -p "$godot_dir/bin"
lipo -create -output "$godot_dir/bin/libtyrian_godot.dylib" "${libs[@]}"
# Apple strip: local symbols of the Rust library, here while only the linker
# has signed it (after the Godot export, strip refuses the bundle copy).
strip -x "$godot_dir/bin/libtyrian_godot.dylib"
info="$(lipo -info "$godot_dir/bin/libtyrian_godot.dylib")"
echo "$info"
for arch in x86_64 arm64; do
    [[ "$info" == *"$arch"* ]] || { echo "universal library without $arch" >&2; exit 1; }
done

bash "$here/build-linux.sh" --no-gdextension "$@"
