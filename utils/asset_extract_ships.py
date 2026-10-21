#!/usr/bin/env python3
"""asset_extract_ships.py — Extract ship, pickup, coin and sidekick sprites
from tyrian.shp / newsh1.shp into PNGs named after sprite_manifest.json.

The sheets are grids of 12x14 tiles (19 columns). The manifest describes
each item as a frame of `frame_width` x `frame_height` (12x14 or a 24x28
composite of tiles index, index+1, index+19, index+20) with an optional
`frames` count; frames are laid out left to right in one strip. The tile
stride between frames is 1 for 12 px frames and 2 for 24 px composites —
the same rules asset_audit.py --content checks against.

Sheets and their binary sources (what the game blits, see JE_drawItem in
src/game_menu.c and the override tables in src/sprite_override.c):
    ships     -> tyrian.shp section 8 (spriteSheet9), plus section 12
                 (spriteSheetT2000) for manifest indices > 500
    pickups   -> weapons/* and shields/*: newsh1.shp (shopSpriteSheet, the
                 shop icons); specials/* and top-level names: tyrian.shp
                 section 9 (spriteSheet10, the in-level pickups)
    coins     -> tyrian.shp section 10 (spriteSheet11)
    sidekicks -> newsh1.shp (shopSpriteSheet) at the sidekick's itemgraphic

Manifest entries whose index lies beyond the binary sheet (custom content,
e.g. ships/blue_talon at 400 on a 304-cell sheet) are skipped and listed:
those PNGs exist only as files and cannot be regenerated.

By default the output is exactly data/reference/sprites/<sheet>/. --tiles
writes every raw 12x14 tile of one sheet as a numbered PNG instead
(<index>.png), for people who need the unnamed cells.

Usage:
    python3 utils/asset_extract_ships.py [--data-dir data/] --sheet ships|pickups|coins|sidekicks [--out DIR]
    python3 utils/asset_extract_ships.py --sheet t2k --tiles --out /tmp/t2k_tiles

Default --out: <data-dir>/reference/sprites/<sheet>/ (named mode);
               required with --tiles.
"""

import argparse
import os
import struct
import sys

try:
    from PIL import Image
except ImportError:
    print("ERROR: Pillow required. Install with: pip install Pillow")
    sys.exit(1)

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _shared import decode_sprite2, load_shp_section, load_vga_palette  # noqa: E402
from _sprite_manifest import load_manifest  # noqa: E402

TILE_W = 12
TILE_H = 14
GRID_COLS = 19

# Binary sources: ("shp", N) = tyrian.shp section N; ("file", name) = a
# standalone Sprite2 file in the data directory.
SHOP = ("file", "newsh1.shp")


# sheet name -> (manifest section, route function: (name, index) -> (source, index offset))
def _route_ships(name, idx):
    return (("shp", 12), 500) if idx > 500 else (("shp", 8), 0)


def _route_pickups(name, idx):
    if name.startswith(("weapons/", "shields/")):
        return SHOP, 0
    return ("shp", 9), 0


SHEETS = {
    "ships": ("ships", _route_ships),
    "pickups": ("pickups", _route_pickups),
    "coins": ("coins", lambda name, idx: (("shp", 10), 0)),
    "sidekicks": ("sidekicks", lambda name, idx: (SHOP, 0)),
}
# --tiles aliases (one source each); the legacy numeric names still work.
TILE_SOURCES = {
    "ships": ("shp", 8),
    "9": ("shp", 8),
    "pickups": ("shp", 9),
    "10": ("shp", 9),
    "coins": ("shp", 10),
    "11": ("shp", 10),
    "t2k": ("shp", 12),
    "sidekicks": SHOP,
}


