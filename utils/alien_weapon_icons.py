#!/usr/bin/env python3
"""alien_weapon_icons.py — shop icons of the Alien Weapon Pack, built from parts of the original icons.

Each icon is a faction variant of the original gun housings: parts (barrels,
housings, cores) are cut from the original 24x28 shop icons, pasted on a 24x28
canvas, recoloured with the game palette (palette 5) into the faction look,
then the near-black rim (index 16) is closed and the two-step drop shadow
(64 then 161, down-right) is redrawn, as on the originals.

    Zica    : hot metal (dark greys pushed to the dark red ramp 4), orange/red
              vents and white-hot fire cores (ramp 7).
    Hazudra : bone / olive carapace plating (ramps 2 and 10) over the grey
              housing, neon green bio-cores (ramp 12).

Source icons: the wiki extracts of the original shop icons
(weapon-NN-<name>-<hash>.png, the 24x28 icon cropped to its pixels and
enlarged 2x); they are reduced back to 1x. Coordinates below are in that
cropped 1x space.

Usage:
    python utils/alien_weapon_icons.py --orig <wiki>/site/wiki/assets/sprites/items \
        --out rewrite/mods/alien_weapon_pack/sprites/pickups/weapons
"""

from __future__ import annotations

import argparse
import glob
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mod_art_clean import OUTLINE, SHADOW, palette  # noqa: E402

W, H = 24, 28
PAL = palette()
INV = {}
for _i, _c in enumerate(PAL):
    INV.setdefault(_c, _i)


# ---------------------------------------------------------------- sources


