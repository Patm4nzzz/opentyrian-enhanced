#!/usr/bin/env python3
"""Verify all locale files share the same set of keys.

Uses locale_en.txt as the reference. Any other locale missing or
adding keys fails the check.

Usage:
    python3 utils/locale_check.py [--strict]

Exit 0 on success, 1 on missing/extra keys. --strict also fails on
empty values or lines that don't follow `key=value` format.
"""

import argparse
import re
import sys
from collections import OrderedDict
from pathlib import Path

LOCALES_DIR = Path(__file__).resolve().parent.parent / "data" / "locales"
REFERENCE = "locale_en.txt"


# Form faults found in --strict mode (empty value, line without "=").
# They were only printed: the option promised a failure and never
# produced one. main() counts this list in the verdict.
STRICT_PROBLEMS = []


def load_keys(path: Path, strict: bool = False) -> "OrderedDict[str, str]":
    """Return an ordered dict of key->value. Ignores blank/comment lines.

    Datacube files use a backtick multi-line body syntax; every line
    inside such a body is part of the value, not a malformed entry."""
    out: OrderedDict[str, str] = OrderedDict()
    in_body = False
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if in_body:
            if raw.rstrip().endswith("`"):
                in_body = False
            continue
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            if strict:
                msg = f"{path.name}:{lineno} not a key=value line: {line!r}"
                print(msg, file=sys.stderr)
                STRICT_PROBLEMS.append(msg)
            continue
        key, _, val = line.partition("=")
        key = key.strip()
        val = val.strip()
        if val.startswith("`") and not (len(val) > 1 and val.endswith("`")):
            in_body = True
        if strict and not val:
            msg = f"{path.name}:{lineno} empty value for key {key!r}"
            print(msg, file=sys.stderr)
            STRICT_PROBLEMS.append(msg)
        out[key] = val
    return out


def check_group(ref_name: str, pattern: str, strict: bool, allow_missing: bool = False) -> bool:
    """Check every locale matching `pattern` against `ref_name` as reference.
    Returns True if any locale drifted."""
    ref_path = LOCALES_DIR / ref_name
    if not ref_path.exists():
        print(f"error: reference locale missing: {ref_path}", file=sys.stderr)
        return True
    ref_keys = set(load_keys(ref_path, strict).keys())
    drift = False
    for loc in sorted(LOCALES_DIR.glob(pattern)):
        if loc.name == ref_name:
            continue
        keys = set(load_keys(loc, strict).keys())
        missing = ref_keys - keys
        extra = keys - ref_keys
        if missing and allow_missing:
            # Datacubes fall back KEY BY KEY: game_menu.c only applies
            # the override if the key exists, otherwise it keeps the text of
            # cubetxt<N>.dat. A missing key is therefore a cube not yet
            # translated, not a drift -- locale_cubes.py keeps
            # that count. An EXTRA key remains a fault: it designates
            # no cube and will never be displayed.
            print(f"{loc.name}: {len(missing)} untranslated keys (engine falls back to English)")
        elif missing:
            print(f"{loc.name}: missing {len(missing)} keys:", file=sys.stderr)
            for k in sorted(missing):
                print(f"  - {k}", file=sys.stderr)
            drift = True
        if extra:
            print(f"{loc.name}: {len(extra)} extra keys not in {ref_name}:", file=sys.stderr)
            for k in sorted(extra):
                print(f"  + {k}", file=sys.stderr)
            drift = True
        if not missing and not extra:
            print(f"{loc.name}: OK ({len(keys)} keys)")
    return drift


def dest_capacities():
    """Size of the destination C buffer for each locale key.

    The old locale_override() copied into a fixed-size char array and
    silently truncated beyond it. The capacities, collected from the retired C
    version (tag c-final), are frozen in utils/c_tables.json: they
    remain a prudent bound for the same keys in the Godot client.

    Returns {key: capacity_in_bytes_including_NUL}.
    """
    from _shared import c_tables

    return dict(c_tables()["buffer_capacities"])


def cp437_len(text: str) -> int:
    """Bytes of the value once loaded: locale.c converts each
    value to CP437 at load time (utf8_to_cp437), before any copy into
    a buffer -- "é" takes 1 byte, "ß" 2 (ss), "…" 3 (...)."""
    n = 0
    for ch in text:
        if ord(ch) < 0x80:
            n += 1
        else:
            n += MULTI_BYTE.get(ch, 1)
    return n


# locale.c conversions that do not write exactly one byte.
MULTI_BYTE = {"ß": 2, "…": 3, "°": 0}


