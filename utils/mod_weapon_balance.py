#!/usr/bin/env python3
"""mod_weapon_balance.py — damage per second of mod weapons next to the originals.

Reads the original weapon ports from data/tyrian.hdt (episodes 1-3 item table,
same layout as episodes.c:JE_loadItemDat / tyrian-formats hdt.rs) and the
weapons of one mod.json, and prints Markdown tables a pack README can carry.

Formula (the one every pack README uses):

    DPS = average salvo damage x 35 / (fire_delay + 1)

the game running at 35 ticks per second. A salvo is `projectile_count`
projectiles taken in turn from the `max_on_screen` first entries of the level's
projectile list, so the average salvo damage is
projectile_count x mean(damage of those entries). A piercing projectile
(`{"pierce": n}`, original codes 250..255) counts n once (single target); a
chain projectile (codes 100..249) counts 1. "All hits" adds every projectile of
the salvo, i.e. what a wide target takes from a spread; the single-target column
is what the README tables compare.

Usage:
    python utils/mod_weapon_balance.py rewrite/mods/alien_weapon_pack/mod.json
    python utils/mod_weapon_balance.py MOD.json --originals 700-1500     # price window of the originals shown
    python utils/mod_weapon_balance.py MOD.json --names "Zica Laser,HotDog Rear"
    python utils/mod_weapon_balance.py MOD.json --energy                  # generator energy per second instead
    python utils/mod_weapon_balance.py MOD.json --names "..." --median   # plus the median of those originals
    python utils/mod_weapon_balance.py --list-originals                   # every original port, both modes

Output: one table per bay (front, rear): name, price, power use, DPS (or, with
--energy, generator energy per second = power_use x 35 / (fire_delay + 1)) at
each power level (L1..L11), then the chosen originals in italics.
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _json5  # noqa: E402

TICKS_PER_SECOND = 35
# Bay of each original weapon port (index in the item table). The data has no
# bay: it comes from the shops that sell the port (levelsN.dat "]I[" lines, the
# wiki's shop_front_eps / shop_rear_eps), and for the ports no shop sells, from
# the name ("HotDog Rear") or the two-mode rear design. A rear port's second
# mode is the alternate fire (rear-fire key); only its first mode is compared.
FRONT_PORTS = {1, 2, 3, 4, 5, 6, 7, 8, 13, 15, 17, 18, 19, 20, 23, 25, 27, 36, 39, 40, 41, 42, 43, 45}
REAR_PORTS = {9, 10, 11, 12, 14, 15, 18, 21, 22, 24, 26, 28, 29, 30, 31, 32, 33, 34, 35, 37, 38}
WEAPON_RECORDS = 819 * 2
PORTS = 61
WEAPON_SIZE = 80  # drain u16, shotrepeat, multi, weapani u16, max, tx, ty, aim, 6x8 bytes, sg 8xu16, 6 bytes


def _hit(code: int) -> int:
    """Damage of one projectile on a single target, from an original attack code."""
    if code >= 250:
        return code - 250  # piercing: n to each enemy, counted once
    if 100 <= code < 250:
        return 1  # chain reaction
    if code == 99:
        return 0  # freeze
    return code


def read_originals(data_dir: Path) -> list[dict]:
    """Original weapon ports: name, cost, power use and, per mode, a list of power levels."""
    hdt = (data_dir / "tyrian.hdt").read_bytes()
    pos = struct.unpack_from("<i", hdt, 0)[0] + 14  # 7 u16 counts
    weapons = []
    for _ in range(WEAPON_RECORDS):
        b = hdt[pos : pos + WEAPON_SIZE]
        shotrepeat, multi = b[2], b[3]
        mx = b[6]
        attack = list(b[10:18])
        weapons.append({"fire_delay": shotrepeat, "count": multi, "max": mx, "attack": attack})
        pos += WEAPON_SIZE
    ports = []
    for _ in range(PORTS):
        n = hdt[pos]
        name = hdt[pos + 1 : pos + 1 + min(n, 30)].decode("latin-1").strip()
        pos += 31
        pos += 1  # opnum
        ops = [struct.unpack_from("<11H", hdt, pos), struct.unpack_from("<11H", hdt, pos + 22)]
        pos += 44
        cost, _graphic, power = struct.unpack_from("<3H", hdt, pos)
        pos += 6
        modes = []
        for op in ops:
            levels = []
            for rec in op:
                if rec == 0:
                    break
                w = weapons[rec] if rec <= 818 else weapons[819 + rec - 1000] if rec >= 1000 else None
                if w is None:
                    break
                levels.append(
                    {
                        "fire_delay": w["fire_delay"],
                        "count": w["count"],
                        "hits": [_hit(a) for a in w["attack"][: max(1, w["max"])]],
                    }
                )
            modes.append(levels)
        idx = len(ports)
        bays = [b for b, ids in (("front", FRONT_PORTS), ("rear", REAR_PORTS)) if idx in ids]
        ports.append({"index": idx, "name": name, "cost": cost, "power_use": power, "modes": modes, "bays": bays})
    return ports


def mod_levels(levels: list) -> list[dict]:
    out = []
    for lv in levels:
        projs = lv.get("projectiles", [])
        mx = max(1, min(int(lv.get("max_on_screen", len(projs) or 1)), len(projs) or 1))
        hits = []
        for p in projs[:mx]:
            d = p.get("damage", 0)
            if isinstance(d, dict) and "pierce" in d:
                hits.append(int(d["pierce"]))
            elif isinstance(d, dict):
                hits.append(1)
            elif d == "freeze":
                hits.append(0)
            else:
                hits.append(int(d))
        out.append(
            {"fire_delay": int(lv.get("fire_delay", 0)), "count": int(lv.get("projectile_count", 1)), "hits": hits}
        )
    return out


def dps(level: dict) -> float:
    hits = level["hits"] or [0]
    mean = sum(hits) / len(hits)
    salvo = level["count"] * mean
    return salvo * TICKS_PER_SECOND / (level["fire_delay"] + 1)


def energy(level: dict, power_use: int) -> float:
    """Generator energy drawn per second: power_use is taken at every volley."""
    return power_use * TICKS_PER_SECOND / (level["fire_delay"] + 1)


def row(
    name: str,
    cost: int,
    power: int,
    levels: list[dict],
    italic: bool = False,
    width: int = 11,
    use_energy: bool = False,
) -> str:
    fn = (lambda lv: f"{energy(lv, power):.0f}") if use_energy else (lambda lv: f"{dps(lv):.1f}")
    cells = [fn(lv) for lv in levels] + ["-"] * (width - len(levels))
    nm = f"*{name} (original)*" if italic else name
    return f"| {nm} | {cost} | {power} | " + " | ".join(cells[:width]) + " |"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mod_json", nargs="?", type=Path)
    ap.add_argument("--data", type=Path, default=Path(__file__).resolve().parent.parent / "data")
    ap.add_argument(
        "--originals", default="", help="price window LOW-HIGH of the originals shown (default: mod prices ±300)"
    )
    ap.add_argument("--names", default="", help="comma-separated original names to show instead of a price window")
    ap.add_argument("--list-originals", action="store_true")
    ap.add_argument("--energy", action="store_true", help="energy per second per level instead of DPS")
    ap.add_argument("--median", action="store_true", help="add a row with the median of the originals shown")
    a = ap.parse_args()

    originals = read_originals(a.data)
    if a.list_originals:
        for p in originals:
            for m, levels in enumerate(p["modes"]):
                if levels and p["bays"]:
                    print(
                        row(
                            f"#{p['index']} {p['name']} [{'/'.join(p['bays'])}, mode {m + 1}]",
                            p["cost"],
                            p["power_use"],
                            levels,
                        )
                    )
        return 0
    if a.mod_json is None:
        ap.error("mod.json required")
    mod = _json5.load(str(a.mod_json))
    weapons = mod.get("weapons", [])
    costs = [int(w.get("cost", 1000)) for w in weapons] or [1000]
    if a.originals:
        lo, hi = (int(v) for v in a.originals.split("-"))
    else:
        lo, hi = min(costs) - 300, max(costs) + 300
    names = [n.strip().lower() for n in a.names.split(",") if n.strip()]
    what = "generator energy per second" if a.energy else "DPS, single target"
    for bay in ("front", "rear"):
        lines = []
        for w in weapons:
            if bay in w:
                levels = mod_levels(w[bay])
                lines.append(
                    row(w["name"], int(w.get("cost", 1000)), int(w.get("power_use", 20)), levels, use_energy=a.energy)
                )
        if not lines:
            continue
        print(f"\n### {bay.capitalize()} ({what})\n")
        print("| Weapon | Price | Power | " + " | ".join(f"L{i}" for i in range(1, 12)) + " |")
        print("|---|---|---|" + "---|" * 11)
        for ln in lines:
            print(ln)
        shown = []
        for p in sorted(originals, key=lambda p: p["cost"]):
            levels = p["modes"][0]
            if not levels or bay not in p["bays"]:
                continue
            chosen = p["name"].lower() in names if names else lo <= p["cost"] <= hi
            if chosen:
                shown.append(p)
                print(row(p["name"], p["cost"], p["power_use"], levels, italic=True, use_energy=a.energy))
        if a.median and shown:
            cells = []
            for i in range(11):
                vals = sorted(
                    energy(p["modes"][0][i], p["power_use"]) if a.energy else dps(p["modes"][0][i])
                    for p in shown
                    if len(p["modes"][0]) > i
                )
                n = len(vals)
                m = vals[n // 2] if n % 2 else (vals[n // 2 - 1] + vals[n // 2]) / 2
                cells.append(f"{m:.0f}" if a.energy else f"{m:.1f}")
            print("| *median of the originals above* | | | " + " | ".join(cells) + " |")
    return 0


if __name__ == "__main__":
    sys.exit(main())
