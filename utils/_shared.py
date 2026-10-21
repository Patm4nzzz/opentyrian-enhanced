"""_shared — common primitives for OpenTyrian2000 util scripts.

Before this module, every asset_extract_* script rolled its own palette
loader, nibble-RLE decoder, and SHP offset parser. Each copy had
slightly different bugs: some clamped 6→8 bit with `*255//63`, others
with `<<2 | >>4`; some bounds-checked the offset table, others
crashed on malformed files; some handled missing palette.dat, others
silently returned garbage.

The same thing happened again with the locale lints: three copies of
the UTF-8 → CP437 table read out of the C source (now frozen in c_tables.json), and the one in
locale_widths.py had lost the 3-byte typographic block, so the CI
flagged curly quotes and en-dashes that the game renders fine.

Consolidation lives here. Importing scripts should prefer:
    from _shared import (
        load_vga_palette, decode_sprite2, load_sprite2_array,
        load_shp_section, shp_section_sprite_count,
        atomic_write_bytes, atomic_write_text,
        font_tables, utf8_to_cp437_map, text_to_cp437,
        repo_root,
    )

Keep this module stdlib-only: it must import in a bare CI checkout.
Pillow-dependent helpers belong in the scripts that need them.
"""

from __future__ import annotations

import os
import struct
import tempfile
from pathlib import Path

# ── VGA Palette ────────────────────────────────────────────────────────

# Tyrian stores 24 palettes in palette.dat, 256 colors × 3 bytes, each
# channel a 6-bit VGA DAC value (0..63). Scaling to 8-bit: the textbook
# formula `(v << 2) | (v >> 4)` replicates the high bits into the low
# bits so pure-white (63,63,63) maps to (255,255,255) exactly. The
# simpler `v * 255 // 63` yields the same endpoints but slightly
# different midrange values — we pick the bit-replication form because
# that's what the game's palette code does, matching extracted sprites
# to in-game color exactly.
PALETTE_COUNT = 24
PALETTE_SIZE_BYTES = 256 * 3


def load_vga_palette(data_dir, palette_index: int = 0):
    """Load one 256-color VGA palette from <data_dir>/palette.dat.

    Returns a list of 256 (r, g, b) tuples in 8-bit range. Falls back
    to a grayscale ramp if palette.dat is missing so callers can still
    produce visually inspectable output (e.g. when a modder is sanity-
    checking a SHP without the full data/ tree).
    """
    pal_path = os.path.join(str(data_dir), "palette.dat")
    if not os.path.exists(pal_path):
        return [(i, i, i) for i in range(256)]

    with open(pal_path, "rb") as f:
        f.seek(palette_index * PALETTE_SIZE_BYTES)
        raw = f.read(PALETTE_SIZE_BYTES)

    if len(raw) < PALETTE_SIZE_BYTES:
        # Truncated palette.dat — pad with zeros rather than crashing.
        raw = raw + b"\x00" * (PALETTE_SIZE_BYTES - len(raw))

    palette = []
    for i in range(256):
        r6 = raw[i * 3 + 0]
        g6 = raw[i * 3 + 1]
        b6 = raw[i * 3 + 2]
        palette.append(
            (
                ((r6 & 0x3F) << 2) | ((r6 & 0x3F) >> 4),
                ((g6 & 0x3F) << 2) | ((g6 & 0x3F) >> 4),
                ((b6 & 0x3F) << 2) | ((b6 & 0x3F) >> 4),
            )
        )
    return palette


# ── Sprite2 (nibble-RLE) decoder ──────────────────────────────────────


def decode_sprite2(data, offset: int, width: int = 12, height: int = 14):
    """Decode one nibble-RLE Sprite2 cell.

    Format: each control byte packs a skip count (low nibble) and a
    fill count (high nibble). When fill is 0, the row ends. The byte
    0x0F (skip 15, fill 0) terminates the sprite.

    Returns a 2D list (rows × cols) of 8-bit palette indices where 0
    means transparent.
    """
    pixels = [[0] * width for _ in range(height)]
    pos = offset
    row = 0
    col = 0

    while row < height and pos < len(data):
        byte = data[pos]
        pos += 1

        if byte == 0x0F:
            break

        skip_count = byte & 0x0F
        fill_count = (byte >> 4) & 0x0F

        col += skip_count

        if fill_count == 0:
            row += 1
            col = 0
            continue

        for _ in range(fill_count):
            if row < height and col < width and pos < len(data):
                pixels[row][col] = data[pos]
            pos += 1
            col += 1

    return pixels


def load_sprite2_array(filepath):
    """Load a Sprite2_array .shp file.

    The file begins with a uint16-LE offset table; the very first entry
    equals `2 * num_sprites` (since the table itself takes 2 bytes per
    sprite). Returns `(data_blob, offsets_list)`. Returns `(data, [])`
    if the file is too small or the header looks malformed.
    """
    with open(filepath, "rb") as f:
        data = f.read()

    if len(data) < 4:
        return data, []

    first_offset = struct.unpack_from("<H", data, 0)[0]
    if first_offset == 0 or first_offset > len(data) or first_offset & 1:
        return data, []

    count = first_offset // 2
    offsets = [struct.unpack_from("<H", data, i * 2)[0] for i in range(count)]
    return data, offsets


