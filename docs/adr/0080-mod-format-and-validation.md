# ADR 0080 — Mods: loading C mods without conversion, bounded validation

Status: accepted (mods batch, block 8). Format superseded by ADR 0083 (single format, items designated by name):
the mentions of `data/mods`, `"format": 2`, `new_enemies` and the numbers below are historical.

## Context
The C version loads `data/mods/<name>/mod.json` folders (`src/mod_manager.c:mod_manager_load`,
`load_single_mod`), applies weapons, ships, sidekicks, shields, specials and enemies onto the
HDT tables (`hdt_json.c`) and assigns stable identifiers (`mod_identity.c`, branch
`wip/sept-2026`). The existing mods (alien_pack, alien_ships, example_nova_weapons and the 11 from
`wip/sept-2026`) must run in the rewrite unchanged. These files are untrusted.

## Decision
- New crate `tyrian-mods` (Rust), the only one that reads mod files. Godot interprets no
  file: the `TyrianMods` node (`tyrian-godot/src/mods.rs`) hands it the list, the diagnostics,
  the sprites, the shop items and the starting equipment, all already validated.
- Same dialect as the C version (`json5.c:cJSON_Parse5`): JSON + comments + trailing commas, duplicate key =
  first value, numbers converted like `cJSON.valueint`.
- Resolution in the C version's order, on a copy of the tables (`tyrian_sim::tables::Tables`); the
  result is a `TablePatch` (modified records) handed to `LevelSetup::patch`.
  Empty `patch` = original game: golden masters unchanged by construction.
- Bounds: 1 MiB text, depth 48, 16 KiB string, 200,000 JSON nodes (`json.rs`); 32 mods
  (`folder.rs:MAX_MODS`), PNG ≤ 4 MiB and ≤ 2048 px per side, 1024 PNGs per folder; 2500 events
  per level (`lvlmast.h:EVENT_MAXIMUM`), 40 random enemies, 64 level files per mod.
  Any violation becomes a diagnostic (error or warning); never a panic.
- Folders read, in order: `user://mods`, `rewrite/mods`, `data/mods`. Activation preferences
  in `user://mods.cfg` (taking priority over `"enabled"`); `mod.json` is never rewritten.
- In-game access: Setup → Mods, like `setup_menu.c`.

## Consequences
`cargo run -p tyrian-mods -- check` validates a folder outside the game, with the same messages.
Fields that the C engine ignores (e.g. ship `speed`) are reported as `[info]`.
