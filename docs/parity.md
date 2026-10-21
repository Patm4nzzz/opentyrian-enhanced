# C ↔ Rust gameplay parity (phase 5)

> The C version is no longer in the repository (ADR 0132): the golden masters are frozen
> files, replayed without the C code. New ones can only be recorded from the `c-final` tag.

Measured by `tyrian-golden` (`rewrite/crates/tyrian-golden`): each golden
master recorded by the instrumented C version (`make GOLDEN=1`,
`utils/golden/record.sh` from the `c-final` tag, ADR 0006 format) is replayed tick by tick by
`tyrian-sim` from its inputs alone (key mask) and its
header (episode, level file, difficulty, seed, helpers).

Command (from `rewrite/`):

```
cargo run --release -p tyrian-golden -- --data ../data golden/*.gm
```

The JSON summary is written to `rewrite/golden/parity.json`. The test
`crates/tyrian-golden/tests/parity.rs` forbids any regression relative
to `crates/tyrian-golden/tests/baseline.json`.

## Compared groups

| Group     | Content (see `src/golden_master.c`)                                                    |
|-----------|----------------------------------------------------------------------------------------|
| `player`  | x, y, armor, shield, money + `hash_player` hash (2 players, life, invulnerability, explosion, weapon power) |
| `scroll`  | `curLoc` + `hash_scroll` hash (curLoc, eventLoc, mapY×3, backMove×3)                   |
| `enemies` | enemy count + `hash_enemies` (slot, enemyAvail, position, speed, armor, type)          |
| `pshots`  | player shot count + `hash_player_shots` (position, sprite, damage)                     |
| `eshots`  | enemy shot count + `hash_enemy_shots`                                                  |
| `rng`     | `rng.c` state (xoshiro256**) — reveals any missing or extra draw                       |
| `mt`      | `mtrand.c` state (MT19937; not consumed in game, serves as a witness)                  |

## Results (2026-10-04)

| Golden master                 | Ticks | Identical from the start | player | scroll | enemies | pshots | eshots | rng | mt | End of level |
|-------------------------------|------:|---------------------------:|:------:|:------:|:-------:|:------:|:------:|:---:|:--:|:-------------:|
| e1-tyrian-script              | 1 162 | 1 162 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-tyrian-autopilot           | 6 967 | 6 967 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-tyrian-hard-autopilot      | 6 968 | 6 968 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-asteroid1-autopilot        | 5 106 | 5 106 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-asteroid2-autopilot        | 7 464 | 7 464 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-savara-autopilot           | 4 909 | 4 909 | OK | OK | OK | OK | OK | OK | OK | identical |
| e2-first-autopilot            | 6 423 | 6 423 | OK | OK | OK | OK | OK | OK | OK | identical |
| e4-first-autopilot            | 7 445 | 7 445 | OK | OK | OK | OK | OK | OK | OK | identical |
| e3-first-autopilot            | 3 051 | 3 051 | OK | OK | OK | OK | OK | OK | OK | identical |
| e4-dreadnot-autopilot         | 12 029 | 12 029 | OK | OK | OK | OK | OK | OK | OK | identical |
| e4-lavaexit-autopilot         | 5 063 | 5 063 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-assassin-autopilot         | 17 228 | 17 228 | OK | OK | OK | OK | OK | OK | OK | identical |
| e5-first-autopilot            | 3 462 | 3 462 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-orbit-autopilot            | 6 954 | 6 954 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-pearl-autopilot    | 6 965 | 6 965 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-blade-autopilot    | 6 969 | 6 969 | OK | OK | OK | OK | OK | OK | OK | identical |
| e2-special-vulcan-autopilot   | 6 423 | 6 423 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-repulsor-autopilot | 6 968 | 6 968 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-zinglon-autopilot  | 7 155 | 7 155 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-attractor-autopilot | 6 968 | 6 968 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-flare-autopilot    | 6 967 | 6 967 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-sandstorm-autopilot | 6 966 | 6 966 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-protron-autopilot  | 6 971 | 6 971 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-astral-autopilot   | 6 970 | 6 970 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-minefield-autopilot | 7 156 | 7 156 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-microsol-autopilot | 7 085 | 7 085 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-blade-pulse        | 1 242 | 1 242 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-sandstorm-pulse    | 5 941 | 5 941 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-astral-pulse       | 5 978 | 5 978 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-flare-pulse        | 5 664 | 5 664 | OK | OK | OK | OK | OK | OK | OK | identical |
| e2-special-vulcan-pulse       | 184 | 184 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-repulsor-pulse     | 2 190 | 2 190 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-zinglon-pulse      | 5 721 | 5 721 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-invuln-pulse       | 9 319 | 9 319 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-repair43-pulse     | 7 677 | 7 677 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-repair23-pulse     | 9 319 | 9 319 | OK | OK | OK | OK | OK | OK | OK | identical |
| e1-special-microsol2-pulse    | 9 319 | 9 319 | OK | OK | OK | OK | OK | OK | OK | identical |

