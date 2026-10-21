#!/usr/bin/env python3
"""
asset_extract_explosions.py — Extract and name explosion sprites from newsh6.shp.

Cross-references the explosion_data[54] table in varz.c to assign meaningful names.
Visually verified naming against rendered sprite sheets. The names are the
"explosions" section of data/sprite_manifest.json; the default output is the
reference set data/reference/sprites/explosions/ (241 PNGs).

Usage:
    python3 utils/asset_extract_explosions.py [--data-dir data/] [--out DIR]
        [--manifest-out explosions.json]

Default --out: <data-dir>/reference/sprites/explosions/

Animation notes (from tyrian2.c):
  - Non-fixed explosions: sprite++ BEFORE render, blit at sprite+1.
    So for base B, ttl T: 0-based frames are B+1, B+2, ..., B+T.
  - Fixed-position (coins): NO sprite++, blit at sprite+1.
    So only 0-based index B is used (rendered ttl times).
"""

import argparse
import json
import os
import sys
from collections import Counter

try:
    from PIL import Image
except ImportError:
    print("ERROR: Pillow required. Install with: pip install Pillow", file=sys.stderr)
    sys.exit(1)

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _shared import (
    decode_sprite2,
    load_sprite2_array,
)
from _shared import (  # noqa: E402
    load_vga_palette as load_palette,
)


def is_empty(pixels):
    return all(px == 0 for row in pixels for px in row)


# ── Color analysis ───────────────────────────────────────────────────────


def classify_color(r, g, b):
    """Classify an RGB color into a named category."""
    brightness = (r + g + b) / 3.0
    if brightness < 20:
        return "dark"
    max_c = max(r, g, b)
    min_c = min(r, g, b)
    sat = (max_c - min_c) / max(max_c, 1)
    if sat < 0.15:
        if brightness > 200:
            return "white"
        elif brightness > 120:
            return "lightgray"
        return "gray"
    if max_c == r:
        if g > r * 0.7:
            return "yellow"
        if g > r * 0.35:
            return "orange"
        if b > r * 0.5:
            return "pink"
        return "red"
    elif max_c == g:
        if b > g * 0.6:
            return "cyan"
        if r > g * 0.5:
            return "yellow"
        return "green"
    else:
        if r > b * 0.5:
            return "purple"
        if g > b * 0.5:
            return "cyan"
        return "blue"


def dominant_color(pixels, palette):
    """Return the dominant color category (excluding dark/black)."""
    counts = Counter()
    for row in pixels:
        for idx in row:
            if idx == 0:
                continue
            r, g, b = palette[idx]
            cat = classify_color(r, g, b)
            if cat != "dark":
                counts[cat] += 1
    if not counts:
        return "dark"
    if "lightgray" in counts:
        counts["gray"] += counts.pop("lightgray")
    return counts.most_common(1)[0][0]


def pixel_count(pixels):
    return sum(1 for row in pixels for px in row if px != 0)


# ── Explosion sequence definitions ──────────────────────────────────────
# Visually verified against rendered sprite sheets.
#
# Format: (base_0based, ttl, fixed, name_prefix)
# Non-fixed: frames at indices base+1 .. base+ttl (0-based)
# Fixed: single index = base (0-based)

SEQUENCES = [
    # ── Large fire explosions (12 frames, bright orange/red fireball expanding) ──
    (1, 12, False, "explosion_fire_large_a"),
    (20, 12, False, "explosion_fire_large_b"),
    (39, 12, False, "explosion_fire_large_c"),
    (58, 12, False, "explosion_fire_large_d"),
    # ── Medium green flash (7 frames, pale green puff dissipating) ──
    (76, 7, False, "explosion_green"),
    # ── Short flashes (3 frames each) ──
    (91, 3, False, "flash_red_small"),  # type 13: tiny red sparks
    (96, 3, False, "flash_yellow"),  # type 53: yellow sparks
    (110, 3, False, "flash_red_dark"),  # type 11: dark red sparks
    # ── Standard fire ring (12 frames, red/orange expanding ring) ──
    (120, 12, False, "explosion_ring_a"),
    # ── White smoke puff (7 frames, white/gray dissipating) ──
    (133, 7, False, "smoke_white"),
    # ── Small fire burst (7 frames, compact red/orange) ──
    (144, 7, False, "explosion_fire_small"),
    # ── Standard fire rings (12 frames each, red/orange expanding crescents) ──
    (152, 12, False, "explosion_ring_b"),
    (171, 12, False, "explosion_ring_c"),
    (190, 12, False, "explosion_ring_d"),
    (209, 12, False, "explosion_ring_e"),
    # ── Coin/gem point value text (fixed position, single sprite) ──
    # Visually verified from magnified renders
    (165, 8, True, "coin_value_10"),
    (168, 8, True, "coin_value_100"),
    (169, 10, True, "coin_value_2k"),
    (170, 14, True, "coin_value_150"),
    (184, 8, True, "coin_value_25"),
    (187, 8, True, "coin_value_250"),
    (188, 10, True, "coin_value_5k"),
    (189, 14, True, "coin_value_350"),
    (203, 8, True, "coin_value_50"),
    (206, 8, True, "coin_value_500"),
    (207, 20, True, "coin_value_ale375"),
    (208, 14, True, "coin_value_500_fire"),
    (222, 8, True, "coin_value_75"),
    (225, 10, True, "coin_value_1k"),
    (226, 14, True, "coin_value_800"),
    (227, 14, True, "coin_value_50_fire"),
    # ── Player death debris (3 frames each, cyan ship fragments flying apart) ──
    (227, 3, False, "debris_hull_a"),
    (230, 3, False, "debris_hull_b"),
    (233, 3, False, "debris_hull_c"),
    (236, 3, False, "debris_wing_a"),
    (239, 3, False, "debris_wing_b"),
    (242, 3, False, "debris_wing_c"),
    (246, 3, False, "debris_strut_a"),
    (249, 3, False, "debris_strut_b"),
    (252, 3, False, "debris_chunk_a"),
    (261, 3, False, "debris_shard_a"),
    (265, 3, False, "debris_shard_b"),
    (268, 3, False, "debris_shard_c"),
    (271, 3, False, "debris_shard_d"),
    (274, 3, False, "debris_panel_a"),
    (277, 3, False, "debris_panel_b"),
    (280, 3, False, "debris_panel_c"),
    (284, 3, False, "debris_plate_a"),
    (287, 3, False, "debris_plate_b"),
    (290, 3, False, "debris_plate_c"),
    (293, 3, False, "debris_plate_d"),
    (299, 3, False, "debris_fragment"),
]


