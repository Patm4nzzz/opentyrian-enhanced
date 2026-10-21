#!/usr/bin/env python3
"""ref_ship_grid.py — reference grid PNG of the stock ships in
data/reference/sprites/ships/, as a style reference for pixel-art tools
such as pixellab.ai (asset_extract_ship_grid.py is the inverse: it cuts a
generated grid back into 24x28 ship PNGs).

Output: 5x4 grid of 24x28 cells with 1px borders, upscaled 4x to 484x468.

Usage:
    python3 utils/ref_ship_grid.py [--ships-dir DIR] [--out FILE]

Default --out: data/reference/ships_reference.png (untracked reference artifact)
"""

import argparse
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("error: pip3 install Pillow", file=sys.stderr)
    sys.exit(2)

ROOT = Path(__file__).resolve().parent.parent
SHIPS_DIR = ROOT / "data" / "reference" / "sprites" / "ships"
DEFAULT_OUT = ROOT / "data" / "reference" / "ships_reference.png"
CELL_W, CELL_H = 24, 28
BORDER = 1
COLS, ROWS = 5, 4
SCALE = 4


def main():
    parser = argparse.ArgumentParser(description="Build a 5x4 reference grid PNG of the stock ship sprites.")
    parser.add_argument(
        "--ships-dir",
        default=str(SHIPS_DIR),
        help="folder of 24x28 ship PNGs (default: data/reference/sprites/ships)",
    )
    parser.add_argument(
        "--out",
        default=str(DEFAULT_OUT),
        help="output PNG (default: data/reference/ships_reference.png)",
    )
    args = parser.parse_args()

    ships_dir = Path(args.ships_dir)
    out_path = Path(args.out)
    pngs = sorted(ships_dir.glob("*.png"))[: COLS * ROWS]
    if not pngs:
        print(f"error: no PNGs in {ships_dir}", file=sys.stderr)
        sys.exit(1)

    w = COLS * CELL_W + (COLS + 1) * BORDER
    h = ROWS * CELL_H + (ROWS + 1) * BORDER
    grid = Image.new("RGBA", (w, h), (0, 0, 0, 255))

    for i, p in enumerate(pngs):
        col, row = i % COLS, i // COLS
        x = BORDER + col * (CELL_W + BORDER)
        y = BORDER + row * (CELL_H + BORDER)
        ship = Image.open(p).convert("RGBA")
        if ship.size != (CELL_W, CELL_H):
            ship = ship.resize((CELL_W, CELL_H), Image.NEAREST)
        grid.paste(ship, (x, y), ship)

    if SCALE > 1:
        grid = grid.resize((w * SCALE, h * SCALE), Image.NEAREST)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    grid.save(out_path)
    print(f"wrote {out_path} ({grid.size[0]}x{grid.size[1]}, {len(pngs)} ships)")


if __name__ == "__main__":
    main()
