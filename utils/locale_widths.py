#!/usr/bin/env python3
"""locale_widths.py -- do the menu labels fit in their space?

Measures the REAL width in pixels of each string: sprite widths
added up from the font sheet (they are variable), UTF-8 -> CP437 conversion
of locale.c, and per-font fallback of fonthand.c.

Two points that took three attempts to get right:

  * The ROLE of each key (label or description) is extracted from the MenuItem
    tables of the C sources. Guessing it from the name classified
    `setup_done_back` -- which is the description of the Done row -- as a
    label, and produced dozens of false positives.
  * The FONT differs: the help band goes through JE_textShade -> JE_outText,
    which blits in TINY_FONT (section 2); the menu_tree labels go
    through draw_font_hv_shadow in normal_font (section 1). Measuring both
    with the same font ruins everything.

Text overflows are the most frequent class of bug in the
CHANGELOG on the menus and had no automatic check.

    python3 utils/locale_widths.py            # report
    python3 utils/locale_widths.py --quiet    # exit code only
"""

import argparse
import collections
import os
import struct
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "utils"))
from _shared import font_tables, load_shp_section, text_to_cp437, utf8_to_cp437_map  # noqa: E402

LABEL_LIMIT, LABEL_FONT = 158, 1  # VALUE_X_OFFSET=160 in menu_tree.c, minus a 2 px margin
DESC_LIMIT, DESC_FONT = 296, 2  # help band, from x=20 to the edge
BIG_LIMIT = 316  # large-font header, centered on 320 px
MENUINT_LIMIT = 310 - 166  # shop: text at x = 166, panel up to 310
CAPTURE_LIMIT = 244  # gamepad capture prompt text, in the 260 px box
CONTROLS_LIMIT = 140 - 20 - 4  # Controls (Godot): label from x = 20 to the key (x = 140)
CONTROLS_LABELS = {
    "joym_up",
    "joym_down",
    "joym_left",
    "joym_right",
    "joym_fire",
    "joym_change_fire",
    "joym_left_sidekick",
    "joym_right_sidekick",
    "gd_controls_help",
    "gd_controls_cancel",
    "gd_controls_pause",
}


def wrap_two_lines(text, sheet_):
    """Cuts at spaces like joystick_menu.gd:wrap_lines; refuses more than two lines."""
    lines, cur = [], ""
    for word in text.split():
        cand = word if not cur else cur + " " + word
        if cur and px(cand, sheet_) > CAPTURE_LIMIT:
            lines.append(cur)
            cur = word
        else:
            cur = cand
    lines.append(cur)
    return lines if len(lines) <= 2 else [text]


def menu_key_roles():
    """{key: 'label'|'desc'}: role of each key of the MenuItem tables of the
    retired C version (tag c-final), frozen in c_tables.json."""
    from _shared import c_tables

    return dict(c_tables()["menu_key_roles"])


# The tables come from _shared (a single read of locale.c and of
# fonthand.c for the three lints). The local copy of the conversion had
# lost the 0xE2 block of locale.c: "’" and "–" passed for
# characters with no glyph in locale_cubes.py, which imported it.
_TABLES = {}


def tables():
    """(font_ascii[256], cp437_unaccent fallbacks, conversion UTF-8 -> CP437)."""
    if not _TABLES:
        fa, unacc = font_tables()
        _TABLES["fa"], _TABLES["unacc"], _TABLES["conv"] = fa, unacc, utf8_to_cp437_map()
    return _TABLES["fa"], _TABLES["unacc"], _TABLES["conv"]


def conv():
    return tables()[2]


def sheet(section):
    blob = load_shp_section(os.path.join(ROOT, "data"), section)
    if not blob:
        return 0, {}
    n = struct.unpack("<H", blob[:2])[0]
    pos, widths = 2, {}
    for i in range(n):
        if blob[pos] == 0:
            pos += 1
            widths[i] = 0
            continue
        pos += 1
        w, _h, size = struct.unpack("<HHH", blob[pos : pos + 6])
        data = blob[pos + 6 : pos + 6 + size]
        pos += 6 + size
        # A sprite can EXIST and draw nothing. The large font
        # does contain "! ? . , ' \"" at indexes 26-30, 32, 33, but their
        # data is 0xFE 0xFE -- the RLE skip opcode, zero pixels.
        # Only ":" (31) has content. Counting their presence is
        # misleading: "Livello succ." loses its period, "Congratulations!" its
        # exclamation mark. They are declared absent.
        widths[i] = 0 if all(b == 0xFE for b in data) else w
    return n, widths


