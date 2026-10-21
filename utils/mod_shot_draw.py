#!/usr/bin/env python3
"""mod_shot_draw.py — draws projectile frames in the style of the original shots.

The original player shots (data/reference/sprites/shots/, 12x14 cells) are
small glowing shapes: a near-white core, a saturated body, a dark rim of the
same hue, no black outline. Image generators blur at that size, so the shots
of a mod are drawn here from a few shape functions; every pixel is a colour of
the game palette (palette 5), taken from one gradient of palette indices.

Each shape is a function intensity(x, y, frame) -> 0..1 sampled at pixel
centres (4x4 supersampling); the intensity picks a step of the gradient
(0 = transparent). Frames vary the shape slightly (flicker, spin, pulse) so a
weapon level with "animation": N-1 cycles through N frames, one per game tick.
The frames are saved as <name>_1.png .. <name>_N.png: the engine numbers the
PNGs of sprites/shots/ in name order, so the frames of one shot get
consecutive sprite slots and "sprite": "<name>_1" starts the cycle.

Usage:
    python utils/mod_shot_draw.py --list
    python utils/mod_shot_draw.py lance fire OUT_DIR/thermal_lance --frames 4
    python utils/mod_shot_draw.py orb green OUT_DIR/spore_pod --frames 4 --size 8

Shapes:   lance, orb, crescent, dart, ring, ember
Gradients: fire (Zica: red -> orange -> yellow -> white),
           green (Hazudra: deep green -> neon green -> pale bone),
           bone (Hazudra bone: brown -> sand -> white)
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover
    print("ERROR: Pillow required (pip install -r utils/requirements.txt)", file=sys.stderr)
    sys.exit(2)

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _shared import load_vga_palette  # noqa: E402

DATA = Path(__file__).resolve().parent.parent / "data"
W, H = 12, 14  # one original shot cell
SS = 4  # supersampling per axis

# Palette indices, darkest (rim) to brightest (core). Ramp 7 = fire, ramp 12 =
# neon green, ramp 2 = olive to bone, ramp 10 = sand/bone (python utils/mod_art_clean.py ramps).
GRADIENTS = {
    "fire": [114, 116, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127],
    "green": [194, 196, 198, 200, 202, 204, 205, 206, 207, 43, 44, 46],
    "bone": [164, 166, 168, 170, 171, 172, 173, 174, 175, 45, 46, 15],
}


def _smooth(e0: float, e1: float, x: float) -> float:
    t = max(0.0, min(1.0, (x - e0) / (e1 - e0)))
    return t * t * (3 - 2 * t)


# ---------------------------------------------------------------- shapes
# Coordinates: cx, cy = cell centre; y grows downward; shots fly up (towards -y).


def shape_lance(x, y, f, size):
    """Vertical thermal lance: white-hot core, tapered tip at the top, flickering width."""
    cx = (W - 1) / 2
    half = size / 2 + 0.25 * math.sin(f * 1.7 + y * 0.9)
    tip = 1.0 + (f % 2)  # the tip shimmers by one pixel
    if y < tip:
        return 0.0
    taper = _smooth(tip, tip + 4.0, y)
    tail = 1 - _smooth(H - 3.0, H + 0.5, y)
    d = abs(x - cx) / max(0.4, half * (0.45 + 0.55 * taper))
    return max(0.0, 1 - d) ** 0.7 * (0.35 + 0.65 * taper) * (0.55 + 0.45 * tail)


def shape_orb(x, y, f, size):
    """Round pulsing orb (spore pod, plasma ball) with the light from the upper left."""
    cx, cy = (W - 1) / 2, (H - 1) / 2
    r = size / 2 * (1 + 0.06 * math.sin(f * math.pi / 2))
    d = math.hypot(x - cx, y - cy) / r
    if d > 1:
        return 0.0
    hl = math.hypot(x - (cx - r * 0.3), y - (cy - r * 0.3)) / r
    return min(1.0, 0.25 + 0.55 * (1 - d) ** 0.6 + 0.45 * max(0, 1 - hl * 1.6))


def shape_crescent(x, y, f, size):
    """Crescent opening downward (the leading edge flies first), bright leading rim."""
    cx, cy = (W - 1) / 2, (H - 1) / 2 + 2
    r_out = size / 2
    r_in = r_out * 0.52
    oy = r_out * 0.40  # offset of the inner circle: makes the horns
    d_out = math.hypot(x - cx, (y - cy) * 1.15) / r_out
    d_in = math.hypot(x - cx, (y - cy - oy) * 1.15) / r_in
    if d_out > 1 or d_in < 1 or y > cy + 1.5:
        return 0.0
    edge = 1 - abs(d_out - 0.88) * 3.0
    shimmer = 0.08 * math.sin(f * math.pi / 2 + x)
    return max(0.25, min(1.0, 0.4 + 0.6 * edge + shimmer))


def shape_dart(x, y, f, size):
    """Seeker dart: bright diamond head, short flickering tail."""
    cx = (W - 1) / 2
    head_y = 4.5
    w = size / 2
    dy = y - head_y
    if dy < -4.5:
        return 0.0
    d = abs(x - cx) / w + abs(dy) / 4.5  # diamond head
    if d <= 1:
        return min(1.0, 0.3 + 1.0 * (1 - d))
    tail_len = 4.0 + (f % 2) * 1.5  # tail flicker
    t = (dy - 1.0) / tail_len
    if dy < 1.0 or t > 1 or abs(x - cx) > 1.3 * (1 - t) + 0.2:
        return 0.0
    return 0.55 * (1 - t) + 0.1


def shape_ring(x, y, f, size):
    """Spinning plasma ring: dark hole, one bright arc turning a quarter per frame."""
    cx, cy = (W - 1) / 2, (H - 1) / 2
    r = size / 2
    d = math.hypot(x - cx, y - cy)
    band = 1 - abs(d - r * 0.68) / (r * 0.34)
    if band <= 0:
        return 0.0
    a = math.atan2(y - cy, x - cx)
    spin = 0.5 + 0.5 * math.cos(a - f * math.pi / 2)
    return max(0.2, min(1.0, band * (0.45 + 0.6 * spin)))


def shape_ember(x, y, f, size):
    """Ember shell: hot ball with flame tongues that change every frame."""
    cx, cy = (W - 1) / 2, (H - 1) / 2 + 1
    r = size / 2
    a = math.atan2(y - cy, x - cx)
    flame = 1 + 0.18 * math.sin(5 * a + f * 1.9) + 0.1 * math.sin(3 * a - f * 2.3)
    d = math.hypot(x - cx, y - cy) / (r * flame)
    if d > 1:
        return 0.0
    hl = math.hypot(x - (cx - r * 0.25), y - (cy - r * 0.3)) / r
    return min(1.0, 0.2 + 0.6 * (1 - d) ** 0.8 + 0.4 * max(0, 1 - hl * 1.8))


SHAPES = {
    "lance": shape_lance,
    "orb": shape_orb,
    "crescent": shape_crescent,
    "dart": shape_dart,
    "ring": shape_ring,
    "ember": shape_ember,
}
DEFAULT_SIZE = {"lance": 5.0, "orb": 9.0, "crescent": 11.0, "dart": 9.0, "ring": 11.0, "ember": 9.0}


def draw(shape: str, gradient: str, frame: int, size: float) -> Image.Image:
    pal = [tuple(c) for c in load_vga_palette(str(DATA), 5)]
    grad = GRADIENTS[gradient]
    fn = SHAPES[shape]
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    for py in range(H):
        for px in range(W):
            acc, hits = 0.0, 0
            for sy in range(SS):
                for sx in range(SS):
                    v = fn(px + (sx + 0.5) / SS - 0.5, py + (sy + 0.5) / SS - 0.5, frame, size)
                    if v > 0:
                        acc += v
                        hits += 1
            if hits < SS * SS * 0.4:  # less than 40% covered: transparent (crisp edge)
                continue
            v = acc / hits
            step = min(len(grad) - 1, max(0, int(v * len(grad))))
            im.putpixel((px, py), pal[grad[step]] + (255,))
    return darken_rim(im, pal, grad)


def darken_rim(im: Image.Image, pal, grad) -> Image.Image:
    """Edge pixels take the darkest two steps of the gradient: the dark rim of the originals."""
    px = im.load()
    out = im.copy()
    o = out.load()
    rgb_to_step = {pal[i]: k for k, i in enumerate(grad)}
    for y in range(H):
        for x in range(W):
            if px[x, y][3] == 0:
                continue
            edge = any(
                not (0 <= x + dx < W and 0 <= y + dy < H) or px[x + dx, y + dy][3] == 0
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))
            )
            if edge:
                k = rgb_to_step.get(px[x, y][:3], 0)
                o[x, y] = pal[grad[min(k, 1) if k < 6 else 2]] + (255,)
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("shape", nargs="?", choices=sorted(SHAPES))
    ap.add_argument("gradient", nargs="?", choices=sorted(GRADIENTS))
    ap.add_argument("out", nargs="?", type=Path, help="path prefix; writes <out>_1.png .. <out>_N.png")
    ap.add_argument("--frames", type=int, default=4)
    ap.add_argument("--size", type=float, default=None, help="shape size in pixels (default per shape)")
    ap.add_argument("--list", action="store_true")
    a = ap.parse_args()
    if a.list or not a.out:
        for k in sorted(SHAPES):
            print(f"{k:9s} default size {DEFAULT_SIZE[k]}  — {SHAPES[k].__doc__}")
        print("gradients:", ", ".join(f"{k} {v}" for k, v in GRADIENTS.items()))
        return 0
    size = a.size if a.size is not None else DEFAULT_SIZE[a.shape]
    a.out.parent.mkdir(parents=True, exist_ok=True)
    for f in range(a.frames):
        p = a.out.parent / f"{a.out.name}_{f + 1}.png"
        draw(a.shape, a.gradient, f, size).save(p)
        print(p)
    return 0


if __name__ == "__main__":
    sys.exit(main())
