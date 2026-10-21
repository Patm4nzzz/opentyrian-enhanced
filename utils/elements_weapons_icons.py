#!/usr/bin/env python3
"""elements_weapons_icons.py — shop icons of the Elements Weapon Pack.

Each 24x28 icon is built from the housing of one original shop icon
(data/reference/sprites/pickups/weapons/), read as palette indices of the
gameplay palette (palette 5), then re-shaped and repainted part by part:

    Flame Lance     guided_bombs housing, flared nozzle with a white-hot mouth, glowing vents
    Frost Shard     protron hood, ice-crystal barrel instead of the red core
    Arc Caster      pulse_cannon twin barrels turned into copper coils, yellow-white arc between them
    Boulder Mortar  mega_cannon box, stone plates and a mortar bore holding a boulder
    Acid Rain       poison_bomb spire, two green-yellow acid tanks instead of the side rails
    Gale Blades     multi_cannon arms, teal turbine rotor in the middle

The original rim (index 16) is redrawn around every new part, then the
two-step drop shadow of the originals (index 64 one pixel down-right, 161 two
pixels down-right). Every pixel is a palette colour.

Usage:
    python utils/elements_weapons_icons.py OUT_DIR [--sheet SHEET.png]
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _shared import load_vga_palette  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
REF = ROOT / "data" / "reference" / "sprites" / "pickups" / "weapons"
W, H = 24, 28
RIM, SH1, SH2 = 16, 64, 161
PAL = [tuple(c) for c in load_vga_palette(str(ROOT / "data"), 5)]
INV: dict[tuple, int] = {}
for _i, _c in enumerate(PAL):
    INV.setdefault(_c, _i)

# Colour ramps, darkest to lightest (palette indices), 16 steps each.
FIRE = list(range(112, 128))
ICE = [144, 146, 148, 150, 152, 154, 155, 156, 157, 158, 159, 139, 140, 141, 142, 143]
COPPER = [17, 18, 19, 244, 245, 246, 247, 247, 248, 248, 249, 249, 250, 251, 252, 253]
STONE = list(range(16, 32))  # ramp 1: earthy brown
ACID = [192, 193, 194, 195, 196, 197, 198, 200, 202, 204, 206, 207, 251, 252, 253, 15]
TEAL = list(range(208, 224))


class Icon:
    def __init__(self, base: str):
        im = Image.open(REF / f"{base}.png").convert("RGBA")
        self.g = [[0] * W for _ in range(H)]
        for y in range(H):
            for x in range(W):
                r, g, b, a = im.getpixel((x, y))
                if a:
                    self.g[y][x] = INV[(r, g, b)]
        self._strip_shadow()

    def _strip_shadow(self):
        """Shadow = the 64/161 pixels reachable from outside through transparent or shadow pixels."""
        seen = set()
        stack = [(x, y) for x in range(W) for y in (0, H - 1)] + [(x, y) for y in range(H) for x in (0, W - 1)]
        while stack:
            x, y = stack.pop()
            if (x, y) in seen or not (0 <= x < W and 0 <= y < H):
                continue
            if self.g[y][x] not in (0, SH1, SH2):
                continue
            seen.add((x, y))
            self.g[y][x] = 0
            stack += [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]

    # ---- editing helpers
    def put(self, x, y, i):
        if 0 <= x < W and 0 <= y < H:
            self.g[y][x] = i

    def art(self, x0, y0, rows: list[str], legend: dict[str, int]):
        """Overlay: '.' keeps the pixel, '_' clears it, other chars from the legend (# = rim)."""
        lg = {"#": RIM, "_": 0, **legend}
        for dy, row in enumerate(rows):
            for dx, ch in enumerate(row):
                if ch != ".":
                    self.put(x0 + dx, y0 + dy, lg[ch])

    def remap(self, box, src_range, ramp, shift=0):
        """Repaint the pixels of box (x0, y0, x1, y1 inclusive) whose index is in src_range:
        step k of the source 16-colour ramp becomes step k+shift of `ramp`."""
        x0, y0, x1, y1 = box
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                i = self.g[y][x]
                if i in src_range:
                    k = (i - src_range.start) + shift
                    self.g[y][x] = ramp[max(0, min(15, k))]

    def finish(self) -> Image.Image:
        g = self.g
        body = {(x, y) for y in range(H) for x in range(W) if g[y][x]}
        # rim around any body pixel touching the outside
        for x, y in list(body):
            if g[y][x] == RIM:
                continue
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                nx, ny = x + dx, y + dy
                if 0 <= nx < W and 0 <= ny < H and not g[ny][nx]:
                    g[ny][nx] = RIM
        body = {(x, y) for y in range(H) for x in range(W) if g[y][x]}
        for (dx, dy), idx in (((1, 1), SH1), ((2, 2), SH2)):
            for x, y in sorted(body):
                nx, ny = x + dx, y + dy
                if 0 <= nx < W and 0 <= ny < H and not g[ny][nx]:
                    g[ny][nx] = idx
        im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
        for y in range(H):
            for x in range(W):
                if g[y][x]:
                    im.putpixel((x, y), PAL[g[y][x]] + (255,))
        return im


def flame_lance() -> Image.Image:
    ic = Icon("guided_bombs")
    # flared flamethrower nozzle instead of the central spike: white-hot mouth
    ic.art(
        6,
        0,
        [
            "_#########_",
            "#rOYWWWYOr#",
            "#rOYWWWYOr#",
            "_##rOYOr##_",
            "__#ahbhc#__",
            "__#ahbhc#__",
            "__#ahbhc#__",
            "_##ahbhc##_",
        ],
        {"r": 117, "O": 121, "Y": 124, "W": 127, "a": 4, "h": 13, "b": 9, "c": 3},
    )
    # vents: brighter and one more row on each side
    for y, (left, right) in {
        10: ((125, 122, 122, 119), (124, 121, 121, 118)),
        12: ((126, 123, 123, 120), (125, 122, 122, 119)),
        14: ((125, 122, 122, 119), (124, 121, 121, 118)),
    }.items():
        for k in range(4):
            ic.put(4 + k, y, left[k])
            ic.put(14 + k, y, right[k])
    ic.remap((4, 15, 17, 15), range(112, 128), FIRE, 1)
    # hot core line down the middle of the body
    for y, i in ((8, 123), (9, 121), (10, 119), (11, 117)):
        ic.put(11, y, i)
    return ic.finish()


def frost_shard() -> Image.Image:
    ic = Icon("protron")
    # red core -> ice crystal barrel
    ic.remap((5, 4, 15, 16), range(112, 128), ICE, 2)
    # tall crystal spike rising out of the split hood tip
    ic.art(
        7,
        0,
        [
            "__####__",
            "_#wwcm#_",
            "#wwcmmD#",
            "#wcmmDD#",
            "#wcmmDD#",
            "_#cmmD#_",
            "_#cmmD#_",
            "__#mD#__",
        ],
        {"w": 15, "c": 142, "m": 157, "D": 152},
    )
    # frost on the shoulder pods
    for x, y, i in ((3, 14, 142), (3, 15, 158), (18, 14, 157), (18, 15, 154)):
        ic.put(x, y, i)
    return ic.finish()


def arc_caster() -> Image.Image:
    ic = Icon("pulse_cannon")
    # outer barrels -> copper coils: bright turns and dark windings
    coil = ["#####", "#WwC#", "#dDd#", "#WwC#", "#dDd#", "#WwC#", "#dDd#", "#WwC#", "#####"]
    lg = {"W": 252, "w": 250, "C": 248, "d": 246, "D": 18}
    ic.art(2, 2, coil, lg)
    ic.art(14, 2, coil, lg)
    # tan base -> grey metal so the copper reads as the element
    ic.remap((2, 14, 18, 23), range(160, 176), list(range(0, 16)), 0)
    # yellow-white arc jumping between the coil tips
    ic.art(
        4,
        0,
        [
            "_____YW______",
            "___YW__WY_YW_",
            "_YWY_____WY_W",
        ],
        {"Y": 125, "W": 127},
    )
    # glowing capacitor in the middle
    ic.art(8, 10, ["#YWY#"], {"Y": 124, "W": 126})
    return ic.finish()


def boulder_mortar() -> Image.Image:
    ic = Icon("mega_cannon")
    # red block -> rough stone blocks split by dark cracks
    ic.art(
        0,
        2,
        [
            "____#############___",
            "___#edc#edcc#edcb#__",
            "__#edcb#edcb#dcbba#_",
            "_#dcbaa#dcba#cbaa#a#",
            "_#cba#ccbba#dcbaa#9#",
        ],
        {"e": 29, "d": 27, "c": 24, "b": 22, "a": 20, "9": 9},
    )
    # red grille -> stone, then a mortar bore with a boulder
    ic.remap((2, 7, 18, 15), range(112, 128), STONE, 2)
    ic.art(
        4,
        8,
        [
            "#1#######1#",
            "#1#2aaa2#1#",
            "#2#abcdd#2#",
            "#2adceedb2#",
            "#2acddcba2#",
            "#2#abcba#2#",
            "#1#2aaa2#1#",
        ],
        {"1": 3, "2": 2, "a": 164, "b": 167, "c": 170, "d": 172, "e": 175},
    )
    # heavier stone side plates
    ic.remap((1, 7, 3, 13), range(1, 16), STONE, 1)
    ic.remap((16, 7, 18, 13), range(1, 16), STONE, 1)
    return ic.finish()


def acid_rain() -> Image.Image:
    ic = Icon("poison_bomb")
    tank = [
        "_###_",
        "#9a5#",
        "#HgD#",
        "#HoD#",
        "#HgD#",
        "#ggD#",
        "#HgD#",
        "#oGD#",
        "#HgD#",
        "#HGD#",
        "#ggD#",
        "_###_",
    ]
    lg = {"9": 9, "a": 12, "5": 5, "H": 252, "g": 205, "G": 202, "D": 198, "o": 46}
    ic.art(2, 10, tank, lg)
    ic.art(15, 10, tank, lg)
    # orange core -> acid
    ic.remap((7, 12, 13, 17), range(112, 128), ACID, 1)
    # tan base -> grey metal
    ic.remap((2, 16, 18, 23), range(160, 176), list(range(0, 16)), 1)
    # spire tip -> shower head dripping acid
    ic.art(
        7,
        0,
        [
            "_#####_",
            "#9cHc5#",
            "#gGgGg#",
            "_#o#o#_",
        ],
        {"9": 11, "c": 13, "5": 6, "H": 253, "g": 205, "G": 200, "o": 251},
    )
    return ic.finish()


def gale_blades() -> Image.Image:
    ic = Icon("multi_cannon")
    # turbine rotor in the middle: curved teal blades, white hub
    cx, cy, R = 10.5, 14.5, 5.4
    for y in range(8, 21):
        for x in range(4, 18):
            r = math.hypot(x - cx, y - cy)
            if r > R + 0.5:
                continue
            if r > R - 0.4:
                ic.put(x, y, RIM)
                continue
            if r < 1.2:
                ic.put(x, y, 15)
                continue
            a = math.atan2(y - cy, x - cx)
            if r < 2.0:
                ic.put(x, y, RIM)
                continue
            v = math.sin(4 * a + r * 0.95)
            light = 0.5 + 0.5 * math.cos(a + 2.4)  # light from the upper left
            if v > 0.1:
                ic.put(x, y, 15 if light > 0.75 else TEAL[13 + int(2.9 * light)])
            elif v > -0.45:
                ic.put(x, y, TEAL[8 + int(3 * light)])
            else:
                ic.put(x, y, TEAL[4])
    # teal sheen on the arm tips
    for x, y, i in ((4, 11, 221), (4, 12, 223), (17, 11, 219), (18, 12, 221), (2, 13, 218), (19, 13, 216)):
        ic.put(x, y, i)
    return ic.finish()


ICONS = {
    "flame_lance_icon": flame_lance,
    "frost_shard_icon": frost_shard,
    "arc_caster_icon": arc_caster,
    "boulder_mortar_icon": boulder_mortar,
    "acid_rain_icon": acid_rain,
    "gale_blades_icon": gale_blades,
}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("out", type=Path)
    ap.add_argument("--sheet", type=Path, help="also write an 8x sheet of the icons")
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    ims = []
    for name, fn in ICONS.items():
        im = fn()
        im.save(a.out / f"{name}.png")
        ims.append(im)
        print(a.out / f"{name}.png")
    if a.sheet:
        k = 8
        sheet = Image.new("RGBA", (len(ims) * (W + 4) * k, (H + 4) * k), (32, 32, 40, 255))
        for n, im in enumerate(ims):
            sheet.alpha_composite(im.resize((W * k, H * k), Image.NEAREST), (n * (W + 4) * k + 2 * k, 2 * k))
        sheet.save(a.sheet)
    return 0


if __name__ == "__main__":
    sys.exit(main())
