#!/usr/bin/env python3
"""mod_art_refsheet.py — labelled reference sheet of sprites, for mod art work.

Lays out any set of PNGs (files, folders or glob patterns) on a dark grid,
enlarged nearest-neighbour, each cell with a short label. Used at three
moments of the mod art pipeline (docs/mod-art-pipeline.md):

* before generating: a sheet of the ORIGINAL sprites of the kind being drawn
  (shop icons, shots, ships), uploaded to PixelLab as a labelled style reference;
* while cleaning: candidates side by side with the originals, to pick and judge;
* after: the finished mod sprites next to the originals at the same scale
  (the 1x look is what the player sees; --scale 1 shows it).

Usage:
    python utils/mod_art_refsheet.py data/reference/sprites/pickups/weapons --scale 4 --out ref_icons.png
    python utils/mod_art_refsheet.py "data/reference/sprites/shots/bolt_red_*.png" mods/x/sprites/shots \\
        --scale 3 --cell 24x28 --cols 12 --title "Shots: originals then mod" --out cmp.png

Options:
    --scale N       enlargement (nearest neighbour), default 4
    --cell WxH      cell size in 1x pixels (default: the largest input)
    --cols N        cells per row (default 10)
    --bg #rrggbb    background (default #1c1c22, the in-game shop is near-black)
    --no-labels     no file name under each cell
    --title TEXT    title line at the top
    --frames N      split each input horizontally into N equal frames (animation strips)
"""

from __future__ import annotations

import argparse
import glob
import os
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:  # pragma: no cover
    print("ERROR: Pillow required (pip install -r utils/requirements.txt)", file=sys.stderr)
    sys.exit(2)


def expand_inputs(items: list[str]) -> list[Path]:
    """Files, folders (their *.png, sorted) and glob patterns, in the given order, without duplicates."""
    out: list[Path] = []
    for it in items:
        p = Path(it)
        if p.is_dir():
            found = sorted(p.glob("*.png"))
        elif any(ch in it for ch in "*?["):
            found = [Path(f) for f in sorted(glob.glob(it))]
        else:
            found = [p]
        for f in found:
            if f not in out:
                out.append(f)
    return out


def load_font(size: int):
    """A readable TrueType font if one is installed, otherwise Pillow's default."""
    for name in ("DejaVuSans.ttf", "segoeui.ttf", "arial.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


def build_sheet(
    paths: list[Path],
    scale: int = 4,
    cell: tuple[int, int] | None = None,
    cols: int = 10,
    bg: str = "#1c1c22",
    labels: bool = True,
    title: str = "",
    frames: int = 1,
) -> Image.Image:
    """Returns the sheet image (RGBA)."""
    tiles: list[tuple[str, Image.Image]] = []
    for p in paths:
        im = Image.open(p).convert("RGBA")
        if frames > 1:
            fw = im.width // frames
            for k in range(frames):
                tiles.append((f"{p.stem}#{k}", im.crop((k * fw, 0, (k + 1) * fw, im.height))))
        else:
            tiles.append((p.stem, im))
    if not tiles:
        raise SystemExit("no input image")
    cw = cell[0] if cell else max(t.width for _, t in tiles)
    ch = cell[1] if cell else max(t.height for _, t in tiles)
    cols = max(1, min(cols, len(tiles)))
    rows = (len(tiles) + cols - 1) // cols
    pad = 4 * scale
    label_h = 12 if labels else 0
    title_h = 22 if title else 0
    cell_w = max(cw * scale, 56 if labels else 0)
    cell_h = ch * scale + label_h
    W = pad + cols * (cell_w + pad)
    H = title_h + pad + rows * (cell_h + pad)
    sheet = Image.new("RGBA", (W, H), bg)
    draw = ImageDraw.Draw(sheet)
    font = load_font(10)
    if title:
        draw.text((pad, 4), title, fill=(220, 230, 220, 255), font=load_font(14))
    for i, (name, im) in enumerate(tiles):
        r, c = divmod(i, cols)
        x0 = pad + c * (cell_w + pad)
        y0 = title_h + pad + r * (cell_h + pad)
        big = im.resize((im.width * scale, im.height * scale), Image.NEAREST)
        ox = x0 + (cell_w - big.width) // 2
        oy = y0 + (ch * scale - big.height) // 2
        sheet.alpha_composite(big, (max(0, ox), max(0, oy)))
        if labels:
            short = name if len(name) <= 14 else name[:13] + "…"
            draw.text((x0, y0 + ch * scale + 1), short, fill=(170, 170, 180, 255), font=font)
    return sheet


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("inputs", nargs="+", help="PNG files, folders or glob patterns")
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--scale", type=int, default=4)
    ap.add_argument("--cell", default="")
    ap.add_argument("--cols", type=int, default=10)
    ap.add_argument("--bg", default="#1c1c22")
    ap.add_argument("--no-labels", action="store_true")
    ap.add_argument("--title", default="")
    ap.add_argument("--frames", type=int, default=1)
    a = ap.parse_args()
    cell = tuple(int(v) for v in a.cell.lower().split("x")) if a.cell else None
    paths = expand_inputs(a.inputs)
    sheet = build_sheet(paths, a.scale, cell, a.cols, a.bg, not a.no_labels, a.title, a.frames)
    a.out.parent.mkdir(parents=True, exist_ok=True)
    tmp = a.out.with_suffix(".tmp.png")
    sheet.save(tmp)
    os.replace(tmp, a.out)
    print(f"{a.out}: {len(paths)} images, {sheet.width}x{sheet.height}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
