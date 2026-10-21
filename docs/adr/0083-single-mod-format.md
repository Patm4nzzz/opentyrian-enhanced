# ADR 0083 — Single mod format, designated by name

Status: accepted (2026-10-05, the maintainer's decision), completed by ADR 0128 (original content by name). Replaces the "formats 1 and 2" part of
ADRs 0080 and 0081.

## Context
The Rust loader (`tyrian-mods`) copied the C version's: two formats (`mod.json`
without `format` = C mod, `"format": 2` = rewrite sections), numbers inherited from the
original tables that the author could or had to write (`_weapon_id`, enemy `id`, `sprite`
by index or `sprite_NNN`, PNGs named by index, `starter` by number, a special's `weapon_id`,
`bigshipgraphic`), aliases (`fire`, `count`, `max`, `dmg`, `life`, `speed`), fields
with no effect (`power_drain`, a ship's `speed`) and a list of known keys mixing
both formats, with unknown keys only reported. the maintainer's rule: a legacy is
eliminated, no compatibility mode.

## Decision
- A single format, `"format_version": 1` mandatory. A `mod.json` without this key (a C
  version mod) or with another version is listed as an error, never interpreted.
- Everything the mod creates is designated by its name: weapons, ships, sidekicks, shields,
  specials, enemy weapons, new enemies, sprites. The slots in the tables are
  assigned by the loader, in the C version's order, and are never written in the mod.
- A number designates only an original entry that has no name: an enemy's `base` / `original`,
  a level's `eventdat`, a turret's original weapon (`tur`, `turrets[].weapon` outside the
  mod weapons).
- Single `enemies[]`: `name` = new enemy, `original` = modification of an original enemy.
  `new_enemies`, `id` and `behavior.version` go away.
- Any unknown key, at every level (root, objects, weapon levels, projectiles,
  behaviors, start, levels), is an error.
- The three C version mods (`data/mods/`) are converted once into `rewrite/mods/`
  (the originals stay for the C executable). The rewrite no longer reads `data/mods/`:
  folders read are `user://mods` then `rewrite/mods`.
  (Note of 2026-10-12: the C executable was removed by ADR 0132; `data/mods/` now only
  keeps the originals as the source of the conversion, read by nothing.)

## Evidence
- Equivalence of the conversion: for the four mods, the full resolution (table
  patch, sprites, shop, identities, start, enemies and enemy weapons, levels) is
  byte-for-byte identical to that of the old loader on the original mods, episodes 1 to 5;
  2000-tick runs with identical fingerprints on ep1_09, ep2_01, ep3_01, ep4_01, ep5_01.
- Tests `c_format_mods_are_rejected`, `legacy_numbering_is_rejected`,
  `turret_weapon_of_mod_is_cited_by_name_only` (`tyrian-mods/tests/mods.rs`).
- `--strict` golden masters unchanged (no active mod: empty patch).

## Consequences
A C version mod must be converted (MODDING.md §11). The editor's Tyrian dock
exports the single format; `tyrian-mods check` reads `rewrite/mods` by default.
