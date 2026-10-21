#!/usr/bin/env python3
"""bio_ships_art.py — sprites of the Bio Ships Pack (rewrite/mods/bio_ships_pack).

Turns a top-down PixelLab ship draft into the five 24x28 poses of a player
ship in the original Tyrian style (docs/mod-art-pipeline.md):

    neutral    crop to the subject, box-downscale into a per-ship box (a light
               scout is drawn smaller than a flagship), raise the contrast,
               snap every pixel to the bio ramps of palette 5 (olive green,
               bone, flesh; saturated pixels go to the neon-green glow ramp),
               optionally mirror the left half for a symmetrical hull, then
               the one-pixel dark rim (index 16) of the original ships.
    poses      left / left_full / right / right_full drawn from the neutral
               one as a roll: the hull slides one or two pixels toward the
               turn, the lowered wing is shortened and one ramp step darker,
               the raised wing barely shortened and one step lighter.

The cleanup itself reuses utils/mod_art_clean.py (imported, never modified).

Usage:
    python utils/bio_ships_art.py ship draft.png out_dir name [--box 20x24]
        [--vflip] [--mirror] [--ramps 2,10,4] [--glow 12] [--contrast 1.5]
        [--lift -1] [--glow-sat 0.70] [--glow-box x0,y0,x1,y1 ...]
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from PIL import Image, ImageEnhance

sys.path.insert(0, str(Path(__file__).resolve().parent))
import mod_art_clean as mac  # noqa: E402

SIZE = (24, 28)
GLOW_RAMP = 12
GLOW_MIN_SAT = 0.70  # only very saturated, light greens (sacs, eyes) glow
GLOW_MIN_VAL = 0.45


def is_bio_glow(rgb) -> bool:
    """The hulls are green too: only the near-pure bright greens of the draft
    (spore sacs, eyes) go to the neon ramp, the rest of the green snaps to the
    olive ramp."""
    import colorsys

    h, s, v = colorsys.rgb_to_hsv(*(c / 255 for c in rgb))
    return 0.20 < h < 0.45 and s > GLOW_MIN_SAT and v > GLOW_MIN_VAL


mac.is_glow = is_bio_glow  # replaces the mod_art_clean criterion for this script only


def neutral_sprite(src: Path, box, ramps, glow, contrast, lift, vflip, mirror, glow_boxes=()) -> Image.Image:
    pal = mac.palette()
    allowed = mac.ramp_indices(ramps)
    im = Image.open(src).convert("RGBA")
    if vflip:
        im = im.transpose(Image.Transpose.FLIP_TOP_BOTTOM)
    sub0 = mac.fit_subject(im, box[0], box[1])
    sub = sub0
    if contrast != 1.0:
        alpha = sub.getchannel("A")
        sub = ImageEnhance.Contrast(sub.convert("RGB")).enhance(contrast).convert("RGBA")
        sub.putalpha(alpha)
    # the glow mask is read from the draft colours BEFORE the contrast
    # (the contrast saturates all the green of the hull)
    sub = mac.snap_image(sub, pal, allowed, None, 9, 0)
    idx = _index_map(pal)
    p0, ps = sub0.load(), sub.load()
    import colorsys

    for y in range(sub.height):
        for x in range(sub.width):
            if not ps[x, y][3]:
                continue
            r, g, b, _ = p0[x, y]
            if glow is not None and is_bio_glow((r, g, b)):
                v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)[2]
                ps[x, y] = pal[glow * 16 + max(9, min(15, round(v * 15) + 2))] + (255,)
            elif lift:
                ps[x, y] = pal[_shift_step(idx[ps[x, y][:3]], lift)] + (255,)
    sub = mac.strip_dark_edge(sub)
    canvas = Image.new("RGBA", SIZE, (0, 0, 0, 0))
    x0 = (SIZE[0] - sub.width) // 2
    y0 = (SIZE[1] - sub.height) // 2
    canvas.alpha_composite(sub, (x0, y0))
    if mirror:
        # symmetric hull: the (lit) left half is copied to the right
        px = canvas.load()
        W = SIZE[0]
        for y in range(SIZE[1]):
            for x in range(W // 2):
                px[W - 1 - x, y] = px[x, y]
    # hand touch-up: the chosen sacs / eyes move to the neon ramp,
    # graded from the centre to the edge
    px = canvas.load()
    for x0, y0, x1, y1 in glow_boxes:
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                p = px[x, y]
                if p[3]:
                    # gradient: near-white core, darker edge (light from the top left)
                    cx, cy = (x0 + x1) / 2 - 0.5, (y0 + y1) / 2 - 0.5
                    rx, ry = max(1, (x1 - x0) / 2 + 0.5), max(1, (y1 - y0) / 2 + 0.5)
                    d = (((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2) ** 0.5
                    px[x, y] = pal[GLOW_RAMP * 16 + max(7, min(15, round(15 - 7 * d)))] + (255,)
    canvas = shade_upper_left(canvas, pal)
    return mac.add_outline_and_shadow(canvas, pal, shadow=False)


def shade_upper_left(im: Image.Image, pal) -> Image.Image:
    """Light from the upper left like the original ships: the right half of the
    hull one ramp step darker, the pixels on a right or bottom edge one more."""
    idx = _index_map(pal)
    W, H = im.size
    px = im.load()
    out = im.copy()
    op = out.load()
    for y in range(H):
        for x in range(W):
            p = px[x, y]
            if not p[3]:
                continue
            i = idx[p[:3]]
            d = -1 if x >= W // 2 else 0
            right = x + 1 >= W or not px[x + 1, y][3]
            below = y + 1 >= H or not px[x, y + 1][3]
            left = x == 0 or not px[x - 1, y][3]
            above = y == 0 or not px[x, y - 1][3]
            if right or below:
                d -= 1
            elif (left or above) and x < W // 2:
                d += 1
            op[x, y] = pal[_shift_step(i, d)] + (255,)
    return out


def _index_map(pal):
    return {tuple(c): i for i, c in enumerate(pal) if i}


def _shift_step(i: int, d: int) -> int:
    """Move a palette index d steps inside its 16-colour ramp (glow untouched)."""
    if i // 16 == GLOW_RAMP or i == mac.OUTLINE:
        return i
    base = i - i % 16
    return max(base + 1, min(base + 15, i + d))


def bank_pose(neutral: Image.Image, side: str, full: bool) -> Image.Image:
    """Roll toward `side` ('left' or 'right'): see the module docstring."""
    pal = mac.palette()
    idx = _index_map(pal)
    src = neutral if side == "left" else neutral.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
    W, H = src.size
    sp = src.load()
    # the body without the outline: the outline is redrawn after the warp
    body = Image.new("RGBA", src.size, (0, 0, 0, 0))
    bp = body.load()
    for y in range(H):
        for x in range(W):
            p = sp[x, y]
            if p[3] and idx.get(p[:3]) != mac.OUTLINE:
                bp[x, y] = p
    cx = (W - 1) / 2
    shift = 2 if full else 1  # the hull slides towards the turn
    k_low = 0.62 if full else 0.80  # lowered wing (turn side) shortened
    k_high = 0.90 if full else 0.96  # raised wing barely shortened
    new_c = cx - shift
    out = Image.new("RGBA", src.size, (0, 0, 0, 0))
    op = out.load()
    for y in range(H):
        for x in range(W):
            if x <= new_c:
                s = cx - (new_c - x) / k_low
                d = -1 if full else (-1 if (new_c - x) > 3 else 0)
            else:
                s = cx + (x - new_c) / k_high
                d = 1 if (x - new_c) > 3 else 0
            sx = round(s)
            if not 0 <= sx < W:
                continue
            p = bp[sx, y]
            if not p[3]:
                continue
            i = idx.get(p[:3])
            if i is None:
                continue
            op[x, y] = pal[_shift_step(i, d)] + (255,)
    out = mac.add_outline_and_shadow(out, pal, shadow=False)
    return out if side == "left" else out.transpose(Image.Transpose.FLIP_LEFT_RIGHT)


def main() -> int:
    global GLOW_MIN_SAT
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sh = sub.add_parser("ship")
    sh.add_argument("src", type=Path)
    sh.add_argument("out_dir", type=Path)
    sh.add_argument("name")
    sh.add_argument("--box", default="22x26")
    sh.add_argument("--ramps", default="2,10,4")
    sh.add_argument("--glow", type=int, default=GLOW_RAMP)
    sh.add_argument("--contrast", type=float, default=1.5)
    sh.add_argument("--lift", type=int, default=-1)
    sh.add_argument("--glow-sat", type=float, default=GLOW_MIN_SAT)
    sh.add_argument("--vflip", action="store_true")
    sh.add_argument("--mirror", action="store_true")
    sh.add_argument("--glow-box", action="append", default=[], help="x0,y0,x1,y1 repainted in the neon ramp")
    a = ap.parse_args()
    GLOW_MIN_SAT = a.glow_sat
    box = tuple(int(v) for v in a.box.lower().split("x"))
    ramps = [int(v) for v in a.ramps.split(",")]
    gb = [tuple(int(v) for v in b.split(",")) for b in a.glow_box]
    n = neutral_sprite(a.src, box, ramps, a.glow, a.contrast, a.lift, a.vflip, a.mirror, gb)
    a.out_dir.mkdir(parents=True, exist_ok=True)
    n.save(a.out_dir / f"{a.name}.png")
    for side in ("left", "right"):
        bank_pose(n, side, False).save(a.out_dir / f"{a.name}_{side}.png")
        bank_pose(n, side, True).save(a.out_dir / f"{a.name}_{side}_full.png")
    print(a.out_dir / a.name)
    return 0


if __name__ == "__main__":
    sys.exit(main())
