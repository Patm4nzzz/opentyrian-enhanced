#!/usr/bin/env python3
"""Export weapons and enemies from tyrian.hdt to JSON files.

Reads the binary tyrian.hdt file using the exact same layout as
JE_loadItemDat() in src/episodes.c and produces mod.json and enemies.json.

Writes <data-dir>/mod.export.json and enemies.export.json; the curated
data/mod.json + enemies.json are only overwritten with --force (episodes 1-3).

Usage:
    python3 utils/asset_export_hdt.py [--data-dir data/] [--episode N] [--force]
"""

import argparse
import json
import os
import struct
import sys

# Constants from src/lvlmast.h
WEAP_END1 = 818
WEAP_START2 = 1000
WEAP_NUM = 1818

PORT_NUM = 60
SPECIAL_NUM = 54
POWER_NUM = 6
SHIP_NUM = 18
OPTION_NUM = 37
SHIELD_NUM = 11

ENEMY_END1 = 850
ENEMY_START2 = 1001
ENEMY_NUM = 1850


def read_u8(f, count=1):
    data = f.read(count)
    if len(data) != count:
        raise EOFError(f"Expected {count} bytes, got {len(data)}")
    if count == 1:
        return struct.unpack("<B", data)[0]
    return list(struct.unpack(f"<{count}B", data))


def read_s8(f, count=1):
    data = f.read(count)
    if len(data) != count:
        raise EOFError(f"Expected {count} bytes, got {len(data)}")
    if count == 1:
        return struct.unpack("<b", data)[0]
    return list(struct.unpack(f"<{count}b", data))


def read_u16(f, count=1):
    size = count * 2
    data = f.read(size)
    if len(data) != size:
        raise EOFError(f"Expected {size} bytes, got {len(data)}")
    if count == 1:
        return struct.unpack("<H", data)[0]
    return list(struct.unpack(f"<{count}H", data))


def read_s16(f, count=1):
    size = count * 2
    data = f.read(size)
    if len(data) != size:
        raise EOFError(f"Expected {size} bytes, got {len(data)}")
    if count == 1:
        return struct.unpack("<h", data)[0]
    return list(struct.unpack(f"<{count}h", data))


def read_s32(f, count=1):
    size = count * 4
    data = f.read(size)
    if len(data) != size:
        raise EOFError(f"Expected {size} bytes, got {len(data)}")
    if count == 1:
        return struct.unpack("<i", data)[0]
    return list(struct.unpack(f"<{count}i", data))


def read_weapon(f):
    """Read one weapon entry matching the layout in episodes.c:81-101."""
    return {
        "drain": read_u16(f),
        "shotrepeat": read_u8(f),
        "multi": read_u8(f),
        "weapani": read_u16(f),
        "max": read_u8(f),
        "tx": read_u8(f),
        "ty": read_u8(f),
        "aim": read_u8(f),
        "attack": read_u8(f, 8),
        "del": read_u8(f, 8),
        "sx": read_s8(f, 8),
        "sy": read_s8(f, 8),
        "bx": read_s8(f, 8),
        "by": read_s8(f, 8),
        "sg": read_u16(f, 8),
        "acceleration": read_s8(f),
        "accelerationx": read_s8(f),
        "circlesize": read_u8(f),
        "sound": read_u8(f),
        "trail": read_u8(f),
        "shipblastfilter": read_u8(f),
    }


def read_enemy(f):
    """Read one enemy entry matching the layout in episodes.c:196-223."""
    return {
        "ani": read_u8(f),
        "tur": read_u8(f, 3),
        "freq": read_u8(f, 3),
        "xmove": read_s8(f),
        "ymove": read_s8(f),
        "xaccel": read_s8(f),
        "yaccel": read_s8(f),
        "xcaccel": read_s8(f),
        "ycaccel": read_s8(f),
        "startx": read_s16(f),
        "starty": read_s16(f),
        "startxc": read_s8(f),
        "startyc": read_s8(f),
        "armor": read_u8(f),
        "esize": read_u8(f),
        "egraphic": read_u16(f, 20),
        "explosiontype": read_u8(f),
        "animate": read_u8(f),
        "shapebank": read_u8(f),
        "xrev": read_s8(f),
        "yrev": read_s8(f),
        "dgr": read_u16(f),
        "dlevel": read_s8(f),
        "dani": read_s8(f),
        "elaunchfreq": read_u8(f),
        "elaunchtype": read_u16(f),
        "value": read_s16(f),
        "eenemydie": read_u16(f),
    }


