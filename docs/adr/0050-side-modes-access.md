# ADR 0050 — Access to side modes in the Godot rewrite

## Context

The C version spreads the side modes across several menu paths ("New Game" → two-player /
arcade, Super Arcade and Super Tyrian codes on the title screen, `menus.c:timedBattleSelect`, Galaga via
section 41 of episode 1, mutator and anomaly screens). Reproducing all these paths before
the Godot campaign loop (shop, episode selection) is complete would block access to the modes.

## Decision

- A single screen `scenes/modes_menu.tscn` (**M** key on the title screen) chooses the mode, the
  mutators and the anomalies, then starts the level through `TyrianSim.set_modes` (10 integers, order documented
  in `tyrian-godot/src/lib.rs`).
- The starting points (episode, level, difficulty, equipment) are those of the matching C golden masters
  (`rewrite/golden/mode-*.gm`), hence starting points whose simulation is proven identical to the C version.
- The modes' rules stay entirely in `tyrian-sim` (`modes.rs` and the functions cited); the
  screen only sets parameters.

## Consequences

- Interface divergence (not gameplay): a menu path different from the C version, to be replaced with the real
  menus once the campaign loop ports them.
- Player 2 plays on the shared keyboard (devices 1:1) until player 2's mouse/gamepad
  is wired up on the Godot side (block 14).
