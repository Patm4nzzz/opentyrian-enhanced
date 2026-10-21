#!/usr/bin/env python3
"""ref_stock_catalog.py — produce a browsable HTML reference of every
stock Tyrian weapon port (name, cost, power use, first power levels).

Intended as a modder's lookup: find a stock item that feels close to
what you want, copy its name into `"base": "<name>"` in your mod.json,
then only override what differs. Saves hunting through tyrian.hdt or
re-exporting to JSON by hand.

Pulls from:
  - data/tyrian.hdt — via asset_export_hdt.open_item_table(), the same
    reader JE_loadItemDat() mirrors (the table starts at episode1DataLoc,
    not at offset 4)
  - data/sprite_manifest.json (for sprite name resolution)

Output: data/reference/stock_catalog.html (untracked reference artifact)

Usage:
    python3 utils/ref_stock_catalog.py [--data-dir data] [--out FILE]
"""

from __future__ import annotations

import argparse
import html
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _sprite_manifest import load_manifest, resolve_index  # noqa: E402
from asset_export_hdt import (  # noqa: E402
    PORT_NUM,
    WEAP_END1,
    WEAP_NUM,
    WEAP_START2,
    open_item_table,
    read_u16,
    read_weapon,
)
from asset_export_mod import read_port  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUT = ROOT / "data" / "reference" / "stock_catalog.html"


def read_hdt(data_dir):
    """Parse weapons[] and weaponPort[] from tyrian.hdt. Returns
    (weapons_by_id, ports_list): weapons_by_id maps the weapon index
    (0..818, 1000..1818 — the two HDT banks) to a raw record from
    asset_export_hdt.read_weapon(); ports_list[i] is weaponPort[i]
    (0..PORT_NUM) from asset_export_mod.read_port()."""
    f, _path = open_item_table(data_dir, 1)
    with f:
        read_u16(f, 7)  # itemNum[7], consumed
        weapons = {}
        for i in range(0, WEAP_END1 + 1):
            weapons[i] = read_weapon(f)
        for i in range(WEAP_START2, WEAP_NUM + 1):
            weapons[i] = read_weapon(f)
        ports = [read_port(f) for _ in range(PORT_NUM + 1)]
    return weapons, ports


def sprite_name_for(index: int, manifest: dict) -> str:
    """Reverse-lookup: index -> human name from sprite_manifest.json."""
    if index == 0:
        return "(none)"
    name = resolve_index(manifest, index, ["shots", "ships", "pickups"])
    return name if name else f"#{index}"


def fmt_list(lst, fmt="%d"):
    return "[" + ", ".join(fmt % v for v in lst if v != 0) + "]"


