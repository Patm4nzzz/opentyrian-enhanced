#!/usr/bin/env python3
"""bio_weapons_art.py — sprites of the Bio Weapons Pack (rewrite/mods/bio_weapons_pack).

Two jobs, both pure Python/Pillow, every pixel a colour of the gameplay palette
(data/palette.dat, palette 5):

    icons   the six 24x28 shop icons. Each one starts from an original Tyrian
            shop icon (data/reference/sprites/pickups/weapons/), keeps its metal
            gun housing and turns it into a living weapon: the coloured core
            becomes a glowing green sac, plates become olive carapace or bone,
            pods, thorns, larvae and seeds are painted over the metal. The rim
            (index 16) and the two-step drop shadow (64, 161) are redrawn by
            utils/mod_art_clean.py (add_outline_and_shadow).
    shots   the 12x14 projectile frames, through the renderer of
            utils/mod_shot_draw.py (supersampled intensity -> palette gradient,
            dark rim), with the organic shapes of this pack registered at run
            time (the shared tool itself is not modified).

Usage:
    python utils/bio_weapons_art.py icons [--out DIR]
    python utils/bio_weapons_art.py shots [--out DIR]
    python utils/bio_weapons_art.py all
Default DIR: rewrite/mods/bio_weapons_pack/sprites/pickups/weapons (icons),
rewrite/mods/bio_weapons_pack/sprites/shots (shots).
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import mod_shot_draw as msd  # noqa: E402
from mod_art_clean import add_outline_and_shadow, palette  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
ORIG = ROOT / "data" / "reference" / "sprites" / "pickups" / "weapons"
PACK = ROOT / "rewrite" / "mods" / "bio_weapons_pack" / "sprites"
PAL = palette()
INV: dict[tuple, int] = {}
for _i, _c in enumerate(PAL):
    INV.setdefault(tuple(_c), _i)

# ------------------------------------------------------------- colour steps
# Green glow (ramp 12) from dark to white-hot, ending in the pale bone-green of ramp 2.
GLOW = [193, 195, 197, 199, 201, 203, 205, 206, 207, 44, 45, 46]
# Olive carapace (ramp 2) and bone (ramp 10), indexed by grey step 0..15.
OLIVE = [32 + k for k in range(16)]
BONE = [160 + k for k in range(16)]
SAC_RIM = 34  # dark olive rim of a sac set into the housing
VEIN = 40
# pale grub flesh (ramp 2, olive to bone-white)
GRUB = [38, 41, 43, 44, 44, 45, 45, 46, 46, 47]


def load_grid(name: str) -> list[list[int]]:
    im = Image.open(ORIG / f"{name}.png").convert("RGBA")
    return [
        [0 if im.getpixel((x, y))[3] == 0 else INV[im.getpixel((x, y))[:3]] for x in range(im.width)]
        for y in range(im.height)
    ]


def strip_rim(g):
    """Drop the shadow (64, 161) and the outer layer of the rim (16, 112 touching the outside)."""
    h, w = len(g), len(g[0])
    for y in range(h):
        for x in range(w):
            if g[y][x] in (64, 161):
                g[y][x] = 0
    snap = [row[:] for row in g]
    for y in range(h):
        for x in range(w):
            if snap[y][x] in (16, 112, 1):
                if any(
                    not (0 <= x + dx < w and 0 <= y + dy < h) or snap[y + dy][x + dx] == 0
                    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))
                ):
                    g[y][x] = 0
    return g


def to_image(g) -> Image.Image:
    h, w = len(g), len(g[0])
    im = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    for y in range(h):
        for x in range(w):
            if g[y][x]:
                im.putpixel((x, y), PAL[g[y][x]] + (255,))
    return im


def finish(g) -> Image.Image:
    return add_outline_and_shadow(to_image(g), PAL, True)


def grey_step(i: int) -> int | None:
    return i if 1 <= i <= 15 else None


def fire_step(i: int) -> int | None:
    return i - 112 if 112 <= i <= 127 else None


def recolor(g, box, fn):
    """Apply fn(index) -> index to the pixels of box (x0, y0, x1, y1 inclusive)."""
    x0, y0, x1, y1 = box
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            if 0 <= y < len(g) and 0 <= x < len(g[0]) and g[y][x]:
                g[y][x] = fn(g[y][x])


def grey_to(ramp, lift=0):
    def f(i):
        s = grey_step(i)
        return ramp[min(15, s + lift)] if s is not None else i

    return f


def fire_to_glow(i):
    s = fire_step(i)
    if s is None:
        return i
    return GLOW[min(len(GLOW) - 1, max(0, round(s * (len(GLOW) - 1) / 15)))]


def other_to_bone(i):
    """Tan/sand ramps (1, 10, 11) -> bone ramp 10 at the same step; the yellow of ramp 7 -> glow."""
    for base in (16, 160, 176):
        if base + 1 <= i <= base + 15 and i != 16:
            return BONE[i - base]
    return fire_to_glow(i)


def put(g, x, y, i):
    if 0 <= y < len(g) and 0 <= x < len(g[0]):
        g[y][x] = i


def sac(g, cx, cy, rx, ry, rim=SAC_RIM, steps=GLOW, light=(-0.45, -0.5), spec=True):
    """Glowing sac: dark rim, body shaded from the upper left, one white-hot highlight."""
    hx, hy = cx + light[0] * rx, cy + light[1] * ry
    for y in range(int(cy - ry - 1), int(cy + ry + 2)):
        for x in range(int(cx - rx - 1), int(cx + rx + 2)):
            d = math.hypot((x - cx) / (rx + 0.5), (y - cy) / (ry + 0.5))
            if d > 1:
                continue
            if d > 0.78 and rim is not None:
                put(g, x, y, rim)
                continue
            hd = math.hypot((x - hx) / (rx + 0.5), (y - hy) / (ry + 0.5))
            v = 0.9 - 0.6 * d - 0.38 * hd
            k = max(0, min(len(steps) - 1, int(v * len(steps))))
            put(g, x, y, steps[k])
    if spec:
        put(g, round(hx), round(hy), steps[-1])


def line(g, pts, i):
    for (x0, y0), (x1, y1) in zip(pts, pts[1:], strict=False):
        n = max(abs(x1 - x0), abs(y1 - y0), 1)
        for k in range(n + 1):
            put(g, round(x0 + (x1 - x0) * k / n), round(y0 + (y1 - y0) * k / n), i)


# ------------------------------------------------------------------ icons


def icon_spore_burst():
    """Front, spread: the three-barrel fan of the Starburst, each barrel ending in a spore pod."""
    g = strip_rim(load_grid("starburst"))
    # side arms and base plates become olive carapace, the centre stays metal
    recolor(g, (0, 9, 7, 16), grey_to(OLIVE, 3))
    recolor(g, (14, 9, 23, 16), grey_to(OLIVE, 3))
    recolor(g, (2, 17, 6, 23), grey_to(OLIVE, 3))
    recolor(g, (15, 17, 20, 23), grey_to(OLIVE, 3))
    # spore pods: a big one on the central barrel, two on the side barrels
    sac(g, 10.5, 5.0, 2.6, 3.0)
    sac(g, 3.0, 11.0, 2.0, 2.0)
    sac(g, 18.0, 11.0, 2.0, 2.0)
    # small spores budding around the pods
    for x, y in ((7, 3), (14, 3), (1, 8), (20, 8)):
        put(g, x, y, GLOW[7])
    for x, y in ((6, 2), (15, 2)):
        put(g, x, y, 0)
    # a green sac in the hub, veins running to the pods
    sac(g, 10.5, 18.5, 1.8, 1.8, spec=False)
    line(g, [(9, 16), (6, 13), (4, 12)], VEIN)
    line(g, [(12, 16), (15, 13), (17, 12)], VEIN)
    return finish(g)


def icon_acid_spitter():
    """Front, heavy: the Mega Cannon block, its furnace turned into a bubbling acid tank."""
    g = strip_rim(load_grid("mega_cannon"))
    recolor(g, (0, 0, 23, 27), fire_to_glow)
    # acid tank window: a few bubbles rising in the acid
    for x, y in ((6, 12), (9, 10), (13, 11), (11, 13)):
        put(g, x, y, GLOW[-2])
    put(g, 9, 11, GLOW[-1])
    # the lid swells into a living sac, acid dripping from its lip
    sac(g, 10.0, 4.0, 6.0, 1.6)
    for x, y in ((4, 6), (4, 7), (15, 6)):
        put(g, x, y, GLOW[6])
    # frame pillars: carapace ribs over the metal
    for y in (9, 11, 13):
        put(g, 2, y, OLIVE[9])
        put(g, 3, y, OLIVE[6])
        put(g, 16, y, OLIVE[9])
        put(g, 17, y, OLIVE[6])
    # lower body: carapace plates, acid dripping between the feet
    recolor(g, (1, 17, 5, 23), grey_to(OLIVE, 2))
    recolor(g, (14, 17, 19, 23), grey_to(OLIVE, 2))
    sac(g, 10.0, 19.0, 1.6, 1.6, spec=False)
    return finish(g)


def icon_thorn_needler():
    """Front, piercing: the Zica Laser barrel grown into a bone thorn, thorns on the fins."""
    g = strip_rim(load_grid("zica_laser"))
    recolor(g, (0, 0, 23, 27), fire_to_glow)
    # barrel -> bone, sharpened into a thorn tip
    recolor(g, (6, 0, 15, 7), grey_to(BONE, 3))
    for y in range(0, 5):
        for x in range(6, 16):
            if g[y][x] and abs(x - 10.5) > 0.6 + y * 0.9:
                g[y][x] = 0
    for y, x0, x1 in ((0, 10, 11), (1, 10, 11), (2, 9, 12), (3, 9, 12), (4, 8, 13)):
        for x in range(x0, x1 + 1):
            g[y][x] = BONE[15 if x == x0 else (12 if x < 10.5 else 8)]
    # fins: bone thorns curving up and out
    recolor(g, (2, 14, 6, 23), grey_to(BONE, 2))
    recolor(g, (15, 14, 19, 23), grey_to(BONE, 2))
    for x, y in ((3, 13), (2, 12), (1, 11), (18, 13), (19, 12), (20, 11), (4, 9), (3, 8), (17, 9), (18, 8)):
        put(g, x, y, BONE[13] if x < 12 else BONE[9])
    # green sap core: brighter and wider
    sac(g, 10.5, 12.0, 2.2, 4.2)
    # carapace on the lower body
    recolor(g, (6, 17, 15, 23), grey_to(OLIVE, 4))
    line(g, [(10, 18), (10, 21)], GLOW[6])
    line(g, [(11, 18), (11, 21)], GLOW[8])
    return finish(g)


def icon_larva_seekers():
    """Rear, homing: the twin pods of the Guided Micro Bombs hatched into two segmented larvae."""
    g = strip_rim(load_grid("guided_micro_bombs"))
    # clear the two red bulbs and the tops of the pods, then grow larvae in their place
    for y in range(0, 15):
        for x in range(0, 24):
            if g[y][x] and y < 12:
                g[y][x] = 0
    for cx in (5.0, 14.0):
        # body: three segments, head on top with two dark eyes and mandibles
        for k, (cy, r) in enumerate(((11.5, 3.2), (8.0, 3.0), (4.8, 2.6))):
            sac(g, cx, cy, r, 1.9, rim=SAC_RIM, steps=GRUB, spec=k == 2)
            put(g, int(cx + 1), int(cy), GLOW[8])  # glowing spot on each segment
        put(g, int(cx - 1), 4, 16)
        put(g, int(cx + 1), 4, 16)
        put(g, int(cx - 2), 2, BONE[13])
        put(g, int(cx + 2), 2, BONE[10])
        put(g, int(cx - 2), 1, BONE[15])
        put(g, int(cx + 2), 1, BONE[12])
    # pod sockets: carapace; lower launcher body stays metal with green vents
    recolor(g, (1, 12, 19, 16), grey_to(OLIVE, 4))
    recolor(g, (0, 0, 23, 27), fire_to_glow)
    for x in (5, 6, 14, 15):
        put(g, x, 13, GLOW[8])
    line(g, [(8, 18), (11, 18)], GLOW[5])
    return finish(g)


def icon_pollen_cloud():
    """Rear, area: the Sonic Wave dish opened into a pollen flower, a puff of pollen above it."""
    g = strip_rim(load_grid("sonic_wave"))
    pollen = [42, 43, 44, 45, 46, 47]
    for y in range(0, 28):
        for x in range(0, 24):
            i = g[y][x]
            s = fire_step(i)
            if s is not None or i == 31:
                s = 15 if i == 31 else s
                g[y][x] = GLOW[min(11, max(0, s * 11 // 15))] if y % 2 == 1 else pollen[min(5, s * 6 // 16)]
    # petal edges: carapace olive lobes at both sides of the dish
    for y, xs in ((5, (2, 18)), (7, (1, 19)), (9, (1, 19))):
        for x in xs:
            put(g, x, y, OLIVE[8])
    # pollen grains on top
    for x, y in ((6, 2), (9, 1), (12, 2), (15, 1), (8, 3), (11, 3), (14, 3), (4, 3), (17, 3)):
        put(g, x, y, 125 if (x + y) % 3 else 126)
    # stalk: green veins on the metal neck, carapace foot
    line(g, [(10, 11), (10, 18)], VEIN)
    recolor(g, (4, 15, 16, 22), grey_to(OLIVE, 4))
    sac(g, 10.0, 16.5, 1.8, 1.8, spec=False)
    return finish(g)


def icon_seed_mortar():
    """Rear, lobbed: the Vulcan Cannon drum as a ribbed seed husk, glowing seeds in its slots."""
    g = strip_rim(load_grid("vulcan_cannon"))
    recolor(g, (0, 0, 23, 27), other_to_bone)
    # slots of the drum (grey bars between the bands) -> green seeds
    for y0, y1 in ((6, 6), (9, 12), (17, 17)):
        for y in range(y0, y1 + 1):
            for x in range(6, 17):
                s = grey_step(g[y][x])
                if s is not None and s >= 2 and g[y][x] != 16:
                    g[y][x] = GLOW[min(11, 2 + s * 9 // 13)]
    # the muzzle: a big seed bulging out of the top
    for y in range(0, 3):
        for x in range(5, 18):
            g[y][x] = 0
    sac(g, 11.0, 2.4, 4.0, 2.4)
    # breech block: carapace plates
    recolor(g, (3, 18, 8, 24), grey_to(OLIVE, 4))
    recolor(g, (14, 18, 19, 24), grey_to(OLIVE, 4))
    return finish(g)


ICONS = {
    "spore_burst_icon": icon_spore_burst,
    "acid_spitter_icon": icon_acid_spitter,
    "thorn_needler_icon": icon_thorn_needler,
    "larva_seekers_icon": icon_larva_seekers,
    "pollen_cloud_icon": icon_pollen_cloud,
    "seed_mortar_icon": icon_seed_mortar,
}


# ------------------------------------------------------------------ shots
W, H = msd.W, msd.H


def _clamp01(v):
    return max(0.0, min(1.0, v))


def shape_spore(x, y, f, size):
    """Spore cluster: a bright central spore ringed by three smaller ones that turn around it."""
    cx, cy = (W - 1) / 2, (H - 1) / 2
    v = 0.0
    r0 = size / 2
    blobs = [(cx, cy, r0 * 0.5, 0.15)]
    for k in range(3):
        a = f * math.pi / 6 + k * 2 * math.pi / 3 - math.pi / 2
        blobs.append((cx + math.cos(a) * r0 * 0.8, cy + math.sin(a) * r0 * 0.88, r0 * 0.44, 0.0))
    for bx, by, r, bonus in blobs:
        d = math.hypot(x - bx, y - by) / r
        if d <= 1:
            hl = math.hypot(x - (bx - r * 0.35), y - (by - r * 0.35)) / r
            v = max(v, min(1.0, 0.25 + 0.45 * (1 - d) + 0.45 * max(0, 1 - hl * 1.5) + bonus))
    return v


def shape_acid(x, y, f, size):
    """Acid glob: a drop pointing up, wobbling, with a bubble inside and a drip below."""
    cx, cy = (W - 1) / 2, (H - 1) / 2 + 1
    r = size / 2 * (1 + 0.07 * math.sin(f * math.pi / 2))
    ry = r * (1.0 + 0.06 * math.cos(f * math.pi / 2))
    dy = y - cy
    # pointed top (teardrop), round bottom
    k = 1 + max(0.0, -dy / ry) * 0.9
    d = math.hypot((x - cx) * k, dy / 1.08) / r
    drip = abs(x - cx - (0.5 if f % 2 else -0.5)) < 0.9 and cy + ry * 0.9 < y < cy + ry + 1.8 + (f % 2)
    if d > 1 and not drip:
        return 0.0
    if drip:
        return 0.45
    hl = math.hypot(x - (cx - r * 0.3), y - (cy - r * 0.35)) / r
    bub = math.hypot(x - (cx + r * 0.3), y - (cy + r * 0.25 - f * 0.4)) / 1.1
    v = 0.3 + 0.45 * (1 - d) + 0.5 * max(0, 1 - hl * 1.6)
    if bub < 1:
        v = 0.95
    return min(1.0, v)


def shape_thorn(x, y, f, size):
    """Thorn: a long tapered needle, bright sharp tip at the top, a glint running down the shaft."""
    cx = (W - 1) / 2
    tip, base = 0.0, H - 0.5
    if y < tip or y > base:
        return 0.0
    t = (y - tip) / (base - tip)
    half = 0.45 + size / 2 * t**0.8
    dx = abs(x - cx + 0.25)
    if dx > half:
        return 0.0
    edge = dx / half
    glint = 0.25 if abs(y - (2 + 3 * f)) < 1.0 else 0.0
    return min(1.0, 0.3 + 0.45 * (1 - edge) + 0.45 * (1 - t) + glint)


def shape_larva(x, y, f, size):
    """Larva: a segmented grub wriggling (its body sways from frame to frame), head on top."""
    cx = (W - 1) / 2
    v = 0.0
    segs = ((2.6, size / 2 * 0.85), (5.6, size / 2), (8.8, size / 2 * 0.9), (11.4, size / 2 * 0.62))
    for k, (cy, r) in enumerate(segs):
        sway = math.sin(f * math.pi / 2 + k * 1.3) * 0.9
        d = math.hypot((x - cx - sway) / r, (y - cy) / 1.75)
        if d <= 1:
            hl = math.hypot(x - (cx + sway - r * 0.35), y - (cy - 0.7)) / r
            s = 0.25 + 0.55 * (1 - d) + 0.35 * max(0, 1 - hl * 1.6) + (0.15 if k == 0 else 0)
            s *= 1 - 0.45 * _clamp01(abs(y - cy) / 1.75 - 0.35)  # dark line between the segments
            v = max(v, min(1.0, s))
    return v


def shape_pollen(x, y, f, size):
    """Pollen puff: a soft ring of grains that swells and turns, bright grains on the edge."""
    cx, cy = (W - 1) / 2, (H - 1) / 2
    r = size / 2 * (0.85 + 0.1 * (f % 4) / 3)
    d = math.hypot(x - cx, y - cy)
    v = 0.0
    if d < r * 0.55:
        v = 0.3 + 0.25 * (1 - d / (r * 0.55))
    for k in range(6):
        a = k * math.pi / 3 + f * math.pi / 8
        gx, gy = cx + math.cos(a) * r * 0.75, cy + math.sin(a) * r * 0.75
        gd = math.hypot(x - gx, y - gy) / 1.45
        if gd <= 1:
            v = max(v, 0.5 + 0.5 * (1 - gd))
    return v


def shape_seed(x, y, f, size):
    """Seed: an oval husk tumbling (its long axis turns a quarter per frame), green germ glowing."""
    cx, cy = (W - 1) / 2, (H - 1) / 2
    a = f * math.pi / 4
    rx, ry = size / 2 * 0.62, size / 2
    u = (x - cx) * math.cos(a) + (y - cy) * math.sin(a)
    w = -(x - cx) * math.sin(a) + (y - cy) * math.cos(a)
    d = math.hypot(u / rx, w / ry)
    if d > 1:
        return 0.0
    ridge = abs(u) < 0.55
    germ = math.hypot(u, w + ry * 0.35) / (rx * 0.55)
    v = 0.25 + 0.5 * (1 - d)
    if germ < 1:
        v = max(v, 0.75 + 0.25 * (1 - germ))
    if ridge and germ >= 1:
        v = min(v, 0.3)
    return min(1.0, v)


msd.SHAPES.update(
    {
        "spore": shape_spore,
        "acid": shape_acid,
        "thorn": shape_thorn,
        "larva": shape_larva,
        "pollen": shape_pollen,
        "seed": shape_seed,
    }
)
# gradients, rim (darkest) to core (brightest)
msd.GRADIENTS.update(
    {
        "bio": [194, 196, 198, 200, 202, 204, 205, 206, 207, 44, 45, 46],
        "acid": [195, 197, 199, 201, 203, 205, 206, 207, 44, 45, 46, 47],
        "thorn": [36, 38, 40, 42, 43, 44, 202, 205, 207, 45, 46, 47],
        "pollen": [40, 42, 43, 44, 45, 46, 123, 124, 125, 126, 127, 15],
        "seed": [165, 168, 170, 172, 173, 174, 175, 204, 206, 207, 46, 47],
    }
)

# name, shape, gradient, size, frames
SHOTS = [
    ("spore_burst", "spore", "bio", 10.0, 4),
    ("spore_seed", "orb", "bio", 8.0, 4),
    ("acid_glob", "acid", "acid", 8.0, 4),
    ("acid_glob_large", "acid", "acid", 10.5, 4),
    ("thorn_needle", "thorn", "thorn", 6.0, 4),
    ("larva", "larva", "bio", 8.5, 4),
    ("pollen_puff", "pollen", "pollen", 10.0, 4),
    ("seed_pod", "seed", "seed", 10.5, 4),
    ("seed_pod_large", "seed", "seed", 12.5, 4),
]


def make_shots(out: Path):
    out.mkdir(parents=True, exist_ok=True)
    for name, shape, grad, size, frames in SHOTS:
        for f in range(frames):
            msd.draw(shape, grad, f, size).save(out / f"{name}_{f + 1}.png")
    print(f"{sum(s[4] for s in SHOTS)} shot frames -> {out}")


def make_icons(out: Path):
    out.mkdir(parents=True, exist_ok=True)
    for name, fn in ICONS.items():
        fn().save(out / f"{name}.png")
    print(f"{len(ICONS)} icons -> {out}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("what", choices=["icons", "shots", "all"])
    ap.add_argument("--out", type=Path, default=None)
    a = ap.parse_args()
    if a.what in ("icons", "all"):
        make_icons(a.out or PACK / "pickups" / "weapons")
    if a.what in ("shots", "all"):
        make_shots(a.out or PACK / "shots")
    return 0


if __name__ == "__main__":
    sys.exit(main())
