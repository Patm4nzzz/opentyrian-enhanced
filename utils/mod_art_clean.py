#!/usr/bin/env python3
"""mod_art_clean.py — turn a generated draft into a sprite in the original Tyrian style.

Every pixel of the result is a colour of the game palette (data/palette.dat,
palette 5, the gameplay palette), limited to the colour ramps you choose.
Used by the mod art pipeline (docs/mod-art-pipeline.md) after PixelLab:

    icon   draft (any size, transparent background) -> 24x28 shop icon:
           crop to the subject, box-downscale to fit, snap to the ramps,
           near-black outline (index 16) and the two-step lower-right drop
           shadow of the original icons (indices 64 then 161).
    snap   snap every opaque pixel of a PNG to the ramps (no resize).
    check  list the PNGs of a folder (recursive) with a colour outside the game
           palette or a size other than the expected one; exit code 1 if any.
    ramps  print the 16 palette ramps (index, hex) to choose from.

Ramps are the 16-colour rows of the palette: 0 grey, 1 tan, 2 olive/bone-green,
3 steel blue, 4 dark red, 7 fire (red -> yellow -> white), 10 bone/sand,
12 neon green, 15 purple -> yellow. Index 0 is never used (transparent).

Usage:
    python utils/mod_art_clean.py ramps
    python utils/mod_art_clean.py icon draft.png out.png --ramps 0,10,2,12 --glow 12
    python utils/mod_art_clean.py snap in.png out.png --ramps 0,7
    python utils/mod_art_clean.py check rewrite/mods/alien_weapon_pack/sprites

Options (icon):
    --size WxH      final canvas (default 24x28)
    --ramps LIST    ramps the result may use (default 0,10)
    --glow RAMP     saturated pixels of the draft are pushed into this ramp at
                    full brightness (the coloured core of the original icons)
    --glow-min N    minimum ramp step (0..15) of a glow pixel (default 9)
    --lift N        add N ramp steps to every non-glow pixel (default 0)
    --glow-sat F    saturation above which a draft pixel counts as glow (default 0.45)
    --contrast F    contrast boost before snapping (1.3 restores what the downscale flattens)
    --no-shadow     outline only
"""

from __future__ import annotations

import argparse
import colorsys
import os
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover
    print("ERROR: Pillow required (pip install -r utils/requirements.txt)", file=sys.stderr)
    sys.exit(2)

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _shared import load_vga_palette  # noqa: E402

GAMEPLAY_PALETTE = 5  # tyrian-formats palette.rs: PCX_PALETTE[2]
OUTLINE = 16  # near-black brown rim of the original shop icons
SHADOW = (64, 161)  # drop shadow, one then two pixels down-right
DATA = Path(__file__).resolve().parent.parent / "data"


def palette() -> list[tuple[int, int, int]]:
    return [tuple(c) for c in load_vga_palette(str(DATA), GAMEPLAY_PALETTE)]


def ramp_indices(ramps: list[int]) -> list[int]:
    out = []
    for r in ramps:
        out += [i for i in range(r * 16, r * 16 + 16) if i != 0]
    return out


def _dist(a, b) -> float:
    # weighted RGB distance (red/blue weights depend on mean red), cheap and good enough
    rm = (a[0] + b[0]) / 2
    dr, dg, db = a[0] - b[0], a[1] - b[1], a[2] - b[2]
    return (2 + rm / 256) * dr * dr + 4 * dg * dg + (2 + (255 - rm) / 256) * db * db


def nearest(rgb, pal, allowed: list[int]) -> int:
    return min(allowed, key=lambda i: _dist(rgb, pal[i]))


GLOW_SAT = 0.45


def is_glow(rgb) -> bool:
    h, s, v = colorsys.rgb_to_hsv(*(c / 255 for c in rgb))
    return s > GLOW_SAT and v > 0.40


def snap_image(im: Image.Image, pal, allowed, glow_ramp=None, glow_min=9, lift=0) -> Image.Image:
    im = im.convert("RGBA")
    out = Image.new("RGBA", im.size, (0, 0, 0, 0))
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = im.getpixel((x, y))
            if a < 128:
                continue
            if glow_ramp is not None and is_glow((r, g, b)):
                _, _, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
                step = max(glow_min, min(15, round(v * 15)))
                i = glow_ramp * 16 + step
            else:
                i = nearest((r, g, b), pal, allowed)
                if lift:
                    base = i - i % 16
                    i = min(base + 15, i + lift)
            out.putpixel((x, y), pal[i] + (255,))
    return out


def fit_subject(im: Image.Image, box_w: int, box_h: int) -> Image.Image:
    """Crop to the opaque pixels and box-downscale (never upscale) to fit box_w x box_h."""
    im = im.convert("RGBA")
    bbox = im.getchannel("A").point(lambda a: 255 if a >= 128 else 0).getbbox()
    if bbox:
        im = im.crop(bbox)
    k = min(box_w / im.width, box_h / im.height, 1.0)
    w, h = max(1, round(im.width * k)), max(1, round(im.height * k))
    if (w, h) != im.size:
        # premultiplied box filter: transparent pixels do not darken the edges
        im = im.resize((w, h), Image.Resampling.BOX)
    return im


def strip_dark_edge(im: Image.Image, max_lum: int = 40) -> Image.Image:
    """Removes the draft's own black outline (very dark pixels touching transparency):
    the original icons get a single rim, added afterwards."""
    W, H = im.size
    px = im.load()
    out = im.copy()
    o = out.load()
    for y in range(H):
        for x in range(W):
            r, g, b, a = px[x, y]
            if a == 0 or (r * 3 + g * 6 + b) / 10 > max_lum:
                continue
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                qx, qy = x + dx, y + dy
                if not (0 <= qx < W and 0 <= qy < H) or px[qx, qy][3] == 0:
                    o[x, y] = (0, 0, 0, 0)
                    break
    return out