"OK": no divergent tick over the whole file. "End of level
identical": `reallyEndLevel` occurs on the Rust side exactly after the last
recorded tick (`end level`). Total for this table: 240,348 ticks across 37 recordings, all identical. The `*-pulse`
scenarios replay `utils/golden/inputs/pulse_fire.in` (fire button released
periodically): 5 to 81 special triggers per level
(`rewrite/scripts/count_specials.sh`), versus a single one with the autopilot.

Special weapons (`GOLDEN_ITEMS=0:0:N`, triggered by pressing fire:
`varz.c:JE_doSpecialShot`): Pearl Wind (2, stype 1), Blade Field (7, stype 8,
shots at random positions), Dual Vulcan (10, stype 9, bound to the player), Repulsor (1, stype 2), Soul of Zinglon
(3, stype 3), Attractor (4, stype 4), Flare (6, stype 5),
SandStorm (8, stype 6), Protron Dispersal (12, stype 10), Astral Zone (13,
stype 11), Minefield (20, stype 16), MicroSol Option (31, stype 17).
Blade Field revealed a discrepancy at tick 0: in
`player_shot_create(..., rng_next() % 280, rng_next() % 180, ...)`, GCC x86-64
evaluates arguments right to left (y drawn before x). Fixed, and applied
to the two other calls of this kind (`tyrian2.c` random explosions,
`player_movement.c` Nort sparks), not yet covered by a golden master.

`e1-orbit-autopilot` equips two orbiting sidekicks (style 4: Satellite
Marlo 30 on the left, Bubble Gum-Gun 31 on the right) via `GOLDEN_ITEMS=30:31:0`
(`golden_master.c:apply_golden_items`). Counter-test: shifting the left
sidekick by one pixel on the Rust side makes `player`/`enemies` diverge at tick 242.

Counter-test: the same file replayed with a different seed diverges from
tick 0 on `rng`/`mt` (the comparator does detect discrepancies).

## Actual coverage

The 37 recordings of the table above exercise: scrolling of the three layers and
level events of episodes 1, 2 and 4, scripted and random
spawns, turrets and enemy launches, aimed shots (floating-point
computation reproduced), shot/enemy and player/enemy collisions, linked
destruction chains (`linknum`), pickable objects, bosses (health bars,
BOSS_DEFEATED listener), repeated explosions, end of level, difficulties 2 and 4, player
death without helpers (`e1-tyrian-script`).

They also cover the special weapons listed above (`e1-special-*`,
`e2-special-*`) and the first level of episodes 3 and 5. They do not
exercise: special-weapon key combinations, sidekicks other than orbit,
the Nort ship, two-player mode, Super Arcade and Galaga.

The table is the 2026-10-04 snapshot. `rewrite/golden/` has since grown
to 58 top-level `.gm` files (44 `e*`, 13 `mode-*`, 1 `supertyrian-*`) plus
the `campaign/`, `demo/`, `endless/` and `endless-chain/` subfolders; some of
them cover the paths listed just above (`e1-nort-pulse`, `mode-2p-*`,
`mode-galaga`, `mode-superarcade*`). The latest run written to
`rewrite/golden/parity.json` holds 76 entries, all identical to the end.
Paths not covered by any golden master are ported but not validated: see
`docs/divergences.md`.
