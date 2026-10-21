"""sprite_manifest — name↔index resolution for mod sprites.

`data/sprite_manifest.json` maps human names to binary indices across
every moddable category (ships, shots, weapon_icons, sidekicks,
pickups, coins, explosions, enemies). Half the util scripts need to
ask "does this sprite name exist?" or "what's the next free index for
an auto-registered PNG?" and each script had its own half-answer.

All of that lives here now. Keep this module stdlib-only so scripts
running in minimal CI environments can still import it.
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

_UTILS_DIR = Path(__file__).resolve().parent
if str(_UTILS_DIR) not in sys.path:
    sys.path.insert(0, str(_UTILS_DIR))
import _json5 as json5_py  # noqa: E402

# Sections in mod.json → sprites/<subdir>/*.png lookup path(s).
# When a `ship_sprite` / `icon_sprite` / projectile `sprite` is given,
# the loader (src/sprite_override.c) looks for matching PNGs under
# these subfolders of the mod's sprites/ tree. This table mirrors the
# `SECTION_SPRITE_DIRS` constant in validate_mod.py; importing from
# here keeps them in lockstep.
SECTION_SPRITE_DIRS: dict[str, list[str]] = {
    "ships": ["ships"],
    "weapons": ["shots", "weapon_icons", "pickups/weapons"],
    "sidekicks": ["sidekicks"],
    "shields": ["pickups", "pickups/shields"],
    "specials": ["pickups", "pickups/specials"],
}

# Matching keys in sprite_manifest.json.
SECTION_MANIFEST_KEYS: dict[str, list[str]] = {
    "ships": ["ships"],
    # "pickups" was missing from the call in previous versions.
    #
    # The `itemgraphic` of a weapon port indexes shopSpriteSheet -- the
    # "pickups" space -- and not spriteSheet8. Without this entry, mod_migrate.py
    # translated `itemgraphic: 7` to "orange_fireball_right" (a PROJECTILE)
    # instead of "weapons/pulse_cannon" (the shop icon), and
    # mod_validate.py refused the correct name while accepting the wrong one.
    # "shields" and "specials" were already correct; only "weapons" was
    # not.
    "weapons": ["pickups", "shots", "weapon_icons"],
    "sidekicks": ["sidekicks"],
    "shields": ["pickups"],
    "specials": ["pickups"],
}


def repo_root() -> Path:
    return _UTILS_DIR.parent


def load_manifest(path: Path | None = None) -> dict[str, Any]:
    """Parse data/sprite_manifest.json (JSON5). Returns {} on failure."""
    if path is None:
        path = repo_root() / "data" / "sprite_manifest.json"
    try:
        with open(path, encoding="utf-8") as f:
            return json5_py.load(f)
    except OSError:
        return {}
    except Exception:
        return {}


def resolve_name(manifest: dict[str, Any], name: str, section_keys: list[str]) -> int | None:
    """Look up `name` in the given manifest subsections.

    Returns the numeric index or None. Matches exact basenames (no
    stripping/normalization) — callers should strip `.png` themselves
    if needed.
    """
    for key in section_keys:
        sect = manifest.get(key)
        if isinstance(sect, dict) and name in sect:
            v = sect[name]
            if isinstance(v, int):
                return v
            if isinstance(v, dict) and isinstance(v.get("index"), int):
                return v["index"]
    return None


def resolve_index(manifest: dict[str, Any], index: int, section_keys: list[str]) -> str | None:
    """Reverse lookup: index → first matching name in the subsections."""
    for key in section_keys:
        sect = manifest.get(key)
        if not isinstance(sect, dict):
            continue
        for name, v in sect.items():
            hit = v if isinstance(v, int) else (v.get("index") if isinstance(v, dict) else None)
            if hit == index:
                return name
    return None


def sprite_exists(name: str, section: str, sprites_dir: Path, manifest: dict[str, Any]) -> bool:
    """Return True if `name` resolves to a mod-local PNG or manifest entry.

    `section` is one of SECTION_SPRITE_DIRS keys. `sprites_dir` is the
    mod folder's `sprites/` root (may not exist for manifest-only mods).
    """
    base = name[:-4] if name.lower().endswith(".png") else name

    for sub in SECTION_SPRITE_DIRS.get(section, []):
        if (sprites_dir / sub / f"{base}.png").is_file():
            return True

    return resolve_name(manifest, base, SECTION_MANIFEST_KEYS.get(section, [])) is not None
