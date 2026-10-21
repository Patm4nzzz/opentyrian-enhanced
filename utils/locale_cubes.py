#!/usr/bin/env python3
"""Datacubes: extraction from cubetxt<N>.dat and check of the translations.

Why this tool exists
--------------------
The ~112 datacubes tell the story of the game. Only episode 1 had locale
keys; episodes 2 to 5 (114 cubes, 85,000 characters) could be translated
by nobody, because nobody could READ them: they
sleep in encrypted pascal strings inside cubetxt<N>.dat.

    python3 utils/locale_cubes.py dump --all > /tmp/cubes.txt
    python3 utils/locale_cubes.py dump --episode 2
    python3 utils/locale_cubes.py check          # checks all the locales

Rendering constraints checked by `check` (game_menu.c load_cube):
  * title    <= 80 characters  (char title[81])
  * header   <= 12 characters  (char header[13])
  * body     <= 90 lines after wrapping  (char text[90][36])
  * no WORD exceeds 35 characters or 150 px in the TINY font:
    the wrapper does a `break` on such a word and SILENTLY SWALLOWS the end
    of the paragraph. This is the main trap of compound-word languages.
"""

import argparse
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
DATA = os.path.join(ROOT, "data")
LOC = os.path.join(DATA, "locales")

MAX_TITLE, MAX_HEADER, MAX_LINES = 80, 12, 90
MAX_WORD_CHARS, MAX_LINE_WIDTH = 35, 150


KEY = bytes([204, 129, 63, 255, 71, 19, 25, 62, 1, 99])


def decrypt(b):
    """Encrypted pascal string of Tyrian (same scheme as shops/levels)."""
    s = bytearray(b)
    for i in range(len(s) - 1, -1, -1):
        s[i] ^= KEY[i % 10]
        if i:
            s[i] ^= s[i - 1]
    return bytes(s)


def read_strings(path):
    d = open(path, "rb").read()
    off, rows = 0, []
    while off < len(d):
        n = d[off]
        off += 1
        if off + n > len(d):
            break
        rows.append(decrypt(d[off : off + n]).decode("cp437"))
        off += n
    return rows


def parse_cubes(path):
    """Replays game_menu.c:load_cube for EVERY addressable index.

    The detail that almost threw everything off: load_cube looks for its target by
    counting the lines that start with "*", then reads unconditionally
    the next two as title and header. cubetxt4.dat opens on
    FOUR markers in a row ("*01 02", "*02 02", "*03 02",
    "*04 02   Desert world..."): a naive split merges the first
    three and shifts the whole numbering of episode 4 by three ranks.
    The keys would no longer designate the right cube.

    The last marker of a file is a sentinel: the body loop
    reads until a "*" and *crashes* (fread_die -> exit) if the file
    ends before. The valid indexes therefore go from 1 to (nb_markers - 1).
    """
    rows = read_strings(path)
    stars = sum(1 for r in rows if r.startswith("*"))
    cubes = []
    for index in range(1, stars):  # the final sentinel is not a cube
        i, left, buf = 0, index, ""
        while left > 0 and i < len(rows):
            buf = rows[i]
            i += 1
            if buf.startswith("*"):
                left -= 1
        m = re.search(r"-?\d+", buf[4:])
        face = int(m.group()) if m else 1
        title = rows[i] if i < len(rows) else ""
        header = rows[i + 1] if i + 1 < len(rows) else ""
        body, j = [], i + 2
        while j < len(rows) and not rows[j].startswith("*"):
            body.append(rows[j])
            j += 1
        cubes.append((face, title, header, body))
    # Slots with NEITHER title NOR header are not cubes: the reserve
    # markers at the head of episode 4, and the developer note
    # that closes the same file ("12   *: Beppo - Pirate from Torm - stupid").
    # They are kept at their rank -- the numbering belongs to the game -- but
    # they are not offered for translation.
    return cubes


def is_real(c):
    """An addressable index is not necessarily a displayable cube.

    cubetxt4.dat opens on three reserve markers. Since load_cube reads
    the two lines following the marker *without examining them*, indexes 1
    to 3 return a sliding window of markers: title "*02 02",
    header "*03 02". The first real cube of episode 4 is no. 4, and
    that is indeed where the "]?" list of the level files starts (4 to
    67, 14 excepted). A title that starts with "*" is the signature
    of this artifact.
    """
    return bool((c[1].strip() or c[2].strip()) and not c[1].startswith("*"))


