#!/usr/bin/env python3
"""Export tyrian.hdt into a human-readable moddable JSON file.

Reads the binary tyrian.hdt and data/sprite_manifest.json (section "shots",
via _sprite_manifest.load_manifest), then produces a mod.json-shaped file with
weapons, sidekicks, ships, shields and specials in the modder-friendly format
(C-version format, guide under the c-final tag). Projectile sprites are written by manifest name; indices the
manifest does not name fall back to "sprite_NNN", which the loader accepts.

Like asset_export_hdt.py, the default output is <data-dir>/mod.export.json.
<data-dir>/mod.json is the curated working file (142 K lines of layered custom
content): it is only overwritten with --force.

Usage:
    python3 utils/asset_export_mod.py [--data-dir data/] [--out FILE] [--force]
"""

import argparse
import json
import os as _os_j5
import sys as _sys_j5

_sys_j5.path.insert(0, _os_j5.path.dirname(_os_j5.path.abspath(__file__)))
import os
import re
import struct
import sys

from _sprite_manifest import load_manifest  # noqa: E402

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


# ---------------------------------------------------------------------------
# Name sanitization
# ---------------------------------------------------------------------------


def clean_name(name):
    """Collapse multiple spaces, strip invalid filename chars, trim."""
    name = re.sub(r"  +", " ", name)
    name = re.sub(r'[<>:"/\\|?*]+', "", name)
    name = re.sub(r"\s+Ammo\s+\d+$", "", name)
    return name.strip()


# ---------------------------------------------------------------------------
# Binary readers
# ---------------------------------------------------------------------------


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


# ---------------------------------------------------------------------------
# HDT record readers
# ---------------------------------------------------------------------------


def read_weapon(f):
    """Read one weapon entry (80 bytes)."""
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


def read_port(f):
    """Read one weapon port entry (82 bytes)."""
    name_len = read_u8(f)
    name_raw = f.read(30)
    name = clean_name(name_raw[:name_len].decode("ascii", errors="replace"))
    opnum = read_u8(f)
    op0 = read_u16(f, 11)
    op1 = read_u16(f, 11)
    cost = read_u16(f)
    itemgraphic = read_u16(f)
    poweruse = read_u16(f)
    return {
        "name": name,
        "opnum": opnum,
        "op": [op0, op1],
        "cost": cost,
        "itemgraphic": itemgraphic,
        "poweruse": poweruse,
    }


def read_special(f):
    """Read one special entry (37 bytes)."""
    name_len = read_u8(f)
    name_raw = f.read(30)
    name = clean_name(name_raw[:name_len].decode("ascii", errors="replace"))
    itemgraphic = read_u16(f)
    pwr = read_u8(f)
    stype = read_u8(f)
    wpn = read_u16(f)
    return {
        "name": name,
        "itemgraphic": itemgraphic,
        "power": pwr,
        "special_type": stype,
        "weapon_id": wpn,
    }


def read_power(f):
    """Read one power system entry (37 bytes)."""
    name_len = read_u8(f)
    name_raw = f.read(30)
    name = clean_name(name_raw[:name_len].decode("ascii", errors="replace"))
    itemgraphic = read_u16(f)
    power = read_u8(f)
    speed = read_s8(f)
    cost = read_u16(f)
    return {
        "name": name,
        "itemgraphic": itemgraphic,
        "power": power,
        "speed": speed,
        "cost": cost,
    }


def read_ship(f):
    """Read one ship entry (41 bytes)."""
    name_len = read_u8(f)
    name_raw = f.read(30)
    name = clean_name(name_raw[:name_len].decode("ascii", errors="replace"))
    shipgraphic = read_u16(f)
    itemgraphic = read_u16(f)
    ani = read_u8(f)
    spd = read_s8(f)
    dmg = read_u8(f)
    cost = read_u16(f)
    bigshipgraphic = read_u8(f)
    return {
        "name": name,
        "shipgraphic": shipgraphic,
        "itemgraphic": itemgraphic,
        "animation_frames": ani,
        "speed": spd,
        "damage_resistance": dmg,
        "cost": cost,
        "bigshipgraphic": bigshipgraphic,
    }


