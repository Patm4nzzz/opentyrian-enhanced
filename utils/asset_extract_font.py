#!/usr/bin/env python3
"""
asset_extract_font.py — Convert Tyrian's bitmap fonts (tyrian.shp sections 0/1/2)
into modern .ttf / .woff2 files usable on a website.

Output:
    data/reference/fonts/
        tyrian-large.ttf  tyrian-large.woff2
        tyrian-small.ttf  tyrian-small.woff2
        tyrian-tiny.ttf   tyrian-tiny.woff2
        preview.html      (QA + usage example)

Dependencies:
    pip install fonttools brotli

Usage:
    python3 utils/asset_extract_font.py
    python3 utils/asset_extract_font.py --data-dir data --out data/reference/fonts
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

try:
    from fontTools.fontBuilder import FontBuilder
    from fontTools.pens.ttGlyphPen import TTGlyphPen
except ImportError:
    sys.exit("ERROR: install dependencies with: pip install fonttools brotli")


# ── Tyrian's font_ascii[] table (src/fonthand.c:32-51) ────────────────────
# Maps (unicode code point) → sprite index. Reverse of the C array.
# CP437 accented characters and ASCII punctuation included.

FONT_ASCII: dict[int, int] = {
    # Digits
    ord("0"): 79,
    ord("1"): 70,
    ord("2"): 71,
    ord("3"): 72,
    ord("4"): 73,
    ord("5"): 74,
    ord("6"): 75,
    ord("7"): 76,
    ord("8"): 77,
    ord("9"): 78,
    # Uppercase A-Z (sprites 0-25)
    **{ord(c): i for i, c in enumerate("ABCDEFGHIJKLMNOPQRSTUVWXYZ")},
    # Lowercase a-z (sprites 34-59)
    **{ord(c): 34 + i for i, c in enumerate("abcdefghijklmnopqrstuvwxyz")},
    # Basic punctuation
    ord("!"): 26,
    ord("?"): 27,
    ord("."): 28,
    ord(","): 29,
    ord(";"): 30,
    ord(":"): 31,
    ord("'"): 32,
    ord('"'): 33,
    ord("#"): 60,
    ord("$"): 61,
    ord("%"): 62,
    ord("*"): 63,
    ord("("): 64,
    ord(")"): 65,
    ord("{"): 66,
    ord("}"): 67,
    ord("["): 68,
    ord("]"): 69,
    ord("/"): 80,
    ord("|"): 81,
    ord("\\"): 82,
    ord("-"): 83,
    ord("+"): 84,
    ord("="): 85,
    # CP437 extended (accented characters) — 86 → 126
    ord("Ç"): 86,
    ord("ü"): 87,
    ord("é"): 88,
    ord("â"): 89,
    ord("ä"): 90,
    ord("à"): 91,
    ord("å"): 92,
    ord("ç"): 93,
    ord("ê"): 94,
    ord("ë"): 95,
    ord("è"): 96,
    ord("ï"): 97,
    ord("î"): 98,
    ord("ì"): 99,
    ord("Ä"): 100,
    ord("Å"): 101,
    ord("É"): 102,
    ord("æ"): 103,
    ord("Æ"): 104,
    ord("ô"): 105,
    ord("ö"): 106,
    ord("ò"): 107,
    ord("û"): 108,
    ord("ù"): 109,
    ord("ÿ"): 110,
    ord("Ö"): 111,
    ord("Ü"): 112,
    ord("¢"): 113,
    ord("£"): 114,
    ord("¥"): 115,
    ord("₧"): 116,
    ord("ƒ"): 117,
    ord("á"): 118,
    ord("í"): 119,
    ord("ó"): 120,
    ord("ú"): 121,
    ord("ñ"): 122,
    ord("Ñ"): 123,
    ord("ª"): 124,
    ord("º"): 125,
    ord("¿"): 126,
}


# ── tyrian.shp Sprite format parser ───────────────────────────────────────


def parse_shp_header(path: Path) -> tuple[bytes, list[int]]:
    """Return (blob, shpPos[]). shpPos has `shpNumb + 1` entries (last = EOF)."""
    blob = path.read_bytes()
    shp_numb = struct.unpack_from("<H", blob, 0)[0]
    offsets = list(struct.unpack_from(f"<{shp_numb}i", blob, 2))
    offsets.append(len(blob))  # sentinel
    return blob, offsets


def parse_sprite_array(blob: bytes, offset: int) -> list[dict | None]:
    """Parse a Sprite_array (sections 0-6) at `offset`. Returns list of dicts
    {width, height, pixels: List[List[int]]} or None for empty slots."""
    pos = offset
    count = struct.unpack_from("<H", blob, pos)[0]
    pos += 2

    sprites: list[dict | None] = []
    for _ in range(count):
        populated = blob[pos] != 0
        pos += 1
        if not populated:
            sprites.append(None)
            continue
        width, height, size = struct.unpack_from("<HHH", blob, pos)
        pos += 6
        data = blob[pos : pos + size]
        pos += size
        pixels = decode_sprite_rle(data, width, height)
        sprites.append({"width": width, "height": height, "pixels": pixels})
    return sprites


def decode_sprite_rle(data: bytes, width: int, height: int) -> list[list[int]]:
    """Decode RLE format: 255=N-trans, 254=EOL, 253=1-trans, else=palette-idx.
    Matches the decoder in src/sprite.c:111-175 (blit_sprite)."""
    pixels = [[0] * width for _ in range(height)]
    i = 0
    row = 0
    col = 0
    n = len(data)

    while i < n and row < height:
        b = data[i]
        i += 1
        if b == 255:  # skip N transparent pixels
            if i >= n:
                break
            skip = data[i]
            i += 1
            col += skip
        elif b == 254:  # advance to next row
            col = width  # force wrap below
        elif b == 253:  # 1 transparent pixel
            col += 1
        else:  # opaque pixel (we only need "on"/"off" for a monochrome font)
            if row < height and col < width:
                pixels[row][col] = 1
            col += 1

        # Wrap row when column overflows
        while col >= width and row < height:
            col -= width
            row += 1

    return pixels


# ── TTF builder ───────────────────────────────────────────────────────────

# Font units per em (standard TTF value).
UPM = 1000

# Vertical metrics shared across all three sizes. Cap height (height of a
# capital 'A') is normalized to CAP_UNITS font units, so Large/Small/Tiny
# capitals render at identical visible heights at the same CSS font-size and
# baselines align in fallback stacks.
CAP_UNITS = 700  # cap letter height ≈ 70% of em (typographic standard)
ASCENT_UNITS = 800  # 100 units above cap for accents / line breathing
DESCENT_UNITS = 200  # below baseline for descenders ('g','j','p','q','y')

# Fallback glyph advance (used for .notdef and space). 6 source pixels matches
# the game's `x += 6` for space characters (src/fonthand.c).
SPACE_PIXELS = 6


def pixel_to_contour(pen: TTGlyphPen, px: int, py: int, pixel_unit: int) -> None:
    """Emit a single 1×1 pixel square. Counter-clockwise winding (TTF fill rule)."""
    x = px * pixel_unit
    y = py * pixel_unit
    s = pixel_unit
    pen.moveTo((x, y))
    pen.lineTo((x + s, y))
    pen.lineTo((x + s, y + s))
    pen.lineTo((x, y + s))
    pen.closePath()


def measure_cap_height(sprites: list[dict | None]) -> int:
    """Cap height = height of the capital 'A' sprite (index 0).
    Falls back to the most common sprite height if 'A' is missing."""
    if sprites and sprites[0] is not None:
        return sprites[0]["height"]
    from collections import Counter

    heights = Counter(s["height"] for s in sprites if s)
    return heights.most_common(1)[0][0] if heights else 8


def build_glyph(sprite: dict, cap_h: int, pixel_unit: int) -> tuple[object, int]:
    """Build a TTF glyph. Baseline-aligned: row (cap_h-1) of the bitmap sits on
    the baseline (y=0), rows below that become descenders (y<0). Matches how
    Tyrian's top-aligned sprites visually sit on a shared baseline."""
    pen = TTGlyphPen(None)
    width = sprite["width"]
    height = sprite["height"]
    pixels = sprite["pixels"]

    # Bitmap row r → pixel y = (cap_h - 1 - r). Row 0 (top of sprite) lands at
    # y = cap_h - 1; row cap_h-1 lands at y = 0 (baseline); descender rows go
    # below baseline.
    for r in range(height):
        for c in range(width):
            if pixels[r][c]:
                pixel_to_contour(pen, c, cap_h - 1 - r, pixel_unit)

    advance = (width + 1) * pixel_unit  # +1 matches the game's `x += width + 1`
    return pen.glyph(), advance