_SHEETS = {}


def sheets():
    """{section: (nb_sprites, {index: width})} for the three fonts,
    read only once and only when needed (not at import,
    not for --help)."""
    if not _SHEETS:
        _SHEETS.update({0: sheet(0), 1: sheet(1), 2: sheet(2)})
    return _SHEETS


def px(text, sheet_info, fallbacks=None):
    """Width in pixels, and -- if `fallbacks` is provided -- list of the
    characters that this font cannot draw.

    All the accented glyphs live at indexes 86-118 and therefore exist
    ONLY in the tiny font (127 sprites). The normal font has 85:
    it draws none, not even an accented "e". An accented
    menu label falls back to the bare letter and makes font_sprite_id() yelp
    on every frame -- "Ecran:" in French did it all this time,
    and the CI gameplay job counts those warnings as a failure.
    """
    n, widths = sheet_info
    if n == 0:
        return 0
    font_ascii, unacc, cv = tables()
    x = 0
    for ch, cp in zip(text, text_to_cp437(text, cv), strict=False):
        if ch == " ":
            x += 6
            continue
        if ch == "~":  # highlight toggle, not drawn
            continue
        if cp is None:  # no rule in locale.c: nothing is drawn
            continue
        idx = font_ascii[cp] if cp < 256 else -1
        if idx >= n:  # outside this sheet: ASCII fallback
            if fallbacks is not None and ch not in fallbacks:
                fallbacks.append(ch)
            base = unacc.get(cp)
            idx = font_ascii[ord(base)] if base else -1
        if idx < 0 or idx >= n:
            continue
        if widths.get(idx, 0) == 0:
            # sprite present but empty: nothing will be drawn
            if fallbacks is not None and ch not in fallbacks:
                fallbacks.append(ch)
            continue
        x += widths.get(idx, 0) + 1
    return x


def large_font_keys():
    """Keys drawn in the LARGE font (sheet 0, 60 sprites).

    This sheet only contains A-Z, a-z and eight punctuation marks -- no
    digit, no hyphen, no accent. List collected from the call sites
    of the retired C version (tag c-final), frozen in c_tables.json;
    the Godot client draws the same headers in the same font.
    """
    from _shared import c_tables

    return set(c_tables()["large_font_keys"])


