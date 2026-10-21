# ADR 0081 — New weapons and enemies, behaviors described as data

Status: accepted (mods batch, block 16). Format superseded by ADR 0083 (single format, items designated by name):
the mentions of `data/mods`, `"format": 2`, `new_enemies` and the numbers below are historical.

## Context
A mod must be able to create a new weapon and a new enemy without recompiling Rust, and the
original behaviors must stay tick-for-tick identical (ADR 0003: integers only).

## Decision
- `"format": 2` in `mod.json` (`FORMAT_MAX = 2`). The C version silently ignores the sections
  specific to format 2; a format 1 mod is read exactly as the C version reads it.
- `enemy_weapons`: named enemy weapons, allocated in the mod projectile range
  (`hdt_json.c:next_free_mod_weapon_id`, 1800..) and filled by the same code as player weapons.
- `new_enemies`: new enemies, identifiers 900..999, copied from an original enemy (`base`) or
  from safe values; citable by NAME in `levels/*.json`.
- `behavior` (version 1) grafted onto the C enemy model (`enemy_render.c:JE_drawEnemy`) without
  replacing it (`tyrian-sim/src/behavior.rs`):
  - `path`: ≤ 32 segments `{ticks 1..10000, vx, vy -128..127}`, `loop`; they set `exc`/`eyc`,
    movement remains the C version's;
  - `track`: `{axis x|y|xy, max_speed 0..127, every 1..255}`, pursuit WITHOUT random draws;
  - `turrets`: ≤ 4 `{weapon (name or 1..WEAP_NUM), every, start 1..255, direction down|side|side_reverse}`,
    fired by the original turret code (`fire_enemy_turret`).
  An error rejects the entire behavior (never a half-behavior).
- The original content has `behavior = 0`: no new branch is taken and the random
  generator is not consumed. Verified: `tyrian-golden --strict` 10/10, 52,957 ticks.

## Consequences
Example mod `rewrite/mods/example_behaviors` ("Helix Driver" weapon, enemy weapon
`drone_spread`, enemy `zigzag_drone`); test `example_mod_new_weapon_and_enemy_run`
(`tyrian-mods/tests/mods.rs`); capture `docs/screenshots/mods/11-example-behaviors-zigzag-drone.png`.
