#!/usr/bin/env python3
"""Refuses any text that the requested font cannot draw.

font_ascii[] (former fonthand.c, frozen in c_tables.json) is ONE table shared by the three fonts of
tyrian.shp, calibrated on the richest one. The others are shorter, and
blit_sprite* silently skips an out-of-bounds index: the character
disappears without an error, without a trace, and only in the cases that are
not tested.

  section 0  large   60 sprites  A-Z a-z and eight punctuation marks -- NO DIGIT
  section 1  small   85 sprites  + digits, punctuation, a few symbols
  section 2  tiny   127 sprites  + the 41 accented glyphs

Two real defects came out of this table: the accents cut off the menus
in five languages, and every damage >= 60 made invisible because the high tier
of the floating numbers drew digits in the large font.

This check is static: it does not launch the game. It checks that every locale
key drawn in the large font only contains characters that this
font can draw.
"""

import argparse
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "utils"))
from _shared import font_tables, shp_section_sprite_count, utf8_to_cp437_map  # noqa: E402

LOC = os.path.join(ROOT, "data", "locales")
DATA = os.path.join(ROOT, "data")

FONT_NAMES = {
    0: "large (FONT_SHAPES/large_font)",
    1: "small (SMALL_FONT_SHAPES/normal_font)",
    2: "tiny (TINY_FONT/small_font)",
}


def sprite_counts():
    """Number of sprites of the three fonts (sections 0-2 of tyrian.shp)."""
    return {s: shp_section_sprite_count(DATA, s) for s in (0, 1, 2)}


# font_ascii[] and cp437_unaccent() are re-read from fonthand.c by _shared
# (a single read for the three lints), never copied.


def locale_strings():
    """key -> {language: value} for all the locale files."""
    out = {}
    if not os.path.isdir(LOC):
        return out
    for fn in sorted(os.listdir(LOC)):
        if not (fn.startswith("locale_") and fn.endswith(".txt")):
            continue
        lang = fn[len("locale_") : -len(".txt")]
        for line in open(os.path.join(LOC, fn), encoding="utf-8", errors="replace"):
            if "=" in line and not line.lstrip().startswith("#"):
                k, _, v = line.partition("=")
                out.setdefault(k.strip(), {})[lang] = v.rstrip("\n")
    return out


def large_font_keys(loc=None):
    """All the locale keys drawn in the large font, collected from
    the call sites of the retired C version (tag c-final) and frozen in
    c_tables.json. This is THE list that locale_widths.py also consumes: a
    single source, so that a header can no longer pass one lint and fail the other."""
    from _shared import c_tables

    loc = locale_strings() if loc is None else loc
    return {k for k in c_tables()["large_font_keys"] if k in loc}


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.parse_args(argv)
    counts = sprite_counts()
    if not counts[0]:
        print("font_glyph_lint: data/tyrian.shp not found — check skipped")
        return 0
    fa, unacc = font_tables()
    conv = utf8_to_cp437_map()
    loc = locale_strings()

    def renderable(ch, font):
        """A character is rendered if its sprite exists, or if its ASCII
        fallback exists: UTF-8 -> CP437 conversion then sprite choice."""
        code = ord(ch)
        if code >= 0x80:
            code = conv.get(ch)
            if code is None:
                return False  # no conversion rule: raw bytes
        sid = fa[code]
        if sid >= 0 and sid < counts[font]:
            return True
        base = unacc.get(code)
        if base:
            sid = fa[ord(base)]
            return 0 <= sid < counts[font]
        return False

    keys = sorted(large_font_keys(loc))
    problems = []
    for key in keys:
        for lang, val in sorted(loc[key].items()):
            bad = sorted({c for c in val if c != " " and not renderable(c, 0)})
            if bad:
                problems.append(
                    "%s[%s] = %r: %s missing from the %s font" % (key, lang, val, " ".join(bad), FONT_NAMES[0])
                )

    print("fonts: %s" % ", ".join("section %d = %d sprites" % (k, v) for k, v in sorted(counts.items())))
    print("the large font has no digits (font_ascii sends them to 70-79)")
    print("%d keys drawn in the large font checked" % len(keys))
    print()
    if problems:
        print("%d problem(s):" % len(problems))
        for msg in problems:
            print("  " + msg)
    else:
        print("no text that cannot be drawn in the large font.")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