# ── tyrian.shp (multi-section) reader ─────────────────────────────────
#
# tyrian.shp header: u16 section_count + section_count × int32 offsets
# (byte offsets from the start of the file). Each section body is an
# independent Sprite2_array blob with its own offset table at the start.
# The section layout Tyrian ships (nominally 13 sections) is:
#     0-6: fonts, 7: shots, 8: ships, 9: pickups, 10: coins,
#     11: more shots (spriteSheet12), 12: T2000 ships


def load_shp_section(data_dir, section_index: int):
    """Extract one section of tyrian.shp into a standalone blob.

    Returns the section blob as bytes, or b"" if the file is missing,
    truncated, or section_index is out of range.
    """
    shp_path = os.path.join(str(data_dir), "tyrian.shp")
    if not os.path.exists(shp_path):
        return b""

    with open(shp_path, "rb") as f:
        hdr = f.read(2)
        if len(hdr) < 2:
            return b""
        count = struct.unpack("<H", hdr)[0]
        if section_index < 0 or section_index >= count:
            return b""

        tbl = f.read(count * 4)
        if len(tbl) < count * 4:
            return b""
        offsets = struct.unpack(f"<{count}i", tbl)

        start = offsets[section_index]
        if section_index + 1 < count:
            size = offsets[section_index + 1] - start
            f.seek(start)
            return f.read(size)
        f.seek(start)
        return f.read()


# ── Atomic file writes ────────────────────────────────────────────────


def atomic_write_bytes(dst_path, payload: bytes) -> None:
    """Write bytes atomically: tmp file in the same dir, then rename.

    Prevents corrupt half-written files if the script is killed mid-
    write. `dst_path` accepts str or Path.
    """
    dst = Path(dst_path)
    dst.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=dst.name + ".", suffix=".tmp", dir=str(dst.parent))
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(payload)
        os.replace(tmp_name, dst)
    except BaseException:
        try:
            os.unlink(tmp_name)
        except OSError:
            pass
        raise


def atomic_write_text(dst_path, text: str, encoding: str = "utf-8") -> None:
    atomic_write_bytes(dst_path, text.encode(encoding))


# ── Repo root helper ──────────────────────────────────────────────────


def repo_root() -> Path:
    """Return the repo root (assumes this file lives in utils/)."""
    return Path(__file__).resolve().parent.parent


def shp_section_sprite_count(data_dir, section_index: int) -> int:
    """Number of sprites in one section of tyrian.shp (0 if unreadable).

    The section blob opens with a uint16 sprite count. font_glyph_lint,
    locale_glyphs and locale_widths each re-read the file for this one
    number; they now share this call.
    """
    blob = load_shp_section(data_dir, section_index)
    if len(blob) < 2:
        return 0
    return struct.unpack_from("<H", blob, 0)[0]


# ── Tables of the retired C version ──────────────────────────────────────
#
# Three checks (locale_glyphs, locale_widths, font_glyph_lint) must
# know what the UTF-8 → CP437 conversion of the old locale.c does with a
# character, and which sprite the old fonthand.c draws for a CP437 byte.
# The C version is retired (tag c-final): these tables, formerly re-read from
# src/, are frozen in utils/c_tables.json, extracted once from the tag
# c-final. The Godot engine (tyrian_text.gd) applies the same rules.

_C_TABLES = {}


def c_tables() -> dict:
    """Content of utils/c_tables.json (read only once)."""
    if not _C_TABLES:
        import json

        path = Path(__file__).resolve().parent / "c_tables.json"
        with open(path, encoding="utf-8") as f:
            _C_TABLES.update(json.load(f))
    return _C_TABLES


def utf8_to_cp437_map(src_dir=None):
    """{unicode character: CP437 byte}, as the old locale.c converted it.

    When a conversion writes several bytes (ß → "ss", … → "..."), the
    value is the FIRST byte written. A deleted character (the degree sign) or
    one with no rule is absent from the table.
    """
    return dict(c_tables()["utf8_to_cp437"])


def font_tables(src_dir=None):
    """(font_ascii[256], {CP437 byte: fallback ASCII character}).

    font_ascii[] associates a CP437 byte with a sprite index shared by the
    three fonts of tyrian.shp (-1 = no glyph); cp437_unaccent() is
    the fallback applied when the requested font is too short to contain
    the accented glyph.
    """
    t = c_tables()
    return list(t["font_ascii"]), {int(k): v for k, v in t["cp437_unaccent"].items()}


def text_to_cp437(text: str, conv=None):
    """Replays the locale.c conversion on a string: one CP437 byte per
    character, or None for a character with no rule (its raw UTF-8 bytes
    would reach the font and be drawn as garbage)."""
    if conv is None:
        conv = utf8_to_cp437_map()
    out = []
    for ch in text:
        code = ord(ch)
        if code < 0x80:
            out.append(code)
        else:
            out.append(conv.get(ch))
    return out