def read_option(f):
    """Read one sidekick/option entry (86 bytes)."""
    name_len = read_u8(f)
    name_raw = f.read(30)
    name = clean_name(name_raw[:name_len].decode("ascii", errors="replace"))
    pwr = read_u8(f)
    itemgraphic = read_u16(f)
    cost = read_u16(f)
    tr = read_u8(f)
    option = read_u8(f)
    opspd = read_s8(f)
    ani = read_u8(f)
    gr = read_u16(f, 20)
    wport = read_u8(f)
    wpnum = read_u16(f)
    ammo = read_u8(f)
    stop = read_u8(f)
    icongr = read_u8(f)
    return {
        "name": name,
        "power": pwr,
        "itemgraphic": itemgraphic,
        "cost": cost,
        "trail": tr,
        "option_type": option,
        "option_speed": opspd,
        "animation_frames": ani,
        "graphics": gr,
        "weapon_port": wport,
        "weapon_port_id": wpnum,
        "ammo": ammo,
        "stop_on_edge": bool(stop),
        "icon_graphic": icongr,
    }


def read_shield(f):
    """Read one shield entry (37 bytes)."""
    name_len = read_u8(f)
    name_raw = f.read(30)
    name = clean_name(name_raw[:name_len].decode("ascii", errors="replace"))
    tpwr = read_u8(f)
    mpwr = read_u8(f)
    itemgraphic = read_u16(f)
    cost = read_u16(f)
    return {
        "name": name,
        "shield_power": tpwr,
        "max_power": mpwr,
        "itemgraphic": itemgraphic,
        "cost": cost,
    }


# ---------------------------------------------------------------------------
# Sprite name resolution
# ---------------------------------------------------------------------------


def build_sprite_reverse_map(sprite_manifest, section="shots"):
    """Build index -> name mapping from one sprite_manifest.json section.

    Weapon projectiles (`sg[]`) index spriteSheet8 (1-304) and spriteSheet12
    (501+); the manifest's "shots" section covers both ranges. First name
    wins when two names share an index (same rule as
    _sprite_manifest.resolve_index).
    """
    rev = {}
    for name, info in sprite_manifest.get(section, {}).items():
        idx = info.get("index") if isinstance(info, dict) else info
        if isinstance(idx, int) and idx not in rev:
            rev[idx] = name
    return rev


def resolve_sprite(index, rev_map):
    """Resolve a sprite index to a name, falling back to sprite_NNN."""
    if index == 0:
        return "none"
    name = rev_map.get(index)
    if name:
        return name
    return f"sprite_{index}"


# ---------------------------------------------------------------------------
# Weapon → moddable format conversion
# ---------------------------------------------------------------------------


def format_projectile(weap, proj_idx, sprite_rev):
    """Format a single projectile sub-entry from a weapon record."""
    return {
        "damage": weap["attack"][proj_idx],
        "sub_delay": weap["del"][proj_idx],
        "speed_x": weap["sx"][proj_idx],
        "speed_y": weap["sy"][proj_idx],
        "accel_x": weap["bx"][proj_idx],
        "accel_y": weap["by"][proj_idx],
        "sprite": resolve_sprite(weap["sg"][proj_idx], sprite_rev),
    }


def format_weapon_level(weap, sprite_rev):
    """Format a full weapon entry into moddable form."""
    multi = weap["multi"]
    projectiles = []
    for i in range(multi):
        projectiles.append(format_projectile(weap, i, sprite_rev))

    return {
        "power_drain": weap["drain"],
        "fire_delay": weap["shotrepeat"],
        "projectile_count": multi,
        "animation": weap["weapani"],
        "max_on_screen": weap["max"],
        "offset_x": weap["tx"],
        "offset_y": weap["ty"],
        "homing": weap["aim"] != 0,
        "accel_y": weap["acceleration"],
        "accel_x": weap["accelerationx"],
        "circle_radius": weap["circlesize"],
        "sound": weap["sound"],
        "trail": weap["trail"],
        "ship_blast_filter": weap["shipblastfilter"],
        "projectiles": projectiles,
    }


# ---------------------------------------------------------------------------
# Main export logic
# ---------------------------------------------------------------------------


