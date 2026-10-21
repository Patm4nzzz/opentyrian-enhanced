# ADR 0006 — Golden master: per-tick inputs and field-by-field state

Date: 2026-10-04 — status: accepted (decision made alone).

## Context
The existing replay (`replay.c`, `TYRP` v4 format) records positions, not inputs, with no state check. The gameplay consumes an RNG stream shared with the visual effects, and several Enhanced systems depend on the wall clock.

## Decision
- C instrumentation behind `-DGOLDEN_MASTER` (off by default; `make GOLDEN=1`). The normal binary is unchanged.
- In golden mode: fixed seed, gameplay-affecting Enhanced toggles turned off (hitstop, combos, bullet time, particles, screenshake, pitch variation, damage numbers), no wait between ticks, inputs read from a script (`one tick = one mask: up, down, left, right, fire, fire mode switch, left sidekick, right sidekick`).
- Output: text file `golden/<name>.gm` — header (episode, level, seed, equipment) then one line per tick: input mask, raw fields (player position and armor/shield, credits, `curLoc`, RNG state, number of live enemies, number of shots) and the FNV-1a 64 hash of each group (player, scroll, enemies, player shots, enemy shots). Per-group hashes make it possible to measure parity module by module.

## Consequences
`tyrian-golden` replays the same inputs in `tyrian-sim` and reports, per group, the first divergent tick.
