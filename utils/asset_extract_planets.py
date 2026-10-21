#!/usr/bin/env python3
"""
asset_extract_planets.py — Extract planet sprites from tyrian.shp section 3.

Uses palette 17 (the nav map screen palette, newPal=18 in game_menu.c).
Section 3 is the variable-size "Sprite" format (u16 count, then per sprite:
u8 populated, u16 width, u16 height, u16 size, RLE data with 255 = skip run,
254 = next row, 253 = one transparent pixel) — not the 12x14 nibble-RLE
Sprite2 cells of the other extractors, hence its own reader here.

Output files are numbered by sprite index (000.png ...), which is how the
"planets" section of sprite_manifest.json names them; the default output is
the reference set data/reference/sprites/planets/ (152 PNGs).

Usage:
    python3 utils/asset_extract_planets.py [--data-dir data/] [--out DIR]

Default --out: <data-dir>/reference/sprites/planets/
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
from _shared import load_vga_palette as load_palette  # noqa: E402


def load_tyrian_shp_section(data_dir, section_index):
    """Load a specific section from tyrian.shp. Returns list of Sprite tuples."""
    shp_path = os.path.join(data_dir, "tyrian.shp")
    sprites = []

    with open(shp_path, "rb") as f:
        shp_numb = struct.unpack("<H", f.read(2))[0]
        offsets = []
        for _ in range(shp_numb):
            offsets.append(struct.unpack("<i", f.read(4))[0])

        if section_index >= shp_numb:
            print(f"Section {section_index} not found (only {shp_numb} sections)")
            return sprites

        f.seek(offsets[section_index])

        # Read Sprite table: u16 count, then each sprite
        count = struct.unpack("<H", f.read(2))[0]
        print(f"Section {section_index}: {count} sprites")

        for _i in range(count):
            populated = struct.unpack("<B", f.read(1))[0]
            if not populated:
                sprites.append(None)
                continue

            width = struct.unpack("<H", f.read(2))[0]
            height = struct.unpack("<H", f.read(2))[0]
            size = struct.unpack("<H", f.read(2))[0]
            data = f.read(size)
            sprites.append((width, height, data))

    return sprites


def decode_sprite(width, height, data, palette):
    """Decode a Sprite format blob to RGBA image."""
    img = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    pixels = img.load()

    x = 0
    y = 0
    pos = 0

    while pos < len(data) and y < height:
        byte = data[pos]
        pos += 1

        if byte == 255:
            # transparent pixels — next byte is count
            if pos < len(data):
                skip = data[pos]
                pos += 1
                x += skip
                while x >= width:
                    x -= width
                    y += 1
            continue

        if byte == 254:
            # next row
            x = 0
            y += 1
            continue

        if byte == 253:
            # 1 transparent pixel
            x += 1
            if x >= width:
                x = 0
                y += 1
            continue

        # opaque pixel
        if y < height and x < width:
            r, g, b = palette[byte]
            pixels[x, y] = (r, g, b, 255)
        x += 1
        if x >= width:
            x = 0
            y += 1

    return img


def main():
    parser = argparse.ArgumentParser(description="Extract planet sprites from tyrian.shp")
    parser.add_argument("--data-dir", default="data", help="Tyrian data directory")
    parser.add_argument(
        "--out",
        "--output-dir",
        dest="output_dir",
        default=None,
        help="Output directory (default: <data-dir>/reference/sprites/planets)",
    )
    parser.add_argument("--palette", type=int, default=17, help="Palette index (default: 17 = nav map)")
    args = parser.parse_args()

    output_dir = args.output_dir or os.path.join(args.data_dir, "reference", "sprites", "planets")
    os.makedirs(output_dir, exist_ok=True)

    palette = load_palette(args.data_dir, args.palette)
    sprites = load_tyrian_shp_section(args.data_dir, 3)  # section 3 = PLANET_SHAPES

    count = 0
    for i, sp in enumerate(sprites):
        if sp is None:
            continue
        width, height, data = sp
        img = decode_sprite(width, height, data, palette)

        # Check if sprite has any non-transparent pixels
        bbox = img.getbbox()
        if bbox is None:
            continue

        img.save(os.path.join(output_dir, f"{i:03d}.png"))
        count += 1

    print(f"Extracted {count} planets to {output_dir}/ (palette {args.palette})")


if __name__ == "__main__":
    main()