def check_capacities() -> bool:
    """Any value that does not fit in its buffer is a defect: the
    string is cut at the byte, often in the middle of a word."""
    caps = dest_capacities()
    if not caps:
        print(
            "warning: no destination capacity resolved — did the shape of helptext.c change?",
            file=sys.stderr,
        )
        return False
    drift = False
    for loc in sorted(LOCALES_DIR.glob("locale_*.txt")):
        if "_cubes" in loc.name or "_additional" in loc.name:
            continue
        for k, v in load_keys(loc).items():
            cap = caps.get(k)
            if cap is None:
                continue
            n = cp437_len(v)
            if n >= cap:
                print(
                    f"{loc.name}: {k} = {v!r} is {n} CP437 bytes, buffer of "
                    f"{cap} (NUL included) — truncated to {v[: cap - 1]!r}",
                    file=sys.stderr,
                )
                drift = True
    if not drift:
        print(f"capacities: OK ({len(caps)} keys backed by a C buffer)")
    return drift


FORMAT_CONV = re.compile(r"%[-+ #0\']*[0-9*]*(?:\.[0-9*]+)?(?:hh|h|ll|l|z|j|t|L)?([diouxXeEfFgGaAcspn%])")


def format_keys():
    """Locale keys passed to printf AS A FORMAT (retired C version,
    tag c-final; frozen in c_tables.json). Almost everywhere a locale
    value is an ARGUMENT and the translator cannot break anything; when it
    is the format itself, a `%d` written in place of a `%s` makes it read an
    address as an integer."""
    from _shared import c_tables

    return set(c_tables()["printf_format_keys"])


def check_format_strings() -> bool:
    keys = format_keys()
    if not keys:
        return False
    en = load_keys(LOCALES_DIR / REFERENCE)
    drift = False
    for k in sorted(keys):
        ref = FORMAT_CONV.findall(en.get(k, ""))
        for loc in sorted(LOCALES_DIR.glob("locale_*.txt")):
            if "_cubes" in loc.name or "_additional" in loc.name or loc.name == REFERENCE:
                continue
            got = FORMAT_CONV.findall(load_keys(loc).get(k, ""))
            if got != ref:
                print(
                    f"{loc.name}: {k} is used as a printf FORMAT; "
                    f"expected conversions {ref}, found {got} — "
                    f"a mismatch here crashes the game in this language",
                    file=sys.stderr,
                )
                drift = True
    if not drift:
        print(f"formats: OK ({len(keys)} key(s) used as a printf format)")
    return drift


GODOT_DIR = LOCALES_DIR.parent.parent / "rewrite" / "godot"
GODOT_KEY_CALL = re.compile(r'\bL\(\s*"([^"]+)"\s*[,)]')


def check_keys_exist() -> bool:
    """Every TyrianText.L("...") key cited in the Godot scripts must exist
    in locale_en.txt: L() reports nothing, it returns the key itself, so
    the screen would display its own identifier."""
    used = {}
    for path in sorted(GODOT_DIR.rglob("*.gd")):
        if "addons" in path.parts:
            continue
        txt = path.read_text(encoding="utf-8", errors="replace")
        for m in GODOT_KEY_CALL.finditer(txt):
            used.setdefault(m.group(1), path.name)
    have = set(load_keys(LOCALES_DIR / REFERENCE))
    missing = sorted(k for k in used if k not in have)
    for k in missing:
        print(
            f'{REFERENCE}: the key "{k}" is used by {used[k]} but '
            f"exists nowhere — L() will return the key name, and "
            f"the screen will display this identifier",
            file=sys.stderr,
        )
    if not missing:
        print(f"L() keys: OK ({len(used)} key(s) cited in the Godot scripts, all defined)")
    return bool(missing)


# Markers that the engine interprets: printf formats (the real ones: "50%+"
# is not one, hence the requirement of an attached conversion letter), literal
# line endings "\n", and "~" which toggles the highlight.
PLACEHOLDER = re.compile(r"%(?:\d+\$)?[-+#0]?\d*(?:\.\d+)?(?:hh|h|ll|l|z)?[diouxXsc]|%%")


def _markers(v: str):
    return (sorted(PLACEHOLDER.findall(v)), v.count("\\n"), v.count("~") % 2)


def check_placeholders() -> bool:
    """Each translation keeps the printf formats, the "\\n" and the parity
    of the "~" of the English: a lost %s shows a hole, an extra %d reads
    anything on the C stack, an orphan "~" leaves the end of the text
    highlighted."""
    ref = load_keys(LOCALES_DIR / REFERENCE)
    drift = False
    for loc in sorted(LOCALES_DIR.glob("locale_??.txt")):
        if loc.name == REFERENCE:
            continue
        for k, v in load_keys(loc).items():
            if k not in ref:
                continue
            a, b = _markers(ref[k]), _markers(v)
            if a[0] != b[0]:
                print(f"{loc.name}: {k} formats {a[0]} -> {b[0]}", file=sys.stderr)
                drift = True
            if a[1] != b[1]:
                print(f"{loc.name}: {k} {a[1]} \\n in English, {b[1]} here", file=sys.stderr)
                drift = True
            if b[2]:
                print(f'{loc.name}: {k} odd number of "~"', file=sys.stderr)
                drift = True
    if not drift:
        print("markers: OK (formats, \\n and ~ identical to English)")
    return drift


