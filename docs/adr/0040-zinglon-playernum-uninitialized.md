# ADR 0040 — Soul of Zinglon beam: uninitialized `playerNum`

## Status
Revised 2026-10-04 (project lead's reframing: no "C fidelity" mode, C bugs
are eliminated). The first version reproduced the stack value of the GCC binary;
that code is removed.

## Context
`shot_update.c:JE_updatePlayerShots` declares `uint8_t playerNum;` with no value in the
body of the loop over shots. `shots.c:player_shot_move_and_draw` does not write
`*out_playerNum` for slot `MAX_PWEAPON - 1` (Soul of Zinglon beam). An
enemy killed by this beam therefore credits `player[playerNum - 1].cash` with an
indeterminate value (undefined behavior; with 0, an out-of-bounds write to `player[-1]`).

## Decision
Behavior corrected in `tyrian-sim`: the beam belongs to player 1 (the C code already places it
at `player[0].x + 7`), who is credited with the value of the enemies killed.

Revision of 2026-10-05 (card t_401e6b69, point 6): the bug is also fixed in the C code
(`shot_update.c:JE_updatePlayerShots` sets `playerNum = 1` for slot
`MAX_PWEAPON - 1`), then all C references are regenerated. The comparator exception
(`ACCEPTED_CASH_GAPS`, steps +45/+65/+69 on
`e1-special-zinglon-pulse`) and its tests are removed.

## Consequences
The Rust code no longer depends on the stack contents of the C binary, and the C code no longer credits an indeterminate
player. `e1-special-zinglon-pulse` is identical tick for tick, cash
included (C = Rust = 10140 at tick 327); no tolerance remains in
`tyrian-golden`. Entry 14 of `docs/divergences.md` closed.