def build_font(
    sprites: list[dict | None],
    family_name: str,
    style: str,
    out_ttf: Path,
    out_woff2: Path,
) -> None:
    """Build a TTF from a list of Tyrian sprites and save .ttf + .woff2."""
    cap_h = measure_cap_height(sprites)

    # Per-font pixel scale: the cap letter spans CAP_UNITS font units. All
    # three fonts end up with the same visible cap-height at a given CSS
    # font-size, so "font-family: Large, Tiny" fallbacks stay on the same
    # baseline and capitals match in size.
    pixel_unit = CAP_UNITS // cap_h

    glyph_order = [".notdef", "space"]
    cmap: dict[int, str] = {}
    glyphs: dict[str, object] = {}
    advances: dict[str, int] = {}

    notdef_pen = TTGlyphPen(None)
    glyphs[".notdef"] = notdef_pen.glyph()
    advances[".notdef"] = SPACE_PIXELS * pixel_unit

    space_pen = TTGlyphPen(None)
    glyphs["space"] = space_pen.glyph()
    advances["space"] = SPACE_PIXELS * pixel_unit
    cmap[ord(" ")] = "space"

    for codepoint, sprite_idx in sorted(FONT_ASCII.items()):
        if sprite_idx >= len(sprites) or sprites[sprite_idx] is None:
            continue  # sprite slot empty in this font size
        name = f"u{codepoint:04X}"
        glyph, adv = build_glyph(sprites[sprite_idx], cap_h, pixel_unit)
        glyphs[name] = glyph
        advances[name] = adv
        glyph_order.append(name)
        cmap[codepoint] = name

    fb = FontBuilder(UPM, isTTF=True)
    fb.setupGlyphOrder(glyph_order)
    fb.setupCharacterMap(cmap)
    fb.setupGlyf(glyphs)

    metrics = {name: (advances[name], 0) for name in glyph_order}
    fb.setupHorizontalMetrics(metrics)
    # Identical vertical metrics across all three fonts — baselines match in
    # CSS fallback stacks.
    fb.setupHorizontalHeader(ascent=ASCENT_UNITS, descent=-DESCENT_UNITS, lineGap=0)

    full_name = f"Tyrian {style}"
    fb.setupNameTable(
        {
            "familyName": family_name,
            "styleName": style,
            "uniqueFontIdentifier": f"OpenTyrian2000:{full_name}",
            "fullName": full_name,
            "psName": full_name.replace(" ", ""),
            "version": "Version 1.0",
        }
    )
    fb.setupOS2(
        sTypoAscender=ASCENT_UNITS,
        sTypoDescender=-DESCENT_UNITS,
        sTypoLineGap=0,
        usWinAscent=ASCENT_UNITS,
        usWinDescent=DESCENT_UNITS,
        sCapHeight=CAP_UNITS,
        achVendID="OTYR",
    )
    fb.setupPost()

    out_ttf.parent.mkdir(parents=True, exist_ok=True)
    fb.save(str(out_ttf))

    # WOFF2
    from fontTools.ttLib import TTFont

    font = TTFont(str(out_ttf))
    font.flavor = "woff2"
    font.save(str(out_woff2))

    print(f"  wrote {out_ttf.name} + {out_woff2.name} ({len(glyph_order) - 1} glyphs, cap-height {cap_h}px)")


