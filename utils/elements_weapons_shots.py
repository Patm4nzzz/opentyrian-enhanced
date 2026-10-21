#!/usr/bin/env python3
"""elements_weapons_shots.py — projectile frames of the Elements Weapon Pack.

Adds seven element shapes and their palette gradients to utils/mod_shot_draw.py
(same 12x14 cell, 4x4 supersampling, dark rim of the same hue, frames saved as
<name>_1.png .. <name>_N.png) without changing that file:

    flame     fire jet: white-hot head, flickering tongues trailing behind   (gradient fire)
    shard     faceted ice crystal, a sparkle moving over the facets          (gradient ice)
    flake     six-armed frost flake turning 15 degrees per frame             (gradient frost)
    bolt      jagged yellow-white lightning bolt, new zig-zag every frame    (gradient volt)
    boulder   lumpy rock ball with craters turning with the frames           (gradient rock)
    drop      acid droplet that wobbles, with a small drip                   (gradient acid)
    gale      two-armed wind swirl spinning a quarter turn per frame         (gradient gale)

Usage:
    python utils/elements_weapons_shots.py OUT_DIR [--sheet SHEET.png]
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import mod_shot_draw as msd  # noqa: E402

W, H = msd.W, msd.H
CX = (W - 1) / 2

GRADIENTS = {
    "fire": msd.GRADIENTS["fire"],
    "ice": [146, 148, 150, 152, 154, 156, 157, 158, 159, 141, 142, 15],
    "frost": [130, 132, 134, 136, 137, 138, 139, 140, 141, 142, 143, 15],
    "volt": [246, 247, 248, 249, 250, 251, 252, 124, 125, 126, 127, 15],
    "rock": [19, 21, 24, 26, 27, 28, 29, 30, 30, 31, 31, 15],
    "acid": [194, 196, 198, 200, 202, 204, 206, 207, 251, 252, 253, 15],
    "gale": [211, 212, 214, 216, 218, 219, 220, 221, 222, 223, 143, 15],
}


def _seg_dist(px, py, ax, ay, bx, by):
    vx, vy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((px - ax) * vx + (py - ay) * vy) / (vx * vx + vy * vy or 1)))
    return math.hypot(px - ax - t * vx, py - ay - t * vy)


def shape_flame(x, y, f, size):
    """Fire jet flying up: round white-hot head, tongues flickering behind it."""
    head = 2.0
    t = (y - head) / (H - 1 - head)
    if t < 0 or t > 1:
        return 0.0
    sway = 0.9 * math.sin(f * 1.7 + y * 0.8) * t
    w = size / 2 * (0.65 + 0.35 * math.sin(math.pi * min(1.0, t * 1.8))) * (1 - 0.55 * t)
    w += 0.7 * math.sin(f * 2.1 + y * 1.4) * t
    if w <= 0.3:
        return 0.0
    d = abs(x - CX - sway) / w
    if d > 1:
        return 0.0
    tip = 1 - math.hypot((x - CX) / 2.2, (y - head - 0.5) / 2.0) if t < 0.15 else 0  # rounded head
    return max(tip, min(1.0, (1 - d) ** 0.55 * (1.05 - 0.6 * t)))


def shape_shard(x, y, f, size):
    """Ice crystal: long hexagon, light left facet, dark right facet, a moving sparkle."""
    cy = (H - 1) / 2
    a, hh = size * 0.32, size * 0.62
    dx, dy = x - CX, y - cy
    d = max(abs(dx) / a, abs(dx) * 0.9 / a + abs(dy) / hh)
    if d > 1:
        return 0.0
    v = 0.78 if dx < -0.2 else 0.5
    if abs(dx) < 0.55:
        v = 0.92
    sx, sy = ((-1, -2.5), (0.8, -0.5), (-0.8, 1.5), (0.6, 3))[f % 4]
    if math.hypot(dx - sx, dy - sy) < 0.8:
        v = 1.0
    return min(1.0, v + 0.15 * (1 - d))


def shape_flake(x, y, f, size):
    """Six-armed frost flake with a bright centre, turning 15 degrees per frame."""
    cy = (H - 1) / 2
    R = size / 2
    r = math.hypot(x - CX, y - cy)
    if r > R:
        return 0.0
    best = 9.0
    for k in range(6):
        th = f * math.pi / 12 + k * math.pi / 3
        best = min(best, _seg_dist(x, y, CX, cy, CX + R * math.cos(th), cy + R * math.sin(th)))
    if r < 1.6:
        return 1.0
    if best > 0.75:
        return 0.0
    return min(1.0, 1.0 - 0.45 * r / R)


BOLT_X = [
    [0, 2, -2, 1, -2, 1],
    [0, -2, 2, -1, 2, -1],
    [0, 1, -2, 2, -1, 2],
    [0, -1, 2, -2, 1, -2],
]


def shape_bolt(x, y, f, size):
    """Lightning bolt: white core on a zig-zag through the cell, yellow glow, new path every frame."""
    xs = BOLT_X[f % 4]
    pts = [(CX + xs[k], k * (H - 1) / (len(xs) - 1)) for k in range(len(xs))]
    d = min(_seg_dist(x, y, *pts[k], *pts[k + 1]) for k in range(len(pts) - 1))
    glow = size / 2
    if d > glow:
        return 0.0
    return min(1.0, 1.05 - d / glow * 0.85) if d > 0.5 else 1.0


def shape_boulder(x, y, f, size):
    """Lumpy rock ball, light from the upper left, two craters turning with the frames."""
    cy = (H - 1) / 2 + 0.5
    R = size / 2
    a = math.atan2(y - cy, x - CX)
    rr = R * (1 + 0.09 * math.sin(5 * a + 1.3) + 0.06 * math.sin(3 * a - 0.7))
    r = math.hypot(x - CX, y - cy)
    if r > rr:
        return 0.0
    hl = math.hypot(x - (CX - R * 0.35), y - (cy - R * 0.35)) / R
    v = 0.3 + 0.55 * max(0.0, 1 - hl * 0.9) + 0.15 * (1 - r / rr)
    for k in range(2):
        th = f * math.pi / 2 + k * 2.6
        kx, ky = CX + R * 0.45 * math.cos(th), cy + R * 0.45 * math.sin(th)
        if math.hypot(x - kx, y - ky) < R * 0.22:
            v -= 0.3
    return max(0.15, min(1.0, v))


def shape_drop(x, y, f, size):
    """Acid droplet: round body, pointed top, wobble and a small drip that comes and goes."""
    cy = (H - 1) / 2 + 1
    R = size / 2 * (1 + 0.06 * math.sin(f * math.pi / 2))
    dy = y - cy
    w = R * (1.0 if dy > 0 else max(0.15, 1 + dy / (R * 1.9)))
    d = math.hypot((x - CX) / max(0.3, w), dy / (R * (1.6 if dy < 0 else 1.0)))
    if d <= 1:
        hl = math.hypot(x - (CX - R * 0.35), y - (cy - R * 0.1)) / R
        return min(1.0, 0.35 + 0.4 * (1 - d) + 0.5 * max(0.0, 1 - hl * 1.7))
    ty = cy + R + 1.6 + (f % 2)
    if f % 4 != 3 and math.hypot(x - CX, y - ty) < 1.0:
        return 0.75
    return 0.0


def shape_gale(x, y, f, size):
    """Wind swirl: two curved arms, spinning a quarter turn per frame, bright leading edges."""
    cy = (H - 1) / 2
    R = size / 2
    r = math.hypot(x - CX, y - cy)
    if r > R or r < 0.6:
        return 0.9 if r < 0.6 else 0.0
    a = math.atan2(y - cy, x - CX)
    if r < 1.8:
        return 1.0
    v = math.sin(2 * (a - f * math.pi / 4) - r * 0.8)
    if v < -0.25:
        return 0.0
    return min(1.0, 0.45 + 0.55 * (r / R) * max(0.0, v) + 0.2 * (1 - r / R))


SHAPES = {
    "flame": (shape_flame, "fire", 7.0),
    "shard": (shape_shard, "ice", 11.0),
    "flake": (shape_flake, "frost", 10.0),
    "bolt": (shape_bolt, "volt", 3.6),
    "boulder": (shape_boulder, "rock", 9.0),
    "drop": (shape_drop, "acid", 6.0),
    "gale": (shape_gale, "gale", 11.0),
}

# Shots of the pack: file name -> (shape, size)
PACK = {
    "flame_jet": ("flame", 8.0),
    "flame_jet_large": ("flame", 10.0),
    "frost_shard": ("shard", 12.0),
    "frost_flake": ("flake", 10.0),
    "arc_bolt": ("bolt", 2.8),
    "arc_bolt_large": ("bolt", 3.8),
    "boulder": ("boulder", 9.0),
    "boulder_large": ("boulder", 11.0),
    "acid_drop": ("drop", 6.5),
    "acid_drop_large": ("drop", 8.5),
    "gale_blade": ("gale", 11.0),
}


def register():
    for name, (fn, _grad, size) in SHAPES.items():
        msd.SHAPES[f"el_{name}"] = fn
        msd.DEFAULT_SIZE[f"el_{name}"] = size
    for g, steps in GRADIENTS.items():
        msd.GRADIENTS.setdefault(f"el_{g}", steps)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("out", type=Path)
    ap.add_argument("--frames", type=int, default=4)
    ap.add_argument("--sheet", type=Path, help="also write a 6x sheet, one row per shot")
    a = ap.parse_args()
    register()
    a.out.mkdir(parents=True, exist_ok=True)
    rows = []
    for name, (shape, size) in PACK.items():
        grad = SHAPES[shape][1]
        frames = [msd.draw(f"el_{shape}", f"el_{grad}", f, size) for f in range(a.frames)]
        for f, im in enumerate(frames):
            im.save(a.out / f"{name}_{f + 1}.png")
        rows.append(frames)
        print(name, a.frames, "frames")
    if a.sheet:
        k = 6
        sheet = Image.new("RGBA", (a.frames * (W + 2) * k, len(rows) * (H + 2) * k), (32, 32, 40, 255))
        for r, frames in enumerate(rows):
            for c, im in enumerate(frames):
                sheet.alpha_composite(im.resize((W * k, H * k), Image.NEAREST), (c * (W + 2) * k, r * (H + 2) * k))
        sheet.save(a.sheet)
    return 0


if __name__ == "__main__":
    sys.exit(main())