# Values identical to the English accepted after review (proper names,
# shop item names, codes typed on the title screen, words common to
# both languages). One line per key: "<language> <key>" or "* <key>".
SAME_AS_ENGLISH = Path(__file__).resolve().parent / "locale_same_as_english.txt"


def _same_allowed():
    allowed = set()
    if SAME_AS_ENGLISH.exists():
        for line in SAME_AS_ENGLISH.read_text(encoding="utf-8").splitlines():
            line = line.split("#", 1)[0].strip()
            if line:
                lang, _, key = line.partition(" ")
                allowed.add((lang, key.strip()))
    return allowed


def check_english_left() -> bool:
    """A translated value identical to the English (at least one word of three
    letters) is forgotten English, unless it is reviewed and listed in
    locale_same_as_english.txt."""
    allowed = _same_allowed()
    drift = False
    from locale_cubes import parse_locale  # complete multiline bodies

    for ref_name, pattern in (
        (REFERENCE, "locale_??.txt"),
        ("locale_en_cubes.txt", "locale_??_cubes.txt"),
    ):
        read = parse_locale if "_cubes" in ref_name else load_keys
        ref = read(LOCALES_DIR / ref_name)
        for loc in sorted(LOCALES_DIR.glob(pattern)):
            if loc.name == ref_name:
                continue
            lang = loc.name[len("locale_") : len("locale_") + 2]
            for k, v in read(loc).items():
                if v != ref.get(k) or not re.search(r"[A-Za-z]{3,}", v):
                    continue
                if ("*", k) in allowed or (lang, k) in allowed:
                    continue
                print(
                    f"{loc.name}: {k} = {v[:50]!r} identical to English "
                    f"(translate it, or add it to {SAME_AS_ENGLISH.name})",
                    file=sys.stderr,
                )
                drift = True
    if not drift:
        print("English left behind: OK")
    return drift


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument(
        "--strict",
        action="store_true",
        help="also fail on empty values and lines that are not key=value",
    )
    args = ap.parse_args(argv)
    strict = args.strict
    del STRICT_PROBLEMS[:]
    # Main catalog (locale_en.txt reference) — exclude cubes/additional
    # overlays, which are checked as a separate group.
    main_drift = False
    ref_path = LOCALES_DIR / REFERENCE
    if not ref_path.exists():
        print(f"error: reference locale missing: {ref_path}", file=sys.stderr)
        return 1
    ref_keys = set(load_keys(ref_path, strict).keys())
    for loc in sorted(LOCALES_DIR.glob("locale_*.txt")):
        if loc.name == REFERENCE:
            continue
        if "_cubes" in loc.name or "_additional" in loc.name:
            continue
        keys = set(load_keys(loc, strict).keys())
        missing = ref_keys - keys
        extra = keys - ref_keys
        if missing:
            print(f"{loc.name}: missing {len(missing)} keys:", file=sys.stderr)
            for k in sorted(missing):
                print(f"  - {k}", file=sys.stderr)
            main_drift = True
        if extra:
            print(f"{loc.name}: {len(extra)} extra keys not in {REFERENCE}:", file=sys.stderr)
            for k in sorted(extra):
                print(f"  + {k}", file=sys.stderr)
            main_drift = True
        if not missing and not extra:
            print(f"{loc.name}: OK ({len(keys)} keys)")

    # Cubes overlay — locale_en_cubes.txt is the reference for the _cubes
    # group. Previously skipped; FR cubes drifted quietly under that gap.
    cubes_drift = check_group("locale_en_cubes.txt", "locale_*_cubes.txt", strict)

    cap_drift = False

    buf_drift = check_capacities()
    fmt_drift = check_format_strings()
    key_drift = check_keys_exist()
    ph_drift = check_placeholders()
    en_drift = check_english_left()

    strict_drift = False
    if strict and STRICT_PROBLEMS:
        # load_keys is called several times per file; each fault
        # is counted only once.
        n = len(set(STRICT_PROBLEMS))
        print(f"--strict: {n} malformed line(s) / empty value(s)", file=sys.stderr)
        strict_drift = True

    return (
        1
        if (
            main_drift
            or cubes_drift
            or cap_drift
            or buf_drift
            or fmt_drift
            or key_drift
            or strict_drift
            or ph_drift
            or en_drift
        )
        else 0
    )


if __name__ == "__main__":
    sys.exit(main())