def sprite2_offsets(blob):
    """Offset table of a Sprite2_array blob in memory (see _shared.load_sprite2_array)."""
    if len(blob) < 4:
        return []
    first = struct.unpack_from("<H", blob, 0)[0]
    if first == 0 or first > len(blob) or first & 1:
        return []
    return [struct.unpack_from("<H", blob, i * 2)[0] for i in range(first // 2)]


class Sheet:
    """One Sprite2 array (tyrian.shp section or newsh*.shp file), decoded
    lazily by 1-based game index."""

    def __init__(self, data_dir, source):
        kind, what = source
        if kind == "shp":
            self.blob = load_shp_section(data_dir, what)
        else:
            path = os.path.join(data_dir, what)
            self.blob = open(path, "rb").read() if os.path.isfile(path) else b""
        self.offsets = sprite2_offsets(self.blob) if self.blob else []
        self.count = len(self.offsets)

    def tile(self, index):
        """Decoded 12x14 tile for GAME index `index` (1-based), or None."""
        if index < 1 or index > self.count:
            return None
        off = self.offsets[index - 1]
        if off >= len(self.blob):
            return None
        return decode_sprite2(self.blob, off, TILE_W, TILE_H)


def paste_tile(img, pixels, ox, oy, palette):
    for py, row in enumerate(pixels):
        for px, pi in enumerate(row):
            if pi:
                img.putpixel((ox + px, oy + py), (*palette[pi], 255))


def render_frame(sheet, base, fw, fh, palette):
    """Compose the tiles of one frame whose top-left tile is `base`."""
    img = Image.new("RGBA", (fw, fh), (0, 0, 0, 0))
    for ty in range(fh // TILE_H):
        for tx in range(fw // TILE_W):
            pixels = sheet.tile(base + tx + ty * GRID_COLS)
            if pixels:
                paste_tile(img, pixels, tx * TILE_W, ty * TILE_H, palette)
    return img


def extract_named(data_dir, sheet_name, out_dir, palette):
    """Write every manifest entry of the section as <name>.png.
    Returns (written, custom) where custom lists entries beyond the binary."""
    section_key, route = SHEETS[sheet_name]
    manifest = load_manifest(os.path.join(data_dir, "sprite_manifest.json"))
    entries = manifest.get(section_key, {})
    if not entries:
        print(f'ERROR: no "{section_key}" section in sprite_manifest.json')
        sys.exit(1)

    sheets = {}
    count = 0
    custom = []
    for name, info in entries.items():
        idx = info.get("index") if isinstance(info, dict) else info
        if not isinstance(idx, int):
            continue
        fw = info.get("frame_width", TILE_W)
        fh = info.get("frame_height", TILE_H)
        frames = info.get("frames", 1)
        stride = 2 if fw >= 24 else 1

        source, base_off = route(name, idx)
        if source not in sheets:
            sheets[source] = Sheet(data_dir, source)
        sheet = sheets[source]
        if not sheet.count:
            print(f"  skip {name}: {source[1]} missing")
            continue
        last_tile = idx - base_off + (frames - 1) * stride + (fw // TILE_W - 1) + (fh // TILE_H - 1) * GRID_COLS
        if last_tile > sheet.count:
            custom.append((name, idx))
            continue

        strip = Image.new("RGBA", (fw * frames, fh), (0, 0, 0, 0))
        for f in range(frames):
            frame = render_frame(sheet, idx - base_off + f * stride, fw, fh, palette)
            strip.paste(frame, (f * fw, 0))

        path = os.path.join(out_dir, f"{name}.png")  # name may carry a subfolder
        os.makedirs(os.path.dirname(path), exist_ok=True)
        strip.save(path)
        count += 1
    return count, custom


def extract_tiles(data_dir, source, out_dir, palette):
    """Write every raw 12x14 tile of one sheet as <index>.png. Returns count."""
    sheet = Sheet(data_dir, source)
    if not sheet.count:
        print(f"ERROR: {source[1]} missing or malformed")
        sys.exit(1)
    os.makedirs(out_dir, exist_ok=True)
    count = 0
    for index in range(1, sheet.count + 1):
        pixels = sheet.tile(index)
        if pixels is None:
            continue
        img = Image.new("RGBA", (TILE_W, TILE_H), (0, 0, 0, 0))
        paste_tile(img, pixels, 0, 0, palette)
        img.save(os.path.join(out_dir, f"{index:03d}.png"))
        count += 1
    return count


def main():
    parser = argparse.ArgumentParser(
        description="Extract ship/pickup/coin sprites from tyrian.shp to PNGs named after sprite_manifest.json."
    )
    parser.add_argument("--data-dir", default="data", help="Tyrian data directory")
    parser.add_argument(
        "--sheet",
        default="ships",
        choices=sorted(set(SHEETS) | set(TILE_SOURCES)),
        help="ships (sections 8+12), pickups (9 + newsh1.shp), coins (10), "
        "sidekicks (newsh1.shp); with --tiles also t2k (12) and the "
        "numeric aliases 9/10/11",
    )
    parser.add_argument(
        "--out",
        default=None,
        help="Output directory (default: <data-dir>/reference/sprites/<sheet>)",
    )
    parser.add_argument(
        "--tiles",
        action="store_true",
        help="Write raw numbered 12x14 tiles of one section instead of manifest-named frames (requires --out)",
    )
    args = parser.parse_args()

    palette = load_vga_palette(args.data_dir)

    if args.tiles:
        if not args.out:
            parser.error("--tiles writes 304 numbered files: give an explicit --out")
        source = TILE_SOURCES[args.sheet]
        n = extract_tiles(args.data_dir, source, args.out, palette)
        print(f"Extracted {n} tiles ({TILE_W}x{TILE_H}) from {source[1]} to {args.out}/")
        return

    if args.sheet not in SHEETS:
        parser.error(f"--sheet {args.sheet} is only valid with --tiles (named sheets: {', '.join(SHEETS)})")
    out_dir = args.out or os.path.join(args.data_dir, "reference", "sprites", args.sheet)
    n, custom = extract_named(args.data_dir, args.sheet, out_dir, palette)
    print(f"Extracted {n} named {args.sheet} sprites to {out_dir}/")
    for name, idx in custom:
        print(f"  kept as-is (custom, index {idx} beyond the binary sheet): {name}.png")


if __name__ == "__main__":
    main()