def menuint_keys():
    """Keys that land in menuInt[][], hence drawn in font 1
    (the 85-sprite sheet, with no accented glyph at all). List collected
    from the retired C version (tag c-final), frozen in c_tables.json."""
    from _shared import c_tables

    return set(c_tables()["menuint_keys"])


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--quiet", action="store_true", help="exit code only, no report")
    args = ap.parse_args(argv)
    quiet = args.quiet
    roles = menu_key_roles()
    SHEETS = sheets()
    if SHEETS[1][0] == 0:
        print("locale_widths: data/tyrian.shp not found — check skipped")
        return 0
    if not roles:
        print("locale_widths: no menu key — c_tables.json empty?")
        return 1

    MENUINT = menuint_keys()
    BIGFONT = large_font_keys()
    over = collections.defaultdict(list)
    nodraw = collections.defaultdict(list)
    loc_dir = os.path.join(ROOT, "data/locales")
    for fn in sorted(os.listdir(loc_dir)):
        if not fn.startswith("locale_") or not fn.endswith(".txt") or "_cubes" in fn:
            continue
        lang = fn[len("locale_") : -len(".txt")]
        for line in open(os.path.join(loc_dir, fn), encoding="utf-8", errors="replace"):
            if "=" not in line or line.startswith("#"):
                continue
            k, _, v = line.partition("=")
            k, v = k.strip(), v.rstrip("\n")
            # Achievement descriptions: drawn in the TINY font at x+24 on
            # the achievements screen, without wrapping. Only one exceeded
            # (it/achd_comeback_kid, 346 px) and reprinted over
            # itself -- the blitter does not clip on the right.
            if k.startswith("achd_") and v:
                if px(v, SHEETS[2]) > 284:
                    over[lang].append((px(v, SHEETS[2]), 284, "achievement description", k, v))

            if k in BIGFONT and v:
                fb3 = []
                big = SHEETS[0] if 0 in SHEETS else SHEETS[1]
                w3 = px(v, big, fb3)
                if fb3:
                    nodraw[lang].append(("large font header", k, v, "".join(fb3)))
                # Header centered on the 320 px screen: beyond that, it overflows
                # on both sides (the blitter does not cut).
                if w3 > BIG_LIMIT:
                    over[lang].append((w3, BIG_LIMIT, "large font header", k, v))

            if k in MENUINT and v:
                fb2 = []
                w2 = px(v, SHEETS[1], fb2)
                if fb2:
                    nodraw[lang].append(("menuInt label", k, v, "".join(fb2)))
                # Right column of the shop: JE_dString at x = 166
                # (game_menu.c), panel up to x = 310; the French label
                # "Ameliorer le vaisseau" overflowed it by 6 px.
                if w2 > MENUINT_LIMIT:
                    over[lang].append((w2, MENUINT_LIMIT, "shop label", k, v))

            # Gamepad settings (Godot, joystick_menu.gd): the help line
            # is drawn in the tiny font at x = 20; the capture prompt
            # is centered in the normal font in a 260 px box.
            if k == "joym_assign_desc" and v and px(v, SHEETS[2]) > DESC_LIMIT:
                over[lang].append((px(v, SHEETS[2]), DESC_LIMIT, "gamepad help", k, v))
            # The prompt takes two lines at most (joystick_menu.gd:wrap_lines).
            if (
                k == "joym_capture"
                and v
                and max(px(t, SHEETS[1]) for t in wrap_two_lines(v, SHEETS[1])) > CAPTURE_LIMIT
            ):
                over[lang].append((px(v, SHEETS[1]), CAPTURE_LIMIT, "gamepad capture prompt", k, v))

            role = roles.get(k)
            # Controls screen of the Godot version (controls_options.gd):
            # label in the tiny font at x = 20, key at x = 140.
            # "Cancelar captura de tecla" bit into "Escape".
            if k in CONTROLS_LABELS and v and px(v, SHEETS[2]) > CONTROLS_LIMIT:
                over[lang].append((px(v, SHEETS[2]), CONTROLS_LIMIT, "Controls screen", k, v))
            if not v or role is None:
                continue
            limit, font, what = (
                (DESC_LIMIT, DESC_FONT, "help strip") if role == "desc" else (LABEL_LIMIT, LABEL_FONT, "label")
            )
            fb = []
            w = px(v, SHEETS[font], fb)
            if w > limit:
                over[lang].append((w, limit, what, k, v))
            if fb:
                nodraw[lang].append((what, k, v, "".join(fb)))

    n_nodraw = sum(len(v) for v in nodraw.values())
    if n_nodraw:
        print(f"locale_widths: {n_nodraw} string(s) containing a character their font cannot draw:")
        for lang in sorted(nodraw):
            for what, k, _v, chars in sorted(nodraw[lang]):
                print(
                    f"  locale_{lang}.txt: {k} ({what}) contains {chars!r} — "
                    f"replaced by the bare letter, and font_sprite_id() "
                    f"warns on every frame"
                )

    n_over = sum(len(v) for v in over.values())
    if not quiet:
        print(
            f"locale_widths: {len(roles)} menu keys (frozen tables of the C version); "
            f"normal={SHEETS[1][0]} sprites, tiny={SHEETS[2][0]} sprites"
        )
    if n_over:
        print(f"locale_widths: {n_over} string(s) too wide:")
        for lang in sorted(over):
            for w, limit, what, k, v in sorted(over[lang], reverse=True):
                print(f"  [{lang}] {w:3}px > {limit} ({what})  {k} = {v!r}")
    if n_over or n_nodraw:
        return 1
    if not quiet:
        print("locale_widths: OK — everything fits in the planned space")
    return 0


if __name__ == "__main__":
    sys.exit(main())