# Two touch-ups, and only two, separate the extracted text from the raw
# data. They live here so that "dump --all" stays reproducible: a
# correction made by hand in the file would be lost at the next
# regeneration.
#
#   dc_5_12_header: "Captain's Log" is 13 characters for a 12-character field
#     (char header[13], NUL included). The game displays "Captain's Lo".
#   dc_4_44_body  : "103°" -- the font has no degree glyph
#     (font_ascii[0xF8] == -1), so the character is not displayed at all.
FIXUPS = {
    "dc_5_12_header": ("Captain's Log", "Ship's Log"),
}
FIXUP_CHARS = {"\u00b0": " degrees"}


def apply_fixups(key, value):
    old_new = FIXUPS.get(key)
    if old_new and value == old_new[0]:
        value = old_new[1]
    for ch, rep in FIXUP_CHARS.items():
        value = value.replace(ch, rep)
    return value


def emit(ep, cubes, out=sys.stdout):
    for n, c in enumerate(cubes, 1):
        if not is_real(c):
            continue
        _face, title, header, body = c
        k = "dc_%d_%d_" % (ep, n)
        out.write("%stitle=%s\n" % (k, apply_fixups(k + "title", title)))
        out.write("%sheader=%s\n" % (k, apply_fixups(k + "header", header)))
        out.write("%sbody=`%s\n`\n\n" % (k, apply_fixups(k + "body", "\n".join(body).rstrip("\n"))))


# ---------------------------------------------------------------- controle --


def _tiny():
    """TINY font sheet (section 2 of the SHP) + measurer, borrowed from
    locale_widths.py. None if tyrian.shp is absent: the width
    check is then skipped rather than returning a guessed verdict."""
    try:
        sys.path.insert(0, HERE)
        import locale_widths as lw

        tiny = lw.sheets()[2]
        if tiny[0] == 0:
            return None
        return lambda t: lw.px(t, tiny)
    except Exception:
        return None


def wrap(body, px):
    """Replays EXACTLY game_menu.c:load_cube -> (nb_lines, swallowed_words).

    The detail that matters: a word that is too long does not cause a line
    wrap, it does a `break` -- the end of the source line is lost without a
    word of warning. It is the only defect of this function that is
    invisible in the game.
    """
    line = chars = width = 0
    eaten = []
    for src in body.split("\n"):
        if src.startswith("*"):
            break
        if src.strip("\r") == "":
            line += 4 if chars == 0 else 1
            chars = width = 0
            continue
        words = src.split(" ")
        for wi, word in enumerate(words):
            wc = len(word)
            ww = px(word) if px else 0
            if wc > MAX_WORD_CHARS or (px and ww > MAX_LINE_WIDTH):
                eaten.append((word, " ".join(words[wi:])))
                break
            chars += wc + 1
            width += ww + 6
            if chars > MAX_WORD_CHARS or (px and width > MAX_LINE_WIDTH):
                line += 1
                chars, width = wc, ww
    return line + 1, eaten


def parse_locale(path):
    """Reads a locale file (multiline backtick syntax) -> dict."""
    out, key, buf = {}, None, None
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    if lines and lines[-1] == "":
        lines.pop()  # the final '\n' is not a line
    for line in lines:
        line = line.rstrip("\r")
        if buf is not None:
            if line.endswith("`"):
                buf.append(line[:-1])
                out[key] = "\n".join(buf).rstrip("\n")
                key, buf = None, None
            else:
                buf.append(line)
            continue
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        if v.startswith("`"):
            if v.endswith("`") and len(v) > 1:
                out[k] = v[1:-1]
            else:
                key, buf = k, [v[1:]]
        else:
            out[k] = v
    return out


