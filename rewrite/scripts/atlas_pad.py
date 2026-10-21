#!/usr/bin/env python3
"""Mirrored border around each frame of an atlas before upscaling, and
recomposition of the x4 atlas afterwards (called by hd_upscale.py sprites).

Upscaling an atlas in one block lets the network see the neighboring frames (unrelated
to each other): it "bleeds" at the edges and, since the scenery (24x28
tiles) and the big enemies (12x14 pieces) are made of frames laid edge
to edge, a grid shows up on screen. Here each frame is copied alone into
a cell surrounded by PAD pixels mirrored from its own edges, the atlas
of cells is upscaled, then the center of each cell is pasted back in place
of the x4 frame: the rectangles of the atlas JSON remain valid (x4).

  atlas_pad.py pad   <atlas.png> <atlas.json> <cells.png> <cells.json>
  atlas_pad.py unpad <cells_x4.png> <cells.json> <output.png>
"""

import json
import sys

from PIL import Image, ImageOps

PAD = 6
SCALE = 4
TILE_W, TILE_H = 24, 28
MAX_ROW_W = 2048


def frames_of(meta: dict, size: tuple[int, int]) -> list[tuple[int, int, int, int]]:
    if "frames" in meta:
        return [(f["x"], f["y"], f["w"], f["h"]) for f in meta["frames"] if f["w"] > 0 and f["h"] > 0]
    # Tile atlas (tiles/*.json): 24x28 grid over `columns` columns.
    cols = int(meta["columns"])
    rows = -(-size[1] // TILE_H)
    return [(c * TILE_W, r * TILE_H, TILE_W, TILE_H) for r in range(rows) for c in range(cols)]


def mirror_pad(img: Image.Image) -> Image.Image:
    w, h = img.size
    m = Image.new("RGBA", (w * 3, h * 3))
    for dy, fy in enumerate((True, False, True)):
        for dx, fx in enumerate((True, False, True)):
            t = ImageOps.mirror(img) if fx else img
            m.paste(ImageOps.flip(t) if fy else t, (dx * w, dy * h))
    # PAD may exceed the size of a tiny frame: clamp to the frame.
    px, py = min(PAD, w), min(PAD, h)
    out = Image.new("RGBA", (w + 2 * PAD, h + 2 * PAD))
    out.paste(m.crop((w - px, h - py, 2 * w + px, 2 * h + py)), (PAD - px, PAD - py))
    return out


def pad(atlas_png: str, atlas_json: str, out_png: str, out_json: str) -> None:
    atlas = Image.open(atlas_png).convert("RGBA")
    meta = json.load(open(atlas_json, encoding="utf-8"))
    frames = frames_of(meta, atlas.size)
    # Shelf packing: cells of (w + 2 PAD) x (h + 2 PAD).
    places, x, y, row_h, width = [], 0, 0, 0, 0
    for fx, fy, fw, fh in frames:
        cw, ch = fw + 2 * PAD, fh + 2 * PAD
        if x > 0 and x + cw > MAX_ROW_W:
            x, y, row_h = 0, y + row_h, 0
        places.append([fx, fy, fw, fh, x, y])
        x += cw
        row_h = max(row_h, ch)
        width = max(width, x)
    cases = Image.new("RGBA", (max(width, 1), max(y + row_h, 1)))
    for fx, fy, fw, fh, px, py in places:
        cases.paste(mirror_pad(atlas.crop((fx, fy, fx + fw, fy + fh))), (px, py))
    cases.save(out_png)
    json.dump({"size": list(atlas.size), "places": places}, open(out_json, "w"))


def unpad(cases_png: str, cases_json: str, out_png: str) -> None:
    big = Image.open(cases_png).convert("RGBA")
    layout = json.load(open(cases_json))
    w, h = layout["size"]
    k = SCALE
    res = Image.new("RGBA", (w * k, h * k))
    for fx, fy, fw, fh, px, py in layout["places"]:
        sx, sy = (px + PAD) * k, (py + PAD) * k
        res.paste(big.crop((sx, sy, sx + fw * k, sy + fh * k)), (fx * k, fy * k))
    res.save(out_png)


if __name__ == "__main__":
    if sys.argv[1] == "pad":
        pad(*sys.argv[2:6])
    elif sys.argv[1] == "unpad":
        unpad(*sys.argv[2:5])
    else:
        sys.exit(__doc__)
