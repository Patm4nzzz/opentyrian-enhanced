#!/usr/bin/env python3
"""C / Godot gap of an in-game image, per zone.

Usage: visual_region_diff.py C.png GODOT.png [--scale N] [--out DIFF.png]
Zones (tyrian2.c: game_screen = 264x184 play area at the top left,
VGAScreenSeg = dashboard around it):
  play area    x 0..263,  y 0..183
  right bar    x 264..319, y 0..199
  bottom band  x 0..263,  y 184..199
Prints, per zone then in total, the number and % of differing RGB pixels.
"""

import sys

from PIL import Image

ZONES = [
    ("play area", (0, 0, 264, 184)),
    ("right bar", (264, 0, 320, 200)),
    ("bottom strip", (0, 184, 264, 200)),
    ("total", (0, 0, 320, 200)),
]


def load(path, scale):
    im = Image.open(path).convert("RGB")
    if im.size != (320, 200):
        im = im.resize((320, 200), Image.NEAREST) if scale > 1 else im
    return im


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    scale = int(argv[argv.index("--scale") + 1]) if "--scale" in argv else 1
    a, b = load(argv[0], scale), load(argv[1], scale)
    pa, pb = a.load(), b.load()
    diff = Image.new("RGB", (320, 200))
    pd = diff.load()
    for y in range(200):
        for x in range(320):
            pd[x, y] = (255, 0, 0) if pa[x, y] != pb[x, y] else tuple(c // 4 for c in pa[x, y])
    for name, (x0, y0, x1, y1) in ZONES:
        n = sum(1 for y in range(y0, y1) for x in range(x0, x1) if pa[x, y] != pb[x, y])
        t = (x1 - x0) * (y1 - y0)
        print(f"{name:13s} different={n:6d}/{t:6d} {100.0 * n / t:6.2f} %")
    if "--out" in argv:
        diff.resize((960, 600), Image.NEAREST).save(argv[argv.index("--out") + 1])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
