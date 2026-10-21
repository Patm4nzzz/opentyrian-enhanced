#!/usr/bin/env python3
"""ref_sprite_sheet.py — labelled contact sheet of one 12x14 sprite category.

Reads the reference PNGs of data/reference/sprites/<category>/ and the
name -> index mapping of data/sprite_manifest.json, then draws a grid (19
columns, the layout of the binary sheets) with each 12x14 sprite scaled 4x
and labelled with its index and name. Empty grid slots are outlined; a
manifest name without a PNG is flagged in red.

Useful as a printable index chart when writing `"sprite": "<name>"` in a
mod.json (mod_preview.py renders a *mod's* sprites; this one renders the
stock reference).

Usage:
    python3 utils/ref_sprite_sheet.py [--category shots|explosions] [--out FILE]

Default --out: data/reference/<category>_sheet.png (untracked reference artifact)
"""

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _sprite_manifest import load_manifest, repo_root  # noqa: E402

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:
    print("ERROR: Pillow required. Install with: pip install Pillow", file=sys.stderr)
    sys.exit(1)

COLS = 19
SCALE = 4
SPRITE_W = 12
SPRITE_H = 14
CELL_W = SPRITE_W * SCALE  # 48
CELL_H = SPRITE_H * SCALE  # 56
LABEL_H = 28  # space for two lines of text below sprite
CELL_PAD = 4  # padding around each cell
FULL_CELL_W = CELL_W + CELL_PAD * 2
FULL_CELL_H = CELL_H + LABEL_H + CELL_PAD * 2
TITLE_H = 48
BG_COLOR = (24, 24, 32)
LABEL_COLOR = (200, 200, 200)
INDEX_COLOR = (140, 140, 160)
TITLE_COLOR = (255, 255, 255)

# Categories whose reference PNGs are single 12x14 cells.
CATEGORIES = ("shots", "explosions")


def main():
    root = repo_root()
    parser = argparse.ArgumentParser(description="Render a labelled contact sheet of a 12x14 sprite category.")
    parser.add_argument(
        "--category",
        default="shots",
        choices=CATEGORIES,
        help="manifest section / reference folder (default: shots)",
    )
    parser.add_argument(
        "--reference-dir",
        default=str(root / "data" / "reference" / "sprites"),
        help="folder holding <category>/*.png (default: data/reference/sprites)",
    )
    parser.add_argument("--manifest", default=str(root / "data" / "sprite_manifest.json"))
    parser.add_argument("--out", default=None, help="output PNG (default: data/reference/<category>_sheet.png)")
    args = parser.parse_args()

    sprites_dir = os.path.join(args.reference_dir, args.category)
    output_path = args.out or str(root / "data" / "reference" / f"{args.category}_sheet.png")

    section = load_manifest(args.manifest).get(args.category, {})
    if not section:
        print(f"ERROR: no '{args.category}' section in {args.manifest}", file=sys.stderr)
        sys.exit(1)
    if not os.path.isdir(sprites_dir):
        print(f"ERROR: {sprites_dir} not found", file=sys.stderr)
        sys.exit(1)

    # Build index -> name reverse map (first name wins on a shared index)
    index_to_name = {}
    for name, info in section.items():
        idx = info["index"] if isinstance(info, dict) else info
        if isinstance(idx, int) and idx not in index_to_name:
            index_to_name[idx] = name

    total = len(index_to_name)
    lo, hi = min(index_to_name), max(index_to_name)
    rows = (hi - lo) // COLS + 1
    print(f"Found {total} {args.category} sprites in manifest (indices {lo}..{hi}, {rows} rows)")

    canvas_w = COLS * FULL_CELL_W
    canvas_h = TITLE_H + rows * FULL_CELL_H
    img = Image.new("RGBA", (canvas_w, canvas_h), BG_COLOR + (255,))
    draw = ImageDraw.Draw(img)

    try:
        font_small = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 8)
        font_title = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 20)
    except OSError:
        font_small = ImageFont.load_default()
        font_title = font_small

    title = f"{args.category.capitalize()} Sprite Reference — OpenTyrian2000"
    bbox = draw.textbbox((0, 0), title, font=font_title)
    draw.text(((canvas_w - (bbox[2] - bbox[0])) // 2, 10), title, fill=TITLE_COLOR, font=font_title)

    placed = 0
    missing = 0
    for idx in range(lo, lo + COLS * rows):
        name = index_to_name.get(idx)
        col = (idx - lo) % COLS
        row = (idx - lo) // COLS
        x0 = col * FULL_CELL_W + CELL_PAD
        y0 = TITLE_H + row * FULL_CELL_H + CELL_PAD

        if name is None:
            draw.rectangle([x0, y0, x0 + CELL_W - 1, y0 + CELL_H - 1], outline=(60, 60, 70))
            draw.text((x0 + 2, y0 + CELL_H + 2), f"#{idx}", fill=INDEX_COLOR, font=font_small)
            continue

        png_path = os.path.join(sprites_dir, f"{name}.png")
        if not os.path.isfile(png_path):
            print(f"  MISSING: {png_path}")
            draw.rectangle([x0, y0, x0 + CELL_W - 1, y0 + CELL_H - 1], outline=(100, 40, 40))
            draw.text((x0 + 2, y0 + CELL_H + 2), f"#{idx} ?", fill=(200, 80, 80), font=font_small)
            missing += 1
            continue

        sprite = Image.open(png_path).convert("RGBA")
        scaled = sprite.resize((CELL_W, CELL_H), Image.NEAREST)
        img.paste(scaled, (x0, y0), scaled)

        draw.text((x0 + 2, y0 + CELL_H + 1), f"#{idx}", fill=INDEX_COLOR, font=font_small)
        display_name = name if len(name) <= 10 else name[:9] + "…"
        draw.text((x0 + 2, y0 + CELL_H + 12), display_name, fill=LABEL_COLOR, font=font_small)
        placed += 1

    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)
    img.save(output_path, "PNG")
    print(f"Saved {output_path}")
    print(f"  Placed: {placed}, Missing files: {missing}, Grid slots: {COLS * rows}")


if __name__ == "__main__":
    main()