# ── Preview HTML ──────────────────────────────────────────────────────────

PREVIEW_HTML = r"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Tyrian Fonts — QA Preview</title>
<style>
@font-face {
    font-family: "Tyrian Large";
    src: url("tyrian-large.woff2") format("woff2"),
         url("tyrian-large.ttf") format("truetype");
}
@font-face {
    font-family: "Tyrian Small";
    src: url("tyrian-small.woff2") format("woff2"),
         url("tyrian-small.ttf") format("truetype");
}
@font-face {
    font-family: "Tyrian Tiny";
    src: url("tyrian-tiny.woff2") format("woff2"),
         url("tyrian-tiny.ttf") format("truetype");
}

/* Fallback stack: use Large for letters, fall back to Tiny for digits/accents
   that the bigger fonts don't contain. */
.tyrian-heading { font-family: "Tyrian Large", "Tyrian Tiny", monospace; }
.tyrian-body    { font-family: "Tyrian Small", "Tyrian Tiny", monospace; }
.tyrian-hud     { font-family: "Tyrian Tiny", monospace; }

/* Authentic Tyrian palette colors (from data/palette.dat):
   #FFE63C — bright menu yellow  (idx 125)
   #FACA30 — gold / highlight    (idx 124)
   #F2AE24 — amber / dim         (idx 123)
   #FFFFB6 — pale yellow         (idx 253) */
