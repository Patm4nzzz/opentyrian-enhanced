#!/usr/bin/env python3
"""asset_extract_shots.py — Extract the shot sprites of tyrian.shp into
individual PNGs named after sprite_manifest.json ("shots" section).

Both shot banks are covered, with the same index space the game's override
table uses (shot_overrides in src/sprite_override.c):

    section 7  (spriteSheet8)  -> indices   1..304
    section 11 (spriteSheet12) -> indices 501..

By default only sprites the manifest names are written, so the output is
exactly the reference set in data/reference/sprites/shots/ (552 PNGs).
--all also writes the unnamed ones with a numeric filename (<index>.png),
which the override loader resolves directly to the table index.

Usage:
    python3 utils/asset_extract_shots.py [--data-dir data/] [--out DIR] [--all]

Default --out: <data-dir>/reference/sprites/shots/
"""

import argparse
import os
import struct
import sys

try:
    from PIL import Image
except ImportError:
    print("ERROR: Pillow required. Install with: pip install Pillow")
    sys.exit(1)

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _shared import (  # noqa: E402
    decode_sprite2,
    load_shp_section,
    load_vga_palette,
)
from _sprite_manifest import load_manifest  # noqa: E402

SPRITE_WIDTH = 12
SPRITE_HEIGHT = 14

# (tyrian.shp section, index offset) — mirrors asset_audit._CONTENT_SHEETS["shots"]
SHOT_BANKS = [(7, 0), (11, 500)]


def sprite2_offsets(blob):
    """Offset table of a Sprite2_array blob already in memory.

    Same rule as _shared.load_sprite2_array (which only takes a path): the
    first uint16 equals 2 * num_sprites. Returns [] on a malformed header.
    """
    if len(blob) < 4:
        return []
    first = struct.unpack_from("<H", blob, 0)[0]
    if first == 0 or first > len(blob) or first & 1:
        return []
    return [struct.unpack_from("<H", blob, i * 2)[0] for i in range(first // 2)]


def shots_index_to_name(manifest):
    index_to_name = {}
    for name, info in manifest.get("shots", {}).items():
        idx = info["index"] if isinstance(info, dict) else info
        if isinstance(idx, int) and idx not in index_to_name:
            index_to_name[idx] = name
    return index_to_name


def sprite_to_image(pixels, palette):
    img = Image.new("RGBA", (SPRITE_WIDTH, SPRITE_HEIGHT), (0, 0, 0, 0))
    for y, row in enumerate(pixels):
        for x, idx in enumerate(row):
            if idx:
                r, g, b = palette[idx]
                img.putpixel((x, y), (r, g, b, 255))
    return img


def extract_shots(data_dir, output_dir, include_unnamed=False):
    """Write the shot PNGs. Returns (written, total_sprites_in_banks)."""
    palette = load_vga_palette(data_dir)
    index_to_name = shots_index_to_name(load_manifest(os.path.join(data_dir, "sprite_manifest.json")))
    os.makedirs(output_dir, exist_ok=True)

    written = 0
    total = 0
    for section, base in SHOT_BANKS:
        blob = load_shp_section(data_dir, section)
        if not blob:
            if base == 0:
                print(f"ERROR: tyrian.shp missing or section {section} absent")
                sys.exit(1)
            continue
        offsets = sprite2_offsets(blob)
        if not offsets:
            print(f"ERROR: invalid offset table in tyrian.shp section {section}")
            sys.exit(1)
        total += len(offsets)
        for i, offset in enumerate(offsets):
            if offset >= len(blob):
                continue
            # Offset-table entry i corresponds to GAME index i+1 (the C
            # blit reads offsets[index - 1]); manifest indices are 1-based.
            # A 0-based lookup here would shift every extracted name by one
            # sprite and silently corrupt the whole reference set.
            idx = base + i + 1
            name = index_to_name.get(idx)
            if name is None:
                if not include_unnamed:
                    continue
                name = str(idx)
            pixels = decode_sprite2(blob, offset, SPRITE_WIDTH, SPRITE_HEIGHT)
            sprite_to_image(pixels, palette).save(os.path.join(output_dir, f"{name}.png"))
            written += 1

    return written, total


def main():
    parser = argparse.ArgumentParser(
        description="Extract shot sprites from tyrian.shp to individual PNGs named after sprite_manifest.json."
    )
    parser.add_argument("--data-dir", default="data", help="Tyrian data directory")
    parser.add_argument("--out", default=None, help="Output directory (default: <data-dir>/reference/sprites/shots)")
    parser.add_argument(
        "--all",
        action="store_true",
        help="Also write sprites the manifest does not name, as <index>.png",
    )
    args = parser.parse_args()

    output_dir = args.out or os.path.join(args.data_dir, "reference", "sprites", "shots")
    written, total = extract_shots(args.data_dir, output_dir, args.all)
    print(f"Extracted {written}/{total} shot sprites to {output_dir}/")


if __name__ == "__main__":
    main()
