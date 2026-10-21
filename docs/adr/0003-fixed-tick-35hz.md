# ADR 0003 — Fixed step at the original tick, integers only, RNG reproduced

Date: 2026-10-04 — status: accepted (decision made alone). Updated on 2026-10-05 (m5 review): the C `float` computations are reproduced by exact software arithmetic, not by tables.

## Context
`tyrian2.c:JE_main` runs one tick per `level_loop` iteration, paced by the emulated PIT timer (`nortsong.c:setDelay/wait_delay`): at Normal speed, `speed = 0x4300` and `frameCountMax = 2` (`config.c:JE_setNewGameSpeed`), that is one tick every 28.7 ms (≈ 34.8 Hz). Enhanced stretches this duration through the floating-point `game_speed` (bullet time, slow motion) without changing the logic of a tick.

## Decision
- `tyrian-sim` advances one tick per `step`, regardless of the display rate. Godot calls it from `_physics_process` at 35 Hz (integer rounding of 34.8 Hz; Godot only accepts integers). Bullet time acts on the calling clock (`Engine.time_scale`), never inside the core.
- No hardware floating point (`f32`/`f64`) in the game logic. The few C `float` computations that influence the state are reproduced bit for bit by integer software arithmetic (exact rationals then IEEE-754 round to nearest, ties to even):
  - `tyrian-sim/src/f32ops.rs`: `F32` type carried by its 32 bits, `sinf`/`cosf` replayed operation by operation after the glibc 2.39 algorithm of the machine that recorded the golden masters — orbiting sidekicks (`player_movement.c:JE_playerMovement`, `player.rs`), the Dragonwing's linked gun and the orientation of its shots (`link.rs`, `shots.c:player_shot_set_direction`);
  - `tyrian-sim/src/fmath.rs`: rounding of the difficulty multipliers (`enemy_spawn.c:JE_makeEnemy`, IEEE mantissas of 1.2f and 1.8f), of aimed shots (`enemy_render.c:JE_drawEnemy`, `round_div_mul`) and of the Endless armor multiplier (`endless.rs`) and of `mainint.c:adjust_difficulty` (`campaign.rs`).

  The `F32` names do not denote a hardware float: they are integers. The `sincos_match_hardware` test compares `sinf`/`cosf` against the machine's libm. No precomputed table replaces these computations. Any difference in results is recorded in `docs/divergences.md`.
- RNG: xoshiro256** reimplemented identically to `src/rng.c` (same seed → same draws), used only by the gameplay. Cosmetic randomness stays on the Godot side.

## Consequences
Exact parity with the C version requires that the oracle does not use this stream for its effects either: the phase 2 instrumentation turns off the RNG-consuming effects in golden master mode (ADR 0006).
