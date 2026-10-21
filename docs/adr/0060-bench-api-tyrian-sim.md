# ADR 0060 — Benchmark API in `tyrian-sim`

Status: accepted ("ci" batch, block 12). Updated 2026-10-05 (m5 review): the
"bullet hell" mutator has since been ported.

## Context

Block 12 calls for measuring the game with thousands of projectiles. The
"bullet hell" mutator (`enemy_render.c:JE_drawEnemy`, `mut_bullet_hell`: one
extra `eshotwait` countdown per tick) is ported (`modes.rs:MUT_BULLET_HELL`,
applied in `enemies.rs`, proven by the golden master
`mode-mutators-autopilot`), but it only makes shots come sooner: they remain
capped by `ENEMY_SHOT_MAX` = 60 (`varz.h`, `world.rs`). The game itself
therefore cannot reach those volumes.

## Decision

- `tyrian-sim` gets a `bench` module (`bench_set_enemy_shot_capacity`,
  `bench_fill_enemy_shots`, `bench_enemy_shot_count`), exposed to Godot by
  `tyrian-godot/src/bench.rs`. The `bench_` prefix ensures no in-game call
  goes unnoticed during review.
- The loops over enemy shots iterate over `enemy_shots.len()` instead of
  `ENEMY_SHOT_MAX`. The initial capacity stays 60: without a `bench_*` call,
  behavior is that of the C version (golden masters unchanged).
- Injected shots add no rule: they follow
  `shot_update.c:JE_updateEnemyShots` like the others. Their generator is
  local (SplitMix64) so as not to consume the game's random draws.

## Rejected alternatives

- A `bench` Cargo feature: the GDExtension and the example would have to enable it, and
  CI would build two variants of the core for no gain (the code is
  inert without a call).
- Enlarging `ENEMY_SHOT_MAX`: it would change `JE_drawEnemy`'s free-slot search
  (first free among 60) and therefore parity.