def load_orig(folder: Path, num: str) -> list[list[int]]:
    """Original icon number `num` as rows of palette indices (0 = empty), drop shadow removed."""
    files = glob.glob(str(folder / f"weapon-{num}-*.png"))
    if not files:
        raise SystemExit(f"original icon weapon-{num}-*.png not found in {folder}")
    im = Image.open(files[0]).convert("RGBA")
    im = im.resize((im.width // 2, im.height // 2), Image.NEAREST)
    rows = []
    for y in range(im.height):
        row = []
        for x in range(im.width):
            p = im.getpixel((x, y))
            i = 0 if p[3] == 0 else INV[p[:3]]
            row.append(0 if i in SHADOW else i)
        rows.append(row)
    return rows


# ---------------------------------------------------------------- recolouring


def ramp(i: int) -> int:
    return i // 16


def step(i: int) -> int:
    return i % 16


def hot_metal(i: int) -> int:
    """Zica housing: the dark greys become dark red-brown metal, the highlights stay grey."""
    if ramp(i) == 0 and 1 <= i <= 3:
        return 65 + i
    if ramp(i) in (10, 11, 1, 14):  # tan parts of the originals -> heated dark red
        return 64 + max(2, min(11, step(i) - 2))
    if ramp(i) in (3, 9, 13, 8):  # blue glass -> fire
        return 112 + min(15, step(i) + 3)
    return i


def white_hot(i: int) -> int:
    """Light greys of a barrel become the white-hot core."""
    if ramp(i) == 0 and i >= 11:
        return {11: 122, 12: 124, 13: 125, 14: 126, 15: 127}[i]
    if ramp(i) == 0 and i >= 8:
        return 120
    return hot_metal(i)


def fire_up(i: int, k: int = 2) -> int:
    if ramp(i) == 7:
        return min(127, i + k)
    return i


def carapace(i: int) -> int:
    """Hazudra plating: grey -> olive / pale bone ramp 2."""
    if ramp(i) == 0 and i >= 1:
        return 32 + min(15, i + 1)
    if ramp(i) in (10, 11, 1, 14):
        return 32 + min(15, step(i) + 1)
    return i


def bone(i: int) -> int:
    """Hazudra bone: grey -> sand / bone ramp 10."""
    if ramp(i) == 0 and i >= 1:
        return 160 + min(15, i + 1)
    if ramp(i) in (1, 11, 14):
        return 160 + step(i)
    return i


def bio(i: int) -> int:
    """Hazudra core: fire, blue or purple -> neon green ramp 12."""
    if ramp(i) in (7, 3, 9, 13, 8, 4):
        return 192 + min(15, step(i) + 2)
    return i


def chain(*fns):
    def f(i):
        for fn in fns:
            i = fn(i)
        return i

    return f


# ---------------------------------------------------------------- canvas


class Canvas:
    def __init__(self):
        self.px = [[0] * W for _ in range(H)]

    def paste(self, src, box, at, fn=lambda i: i, mirror=False, keep_rim=True):
        """Copy src[box] (x0, y0, x1, y1 inclusive) to `at`, recoloured by fn."""
        x0, y0, x1, y1 = box
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                sx = (x1 - (x - x0)) if mirror else x
                if y >= len(src) or sx >= len(src[0]):
                    continue
                i = src[y][sx]
                if i == 0:
                    continue
                dx, dy = at[0] + x - x0, at[1] + y - y0
                if 0 <= dx < W and 0 <= dy < H:
                    self.px[dy][dx] = i if (i == OUTLINE and keep_rim) else fn(i)

    def put(self, x, y, i):
        if 0 <= x < W and 0 <= y < H:
            self.px[y][x] = i

    def rows(self, x, y, lines: list[str], legend: dict[str, int]):
        """Draw text rows; '.' leaves the pixel untouched."""
        for dy, line in enumerate(lines):
            for dx, ch in enumerate(line):
                if ch != ".":
                    self.put(x + dx, y + dy, legend[ch])

    def disc(self, cx, cy, r, ramp_base, lo=2, hi=15, ring=0.0, rim=True):
        """Shaded ball (or ring when ring > 0: inner radius fraction left dark) lit from the upper left."""
        for y in range(H):
            for x in range(W):
                d = ((x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2) ** 0.5
                if d > r:
                    continue
                if rim and d > r - 1:
                    self.put(x, y, OUTLINE)
                    continue
                if ring and d < r * ring:
                    self.put(x, y, OUTLINE)  # dark bore of the ring
                    continue
                lx = ((x + 0.5 - cx) + (y + 0.5 - cy)) / (2 * r)  # -0.7 (upper left) .. 0.7
                t = 1 - (lx + 0.7) / 1.4
                self.put(x, y, ramp_base + round(lo + (hi - lo) * max(0, min(1, t))))

    def mirror_half(self, axis: int):
        """Copy the left half (x < axis) onto the right half (symmetrical housing)."""
        for y in range(H):
            for x in range(axis):
                m = 2 * axis - x
                if 0 <= m < W:
                    self.px[y][m] = self.px[y][x]

    def finish(self) -> Image.Image:
        px = [r[:] for r in self.px]
        body = {(x, y) for y in range(H) for x in range(W) if px[y][x]}
        # close the rim: every empty pixel touching a coloured (non-rim) pixel becomes rim
        for x, y in sorted(body):
            if px[y][x] == OUTLINE:
                continue
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                qx, qy = x + dx, y + dy
                if 0 <= qx < W and 0 <= qy < H and px[qy][qx] == 0:
                    px[qy][qx] = OUTLINE
        body = {(x, y) for y in range(H) for x in range(W) if px[y][x]}
        for k, idx in ((1, SHADOW[0]), (2, SHADOW[1])):
            for x, y in sorted(body):
                qx, qy = x + k, y + k
                if 0 <= qx < W and 0 <= qy < H and px[qy][qx] == 0:
                    px[qy][qx] = idx
        im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
        for y in range(H):
            for x in range(W):
                if px[y][x]:
                    im.putpixel((x, y), PAL[px[y][x]] + (255,))
        return im


# ---------------------------------------------------------------- the six icons


def zica_thermal_lance(o) -> Canvas:
    """Lance barrel of the missile launcher, white-hot, on the boxy rear-vulcan housing."""
    c = Canvas()
    c.paste(o["15"], (0, 9, 17, 22), (3, 11), chain(hot_metal, fire_up))
    c.paste(o["17"], (1, 0, 7, 13), (8, 1), white_hot)
    return c


def zica_plasma_halo(o) -> Canvas:
    """Curved emitter horns of the fireball gun around a white-hot plasma ring, over the laser's bar housing."""
    c = Canvas()
    c.paste(o["04"], (0, 10, 20, 20), (1, 13), hot_metal)
    c.paste(o["18"], (0, 0, 20, 12), (1, 1), chain(hot_metal, fire_up))
    c.disc(11.5, 9.5, 4.6, 112, lo=6, hi=15, ring=0.5)
    return c


def zica_ember_mortar(o) -> Canvas:
    """Scatter-wave twin housing with fire vents, a fat furnace mouth glowing on top."""
    c = Canvas()
    c.paste(o["30"], (0, 3, 21, 23), (1, 4), chain(hot_metal, fire_up))
    c.disc(12, 6.5, 5.5, 0, lo=3, hi=13)
    c.disc(12, 6.5, 3.6, 65, lo=1, hi=4, rim=True)
    c.disc(12.3, 6.8, 2.2, 112, lo=6, hi=15, rim=False)
    return c


def hazudra_crescent_fan(o) -> Canvas:
    """Starburst fan of arms in bone over the pulse-cannon base, green bio-core at the root."""
    c = Canvas()
    c.paste(o["01"], (0, 11, 20, 23), (1, 13), carapace)
    c.paste(o["09"], (0, 0, 20, 14), (1, 1), bone)
    c.disc(11.5, 12.5, 3.5, 192, lo=4, hi=15)
    return c


def hazudra_spore_cannon(o) -> Canvas:
    """Green spore pod between the twin bone barrels of the pulse cannon, on the grey mega-cannon legs."""
    c = Canvas()
    c.paste(o["03"], (0, 13, 20, 23), (1, 15))
    c.paste(o["01"], (0, 0, 20, 12), (1, 3), bone)
    c.disc(11.5, 9, 5.1, 192, lo=3, hi=15)
    for x, y in ((9, 11), (10, 12), (13, 7), (14, 8)):  # dark veins on the pod
        c.put(x, y, 195)
    return c


def hazudra_seeker_darts(o) -> Canvas:
    """Three missile-launcher darts with green tips on the laser's bar housing in carapace."""
    c = Canvas()
    c.paste(o["04"], (0, 10, 20, 20), (1, 14), carapace)
    for x, y in ((2, 4), (14, 4), (8, 1)):
        c.paste(o["17"], (1, 0, 7, 13), (x, y), chain(bone, bio))
        for dy in range(3):  # green warhead
            for dx in range(7):
                v = c.px[y + dy][x + dx]
                if v and v != OUTLINE:
                    c.px[y + dy][x + dx] = 192 + min(15, (v % 16) + 2)
    return c


ICONS = {
    "zica_thermal_lance_icon": zica_thermal_lance,
    "hazudra_crescent_fan_icon": hazudra_crescent_fan,
    "hazudra_spore_cannon_icon": hazudra_spore_cannon,
    "hazudra_seeker_darts_icon": hazudra_seeker_darts,
    "zica_plasma_halo_icon": zica_plasma_halo,
    "zica_ember_mortar_icon": zica_ember_mortar,
}
SOURCES = ["01", "03", "04", "07", "09", "15", "17", "18", "29", "30", "33"]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--orig", type=Path, required=True, help="folder of the wiki weapon-NN-*.png extracts")
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--only", default="", help="comma-separated icon names")
    a = ap.parse_args()
    o = {n: load_orig(a.orig, n) for n in SOURCES}
    a.out.mkdir(parents=True, exist_ok=True)
    only = set(filter(None, a.only.split(",")))
    for name, fn in ICONS.items():
        if only and name not in only:
            continue
        fn(o).finish().save(a.out / f"{name}.png")
        print(a.out / f"{name}.png")
    return 0


if __name__ == "__main__":
    sys.exit(main())
