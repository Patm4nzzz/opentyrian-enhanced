#!/usr/bin/env bash
# Test of the macOS package (experimental) on a macOS host, without display
# (CI job "macos"), like test-package-windows.ps1 -HeadlessOnly:
#   0. extraction of the .zip by ditto into a blank temporary folder; signature
#      (codesign --verify) and universal binaries (lipo) of the extracted app;
#   1. normal startup (title) stopped by --quit-after -> macos-headless-boot.log;
#   2. startup with --hd-sprites: the HD (AI) images of the bundle must be found
#      (no "HD pack not installed" warning) -> macos-headless-hd.log;
#   3. strict replay of the golden master INSIDE the exported executable
#      (--replay-strict, tools/capture.gd): checks GDExtension + data/ found in
#      Contents/Resources + simulation -> macos-replay-strict.log;
#   4. nothing was written into the bundle (player data outside the .app).
# Each launch fails on a non-zero return code or a "SCRIPT ERROR:" / "ERROR:"
# in its output (same rule as scripts/godot-checked.sh).
#
# Usage: rewrite/scripts/test-package-macos.sh <OpenTyrian2000-Enhanced-v...-macos-universal.zip> <out_dir> [replay.gm]
set -euo pipefail

zip="${1:?macOS package .zip}"
out="${2:?output folder for the logs}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
replay="${3:-$(dirname "$here")/golden/e1-tyrian-autopilot.gm}"
[ "$(uname -s)" = Darwin ] || { echo "test-package-macos.sh needs a macOS host" >&2; exit 1; }
[ -f "$replay" ] || { echo "replay missing: $replay" >&2; exit 1; }

mkdir -p "$out"
out="$(cd "$out" && pwd)"
root="$(mktemp -d "${TMPDIR:-/tmp}/ot-clean-XXXXXX")"
ditto -x -k "$zip" "$root"
app="$root/OpenTyrian2000 Enhanced.app"
exe="$app/Contents/MacOS/OpenTyrian2000 Enhanced"
[ -x "$exe" ] || { echo "executable missing: $exe" >&2; exit 1; }
echo "package extracted to $app"

codesign --verify --deep --strict --verbose=2 "$app"
codesign -dv "$app" 2>&1 | grep -E '^(Identifier|Format|Signature)='
for bin in "$exe" "$app/Contents/Frameworks/libtyrian_godot.dylib"; do
    info="$(lipo -info "$bin")"
    echo "$info"
    for arch in x86_64 arm64; do
        [[ "$info" == *"$arch"* ]] || { echo "not universal, $arch missing: $bin" >&2; exit 1; }
    done
done
for d in data mods upscaled/sprites; do
    [ -d "$app/Contents/Resources/$d" ] || { echo "Contents/Resources/$d missing" >&2; exit 1; }
done
cp "$replay" "$root/replay.gm"
before="$(cd "$app" && find . | LC_ALL=C sort | shasum)"

# Dedicated player profile and HOME: nothing from the runner's account is read.
profile="$root/profile"
mkdir -p "$profile/home"

# run <log> <timeout_s> <args...>: launch with a reduced environment, then
# fail on a non-zero code or an engine/script error in the output. The
# `launcher` array prefixes the command (arch -x86_64: Intel slice).
launcher=()
run() {
    local log="$1" limit="$2" code=0
    shift 2
    env -i HOME="$profile/home" PATH=/usr/bin:/bin TMPDIR="$root/" \
        perl -e 'alarm shift; exec @ARGV' "$limit" ${launcher[@]+"${launcher[@]}"} "$exe" "$@" >"$log" 2>&1 || code=$?
    if [ "$code" -ne 0 ] || grep -Eq 'SCRIPT ERROR:|(^|[[:space:]])ERROR:' "$log"; then
        tail -n 40 "$log"
        echo "failed (code $code): $*" >&2
        return 1
    fi
}

user_dir="--user-dir=$profile/userdata"
run "$out/macos-headless-boot.log" 120 --headless --audio-driver Dummy --quit-after 300 -- "$user_dir"
echo "--headless startup: no error"

run "$out/macos-headless-hd.log" 120 --headless --audio-driver Dummy --quit-after 120 -- "$user_dir" --hd-sprites
if grep -q 'HD pack not installed' "$out/macos-headless-hd.log"; then
    echo "HD (AI) images not found in the bundle" >&2
    exit 1
fi
echo "HD (AI) images found"

run "$out/macos-replay-strict.log" 300 --headless --audio-driver Dummy -- "$user_dir" \
    "--capture=level:$out/macos-replay-strict.png:1000000" "--replay=$root/replay.gm" --replay-strict
grep -E 'strict replay: [0-9]+ ticks identical' "$out/macos-replay-strict.log" \
    || { echo "strict replay: verdict missing" >&2; exit 1; }

# Intel slice: only if Rosetta 2 is installed on the runner (Apple Silicon).
if arch -x86_64 /usr/bin/true 2>/dev/null; then
    launcher=(arch -x86_64)
    run "$out/macos-headless-boot-x86_64.log" 180 --headless --audio-driver Dummy --quit-after 300 -- "$user_dir"
    launcher=()
    echo "x86_64 slice (Rosetta): --headless startup without error"
else
    echo "Rosetta 2 not installed: x86_64 slice not launched (checked by lipo only)"
fi

# Default player data folder (without --user-dir): never inside the bundle.
run "$out/macos-headless-default-dir.log" 120 --headless --audio-driver Dummy --quit-after 60
after="$(cd "$app" && find . | LC_ALL=C sort | shasum)"
[ "$before" = "$after" ] || { echo "files were written into the bundle" >&2; exit 1; }
# Godot resolves ~/Library from HOME or from the account: either home is accepted.
found=""
for home in "$profile/home" "$HOME"; do
    if [ -f "$home/Library/Application Support/OpenTyrian2000 Enhanced/display.cfg" ] \
        || [ -d "$home/Library/Application Support/OpenTyrian2000 Enhanced/mods" ]; then
        found="$home/Library/Application Support/OpenTyrian2000 Enhanced"
    fi
done
[ -n "$found" ] || { echo "player data folder not in ~/Library/Application Support/OpenTyrian2000 Enhanced" >&2; exit 1; }
echo "player data folder: $found"
echo "OK (no display): macOS package launched from $app"
