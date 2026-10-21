# ADR 0134 — Mod limits sized for the shipped packs: ship images, ship poses, shop lists

Status: accepted (2026-10-18). Extends ADR 0080 (mod format) and ADR 0083 (single format).

## Context
Five content packs ship in `rewrite/mods/` (two ship packs, three weapon packs); the C-version
examples `alien_pack`, `example_nova_weapons` and `example_behaviors` were moved to
`rewrite/mods/_retired/` (kept, tested, never loaded or packaged). With the five packs enabled
at once, three limits inherited from the Enhanced C dropped content:

1. **Ship image table.** `sprite_override.c` gives the mods' "ships" table 1024 entries and places
   new ship PNGs from 805 in steps of 5 (each neutral image is copied to -4, -2, +2, +4 around
   it): 44 PNGs, 8.8 ships with five poses. The Alien Ship Pack alone takes 40, so the Bio Ships
   Pack was drawn partly or not at all ("table ships full").
2. **Copies to `shipgraphic - 500`.** The C copied each mod ship and its poses to
   `shipgraphic - 500` as well, the index of the sheet-12 (T2000) ships. From the 41st ship PNG
   these copies land on the original T2000 ships' own images (Storm 505, Red Dragon 543,
   Gencore II 581, PeteZoomer 619, Rum Bottle 657): `level.gd` reads the mods' table at an
   original ship's `shipgraphic` and would draw that ship with a mod image.
3. **Shop lists.** `INJECT_AVAIL` adds the mods' `always_available` items to a list of 10
   (`itemAvail[9][10]`) that already holds the level's items: 14 mod ships or 9 mod front guns
   do not fit, and the last packs in folder order were never offered.

## Decision
1. **Ship image table: 1160 entries** (`tyrian-mods/src/folder.rs`), computed from the mod ship
   slots: 805 + 14 slots × 5 poses × step 5 + the +4 pose copy of the last one. Every one of the
   14 slots (`SHIP_RANGE` 19..=`SHIP_NUM`, checked at compile time) can hold five PNGs. The
   automatic placement (base 805, step 5) is unchanged, so a mod that loaded before resolves to
   the same indices.
2. **No more copies to `shipgraphic - 500`** (`resolve/sprites.rs:auto_bank_ships`): the poses
   sit around the ship's own `shipgraphic` only, and `level.gd` draws a mod ship from the mods'
   table at `shipgraphic + bank × 2` (full index). The `- 500` stays where it belongs, on the
   original sheets (`main_12_ships_t2000`).
3. **Shop list capacity: the level's 10 + the mods' 16** (`tyrian-sim/src/shop.rs:SHOP_LIST_CAPACITY`,
   16 = `MAX_ALWAYS_AVAIL` of `tyrian-mods`); the provisional power array of a weapon list
   follows it. The list scrolls five rows at a time, as before.

Consumers checked, none tied to the old sizes:
- `shipgraphic` is a `u16` in the item tables (`tyrian-formats/src/hdt.rs`); saves record items
  by name and the loadout by item number (ADR 0129), never a sprite index.
- The simulation reads `shipgraphic` only to compare it with 1 (`player.rs`).
- Godot: `Mods.texture()` is a dictionary keyed by "table:index" (no array); `shop.gd` draws a
  mod ship at its neutral `shipgraphic`; `tyrian_screen.gd`, `new_game.gd`, `story_screens.gd`
  draw original ships from the original sheets only.
- The sprite manifest accepts named indices below the table capacity: the larger table only
  admits more names; the original ships' names (1..657) are unchanged.
- The shop list is built per visit and never saved (saves keep the level's `]I[` lists).

## Consequences
- All five packs enable together: `tyrian-mods check --enable-all` gives 0 errors and 0 warnings
  on all five episodes (14 of 14 ship slots, 70 of 70 ship images, 18 of 20 weapon slots).
- Tests: `tyrian-mods` `fourteen_mod_ships_with_five_poses_resolve` (each pose of the 14 ships at
  `shipgraphic + bank × 2`, no mod image over an original ship), `shipped_mods_load_without_errors`
  (no diagnostic at all), `retired_example_mods_still_resolve`; `tyrian-sim`
  `always_available_items_are_injected_like_the_c`, `full_weapon_lists_are_browsable`; gdUnit
  `mod_ship_poses_test.gd`; player journey `shop-mods` with the five packs.
- Divergence 61 (`docs/divergences.md`). Original content and golden masters are unchanged: none
  of these paths runs without a mod.
