#!/usr/bin/env python3
"""locale_glyphs.py -- is every locale character displayable?

Cross-checks three sources of truth of the repository, without launching the game:

  1. src/locale.c      the UTF-8 -> CP437 conversion
  2. src/fonthand.c    font_ascii[] (CP437 -> sprite index) and
                       cp437_unaccent() (fallback when the font lacks the glyph)
  3. data/tyrian.shp   the three font sheets and their real size

Fails if a character used in a locale file:
  - has no conversion rule in locale.c, or
  - has neither a glyph in the small font NOR an ASCII fallback.

The small font (85 sprites) is the reference: it renders the
menu labels, and that is where the accented glyphs are missing.

    python3 utils/locale_glyphs.py            # report + exit code
    python3 utils/locale_glyphs.py --quiet    # exit code only
"""

import argparse
import collections
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "utils"))
from _shared import font_tables, shp_section_sprite_count, utf8_to_cp437_map  # noqa: E402

SMALL_FONT_SECTION = 1  # SMALL_FONT_SHAPES -- the menu label font


# The three tables (locale.c conversion, font_ascii[] and
# cp437_unaccent() of fonthand.c) are read by _shared: this script had
# its own copy, locale_widths.py another, font_glyph_lint.py a
# third, and they had diverged (A04/A05 of the utils audit).


def sheet_size(section):
    return shp_section_sprite_count(os.path.join(ROOT, "data"), section)


def locale_chars():
    """Non-ASCII characters actually present, per language."""
    used = {}
    loc_dir = os.path.join(ROOT, "data/locales")
    for fn in sorted(os.listdir(loc_dir)):
        if not fn.startswith("locale_") or not fn.endswith(".txt"):
            continue
        lang = fn[len("locale_") : -len(".txt")]
        counter = used.setdefault(lang, collections.Counter())
        with open(os.path.join(loc_dir, fn), encoding="utf-8", errors="replace") as f:
            for line in f:
                if line.startswith("#") or "=" not in line:
                    continue
                for ch in line.partition("=")[2]:
                    if ord(ch) > 127:
                        counter[ch] += 1
    return used


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--quiet", action="store_true", help="exit code only, no report")
    args = ap.parse_args(argv)
    quiet = args.quiet
    conv = utf8_to_cp437_map()
    fa, fallback = font_tables()
    small = sheet_size(SMALL_FONT_SECTION)
    if small == 0:
        print("locale_glyphs: data/tyrian.shp not found — check skipped")
        return 0

    problems = []
    for lang, chars in sorted(locale_chars().items()):
        for ch, n in sorted(chars.items(), key=lambda kv: -kv[1]):
            cp = conv.get(ch)
            if cp is None:
                problems.append((lang, ch, n, "no conversion rule in locale.c"))
                continue
            if cp == 0x20:
                continue  # space: advances 6 px, no glyph
            idx = fa[cp] if cp < 256 else -1
            if 0 <= idx < small:
                continue  # glyph present in the small font
            if cp in fallback:
                continue  # ASCII fallback available
            problems.append(
                (
                    lang,
                    ch,
                    n,
                    f"CP437 0x{cp:02X} -> sprite {idx}, outside the {small} "
                    f"of the small font, and no fallback in cp437_unaccent()",
                )
            )

    if not quiet:
        print(f"locale_glyphs: small font = {small} sprites, {len(conv)} conversions, {len(fallback)} ASCII fallbacks")
    if problems:
        print(f"locale_glyphs: {len(problems)} character(s) that cannot be displayed:")
        for lang, ch, n, why in problems:
            print(f"  [{lang}] {ch!r} ×{n} — {why}")
        return 1
    if not quiet:
        print("locale_glyphs: OK — every locale character can be displayed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
