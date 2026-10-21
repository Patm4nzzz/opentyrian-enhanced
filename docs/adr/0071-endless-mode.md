# ADR 0071 — Endless mode in the rewrite

Date: 2026-10-04 — status: accepted (decision taken alone, "endless" batch).

## Context

`src/endless.c` / `src/endless_math.c` exist on `master`, which remains the
reference (ADR 0070). The mode mixes a deterministic part (level rotation drawn from `rng.c`, armor multiplier read by
`enemy_spawn.c:JE_makeEnemy`, arcade lives) and a lot of interface (name entry,
summary, encrypted ELBD leaderboard, ENDR resume, anomaly draw).

## Decision

1. **Dedicated golden master, outside the level .gm format**: an Endless run chains levels and goes through
   modal screens; a per-tick .gm is not suited to it. Instead, the two
   functions that touch the simulation are checked exhaustively:
   - `build_level_pool` for the 5 episodes and the first 60 draws of
     `pick_next_level` (12 blocks, two rotation laps) for 4 seeds;
   - `endless_armor_mult_for_levels(0..130)` bit for bit (glibc `powf`) and
     the scaled armor for every base armor 1..254.
   Instrumentation: `endless.c:endless_golden_dump`, under `#ifdef
   GOLDEN_MASTER` (normal binary unchanged), triggered by
   `GOLDEN_ENDLESS_OUT`; script `utils/golden/record_endless.sh`; files
   `rewrite/golden/endless/*.egm`.
2. **`powf` without floating point**: `armor_mult_for_levels` computes 1.05f^n in
   64-bit fixed point then rounds to 24 bits; equality with glibc is
   proven by the golden master over the whole useful range (beyond ~110
   levels every armor is capped at 254 anyway).
3. **Seed**: the C code calls `rng_seed(time(NULL))`; the rewrite takes
   `Game.rng_seed` (`--seed=`) to keep a run replayable. An
   interface choice, not a game rule.
4. **Godot access**: the C version offers Endless in the New Game submenu
   (`menus.c`, `MENU_ITEM_1_PLAYER_ENDLESS`), which the rewrite does not have;
   we add the **E** key on the title screen and the `--endless` option
   (equivalent to `--goto=endless`), without modifying the title image (visual
   parity of the title captures preserved).
5. **Respawn and level golden master (round 3)**:
   `ModeSetup::endless_lives` carries `s_lives_p[]` in the Sim; the arcade
   branch of `player_movement.c:JE_playerMovement` calls
   `endless::try_respawn` (`endless_try_respawn`, refused with the "one life" mutator
   or the glass hull, counter set to 0) instead of `*lives`, and `tyrian2.c:JE_main`
   tests `endless_lives_of(p) <= 1` for `allPlayersGone`. The level golden master
   (`.gm` v2) adds a line `endless wave W lives A:B armor_mult
   BITS` (written by `golden_master_level_start` when `endlessMode`) and a
   line `endless_end score S lives A:B` at the end. Seed order verified:
   `endless_start` draws the rotation with `rng_seed(time)` — replaced under
   GOLDEN by `GOLDEN_SEED` —, then `golden_master_level_start` reseeds
   with the same seed at the start of the level: the Sim therefore starts from
   `Xoshiro256::new(seed)` as for any `.gm`.
6. **Score**: `endless.c:on_enemy_killed` (value × `combo_multiplier`, 50
   if ≤ 0, saturation) called at both emissions of `EVENT_ENEMY_KILLED`
   (`collision.c`). Combos are disabled under the golden master
   (`combo_enabled = false`), so the multiplier stays 1; porting the
   combos (timer in real milliseconds) remains outside the Sim.
7. **Screens** (Godot): HUD ("WAVE n" TINY_FONT + `JE_textShade`
   FULL_SHADE, score at (30, 175), life icons 285, intro banner
   SMALL_FONT_SHAPES 2 s), `JE_endLevelAni` debriefing between waves (campaign
   `debrief` scene), end of run (name, summary, leaderboard:
   `scenes/endless_end.tscn`), resume. The leaderboard and resume are `user://`
   JSON files, not the encrypted ELBD/ENDR sections of `opentyrian.dat`: an
   interface choice, no game rule. Resume replays the rotation
   from the seed (deterministic) instead of saving ep/lvl.
8. **Out of scope**: anomalies and mutators (divergence no. 6, card
   draw), two-player Endless run, resume choice screen
   (`endless_resume_prompt`: resume is done automatically), restart "R"
   from the Game Over screen.

## Consequences

- The 10 campaign golden masters do not change: `endless_armor_mult`
  is `None` outside Endless.
- Any change to `endless.c` that touches rotation or armor must
  regenerate `rewrite/golden/endless/` (`utils/golden/record_endless.sh`).
- HUD: drawn in RGBA by the level scene (not in the 8-bit VGA) with
  the TINY_FONT / SMALL_FONT sheets and the FULL_SHADE shadow in black; the C
  `colorbank/brightness` tints are approximated (fixed color).
- `level.rs:background_2_allowed`: the "water" condition (`smoothies[1]`)
  removed from layer 2 — at `processorType = 4` the C code draws layer 2
  even under water; SAVARA V (seed 7) revealed it (scroll hash at tick 5).