def export_mod(data_dir, out_path, manifest_path=None):
    """Read tyrian.hdt and the sprite manifest, write the moddable JSON to out_path."""
    hdt_path = os.path.join(data_dir, "tyrian.hdt")
    if manifest_path is None:
        manifest_path = os.path.join(data_dir, "sprite_manifest.json")

    if not os.path.isfile(hdt_path):
        print(f"Error: {hdt_path} not found", file=sys.stderr)
        sys.exit(1)

    # Load sprite manifest (optional — fall back gracefully to sprite_NNN)
    sprite_rev = {}
    if os.path.isfile(manifest_path):
        sprite_rev = build_sprite_reverse_map(load_manifest(manifest_path))
        if sprite_rev:
            print(f"Loaded {len(sprite_rev)} shot sprite names from {manifest_path}")
        else:
            print(
                f'Warning: {manifest_path} has no usable "shots" section, using numeric sprite indices',
                file=sys.stderr,
            )
    else:
        print(f"Warning: {manifest_path} not found, using numeric sprite indices", file=sys.stderr)

    with open(hdt_path, "rb") as f:
        # Read episode1DataLoc (int32 at offset 0)
        episode1_data_loc = read_s32(f)
        f.seek(episode1_data_loc)

        # Read itemNum[7] — 7 x uint16 (consumed, not exported)
        read_u16(f, 7)

        # --- Weapons (raw) ---
        weapons_raw = {}
        for i in range(0, WEAP_END1 + 1):
            weapons_raw[i] = read_weapon(f)
        for i in range(WEAP_START2, WEAP_NUM + 1):
            weapons_raw[i] = read_weapon(f)

        # --- Weapon Ports ---
        ports_raw = []
        for _ in range(PORT_NUM + 1):
            ports_raw.append(read_port(f))

        # --- Specials ---
        specials_raw = []
        for _ in range(SPECIAL_NUM + 1):
            specials_raw.append(read_special(f))

        # --- Power Systems ---
        powers_raw = []
        for _ in range(POWER_NUM + 1):
            powers_raw.append(read_power(f))

        # --- Ships ---
        ships_raw = []
        for _ in range(SHIP_NUM + 1):
            ships_raw.append(read_ship(f))

        # --- Options/Sidekicks ---
        options_raw = []
        for _ in range(OPTION_NUM + 1):
            options_raw.append(read_option(f))

        # --- Shields ---
        shields_raw = []
        for _ in range(SHIELD_NUM + 1):
            shields_raw.append(read_shield(f))

    # Build port name lookup for sidekick weapon references
    port_name_by_id = {}
    for idx, port in enumerate(ports_raw):
        if port["name"]:
            port_name_by_id[idx] = port["name"]

    # --- Format weapons (ports 1-45) ---
    weapons_out = []
    for port_idx in range(1, 46):
        port = ports_raw[port_idx]
        if not port["name"]:
            continue

        entry = {
            "id": port_idx,
            "name": port["name"],
            "cost": port["cost"],
            "power_use": port["poweruse"],
            "itemgraphic": port["itemgraphic"],
            "front": [],
        }

        # Front weapon levels
        for level in range(11):
            wid = port["op"][0][level]
            if wid == 0:
                continue
            if wid not in weapons_raw:
                continue
            level_data = format_weapon_level(weapons_raw[wid], sprite_rev)
            level_data["level"] = level + 1
            level_data["_weapon_id"] = wid
            entry["front"].append(level_data)

        # Rear weapon levels (omit key entirely if all zero)
        has_rear = any(port["op"][1][level] != 0 for level in range(11))
        if has_rear:
            entry["rear"] = []
            for level in range(11):
                wid = port["op"][1][level]
                if wid == 0:
                    continue
                if wid not in weapons_raw:
                    continue
                level_data = format_weapon_level(weapons_raw[wid], sprite_rev)
                level_data["level"] = level + 1
                level_data["_weapon_id"] = wid
                entry["rear"].append(level_data)

        weapons_out.append(entry)

    # --- Format sidekicks ---
    sidekicks_out = []
    for idx, opt in enumerate(options_raw):
        if not opt["name"]:
            continue
        sk = {
            "id": idx,
            "name": opt["name"],
            "power": opt["power"],
            "cost": opt["cost"],
            "itemgraphic": opt["itemgraphic"],
            "trail": opt["trail"],
            "option_type": opt["option_type"],
            "option_speed": opt["option_speed"],
            "animation_frames": opt["animation_frames"],
            "graphics": opt["graphics"],
            "ammo": opt["ammo"],
            "stop_on_edge": opt["stop_on_edge"],
            "icon_graphic": opt["icon_graphic"],
        }
        # Resolve weapon port reference
        wpid = opt["weapon_port_id"]
        if wpid in port_name_by_id:
            sk["weapon"] = port_name_by_id[wpid]
        else:
            sk["weapon_port_id"] = wpid
        sidekicks_out.append(sk)

    # --- Format ships ---
    ships_out = []
    for idx, ship in enumerate(ships_raw):
        if not ship["name"]:
            continue
        ships_out.append(
            {
                "id": idx,
                "name": ship["name"],
                "speed": ship["speed"],
                "damage_resistance": ship["damage_resistance"],
                "cost": ship["cost"],
                "shipgraphic": ship["shipgraphic"],
                "itemgraphic": ship["itemgraphic"],
                "animation_frames": ship["animation_frames"],
                "bigshipgraphic": ship["bigshipgraphic"],
            }
        )

    # --- Format shields ---
    shields_out = []
    for idx, shield in enumerate(shields_raw):
        if not shield["name"]:
            continue
        shields_out.append(
            {
                "id": idx,
                "name": shield["name"],
                "shield_power": shield["shield_power"],
                "max_power": shield["max_power"],
                "cost": shield["cost"],
                "itemgraphic": shield["itemgraphic"],
            }
        )

    # --- Format specials ---
    specials_out = []
    for idx, spec in enumerate(specials_raw):
        if not spec["name"]:
            continue
        specials_out.append(
            {
                "id": idx,
                "name": spec["name"],
                "power": spec["power"],
                "special_type": spec["special_type"],
                "weapon_id": spec["weapon_id"],
                "itemgraphic": spec["itemgraphic"],
            }
        )

    # --- Build output ---
    output = {
        "version": 1,
        "_docs": {
            "damage": "1-255, damage per hit per projectile",
            "fire_delay": "1-20, frames between shots (lower = faster fire rate)",
            "speed_x": "-128 to 127, horizontal velocity (positive = right)",
            "speed_y": "-128 to 127, vertical velocity (positive = up, toward enemies)",
            "accel_x": "-128 to 127, horizontal acceleration per frame",
            "accel_y": "-128 to 127, vertical acceleration per frame",
            "sub_delay": "0-255, frames delay between sub-projectiles (255 = no delay)",
            "sprite": 'sprite name from the "shots" section of sprite_manifest.json, or sprite_NNN for unnamed indices',
            "homing": "true = projectile tracks nearest enemy",
            "trail": "0-255, trail visual type (255 = default)",
            "circle_radius": "0 = straight line, N = circular movement radius",
            "power_drain": "energy consumed per shot from power bar",
            "max_on_screen": "max simultaneous projectiles on screen",
            "sound": "sound effect index (0 = none, 25 = standard shot)",
            "level": "power level 1-11",
            "_weapon_id": "internal weapon array index (do not modify)",
            "always_available": "true = item appears in every shop automatically",
            "starter": "optional section to override starting equipment (-1 = default)",
        },
        "weapons": weapons_out,
        "sidekicks": sidekicks_out,
        "ships": ships_out,
        "shields": shields_out,
        "specials": specials_out,
        "starter": {
            "ship": -1,
            "front_weapon": -1,
            "rear_weapon": -1,
            "shield": -1,
            "generator": -1,
            "sidekick_left": -1,
            "sidekick_right": -1,
        },
    }

    with open(out_path, "w") as out:
        json.dump(output, out, indent=2)

    unnamed = sum(
        1
        for w in weapons_out
        for mode in ("front", "rear")
        for lvl in w.get(mode, [])
        for pr in lvl["projectiles"]
        if pr["sprite"].startswith("sprite_")
    )
    print(f"\nWrote {out_path}")
    print(f"  weapons:   {len(weapons_out)}")
    print(f"  sidekicks: {len(sidekicks_out)}")
    print(f"  ships:     {len(ships_out)}")
    print(f"  shields:   {len(shields_out)}")
    print(f"  specials:  {len(specials_out)}")
    if unnamed:
        print(f"  {unnamed} projectile sprite(s) not named in the manifest, written as sprite_NNN")


def main():
    parser = argparse.ArgumentParser(
        description="Export tyrian.hdt to the moddable mod.json format "
        "(writes <data-dir>/mod.export.json unless --force)"
    )
    parser.add_argument(
        "--data-dir",
        default="data",
        help="Directory containing tyrian.hdt and sprite_manifest.json (default: data)",
    )
    parser.add_argument(
        "--manifest",
        default=None,
        help="sprite_manifest.json to resolve projectile sprite names (default: <data-dir>/sprite_manifest.json)",
    )
    parser.add_argument("--out", default=None, help="Output file (default: <data-dir>/mod.export.json)")
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite <data-dir>/mod.json in place — the curated working file. "
        "Default writes mod.export.json next to it, like asset_export_hdt.py.",
    )
    args = parser.parse_args()

    if args.force and args.out:
        parser.error("--force and --out are mutually exclusive")
    out_path = args.out or os.path.join(args.data_dir, "mod.json" if args.force else "mod.export.json")
    export_mod(args.data_dir, out_path, args.manifest)


if __name__ == "__main__":
    main()