def skip_weapon_ports(f, count):
    """Skip weapon port entries (episodes.c:105-117)."""
    for _ in range(count):
        # nameLen(1) + name(30) + opnum(1) + op[0](11*2) + op[1](11*2) + cost(2) + itemgraphic(2) + poweruse(2)
        f.read(1 + 30 + 1 + 22 + 22 + 2 + 2 + 2)  # 82 bytes


def skip_specials(f, count):
    """Skip special item entries (episodes.c:119-129)."""
    for _ in range(count):
        # nameLen(1) + name(30) + itemgraphic(2) + pwr(1) + stype(1) + wpn(2)
        f.read(1 + 30 + 2 + 1 + 1 + 2)  # 37 bytes


def skip_power_systems(f, count):
    """Skip power system entries (episodes.c:131-141)."""
    for _ in range(count):
        # nameLen(1) + name(30) + itemgraphic(2) + power(1) + speed(1) + cost(2)
        f.read(1 + 30 + 2 + 1 + 1 + 2)  # 37 bytes


def skip_ships(f, count):
    """Skip ship entries (episodes.c:143-156)."""
    for _ in range(count):
        # nameLen(1) + name(30) + shipgraphic(2) + itemgraphic(2) + ani(1) + spd(1) + dmg(1) + cost(2) + bigshipgraphic(1)
        f.read(1 + 30 + 2 + 2 + 1 + 1 + 1 + 2 + 1)  # 41 bytes


def skip_options(f, count):
    """Skip option entries (episodes.c:158-177)."""
    for _ in range(count):
        # nameLen(1) + name(30) + pwr(1) + itemgraphic(2) + cost(2) + tr(1) + option(1) + opspd(1) + ani(1)
        # + gr(20*2) + wport(1) + wpnum(2) + ammo(1) + stop(1) + icongr(1)
        f.read(1 + 30 + 1 + 2 + 2 + 1 + 1 + 1 + 1 + 40 + 1 + 2 + 1 + 1 + 1)  # 86 bytes


def skip_shields(f, count):
    """Skip shield entries (episodes.c:179-189)."""
    for _ in range(count):
        # nameLen(1) + name(30) + tpwr(1) + mpwr(1) + itemgraphic(2) + cost(2)
        f.read(1 + 30 + 1 + 1 + 2 + 2)  # 37 bytes


def open_item_table(data_dir, episode):
    """Open the file holding the item/enemy tables for `episode`, positioned
    at the start of the table.

    Episodes 1-3 read tyrian.hdt. Episodes 4-5 do NOT: src/episodes.c:91 does
    `if (episodeNum <= 3)` … `else` and reloads the whole table from the LAST
    section of tyrian<N>.lvl. That is why the second enemy bank of the HDT is
    847 byte-identical copies of one filler record — it is the address space
    Tyrian 2000 added and the inherited Tyrian 2.1 HDT never populated. The
    real episode 4/5 bestiary lives in the level files, and until this option
    existed nothing in the repo could read it.

    The level layout is: uint16 section count, then that many int32 offsets
    (JE_analyzeLevel in src/lvllib.c). The item table is the last section.
    """
    if episode <= 3:
        path = os.path.join(data_dir, "tyrian.hdt")
        if not os.path.isfile(path):
            print(f"Error: {path} not found", file=sys.stderr)
            sys.exit(1)
        f = open(path, "rb")
        f.seek(read_s32(f))  # episode1DataLoc, int32 at offset 0
        return f, path

    path = os.path.join(data_dir, f"tyrian{episode}.lvl")
    if not os.path.isfile(path):
        print(f"Error: {path} not found", file=sys.stderr)
        sys.exit(1)
    f = open(path, "rb")
    count = read_u16(f)
    offsets = [read_s32(f) for _ in range(count)]
    f.seek(offsets[count - 1])
    return f, path