body {
    background: #0c0c14;
    color: #d8d8d0;
    font-family: sans-serif;
    padding: 2rem;
    max-width: 960px;
    margin: auto;
}
h1 { color: #FFE63C; border-bottom: 1px solid #552; padding-bottom: .5rem; }
h2 { color: #FACA30; margin-top: 2rem; }
h3 { color: #F2AE24; margin-top: 1.5rem; }
.sample {
    background: #000;
    border: 1px solid #332; border-left: 3px solid #FFE63C;
    padding: 1rem;
    margin: .5rem 0 1.5rem;
    /* Preserve the pixel-art aesthetic */
    -webkit-font-smoothing: none;
    -moz-osx-font-smoothing: grayscale;
    font-smooth: never;
    color: #FFE63C;         /* Tyrian menu yellow */
    line-height: 1.2;
}
.note { font-size: .85rem; color: #a89a70; margin-top: -1rem; margin-bottom: 1rem; }
code { background: #221; padding: 1px 4px; border-radius: 3px; color: #FFFFB6; }

.size-row { display: flex; gap: 1rem; align-items: flex-end; flex-wrap: wrap; }
.size-row > div { flex: 1; min-width: 200px; }
.size-row span:first-child { color: #888; font-family: monospace; font-size: .75rem; display: block; }
</style>
</head>
<body>

<h1>OpenTyrian2000 — Exported fonts</h1>
<p>Three sizes extracted from <code>tyrian.shp</code> (sections 0, 1, 2),
   converted to TTF/WOFF2 with <code>utils/asset_extract_font.py</code>.</p>

<h3>Glyph coverage by size</h3>
<ul>
  <li><b>Large</b> (60 glyphs, 12px native) — A-Z, a-z, basic punctuation (<code>! ? . , ; : ' "</code>). <b>No digits, no accents.</b></li>
  <li><b>Small</b> (85 glyphs, 8px native) — all of Large + digits 0-9 + extended punctuation (<code>() {} [] / | \ - + = # $ % *</code>). Still no accents.</li>
  <li><b>Tiny</b> (127 glyphs, 5-6px native) — full coverage: Large + Small + all the CP437 accents (é à è ê ë ï î ô ö û ù ç ü ÿ ñ + capitals Ä Å É Ö Ü Ç Ñ).</li>
</ul>

<h2>Tyrian Large (<code>FONT_SHAPES</code>)</h2>
<div class="sample" style="font-family: 'Tyrian Large'; font-size: 48px;">TYRIAN</div>
<div class="sample" style="font-family: 'Tyrian Large'; font-size: 32px;">The quick brown fox jumps over the lazy dog.</div>

<h2>Tyrian Small (<code>SMALL_FONT_SHAPES</code>)</h2>
<div class="sample" style="font-family: 'Tyrian Small'; font-size: 28px;">Select your ship</div>
<div class="sample" style="font-family: 'Tyrian Small'; font-size: 20px;">The quick brown fox jumps over the lazy dog 0123456789</div>

<h2>Tyrian Tiny (<code>TINY_FONT</code>) — the only one to support accents</h2>
<div class="sample" style="font-family: 'Tyrian Tiny'; font-size: 32px;">HUD: armor 100 / score 01234567</div>
<div class="sample" style="font-family: 'Tyrian Tiny'; font-size: 24px;">Zéphyr grêlé où l'ïambe fâché câline</div>

<h2>Recommended fallback stack</h2>
<p class="note">For text mixing letters + digits + accents, stack Large/Small on top of Tiny
so that the browser falls back to Tiny for the missing glyphs.</p>

<div class="sample tyrian-heading" style="font-size: 48px;">TYRIAN 2000</div>
<div class="note">CSS: <code>font-family: "Tyrian Large", "Tyrian Tiny", monospace;</code> — "TYRIAN" comes from Large, "2000" comes from Tiny.</div>

<div class="sample tyrian-body" style="font-size: 24px;">Crème brûlée: press ENTRÉE to start — score: 42</div>
<div class="note">CSS: <code>font-family: "Tyrian Small", "Tyrian Tiny", monospace;</code> — the accents "è û É" come from Tiny.</div>

<h2>Rendering at different sizes</h2>
<div class="size-row">
  <div><span>20px</span><div class="sample" style="font-family: 'Tyrian Large'; font-size: 20px; padding: .5rem; margin: 0;">TYRIAN</div></div>
  <div><span>40px</span><div class="sample" style="font-family: 'Tyrian Large'; font-size: 40px; padding: .5rem; margin: 0;">TYRIAN</div></div>
  <div><span>80px</span><div class="sample" style="font-family: 'Tyrian Large'; font-size: 80px; padding: .5rem; margin: 0;">TYRIAN</div></div>
</div>

<h2>Pixel-perfect tip</h2>
<p>Sizes that are multiples of the native height (Large: 12, Small: 8, Tiny: 5-6 px)
   give the sharpest rendering. CSS to turn off anti-aliasing:</p>
<pre style="background: #222; padding: 1rem; border-radius: 4px; color: #dfe;"><code>.retro-text {
  font-family: "Tyrian Large", "Tyrian Tiny", monospace;
  -webkit-font-smoothing: none;
  -moz-osx-font-smoothing: grayscale;
  font-smooth: never;
  image-rendering: pixelated;
}</code></pre>

</body>
</html>
"""


def write_preview(out_dir: Path) -> None:
    (out_dir / "preview.html").write_text(PREVIEW_HTML, encoding="utf-8")
    print("  wrote preview.html — open in a browser to QA")


# ── Main ──────────────────────────────────────────────────────────────────

FONT_SIZES = [
    (0, "Large"),  # FONT_SHAPES
    (1, "Small"),  # SMALL_FONT_SHAPES
    (2, "Tiny"),  # TINY_FONT
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-dir", default="data", help="Tyrian data directory")
    parser.add_argument("--out", default="data/reference/fonts", help="Output directory for .ttf/.woff2")
    parser.add_argument("--shp", default="tyrian.shp", help="Source SHP file")
    args = parser.parse_args()

    shp_path = Path(args.data_dir) / args.shp
    if not shp_path.exists():
        print(f"ERROR: {shp_path} not found", file=sys.stderr)
        return 1

    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    blob, offsets = parse_shp_header(shp_path)

    print(f"Parsing {shp_path} ({len(blob)} bytes, {len(offsets) - 1} sections)")

    for section_idx, style in FONT_SIZES:
        sprites = parse_sprite_array(blob, offsets[section_idx])
        present = sum(1 for s in sprites if s)
        print(f"  section {section_idx} ({style}): {present}/{len(sprites)} glyphs")

        basename = f"tyrian-{style.lower()}"
        build_font(
            sprites,
            family_name=f"Tyrian {style}",
            style="Regular",
            out_ttf=out_dir / f"{basename}.ttf",
            out_woff2=out_dir / f"{basename}.woff2",
        )

    write_preview(out_dir)
    print(f"\nDone → {out_dir}/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