def check():
    en = parse_locale(os.path.join(LOC, "locale_en_cubes.txt"))
    px = _tiny()
    errs, warns, bad, counts = [], [], {}, {}
    # The conversion table is that of _shared (read from locale.c), the
    # same as for locale_glyphs and locale_widths. The old copy in
    # locale_widths ignored the 3-byte block of locale.c and this check
    # refused "’" and "–" which the game draws perfectly well.
    try:
        sys.path.insert(0, HERE)
        from _shared import utf8_to_cp437_map

        CONV = utf8_to_cp437_map()
    except Exception:
        CONV = None

    # English is the REFERENCE: it must cover exactly the cubes
    # present in the data. An extra key points to a phantom cube, a
    # missing key to a cube that nobody will be able to translate. Keys are compared,
    # not values: a few editorial touch-ups are legitimate
    # (dc_5_12_header did not fit in its 12-byte field).
    want = set()
    for ep in range(1, 6):
        f = os.path.join(DATA, "cubetxt%d.dat" % ep)
        if not os.path.exists(f):
            want = None
            break
        for n, c in enumerate(parse_cubes(f), 1):
            if not is_real(c):
                continue
            for fld in ("title", "header", "body"):
                want.add("dc_%d_%d_%s" % (ep, n, fld))
    if want is not None:
        for k in sorted(want - set(en)):
            errs.append("locale_en_cubes.txt: %s missing (cube present in the data, not extracted)" % k)
        for k in sorted(set(en) - want):
            errs.append("locale_en_cubes.txt: %s is extra (no matching cube in the data)" % k)

    for path in sorted(os.listdir(LOC)):
        if not path.endswith("_cubes.txt") or path.startswith("_"):
            continue
        lang = path[len("locale_") : -len("_cubes.txt")]
        d = parse_locale(os.path.join(LOC, path))
        if lang != "en":
            counts[lang] = len(d)

        # Phantom translation: beyond half of the keys identical to
        # English, it is a placeholder. Real translations run
        # at 8% (proper names); Italian was at 79%.
        if lang != "en" and len(d) >= 20:
            same = sum(1 for k, v in d.items() if k in en and v.rstrip() == en[k].rstrip())
            if same * 100 >= len(d) * 50:
                errs.append(
                    "%s: %d of %d keys are word for word the English "
                    "(placeholder, not a translation)" % (path, same, len(d))
                )

        for k, v in d.items():
            # A character outside the conversion table is not drawn: it
            # disappears silently. This is the classic trap of translations
            # (typographic quotes, em dashes, curly
            # apostrophes) -- invisible as long as one does not look at the screen.
            for ch in v:
                if ord(ch) < 127 or ch == "\n":
                    continue
                if CONV is not None and ch not in CONV:
                    bad.setdefault((path, ch), 0)
                    bad[(path, ch)] += 1

            if k.endswith("_title") and len(v) > MAX_TITLE:
                errs.append("%s: %s is %d chars (max %d)" % (path, k, len(v), MAX_TITLE))
            elif k.endswith("_header") and len(v) > MAX_HEADER:
                errs.append("%s: %s is %d chars (max %d)" % (path, k, len(v), MAX_HEADER))
            elif k.endswith("_body"):
                nlines, eaten = wrap(v, px)
                for word, rest in eaten:
                    errs.append(
                        "%s: %s -- the word %r exceeds the line; the "
                        "wrapper gives up and LOSES %r" % (path, k, word, rest[:60])
                    )
                if nlines > MAX_LINES:
                    errs.append(
                        "%s: %s takes %d lines (max %d) -> the end is cut off on screen" % (path, k, nlines, MAX_LINES)
                    )

    for (path, ch), n in sorted(bad.items()):
        errs.append(
            "%s: the character %r (U+%04X) appears %d times and "
            "has no glyph -- it would be invisible" % (path, ch, ord(ch), n)
        )

    for lang, n in sorted(counts.items()):
        if n < len(en):
            warns.append(
                "locale_%s_cubes.txt: %d keys out of %d -- %d key(s) "
                "without translation, engine falls back to English" % (lang, n, len(en), len(en) - n)
            )

    for w in warns:
        print("WARNING: %s" % w)
    for e in errs:
        print("ERROR: %s" % e)
    print("-- %d error(s), %d warning(s)" % (len(errs), len(warns)))
    return 1 if errs else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    d = sub.add_parser("dump", help="extract the cubes into locale keys")
    d.add_argument("--episode", type=int)
    d.add_argument("--all", action="store_true")
    sub.add_parser("check", help="check the _cubes.txt files")
    a = ap.parse_args()

    if a.cmd == "check":
        return check()

    eps = range(1, 6) if a.all else [a.episode]
    if eps == [None]:
        ap.error("give --episode N or --all")
    for ep in eps:
        p = os.path.join(DATA, "cubetxt%d.dat" % ep)
        if not os.path.exists(p):
            print("# cubetxt%d.dat missing" % ep, file=sys.stderr)
            continue
        emit(ep, parse_cubes(p))
    return 0


if __name__ == "__main__":
    sys.exit(main())