def render_weapon_row(port_id: int, port: dict, weapons: dict, manifest: dict) -> str:
    if not port["name"]:
        return ""
    rows = []
    # Show first 3 power levels' key stats as a condensed summary
    levels = []
    for level, wid in enumerate(port["op"][0][:3]):
        if wid == 0 or wid not in weapons:
            continue
        w = weapons[wid]
        sprite = sprite_name_for(w["sg"][0], manifest) if w["sg"][0] else "-"
        levels.append(
            f"<div class='lvl'>"
            f"<b>L{level + 1}</b> wid={wid} &nbsp;"
            f"fire={w['shotrepeat']} multi={w['multi']} max={w['max']} "
            f"dmg={w['attack'][0]} sy={w['sy'][0]} trail={w['trail']} sound={w['sound']}"
            f" → sprite <code>{html.escape(sprite)}</code>"
            f"</div>"
        )
    base_key = port["name"].lower().replace(" ", "_").replace("-", "_")
    rows.append(
        f"<tr>"
        f"<td class='idx'>{port_id}</td>"
        f"<td><b>{html.escape(port['name'])}</b><br>"
        f'<code class=\'base\'>"base": "{html.escape(base_key)}"</code></td>'
        f"<td>{port['cost']}</td>"
        f"<td>{port['poweruse']}</td>"
        f"<td>{port['opnum']}</td>"
        f"<td>{port['itemgraphic']}</td>"
        f"<td>{''.join(levels) if levels else '(empty)'}</td>"
        f"</tr>"
    )
    return "\n".join(rows)


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate the stock weapon-port catalogue (HTML) from tyrian.hdt")
    parser.add_argument(
        "--data-dir",
        default=str(ROOT / "data"),
        help="Directory containing tyrian.hdt and sprite_manifest.json (default: <repo>/data)",
    )
    parser.add_argument(
        "--out",
        default=str(DEFAULT_OUT),
        help=f"Output HTML file (default: {DEFAULT_OUT.relative_to(ROOT)})",
    )
    args = parser.parse_args()

    hdt = Path(args.data_dir) / "tyrian.hdt"
    if not hdt.is_file():
        print(f"ERROR: {hdt} not found", file=sys.stderr)
        return 2
    out = Path(args.out)

    weapons, ports = read_hdt(args.data_dir)
    manifest = load_manifest(Path(args.data_dir) / "sprite_manifest.json")

    rows = []
    for pid, port in enumerate(ports):
        row = render_weapon_row(pid, port, weapons, manifest)
        if row:
            rows.append(row)

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(
        f"""<!doctype html>
<html><head><meta charset="utf-8">
<title>OpenTyrian2000 stock catalogue</title>
<style>
 body {{ font-family: -apple-system, sans-serif; background: #1a1a1f; color: #ddd; max-width: 1200px; margin: 2em auto; padding: 0 1em; }}
 h1 {{ color: #aef; }}
 p.intro {{ color: #aaa; }}
 table {{ border-collapse: collapse; width: 100%; margin-top: 1em; }}
 th {{ background: #2a2a30; text-align: left; padding: 8px 12px; border-bottom: 2px solid #555; }}
 td {{ padding: 8px 12px; border-bottom: 1px solid #333; vertical-align: top; }}
 tr:hover {{ background: #222228; }}
 .idx {{ font-family: monospace; color: #888; }}
 code {{ background: #2a2a30; padding: 2px 5px; border-radius: 3px; color: #cfc; }}
 code.base {{ color: #fca; font-size: 0.85em; }}
 .lvl {{ font-size: 0.85em; color: #bbb; margin: 2px 0; }}
 input#search {{ width: 100%; padding: 8px; font-size: 1em; background: #2a2a30; color: #eee; border: 1px solid #555; border-radius: 4px; margin-bottom: 1em; }}
</style>
<script>
 function filterRows() {{
   var q = document.getElementById('search').value.toLowerCase();
   document.querySelectorAll('table tbody tr').forEach(function(row) {{
     row.style.display = row.textContent.toLowerCase().includes(q) ? '' : 'none';
   }});
 }}
</script>
</head><body>
<h1>OpenTyrian2000 — Stock weapon ports</h1>
<p class="intro">Browsable reference for the {len(rows)} stock weapon ports shipped in <code>tyrian.hdt</code>. Copy the <code class="base">"base"</code> key into your mod.json to inherit the stock behavior, then override only what differs. The first 3 power levels' key stats are shown; use <code>python3 utils/asset_export_hdt.py --data-dir data</code> for the full binary dump.</p>
<input id="search" type="text" placeholder="filter by name, stat, sprite…" oninput="filterRows()">
<table>
<thead><tr><th>Port</th><th>Name / base key</th><th>Cost</th><th>PowerUse</th><th>opnum</th><th>Item graphic</th><th>Levels (first 3)</th></tr></thead>
<tbody>
{chr(10).join(rows)}
</tbody>
</table>
<p class="intro" style="margin-top:2em;">Generated by <code>utils/ref_stock_catalog.py</code>. Re-run after HDT changes.</p>
</body></html>
""",
        encoding="utf-8",
    )
    print(f"Stock catalog -> {out} ({len(rows)} ports)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
