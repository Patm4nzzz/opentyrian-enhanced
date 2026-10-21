#!/usr/bin/env bash
# Offline rendering of the 41 songs of data/music.mus to OGG Vorbis (ADR 0004, 0010).
#
#   1. compiles music_render (gcc) with c/lds_play.c, c/opl.c, c/opl_dosbox.c,
#      c/opl_nuked.c, c/nuked_opl3.c AS IS (frozen copies of the C code,
#      extracted from the retired C version, tag c-final);
#   2. for each song: 16-bit mono 44,100 Hz PCM -> OGG (ffmpeg libvorbis);
#   3. writes music.json: title (c/musmast.c:musicTitle), duration, loop point.
#
# Variables: MUSIC_RENDER_BIN (copy of the binary), MUSIC_BUILD_ONLY=1 (compile only),
#             MUSIC_BACKEND (dosbox|nuked, default dosbox = the C default, save_json.c), MUSIC_RATE (44100),
#             MUSIC_QUALITY (vorbis quality, default 6), CC (gcc).
# Usage: rewrite/tools/music-render/render.sh [data_dir] [out_dir]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
data="${1:-$repo/data}"
out="${2:-$repo/rewrite/godot/assets/imported/music}"
backend="${MUSIC_BACKEND:-dosbox}"
rate="${MUSIC_RATE:-44100}"
quality="${MUSIC_QUALITY:-6}"
CC="${CC:-gcc}"

command -v ffmpeg >/dev/null || { echo "ffmpeg not found" >&2; exit 1; }
command -v sdl2-config >/dev/null || { echo "sdl2-config not found (SDL2 headers required by loudness.h)" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# music_render.c includes src/lds_play.c (static state of the player): lds_play.c
# is therefore not listed separately.
# shellcheck disable=SC2046  # sdl2-config returns several options
"$CC" -O2 -std=gnu11 -w -I"$here/c" -I"$here" $(sdl2-config --cflags) \
    "$here/music_render.c" "$here/stubs.c" \
    "$here/c/opl.c" "$here/c/opl_dosbox.c" "$here/c/opl_nuked.c" "$here/c/nuked_opl3.c" \
    -lm -o "$work/music_render"
# MUSIC_RENDER_BIN: keep a copy of the binary (verify.py uses it).
[ -n "${MUSIC_RENDER_BIN:-}" ] && cp "$work/music_render" "$MUSIC_RENDER_BIN"
[ "${MUSIC_BUILD_ONLY:-0}" = 1 ] && exit 0

mkdir -p "$out"
count=$(python3 -c "import struct,sys; print(struct.unpack('<H', open(sys.argv[1],'rb').read(2))[0])" "$data/music.mus")
echo "music.mus: $count songs, OPL $backend, $rate Hz"

meta="$work/meta.jsonl"
: >"$meta"
for ((i = 0; i < count; i++)); do
    "$work/music_render" "$data/music.mus" "$i" "$work/song.pcm" "$backend" "$rate" >>"$meta"
    ffmpeg -nostdin -loglevel error -y -f s16le -ar "$rate" -ac 1 -i "$work/song.pcm" \
        -c:a libvorbis -q:a "$quality" "$out/song_$(printf '%02d' "$i").ogg"
    tail -1 "$meta"
done

python3 - "$meta" "$out/music.json" "$here/c/musmast.c" <<'EOF'
import json, re, sys
meta_path, out_path, musmast = sys.argv[1:]
titles = re.findall(r'^\s*"((?:[^"\\]|\\.)*)",\s*/\*\s*(\d+)\s*\*/', open(musmast, encoding='latin-1').read(), re.M)
titles = {int(n): t for t, n in titles}
songs = []
for line in open(meta_path):
    m = json.loads(line)
    rate = m["rate"]
    songs.append({
        "index": m["index"],
        "file": "song_%02d.ogg" % m["index"],
        "title": titles.get(m["index"], ""),
        "rate": rate,
        "frames": m["frames"],
        "duration": m["frames"] / rate,
        "loop": m["status"] == "loop",
        "loop_start": m["loop_start"] / rate if m["status"] == "loop" else 0.0,
        "loop_start_frame": m["loop_start"],
        "loop_end_frame": m["loop_end"],
        "status": m["status"],
        # lds_play.c: frame where command 0xfc sets `playing` to false (song
        # without a loop); -1 otherwise. tyrian2.c and highscore.c restart a
        # song on "!playing".
        "end_frame": m["ended_at"],
        "peak": m["peak"],
        "rms": m["rms"],
        "backend": m["backend"],
        # lds_play.c: global player volume at the end of rendering; fade_song()
        # (lds_fade(1)) decrements it by 1 per lds_update, at 69.5 Hz.
        "allvolume": m["allvolume"],
    })
json.dump({"version": 1, "songs": songs}, open(out_path, "w"), indent=1, ensure_ascii=False)
print("written", out_path, len(songs), "songs")
EOF