def build_named_indices():
    """Map 0-based sprite index -> name. First assignment wins."""
    named = {}
    for base, ttl, fixed, prefix in SEQUENCES:
        if fixed:
            idx = base
            if idx not in named:
                named[idx] = prefix
        else:
            for frame in range(ttl):
                idx = base + 1 + frame
                if idx not in named:
                    named[idx] = f"{prefix}_{frame + 1}"
    return named


def main():
    parser = argparse.ArgumentParser(description="Extract explosion sprites from newsh6.shp")
    parser.add_argument("--data-dir", default="data", help="Tyrian data directory")
    parser.add_argument(
        "--out",
        "--output-dir",
        dest="output_dir",
        default=None,
        help="Output directory (default: <data-dir>/reference/sprites/explosions)",
    )
    parser.add_argument(
        "--manifest-out",
        default=None,
        help="Write the name -> index manifest (JSON) to this file instead of discarding it",
    )
    parser.add_argument("--palette", type=int, default=0, help="Palette index (0-23)")
    args = parser.parse_args()

    if args.output_dir is None:
        args.output_dir = os.path.join(args.data_dir, "reference", "sprites", "explosions")

    palette = load_palette(args.data_dir, args.palette)
    shp_path = os.path.join(args.data_dir, "newsh6.shp")

    if not os.path.exists(shp_path):
        print(f"ERROR: {shp_path} not found", file=sys.stderr)
        sys.exit(1)

    data, offsets = load_sprite2_array(shp_path)
    print(f"Loaded {len(offsets)} sprites from {shp_path}", file=sys.stderr)

    os.makedirs(args.output_dir, exist_ok=True)

    # Decode all sprites
    sprites = []
    for offset in offsets:
        if offset >= len(data):
            sprites.append(None)
            continue
        pixels = decode_sprite2(data, offset)
        if is_empty(pixels):
            sprites.append(None)
            continue
        sprites.append(pixels)

    # Build name map from explosion table
    named = build_named_indices()

    # Name unused sprites by color and index
    all_names = set(named.values())
    for i, px in enumerate(sprites):
        if px is None or i in named:
            continue
        clr = dominant_color(px, palette)
        pxc = pixel_count(px)
        if pxc <= 3:
            name = f"speck_{clr}_{i}"
        else:
            name = f"unused_{clr}_{i}"
        base_name = name
        counter = 1
        while name in all_names:
            counter += 1
            name = f"{base_name}_{counter}"
        all_names.add(name)
        named[i] = name

    # Save PNGs and build manifest
    manifest = {}
    saved = 0

    for i in sorted(named.keys()):
        if i >= len(sprites) or sprites[i] is None:
            continue
        px = sprites[i]
        name = named[i]

        img = Image.new("RGBA", (12, 14), (0, 0, 0, 0))
        for y, row in enumerate(px):
            for x, idx in enumerate(row):
                if idx == 0:
                    continue
                r, g, b = palette[idx]
                img.putpixel((x, y), (r, g, b, 255))

        img.save(os.path.join(args.output_dir, f"{name}.png"))
        manifest[name] = {"index": i, "frame_width": 12, "frame_height": 14}
        saved += 1

    print(f"Saved {saved} sprites to {args.output_dir}", file=sys.stderr)
    if args.manifest_out:
        with open(args.manifest_out, "w") as f:
            json.dump(manifest, f, indent=2, sort_keys=False)
            f.write("\n")
        print(f"Wrote manifest ({len(manifest)} names) to {args.manifest_out}", file=sys.stderr)


if __name__ == "__main__":
    main()