def export_hdt(data_dir, force=False, episode=1):
    """Read the item/enemy tables for `episode` and export them to JSON."""
    if force and episode > 3:
        print(
            "Refusing --force with --episode 4/5: those tables belong to the "
            "level files, not to data/mod.json. Export without --force.",
            file=sys.stderr,
        )
        sys.exit(1)
    f, src_path = open_item_table(data_dir, episode)
    with f:
        # Read itemNum[7] — 7 x uint16 (not exported, just consumed)
        read_u16(f, 7)

        # --- Weapons ---
        weapons = []
        # Bank 1: indices 0..WEAP_END1 (inclusive)
        for i in range(0, WEAP_END1 + 1):
            w = read_weapon(f)
            w["id"] = i
            weapons.append(w)

        # Bank 2: indices WEAP_START2..WEAP_NUM (inclusive)
        for i in range(WEAP_START2, WEAP_NUM + 1):
            w = read_weapon(f)
            w["id"] = i
            weapons.append(w)

        # --- Skip intermediate sections ---
        skip_weapon_ports(f, PORT_NUM + 1)  # 61 entries
        skip_specials(f, SPECIAL_NUM + 1)  # 55 entries
        skip_power_systems(f, POWER_NUM + 1)  # 7 entries
        skip_ships(f, SHIP_NUM + 1)  # 19 entries
        skip_options(f, OPTION_NUM + 1)  # 38 entries
        skip_shields(f, SHIELD_NUM + 1)  # 12 entries

        # --- Enemies ---
        enemies = []
        # Bank 1: indices 0..ENEMY_END1 (inclusive)
        for i in range(0, ENEMY_END1 + 1):
            e = read_enemy(f)
            e["id"] = i
            enemies.append(e)

        # Bank 2: indices ENEMY_START2..ENEMY_NUM (inclusive)
        for i in range(ENEMY_START2, ENEMY_NUM + 1):
            e = read_enemy(f)
            e["id"] = i
            enemies.append(e)

    # Guard: data/mod.json and data/enemies.json are curated working
    # files (custom entries layered on the export). Overwriting them
    # without --force destroys that work — write *.export.json instead.
    suffix = "" if force else ".export"

    # Write mod.json
    weapons_path = os.path.join(data_dir, f"mod{suffix}.json")
    with open(weapons_path, "w") as out:
        json.dump({"version": 1, "data": weapons}, out, indent=2)
    print(f"Wrote {len(weapons)} weapons to {weapons_path}")

    # Write enemies.json
    enemies_path = os.path.join(data_dir, f"enemies{suffix}.json")
    with open(enemies_path, "w") as out:
        json.dump({"version": 1, "data": enemies}, out, indent=2)
    print(f"Wrote {len(enemies)} enemies to {enemies_path}")


def main():
    parser = argparse.ArgumentParser(description="Export tyrian.hdt weapons and enemies to JSON")
    parser.add_argument("--data-dir", default="data", help="Directory containing tyrian.hdt (default: data)")
    parser.add_argument(
        "--episode",
        type=int,
        default=1,
        choices=[1, 2, 3, 4, 5],
        help="Episode whose tables to export. 1-3 read "
        "tyrian.hdt; 4-5 read the last section of "
        "tyrian<N>.lvl, where the real Tyrian 2000 "
        "bestiary actually lives (default: 1)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite data/mod.json + enemies.json in place (default writes *.export.json)",
    )
    args = parser.parse_args()
    export_hdt(args.data_dir, force=args.force, episode=args.episode)


if __name__ == "__main__":
    main()
