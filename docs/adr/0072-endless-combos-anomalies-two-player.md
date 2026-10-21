# ADR 0072 — Endless: fixed-clock combos, dormant anomalies, two players, choice screens

Status: accepted (endless batch, round 4).

## Context
`combos.c` measures the 1.5 s window in real time (`tyrian2.c:JE_main`,
`combo_update(SDL_GetTicks() - lastFrameTicks)`), and the golden master disables
combos (`golden_master.c`, `combo_enabled = false`). The Endless score
(`endless.c:on_enemy_killed`) and cash (`collision.c`, `evalue *
combo_multiplier`) depend on them. The anomaly deck (`anomalies.c`) is drawn
by an xorshift seeded with `SDL_GetTicks()`; `anomalies_enabled` is false by
default (`anomalies.c`, menu entry removed in `gameplay_menu.c`).

## Decisions
1. **Combos in tyrian-sim** (`combos.rs`), with a fixed loop duration per run
   (`Sim::enable_combos(frame_ms)`). Proof: `GOLDEN_COMBO_MS=N` (under
   `make GOLDEN=1` only) turns `combo_enabled` back on and replaces the wall clock
   with N ms at each loop; header line `combo frame_ms N …`.
   Two golden masters `endless-combo-seed1|7` (N = 28): 3600 identical ticks,
   cash and Endless score with multipliers (1633 / 720 versus 1400 / 632
   without combos). Godot runs at 35 Hz: `COMBO_FRAME_MS = 28`. The existing golden
   masters (combos disabled) do not change.
2. **Anomaly deck and score multipliers** in tyrian-sim
   (`anomalies.rs`): registry as in `data/anomalies.json` (the C code loads it at
   runtime; the 11 cards are hard-copied, `float` bits of
   `score_mult`), `anomaly_pick_three`, `anomaly_deck_score_multiplier`,
   `mutators_score_multiplier`, `endless_apply_score_mult`. Proof:
   `endless.c:endless_anomaly_golden_dump` (`GOLDEN_ANOMALY_OUT`), 4 seeds ×
   306 identical lines. `cooldowns[]` not ported: never set > 0 in the C code.
3. **Anomalies dormant by default**, like the C version; `--endless-anomalies` turns them
   on in Godot (draw between two waves, effects applied to the Sim
   by `set_endless_options`, multiplier applied to the final score). Deck seed:
   `run seed ^ 0x5eed` instead of `SDL_GetTicks()` (replayable run).
4. **Two players**: `tyrian_sim::endless::mode_setup(true, …)` and
   `start_loadouts_two_player()` (`endless.c:endless_start(true)`, ship_for[],
   Dragonwing's Vulcan). C golden masters `endless2-arcade-seed1|7`: 7662
   identical ticks; a test compares the C header with these functions.
5. **Resume and draw screen**: `endless_prompt` scene (8-bit, layout
   of `endless_resume_prompt` and `anomaly_ui_pick_modal`). The save remains
   `user://` JSON (no ENDR section of `opentyrian.dat`); it now carries
   two players, player 2's lives and the deck.

## Limitations
- Card: the name fits on one line (not the two-line split of `name_layout`).
- End summary: the "anomaly bonus" line and the card list are not
  drawn (base score and final score are).
