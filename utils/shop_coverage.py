#!/usr/bin/env python3
"""Which items are actually sold somewhere?

The shops are not defined in the code: each "]I" block of a
levels<N>.dat file lists, for the 9 categories, the identifiers
offered at that point of the scenario. An item that this catalog never cites
exists in tyrian.hdt, has a name, a price and a sprite -- and remains
inaccessible in the game, except for a mod's always_available flag.

No dependency: same primitives as the game (encrypted pascal string,
10-byte XOR key from helptext.c).

    python3 utils/shop_coverage.py            # full report
"""

import argparse
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = os.path.join(ROOT, "data")
KEY = bytes([204, 129, 63, 255, 71, 19, 25, 62, 1, 99])

# The 9 rows carry their own label in the data ("Ship", "WeapF",
# "Opt1"...). It is read rather than guessed: the order of the
# categories is a convention of the file, not of the code.


def decrypt(b):
    s = bytearray(b)
    for i in range(len(s) - 1, -1, -1):
        s[i] ^= KEY[i % 10]
        if i:
            s[i] ^= s[i - 1]
    return bytes(s)


def pascal_strings(path):
    d = open(path, "rb").read()
    off = 0
    while off < len(d):
        n = d[off]
        off += 1
        if off + n > len(d):
            return
        yield decrypt(d[off : off + n]).decode("latin-1")
        off += n


def str_pop_ints(s):
    return [int(x) for x in re.findall(r"-?\d+", s)]


def item_names():
    """Item names, read from tyrian.hdt following the order of episodes.c.

    The asset_export_hdt.py reader is reused rather than redescribing
    the layout: a single description of the format in the repository.
    """
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import asset_export_hdt as H

    f, _ = H.open_item_table(DATA, 1)
    with f:
        H.read_u16(f, 7)
        for _ in range(0, H.WEAP_END1 + 1):
            H.read_weapon(f)
        for _ in range(H.WEAP_START2, H.WEAP_NUM + 1):
            H.read_weapon(f)

        def pascal30():
            n = f.read(1)[0]
            raw = f.read(30)
            return raw[: min(n, 30)].decode("latin-1").strip()

        out = {}
        ports = []
        for _ in range(H.PORT_NUM + 1):
            name = pascal30()
            f.read(1 + 22 + 22 + 2 + 2 + 2)
            ports.append(name)
        out["port"] = ports

        specials = []
        for _ in range(H.SPECIAL_NUM + 1):
            specials.append(pascal30())
            f.read(2 + 1 + 1 + 2)
        out["special"] = specials

        powers = []
        for _ in range(H.POWER_NUM + 1):
            powers.append(pascal30())
            f.read(2 + 1 + 1 + 2)
        out["power"] = powers

        ships = []
        for _ in range(H.SHIP_NUM + 1):
            ships.append(pascal30())
            f.read(2 + 2 + 1 + 1 + 1 + 2 + 1)
        out["ship"] = ships

        # The next two tables are not used for the shop verdict,
        # but mod_validate.py uses them to check that a `base` field
        # designates an original item (options = sidekicks, shields).
        # Same record sizes as skip_options / skip_shields.
        options = []
        for _ in range(H.OPTION_NUM + 1):
            options.append(pascal30())
            f.read(1 + 2 + 2 + 1 + 1 + 1 + 1 + 40 + 1 + 2 + 1 + 1 + 1)
        out["option"] = options

        shields = []
        for _ in range(H.SHIELD_NUM + 1):
            shields.append(pascal30())
            f.read(1 + 1 + 2 + 2)
        out["shield"] = shields
    return out


# mod.json sections -> HDT table. The section names are those
# that mod_manager.c reads; the keys on the right, those of item_names().
STOCK_SECTION_TABLE = {
    "ships": "ship",
    "weapons": "port",
    "sidekicks": "option",
    "shields": "shield",
    "specials": "special",
}


def stock_names():
    """{mod.json section: [original names]} or {} if tyrian.hdt is missing.

    Entry 0 of each table = "nothing", discarded; the HDT's "Test" entries
    are reservations, not items that a mod can clone.
    """
    if not os.path.exists(os.path.join(DATA, "tyrian.hdt")):
        return {}
    names = item_names()
    return {sec: [n for n in names[tbl][1:] if n and n != "Test"] for sec, tbl in STOCK_SECTION_TABLE.items()}


def code_grants():
    """Weapon identifiers that the code assigns directly.

    The shop is not the only path: several weapons are placed in the
    inventory by the scenario (RailGun of the secret level), by a
    pickup (NortShip), by a code (Banana Blast) or by the tables of the
    super-arcade mode. Without this cross-check, "unsold" would wrongly read
    "inaccessible". List collected from the sources of the retired C version
    (tag c-final) and frozen in c_tables.json.
    """
    sys.path.insert(0, os.path.join(ROOT, "utils"))
    from _shared import c_tables

    return set(c_tables()["weapon_ids_granted_by_code"])


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.parse_args(argv)
    sold = {}
    blocks = 0
    for ep in range(1, 6):
        p = os.path.join(DATA, "levels%d.dat" % ep)
        if not os.path.exists(p):
            continue
        rows = list(pascal_strings(p))
        for i, s in enumerate(rows):
            if not s.startswith("]I"):
                continue
            blocks += 1
            for c in range(9):
                if i + 1 + c >= len(rows):
                    break
                line = rows[i + 1 + c]
                label = line[:8].strip() or "cat%d" % c
                # level_load.c skips the first 8 characters (the label)
                sold.setdefault(label, set()).update(str_pop_ints(line[8:] if len(line) > 8 else ""))
    print("%d ]I block(s) over the 5 episodes\n" % blocks)
    if not blocks:
        print("no block found — did the levels<N>.dat format change?", file=sys.stderr)
        return 2
    names = item_names()
    for label in sold:
        ids = sorted(x for x in sold[label] if x > 0)
        print("%-8s %2d item(s): %s" % (label, len(ids), " ".join(str(i) for i in ids) or "-"))

    # Verdict: the weapon ports are the only catalog shared between two
    # categories (WeapF front, WeapR rear). A port cited by neither
    # of the two cannot be bought anywhere.
    ports = names["port"]
    total = len(ports) - 1  # entry 0 is "nothing"
    seen = set(sold.get("WeapF", ())) | set(sold.get("WeapR", ()))
    seen = {i for i in seen if 0 < i <= total}
    missing = [i for i in range(1, total + 1) if i not in seen]
    print()
    granted = code_grants()
    print("weapons (weaponPort): %d in total, %d sold, %d unsold" % (total, len(seen), len(missing)))
    unreachable = []
    for i in missing:
        name = ports[i] or "(unnamed)"
        if name == "Test":
            tag = "placeholder"
        elif i in granted:
            tag = "granted by the code"
        else:
            tag = "no path found"
            unreachable.append(i)
        print("   %2d  %-30s %s" % (i, name, tag))
    print()
    print(
        'verdict: out of %d unsold, %d are "Test" placeholders, '
        "%d are granted by the code, %d have no path found by this check."
        % (
            len(missing),
            sum(1 for i in missing if ports[i] == "Test"),
            sum(1 for i in missing if ports[i] != "Test" and i in granted),
            len(unreachable),
        )
    )
    return sold


if __name__ == "__main__":
    r = main()
    sys.exit(0 if isinstance(r, dict) else r)
