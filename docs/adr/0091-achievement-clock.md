# ADR 0091 — Real-time achievement clock: nominal game time

## Context
Several rules in `src/achievements.c` compare `SDL_GetTicks()` values:
Quick Draw (death < 3 s after `level_start_ticks`), Speed Demon (level < 90 s),
Combo King / Execution Chain (`stats.c:stats_count_kill`, 2 s window),
Firepower / Overkill (`dps.c`, 1 s window), Boss Shredder (< 10 s after
`EVENT_BOSS_SPAWNED`), Perfect Boss and Dodge Artist (`last_hit_ticks`).
The simulation is integer-based and deterministic (ADR 0003) and has no
wall clock. The C golden master runs without waiting: in real time, the
achievement log depends on the machine's speed (Speed Demon and Quick Draw
trigger because the level lasts a few seconds of wall time).

## Decision
- tyrian-sim counts a **nominal game time**: 28.75 ms per frame, the C
  value with the default setting (`config.c:JE_setNewGameSpeed`, fastPlay 0:
  `setDelaySpeed(0x4300)` × `frameCountMax` 2 × `pitPeriod` 12/14318180 s).
  Clock = 1,000,000 ms + frames × 28.75 ms (`achievements.rs:clock_ms`).
- On the C side, only under `make GOLDEN=1` and with `GOLDEN_ACH_CLOCK=1`:
  `-Wl,--wrap=SDL_GetTicks` replaces all clock reads with the
  same formula, the frame counter advancing in `golden_master_frame_top`.
  Outside level frames (menus, wait loops), the clock advances by
  1 ms per call so that waits terminate. Without the variable, the real
  clock is returned: the normal `make` and the golden masters do not change
  (the .gm produced with the virtual clock is identical to the versioned golden master).
- The events read by the listeners (`EVENT_ENEMY_KILLED`, `SHOT_HIT`,
  `PLAYER_HIT`, `BOSS_SPAWNED`, `BOSS_DEFEATED`, shots created) are counted in
  `World::ach_events`, not hashed, with no effect on the simulation.

## Consequences
- On `e1-tyrian-hard-autopilot.gm`, the C log under the virtual clock
  (16 achievements) is reproduced by tyrian-sim identically, same order, nothing
  more (`tests/achievements.rs:full_unlock_log_matches_c_with_virtual_clock`).
- Round 4: the Firepower II gap came from the phase of the one-second DPS window
  (`dps.c:stats_dps_update`). `lastFrameTicks` was taken before the
  level was loaded (tyrian2.c), during which the virtual clock advances by
  1 ms per call. Fix on the instrumentation side only: under
  `GOLDEN_ACH_CLOCK`, `golden_master_level_clock_start()` freezes the clock as soon as
  loading ends and `lastFrameTicks` is re-read there (`#ifdef GOLDEN_MASTER` block
  of tyrian2.c); tyrian-sim counts the first frame as a nominal frame.
  The run's .gm and the C log remain identical to the versioned files.
- In real play (Godot), nominal game time differs from wall time if the
  frame rate is not held or if the user changes the game speed: time-based
  verdicts follow game time, not wall time. Rules based purely on wall time (Night Owl, Night Player,
  session duration) are not ported into the simulation.
