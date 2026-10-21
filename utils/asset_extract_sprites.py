#!/usr/bin/env python3
"""
asset_extract_sprites.py — Dump every Sprite2 (enemy/ship/shot) cell of the
Tyrian data files as numbered PNGs.

This is the generic, unnamed dump (<out>/enemies/newshN/newshN_0001.png,
<out>/game/<section>/...): 39 newsh*.shp sheets plus tyrian.shp sections
7-12, 12x14 cells, RGBA PNG with transparency, 1-based indices matching the
game. For the manifest-named reference sets use asset_extract_shots.py /
asset_extract_ships.py / asset_extract_explosions.py instead.

Usage:
    python3 utils/asset_extract_sprites.py [--data-dir data/] [--out DIR] [--palette N]

Default --out: pipeline/extracted/ (never data/).
"""

import argparse
import os
import struct
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("ERROR: Pillow required. Install with: pip install Pillow")
    sys.exit(1)

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _shared import (
    decode_sprite2,
    load_sprite2_array,
)
from _shared import (  # noqa: E402
    load_vga_palette as load_palette,
)

# ── Extract functions ─────────────────────────────────────────────────────


def extract_sprite2_file(filepath, output_dir, palette, prefix):
    """Extract all sprites from a newsh*.shp or spriteSheet file."""
    data, offsets = load_sprite2_array(filepath)
    if not offsets:
        print(f"  Skipping {filepath} — no valid offset table")
        return 0

    os.makedirs(output_dir, exist_ok=True)
    count = 0

    for i, offset in enumerate(offsets):
        if offset >= len(data):
            continue

        pixels = decode_sprite2(data, offset)

        # Create RGBA image
        img = Image.new("RGBA", (12, 14), (0, 0, 0, 0))
        for y, row in enumerate(pixels):
            for x, idx in enumerate(row):
                if idx == 0:
                    continue  # transparent
                r, g, b = palette[idx]
                img.putpixel((x, y), (r, g, b, 255))

        # 1-based to match game sprite indices (offsets[index - 1] in C).
        name = f"{prefix}_{i + 1:04d}.png"
        img.save(os.path.join(output_dir, name))
        count += 1

    return count


def extract_tyrian_shp_sections(data_dir, output_dir, palette):
    """Extract spriteSheet sections (7-12) from tyrian.shp."""
    tyrian_shp = os.path.join(data_dir, "tyrian.shp")
    if not os.path.exists(tyrian_shp):
        print("  tyrian.shp not found, skipping")
        return 0

    with open(tyrian_shp, "rb") as f:
        shp_numb = struct.unpack("<H", f.read(2))[0]
        offsets = []
        for _ in range(shp_numb):
            offsets.append(struct.unpack("<i", f.read(4))[0])

        total = 0
        os.makedirs(output_dir, exist_ok=True)
        section_names = {
            7: "shots1",
            8: "ships",
            9: "powerups",
            10: "coins",
            11: "shots2",
            12: "t2000",
        }

        for sec_idx, sec_name in section_names.items():
            if sec_idx >= shp_numb:
                continue

            f.seek(offsets[sec_idx])
            # Read the section as a blob
            if sec_idx + 1 < shp_numb:
                sec_size = offsets[sec_idx + 1] - offsets[sec_idx]
            else:
                f.seek(0, 2)
                sec_size = f.tell() - offsets[sec_idx]
                f.seek(offsets[sec_idx])

            sec_data = f.read(sec_size)

            # Write to temp file and extract
            tmp_path = os.path.join(output_dir, f"_tmp_{sec_name}.shp")
            with open(tmp_path, "wb") as tmp:
                tmp.write(sec_data)

            n = extract_sprite2_file(tmp_path, os.path.join(output_dir, sec_name), palette, sec_name)
            os.unlink(tmp_path)
            print(f"  Section {sec_idx} ({sec_name}): {n} sprites")
            total += n

    return total


# ── Main ──────────────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser(description="Extract Tyrian sprites to PNG.")
    parser.add_argument("--data-dir", default="data", help="Tyrian data directory")
    parser.add_argument(
        "--out",
        "--output-dir",
        dest="output_dir",
        default="pipeline/extracted",
        help="Output directory (default: pipeline/extracted)",
    )
    parser.add_argument("--palette", type=int, default=0, help="Palette index (0-23)")
    args = parser.parse_args()

    palette = load_palette(args.data_dir, args.palette)
    total = 0

    # Extract enemy sprite sheets (newsh*.shp)
    print("Extracting enemy sprite sheets...")
    for shp in sorted(Path(args.data_dir).glob("newsh*.shp")):
        name = shp.stem  # e.g., "newsh2"
        out = os.path.join(args.output_dir, "enemies", name)
        n = extract_sprite2_file(str(shp), out, palette, name)
        print(f"  {shp.name}: {n} sprites")
        total += n

    # Extract tyrian.shp sections
    print("Extracting tyrian.shp sections...")
    total += extract_tyrian_shp_sections(args.data_dir, os.path.join(args.output_dir, "game"), palette)

    print(f"\nTotal: {total} sprites extracted to {args.output_dir}/")


if __name__ == "__main__":
    main()
