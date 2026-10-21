#!/usr/bin/env bash
# Upscaled images of HD (AI) mode (ADR 0100): versioned archive published
# as a GitHub release asset, downloaded and checked by SHA-256 fingerprint.
# No rendering depends on the local GPU: CI and package.sh fetch
# exactly the bytes referenced below.
#
#   hd_assets.sh fetch          downloads (cache), checks the fingerprint of
#                               the archive, extracts into godot/assets/upscaled/
#                               and checks that each upscaled image matches
#                               the current imported PNGs (sources.sha256) and
#                               the scenery matches the level grids (tile_maps.sha256).
#   hd_assets.sh pack <folder> <archive.tar.gz>
#                               builds a new archive (reproducible:
#                               order, dates and owners fixed) from the
#                               output of hd_upscale.py (sprites, sharp, interface,
#                               render), and prints
#                               the fingerprint to put in HD_SHA256.
#
# Variables: HD_ASSETS_CACHE (default ~/.cache/opentyrian-hd). Prerequisite: gh logged in.
set -euo pipefail

# Download through `gh release download` (GH_TOKEN in CI). The hd-assets-5 archive
# is a release asset of this repository (same bytes and fingerprint as the copy
# kept in the former repository, Patm4nzzz/opentyrian-enhanced-archive): the
# default CI token (contents: read) is enough, no extra secret.
HD_REPO="Patm4nzzz/opentyrian-enhanced"
HD_TAG="hd-assets-5"
HD_FILE="opentyrian-hd-assets-5.tar.gz"
HD_SHA256="5fb576b8f165edc814adcaba0344c0f4465c5a85abd58d57d5ebc32eed1d3740"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
assets="$(dirname "$here")/godot/assets"
imported="$assets/imported"
dst="$assets/upscaled"

# Fingerprints of the imported images from which each upscaled image derives (same
# relative paths): moving sprites, scenery tile atlases and full-screen
# interface backgrounds.
source_sums() {
    local f rel
    (cd "$imported" && find sprites tiles pics -name '*.png' 2>/dev/null | LC_ALL=C sort) | while read -r rel; do
        f="$imported/$rel"
        [ -f "$f" ] || { echo "imported source missing: $rel" >&2; return 1; }
        printf '%s  %s\n' "$(sha256sum "$f" | cut -d' ' -f1)" "$rel"
    done
}

case "${1:-}" in
    fetch)
        cache="${HD_ASSETS_CACHE:-$HOME/.cache/opentyrian-hd}"
        archive="$cache/$HD_FILE"
        mkdir -p "$cache"
        if [ ! -f "$archive" ] || ! echo "$HD_SHA256  $archive" | sha256sum -c --status; then
            echo "downloading $HD_REPO@$HD_TAG/$HD_FILE"
            rm -f "$archive"
            gh release download "$HD_TAG" --repo "$HD_REPO" --pattern "$HD_FILE" --dir "$cache"
        fi
        echo "$HD_SHA256  $archive" | sha256sum -c --quiet || { echo "unexpected SHA-256 fingerprint: $archive" >&2; exit 1; }
        rm -rf "$dst"
        mkdir -p "$dst"
        tar -xzf "$archive" -C "$dst"
        : >"$dst/.gdignore"
        # The archive must match the imported images of this commit: a
        # change to tyrian-import (frames, palette) requires a new archive.
        (cd "$imported" && sha256sum -c --quiet "$dst/sources.sha256") \
            || { echo "stale HD PNGs: the imported images changed (ADR 0100)" >&2; exit 1; }
        # Scenery: one upscaled map per level layer (hd_upscale.py): same grids.
        [ "$(python3 "$here/hd_upscale.py" digest "$imported")" = "$(cat "$dst/tile_maps.sha256")" ] \
            || { echo "stale HD scenery: the level maps changed (ADR 0100)" >&2; exit 1; }
        echo "HD : $(find "$dst/sprites" -maxdepth 1 -name '*.png' | wc -l) sprite sheet(s), $(( $(find "$dst/pics" -name '*.png' | wc -l) + $(find "$dst/sprites" -mindepth 2 -name '*.png' | wc -l) )) interface image(s), $(find "$dst/maps" -name '*.webp' | wc -l) scenery band(s) verified in $dst"
        ;;
    pack)
        src="${2:?source folder}"
        out="${3:?output archive}"
        source_sums >"$src/sources.sha256"
        python3 "$here/hd_upscale.py" digest "$imported" >"$src/tile_maps.sha256"
        # Counters recorded in the manifest (verifiable after extraction).
        python3 "$here/hd_manifest.py" "$src"
        (cd "$src" && find . -type f \( -name '*.png' -o -name '*.webp' -o -name '*.json' -o -name '*.sha256' \) \
            | LC_ALL=C sort | tar --owner=0 --group=0 --numeric-owner --mtime='2026-01-01 00:00Z' \
                --no-recursion -cf - -T -) | gzip -n -9 >"$out"
        sha256sum "$out"
        ;;
    *)
        echo "usage: hd_assets.sh fetch | pack <folder> <archive.tar.gz>" >&2
        exit 2
        ;;
esac
