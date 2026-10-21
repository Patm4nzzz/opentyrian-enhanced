"""
Tiny JSON5 → JSON preprocessor (Python side).

Mirrors src/json5.c: supports // line comments, /* block comments */ and
trailing commas before ] or }. Deliberately does NOT support unquoted
keys, single-quoted strings, or hex/NaN/Infinity numbers.

Usage:
    from _json5 import loads, load
    data = loads(open("foo.json").read())
    data = load("foo.json")
"""

import json as _json

_NORMAL, _STRING, _STRING_ESCAPE, _LINE_COMMENT, _BLOCK_COMMENT = range(5)


def _strip_comments(src: str) -> str:
    out = []
    state = _NORMAL
    i = 0
    n = len(src)
    while i < n:
        c = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        if state == _NORMAL:
            if c == '"':
                out.append(c)
                state = _STRING
            elif c == "/" and nxt == "/":
                i += 1
                state = _LINE_COMMENT
            elif c == "/" and nxt == "*":
                i += 1
                state = _BLOCK_COMMENT
            else:
                out.append(c)
        elif state == _STRING:
            out.append(c)
            if c == "\\":
                state = _STRING_ESCAPE
            elif c == '"':
                state = _NORMAL
        elif state == _STRING_ESCAPE:
            out.append(c)
            state = _STRING
        elif state == _LINE_COMMENT:
            if c == "\n":
                out.append(c)
                state = _NORMAL
        elif state == _BLOCK_COMMENT:
            if c == "*" and nxt == "/":
                i += 1
                state = _NORMAL
            elif c == "\n":
                out.append(c)
        i += 1
    return "".join(out)


def _strip_trailing_commas(src: str) -> str:
    out = []
    state = _NORMAL
    n = len(src)
    i = 0
    while i < n:
        c = src[i]
        if state == _NORMAL:
            if c == '"':
                out.append(c)
                state = _STRING
            elif c == ",":
                # peek ahead past whitespace
                j = i + 1
                while j < n and src[j] in " \t\n\r":
                    j += 1
                if j < n and src[j] in "]}":
                    # drop the comma
                    i += 1
                    continue
                out.append(c)
            else:
                out.append(c)
        elif state == _STRING:
            out.append(c)
            if c == "\\":
                state = _STRING_ESCAPE
            elif c == '"':
                state = _NORMAL
        elif state == _STRING_ESCAPE:
            out.append(c)
            state = _STRING
        i += 1
    return "".join(out)


# UTF-8 byte order mark (U+FEFF). Windows Notepad puts it
# at the head of any file it saves as UTF-8, and cJSON.c skips it
# (skip_utf8_bom): the game loads such a mod.json without flinching. json.loads,
# for its part, refuses the file with "Unexpected UTF-8 BOM" -- so the four
# modder tools rejected a file that the engine accepted.
_BOM = "\ufeff"


def loads(text: str):
    if isinstance(text, (bytes, bytearray)):
        text = text.decode("utf-8-sig")
    elif text.startswith(_BOM):
        text = text[len(_BOM) :]
    return _json.loads(_strip_trailing_commas(_strip_comments(text)))


def load(path_or_fp):
    """Accepts either a file path (str) or an open file handle, mirroring
    json.load()'s quirk of working with both in practice. A leading UTF-8
    BOM is tolerated either way, like the C loader does."""
    if hasattr(path_or_fp, "read"):
        return loads(path_or_fp.read())
    with open(path_or_fp, encoding="utf-8-sig") as f:
        return loads(f.read())


def dumps_compact(data):
    """Pretty-print top-level keys but keep leaf dicts on one line.
    Writing mod.json with json.dump(indent=2) inflates every ship /
    weapon entry into 10+ lines — tedious to scroll. This keeps the
    outer shape readable while collapsing repeated item dicts.

    Rules:
      - top-level dict: keys on their own line, 2-space indent
      - array of leaf dicts (no nested dicts/arrays): each item
        rendered on one line
      - any other nested value: dumped via json.dumps with compact
        separators on one line
    """
    lines = ["{"]
    keys = list(data.keys())
    for k, v in data.items():
        comma = "," if k != keys[-1] else ""
        if isinstance(v, list) and v and all(isinstance(it, dict) for it in v):

            def _is_leaf(d):
                return not any(isinstance(vv, (dict, list)) for vv in d.values())

            lines.append(f'  "{k}": [')
            for i, item in enumerate(v):
                item_comma = "," if i < len(v) - 1 else ""
                if _is_leaf(item):
                    # Leaf dict (ship, shield...) — one per line.
                    lines.append(f"    {_json.dumps(item, separators=(', ', ': '))}{item_comma}")
                else:
                    # Nested dict (weapon with front[]/rear[] levels) —
                    # fully expanded; a one-line 1000-char weapon entry
                    # is unreadable and uneditable for the modder.
                    block = _json.dumps(item, indent=2)
                    block = "\n".join("    " + ln for ln in block.splitlines())
                    lines.append(f"{block}{item_comma}")
            lines.append(f"  ]{comma}")
        elif isinstance(v, (dict, list)):
            lines.append(f'  "{k}": {_json.dumps(v, separators=(", ", ": "))}{comma}')
        else:
            lines.append(f'  "{k}": {_json.dumps(v)}{comma}')
    lines.append("}")
    return "\n".join(lines)
