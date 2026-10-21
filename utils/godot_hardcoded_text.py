#!/usr/bin/env python3
"""Fail if a Godot script displays a hard-coded string instead of a locale key.

The Godot rewrite draws its menu texts with TyrianText; every text shown to the
player must come from data/locales (TyrianText.L). This check scans
rewrite/godot/scripts and rewrite/godot/editor for string literals passed
directly to the display helpers (`_add(...)`, `draw_font_hv*`, `text_shade`,
`out_text`, `dstring`, `.text = ...`) or held in a `[ "label", callable ]`
menu entry, and reports those containing letters.

Exit 0 when clean, 1 with one line per offending literal.

Usage:
    python3 utils/godot_hardcoded_text.py
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "rewrite" / "godot"
DIRS = ["scripts", "editor"]

# Calls that display their text argument.
DISPLAY_CALL = re.compile(
    r"(?:\b_add|draw_font_hv(?:_shadow|_full_shadow|_blend)?|text_shade|out_text\w*|dstring)\s*\("
)
TEXT_ASSIGN = re.compile(r"\.text\s*=\s*(.+)$")
STRING = re.compile(r'"((?:[^"\\]|\\.)*)"')
# Strings without a displayable word: symbols, pure formats, technical values.
LETTERS = re.compile(r"[A-Za-zÀ-ÿ]{2,}")
FORMAT_ONLY = re.compile(r"^(?:%[-0-9.]*[sd]|[\s:/.+()\[\]-])*$")


def drop_calls(expr: str, name: str) -> str:
    """Remove every `name(...)` call (balanced parentheses) from an expression."""
    out, i = [], 0
    pat = re.compile(r"\b" + re.escape(name) + r"\s*\(")
    while True:
        m = pat.search(expr, i)
        if not m:
            out.append(expr[i:])
            return "".join(out)
        out.append(expr[i : m.start()])
        j = call_args(expr, m.end() - 1)
        i = m.end() - 1 + len(j) + 1


def literals(expr: str):
    """String literals shown by an expression: not inside L(...) (locale
    keys), not a dictionary key (`d["k"]`, `d.get("k"...)`)."""
    expr = drop_calls(expr, "L")
    # Class and method names of ClassDB.class_call_static: never displayed.
    expr = drop_calls(expr, "class_call_static")
    expr = re.sub(r"(?<=[\w)\]])\[\s*\"(?:[^\"\\]|\\.)*\"\s*\]", "[]", expr)
    expr = re.sub(r"\.get\(\s*\"(?:[^\"\\]|\\.)*\"", ".get(", expr)
    for m in STRING.finditer(expr):
        yield m.group(1)


def call_args(line: str, start: int) -> str:
    """Text of the call starting at `start` up to its matching parenthesis."""
    depth = 0
    for i in range(start, len(line)):
        if line[i] == "(":
            depth += 1
        elif line[i] == ")":
            depth -= 1
            if depth == 0:
                return line[start:i]
    return line[start:]


def shown(lit: str) -> bool:
    if not LETTERS.search(lit) or FORMAT_ONLY.match(lit):
        return False
    # Resource paths and node names are not displayed.
    return not lit.startswith(("res://", "user://", "main:", "newsh")) and not lit.endswith(
        (".png", ".tscn", ".json", ".shp")
    )


def scan(path: Path):
    for no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        code = line
        if code.lstrip().startswith("#"):
            continue
        # Developer-only text (debug overlay), flagged as such.
        if "# locale-exempt:" in code:
            continue
        exprs = []
        for m in DISPLAY_CALL.finditer(code):
            exprs.append(call_args(code, m.end() - 1))
        m = TEXT_ASSIGN.search(code)
        if m:
            exprs.append(m.group(1))
        # Menu entry ["label", callable]: lowercase function. A
        # constant in the second column (KEY_PAUSE) is not a callable.
        m = re.match(r'\s*\[\s*("(?:[^"\\]|\\.)*"(?:\s*\+[^,]*)?)\s*,\s*(?:func\b|[a-z_]\w*\s*\])', code)
        if m:
            exprs.append(m.group(1))
        for e in exprs:
            for lit in literals(e):
                if shown(lit):
                    yield no, lit


REPO = ROOT.parent.parent
MODS_FOLDER = REPO / "rewrite" / "crates" / "tyrian-mods" / "src" / "folder.rs"
LOCALES = REPO / "data" / "locales"


def mod_error_problems() -> list:
    """Every mod loading error shown to the player (m.error, Mods menu)
    must have its translation key (m.error_key) in the six languages."""
    problems = []
    lines = MODS_FOLDER.read_text(encoding="utf-8").splitlines()
    keys = set()
    for i, line in enumerate(lines):
        if re.search(r"\bm\.error\s*=", line):
            m = re.search(r'm\.error_key\s*=\s*"([a-z0-9_]+)"', "\n".join(lines[i : i + 6]))
            if not m:
                problems.append(
                    f"{MODS_FOLDER.relative_to(REPO)}:{i + 1}: mod error without a translation key (m.error_key)"
                )
            else:
                keys.add(m.group(1))
    for f in sorted(LOCALES.glob("locale_??.txt")):
        defined = {
            line.split("=", 1)[0].strip()
            for line in f.read_text(encoding="utf-8").splitlines()
            if "=" in line and not line.startswith("#")
        }
        for k in sorted(keys - defined):
            problems.append(f'{f.relative_to(REPO)}: key "{k}" missing (mod error)')
    return problems


def keycode_string_problems() -> list:
    """Key name displayed by OS.get_keycode_string (always English) instead
    of TyrianControls.key_name (key_* locale keys): the Controls screen
    showed "Space", "Enter", "Escape" in all six languages."""
    problems = []
    for d in DIRS:
        for path in sorted((ROOT / d).rglob("*.gd")):
            for no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                code = line.split("#", 1)[0] if not line.lstrip().startswith("#") else ""
                if "get_keycode_string(" in code and "# locale-exempt:" not in line:
                    problems.append(
                        f"{path.relative_to(REPO)}:{no}: English key name "
                        "(OS.get_keycode_string): go through TyrianControls.key_name"
                    )
    return problems


def locale_table_problems() -> list:
    """Constant array one column of which goes through TyrianText.L(T[i][0]):
    each entry of that column must be a key defined in the locales.
    An English entry ("Arcade", "Timed Battle 1") was returned
    as is by L() and was displayed in English everywhere."""
    problems = []
    keys = {
        line.split("=", 1)[0].strip()
        for line in (LOCALES / "locale_en.txt").read_text(encoding="utf-8").splitlines()
        if "=" in line and not line.startswith("#")
    }
    for d in DIRS:
        for path in sorted((ROOT / d).rglob("*.gd")):
            text = path.read_text(encoding="utf-8")
            for name, col in set(re.findall(r"\bL\(\s*([A-Z_]+)\[[^\]]+\]\[(\d+)\]\s*\)", text)):
                m = re.search(r"^const " + name + r"\s*:=\s*\[(.*?)^\]", text, re.S | re.M)
                if not m:
                    continue
                for row_no, row in enumerate(re.findall(r"^\s*\[(.*)\],?\s*$", m.group(1), re.M)):
                    cells = STRING.findall(row)
                    idx = int(col)
                    if idx < len(cells) and cells[idx] not in keys:
                        problems.append(
                            f"{path.relative_to(REPO)}: {name}[{row_no}][{col}] = "
                            f'"{cells[idx]}" is not a locale key (displayed as is)'
                        )
    return problems


def main() -> int:
    bad = 0
    for d in DIRS:
        for path in sorted((ROOT / d).rglob("*.gd")):
            for no, lit in scan(path):
                print(f'{path.relative_to(REPO)}:{no}: hard-coded displayed text: "{lit}" (go through TyrianText.L)')
                bad += 1
    for p in mod_error_problems() + keycode_string_problems() + locale_table_problems():
        print(p)
        bad += 1
    if bad:
        print(f"{bad} hard-coded text(s)", file=sys.stderr)
        return 1
    print("godot_hardcoded_text: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