def add_outline_and_shadow(im: Image.Image, pal, shadow: bool = True) -> Image.Image:
    W, H = im.size
    px = im.load()
    solid = {(x, y) for y in range(H) for x in range(W) if px[x, y][3] > 0}
    out = im.copy()
    o = out.load()
    rim = set()
    for x, y in solid:
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            q = (x + dx, y + dy)
            if q not in solid and 0 <= q[0] < W and 0 <= q[1] < H:
                rim.add(q)
    for q in rim:
        o[q] = pal[OUTLINE] + (255,)
    body = solid | rim
    if shadow:
        for k, idx in ((1, SHADOW[0]), (2, SHADOW[1])):
            for x, y in sorted(body):
                q = (x + k, y + k)
                if 0 <= q[0] < W and 0 <= q[1] < H and o[q][3] == 0:
                    o[q] = pal[idx] + (255,)
    return out


def make_icon(
    src: Path, dst: Path, size=(24, 28), ramps=(0, 10), glow=None, glow_min=9, lift=0, shadow=True, contrast=1.0
):
    pal = palette()
    allowed = ramp_indices(list(ramps))
    margin_r = 3 if shadow else 1  # outline + shadow on the right and bottom
    sub = fit_subject(Image.open(src), size[0] - 1 - margin_r, size[1] - 1 - margin_r)
    if contrast != 1.0:
        # the box filter flattens highlights; the originals are high-contrast
        from PIL import ImageEnhance

        alpha = sub.getchannel("A")
        sub = ImageEnhance.Contrast(sub.convert("RGB")).enhance(contrast).convert("RGBA")
        sub.putalpha(alpha)
    sub = snap_image(sub, pal, allowed, glow, glow_min, lift)
    sub = strip_dark_edge(sub)
    canvas = Image.new("RGBA", size, (0, 0, 0, 0))
    x0 = 1 + (size[0] - 1 - margin_r - sub.width) // 2
    y0 = 1 + (size[1] - 1 - margin_r - sub.height) // 2
    canvas.alpha_composite(sub, (x0, y0))
    canvas = add_outline_and_shadow(canvas, pal, shadow)
    dst.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(dst)
    return canvas


# Expected size per sprite folder (MODDING.md 6.1, original templates).
EXPECTED_SIZE = {"shots": (12, 14), "ships": (24, 28), "weapons": (24, 28)}


def check_folder(folder: Path) -> int:
    colours = set(palette()[1:])
    bad = 0
    files = sorted(folder.rglob("*.png"))
    for f in files:
        im = Image.open(f).convert("RGBA")
        px = list(getattr(im, "get_flattened_data", im.getdata)())
        off = {p[:3] for p in px if p[3] and p[:3] not in colours}
        want = EXPECTED_SIZE.get(f.parent.name)
        problems = []
        if off:
            problems.append(f"{len(off)} colour(s) outside the palette")
        if any(0 < p[3] < 255 for p in px):
            problems.append("semi-transparent pixels")
        if want and im.size != want:
            problems.append(f"size {im.width}x{im.height}, expected {want[0]}x{want[1]}")
        if problems:
            bad += 1
            print(f"{f}: " + "; ".join(problems))
    print(f"{len(files)} PNG(s) checked, {bad} with problems")
    return 1 if bad else 0


def main() -> int:
    global GLOW_SAT
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("ramps")
    ic = sub.add_parser("icon")
    ic.add_argument("src", type=Path)
    ic.add_argument("dst", type=Path)
    ic.add_argument("--size", default="24x28")
    ic.add_argument("--ramps", default="0,10")
    ic.add_argument("--glow", type=int, default=None)
    ic.add_argument("--glow-min", type=int, default=9)
    ic.add_argument("--lift", type=int, default=0)
    ic.add_argument("--no-shadow", action="store_true")
    ic.add_argument("--contrast", type=float, default=1.0)
    ic.add_argument("--glow-sat", type=float, default=GLOW_SAT)
    sn = sub.add_parser("snap")
    sn.add_argument("src", type=Path)
    sn.add_argument("dst", type=Path)
    sn.add_argument("--ramps", default="0")
    ck = sub.add_parser("check")
    ck.add_argument("folder", type=Path)
    a = ap.parse_args()
    if a.cmd == "check":
        return check_folder(a.folder)
    if a.cmd == "ramps":
        pal = palette()
        for r in range(16):
            print(
                f"{r:2d} "
                + " ".join(
                    f"{r * 16 + i}:{pal[r * 16 + i][0]:02x}{pal[r * 16 + i][1]:02x}{pal[r * 16 + i][2]:02x}"
                    for i in range(16)
                )
            )
        return 0
    ramps = [int(v) for v in a.ramps.split(",")]
    if a.cmd == "icon":
        w, h = (int(v) for v in a.size.lower().split("x"))
        GLOW_SAT = a.glow_sat
        make_icon(a.src, a.dst, (w, h), ramps, a.glow, a.glow_min, a.lift, not a.no_shadow, a.contrast)
    else:
        pal = palette()
        out = snap_image(Image.open(a.src), pal, ramp_indices(ramps))
        tmp = a.dst.with_suffix(".tmp.png")
        out.save(tmp)
        os.replace(tmp, a.dst)
    print(a.dst)
    return 0


if __name__ == "__main__":
    sys.exit(main())
