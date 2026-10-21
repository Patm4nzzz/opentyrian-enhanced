#!/usr/bin/env python3
"""mod_preview.py — preview.png and preview_3x.png of a mod folder.

Same layout as the shipped packs (rewrite/mods/alien_ship_pack/preview.png):
a title, then one row per sprite category, on the dark background of the
in-game menus. preview.png is 1x (what the player sees), preview_3x.png the
same image enlarged x3 nearest-neighbour (for the README and reviews).

Rows:
    Ships        sprites/ships/*.png without the banking poses (_left, _right)
    Shop icons   sprites/pickups/weapons/*.png
    Shots        sprites/shots/*.png; animated shots (<name>_1.png .. <name>_N.png)
                 show their first frame, then a second row "Shot frames" shows
                 every frame side by side.

Usage:
    python utils/mod_preview.py rewrite/mods/alien_weapon_pack "Alien Weapon Pack"
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:  # pragma: no cover
    print("ERROR: Pillow required (pip install -r utils/requirements.txt)", file=sys.stderr)
    sys.exit(2)

BG = (32, 32, 40, 255)
FRAME = re.compile(r"^(.*)_(\d+)$")


def load(d: Path, keep=lambda n: True) -> list[tuple[str, Image.Image]]:
    if not d.is_dir():
        return []
    return [(p.stem, Image.open(p).convert("RGBA")) for p in sorted(d.glob("*.png")) if keep(p.stem)]


def rows_of(mod: Path) -> list[tuple[str, list[Image.Image], int]]:
    rows = []
    ships = load(mod / "sprites" / "ships", lambda n: "_left" not in n and "_right" not in n)
    if ships:
        rows.append((f"Ships ({len(ships)})", [im for _, im in ships], 46))
    icons = load(mod / "sprites" / "pickups" / "weapons")
    if icons:
        rows.append((f"Shop icons ({len(icons)})", [im for _, im in icons], 46))
    shots = load(mod / "sprites" / "shots")
    if shots:
        groups: dict[str, list[Image.Image]] = {}
        for name, im in shots:
            m = FRAME.match(name)
            groups.setdefault(m.group(1) if m else name, []).append(im)
        animated = any(len(v) > 1 for v in groups.values())
        label = f"Shots ({len(groups)}" + (", first frame)" if animated else ")")
        rows.append((label, [v[0] for v in groups.values()], 46))
        if animated:
            frames = []
            for v in groups.values():
                frames += v + [None]  # gap between two shots
            rows.append(("Shot frames (one per game tick)", frames[:-1], 14))
    return rows


def build(mod: Path, title: str) -> Image.Image:
    rows = rows_of(mod)
    cell_h = 30
    W = max(sum(cw for _ in items) for _, items, cw in rows) + 8
    H = 16 + sum(12 + cell_h + 10 for _ in rows)
    im = Image.new("RGBA", (W, H), BG)
    d = ImageDraw.Draw(im)
    d.text((4, 2), f"Mod preview: {title}", fill=(170, 230, 170, 255))
    y = 16
    for label, items, cw in rows:
        d.text((4, y), label, fill=(200, 200, 200, 255))
        y += 12
        for k, sp in enumerate(items):
            if sp is None:
                continue
            x = 4 + k * cw + (cw - sp.width) // 2
            im.alpha_composite(sp, (x, y + (cell_h - sp.height) // 2))
        y += cell_h + 10
    return im


def main() -> int:
    if len(sys.argv) != 3 or sys.argv[1] in ("-h", "--help"):
        print(__doc__)
        return 0 if len(sys.argv) == 2 and sys.argv[1] in ("-h", "--help") else 2
    mod, title = Path(sys.argv[1]), sys.argv[2]
    im = build(mod, title)
    for name, img in (("preview.png", im), ("preview_3x.png", im.resize((im.width * 3, im.height * 3), Image.NEAREST))):
        tmp = mod / (name + ".tmp")
        img.save(tmp, format="PNG")
        os.replace(tmp, mod / name)
    print(f"{mod / 'preview.png'}: {im.width}x{im.height}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
