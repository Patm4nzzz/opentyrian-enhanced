# ADR 0070 — Evaluation of the `origin/wip/sept-2026` branch (439eee2e)

Date: 2026-10-04 — status: accepted (decision taken alone, "endless" batch).

## Context

`origin/wip/sept-2026` (commit 439eee2e, "uncommitted C work from 5 to 8
September 2026" — original commit message in French) is a raw backup of the maintainer's WSL copy,
placed on top of `master` @ 394b1fb7 (v0.9.13). 397 files, +15,803 / −10,394.
The branch was not modified: it was read with `git diff`/`git show`, then
built and tested on copies extracted with `git archive` into
`~/ot-eval-t23-endless/` (WSL), deleted after the evaluation.

A large part of the diff is cosmetic: French comments removed or
rewritten in English (more than half of the lines of `src/`), renaming
`opentyrian2000` → `opentyrian` (binary, `Makefile`, `.desktop`, man
page, Windows manifest), rewritten changelog. A comparison of the C without
comments (`/tmp/t23e_strip.py`, lines of code actually modified)
gives the real scope: `title_screen.c` (229), `anomalies_ui.c` (223),
`endless.c` (164), `mod_identity.c` (163, new), `mod_manager.c` (123),
`player_movement.c` (120), `particles.c` (108), `game_menu.c` (89), then
tweaks of fewer than 50 lines.

## Checks actually performed (WSL, gcc, SDL2 2.30)

| Check | Result |
|---|---|
| `make -j32` | builds without errors (7.6 s), `opentyrian` binary |
| `make test` | 1603 tests, 1603 passed (including `test_mod_identity`, `test_endless_math`) |
| `make smoke-test` | PASS |
| `make check` | 10/10 checks (locales, glyphs, mod schema and validation, 43 Python tests); 3 expected warnings on `example_nova_weapons` (incomplete power levels) |
| `--goto=endless` with each mod enabled alone (11 runs of 10 s under Xvfb) | each mod loads (`mod_manager: loaded …`) and wave 1 starts |
| `--autopilot --speed=6 --goto=endless`, 7 min, Sept binary and master binary in parallel | identical rotation for the same draw (block 1 = episode 1 sections 13 23 6 16 3), armor multiplier 1.00 → 1.34, switch to the episode 2 block; no errors |
| The 8 new mods copied into a **master** `data/mods/` (master binary) | all 8 load there too (same number of weapons, ships, sprites): the `mod.json` format remains compatible; `ship_poses` and `spawn_x/spawn_y` are simply ignored by master |

Cross-check (run 3, 2026-10-04 12:00): `git archive` of 439eee2e
into a fresh WSL copy, `make -j32` → exit code 0, `make test` → "1603 tests:
1603 passed, 0 failed". The other rows of the table come from run 2
(interrupted) and were not rerun.

## Mod inventory (for the "mods" batch)

All in `data/mods/<folder>/` on the branch; extraction:
`git archive origin/wip/sept-2026 data/mods | tar -x`. Each new pack
contains `mod.json`, `README.md`, `ART_PROMPTS.json` (image generation
prompts, not loaded by the game), `preview.png`,
`preview_3x.png` and its PNGs under `sprites/`.

| Folder | Name (version) | `mod.json` contents | Files | Enabled by default | Observed state |
|---|---|---|---|---|---|
| `phenix` | Phenix - Sept oiseaux (0.2.0) | 7 ships, `starter` (ship) | 40 (35 pose PNGs) | yes | loads (master and Sept) |
| `plasma` | Plasma - Six modules (0.2.0) | 6 weapons, `starter` (weapons) | 17 | yes | loads (master and Sept) |
| `steampunk` | Steampunk - Copper Wasp (0.3.0) | 1 ship, 2 weapons, `starter` | 14 | no | loads |
| `dragons` | Dragons - Ruby Drake (0.3.0) | 1 ship, 2 weapons, `starter` | 14 | no | loads |
| `bioalien` | Bioalien - Spore Manta (0.3.0) | 1 ship, 2 weapons, `starter` | 14 | no | loads |
| `star_trek` | Star Trek - Constitution (0.1.0) | 1 ship, 2 weapons, `starter` | 14 | no (archived) | loads |
| `star_wars` | Star Wars - X-Wing (0.2.0) | 1 ship, 2 weapons, `starter` | 14 | no (archived) | loads |
| `example_first_mod` | Premier pack (1.0.0) | 1 ship, 1 weapon (example from the `FIRST_MOD.md` guide) | 2 | no | loads |
| `alien_pack`, `alien_ships`, `example_nova_weapons` | already on master | only the `author` field changes | — | no | unchanged |

The ship poses (`ship_poses`: neutral, left, left_full, right,
right_full, 24×28) and the shot offsets (`spawn_x`, `spawn_y`, formerly
`accel_x/accel_y`) are described in the branch's `data/mod.schema.json`.
Related documentation on the branch: `FIRST_MOD.md`,
`docs/MOD_COLLECTION.md`, `docs/MOD_BALANCE.md`, `docs/art/*`.

## Decision

1. **`master` remains the oracle** (ADR 0001). The golden masters are
   recorded on `master`'s C code; nothing from the branch is merged into the
   reference C code.
2. **Integrated into the rewrite: the mod inventory above**, to be loaded
   by the "mods" batch from the branch (data only: `mod.json` +
   PNGs, format compatible with master, verified). The format additions
   (`ship_poses`, `spawn_x/spawn_y`) are to be ported on the rendering side by that batch;
   they do not touch the simulation except `spawn_x/spawn_y` (shot
   origin), to be handled as weapon data.
3. **Endless mode: the porting reference is `master`**
   (`src/endless.c`, `src/endless_math.c`). Both files exist on
   master; the branch changes only the wrapping around Endless (corruption cards
   frozen for the run `s_run_cards`, abandoning from the modal,
   ELBD v4 leaderboard, ENR3 snapshot tied to `mod_identity`, mouse in the end
   screens, leaderboard colors). Rotation, armor
   multiplier, lives and score are identical (verified by the same rotation draw
   on both binaries).
4. **Rejected**:
   - the split of the cosmetic generator (`rng_fx_*` in `rng.c`,
     called by `varz.c`, `enemy_render.c`, `explosion_update.c`,
     `hitstop.c`, `particles.c`, `mainint.c`…): it changes the consumption
     of the gameplay stream; adopting it would invalidate the ten
     golden masters of `master` and the Rust parity. It does go in the
     direction of ADR 0003 (cosmetic randomness stays on the Godot side): nothing
     to port;
   - `mod_identity` (save identities of mod objects, MIDS
     section): specific to the C save; the rewrite has no
     compatible save. To be reconsidered by the "mods" batch if a
     save is made;
   - the corruption cards / anomalies, reworked on the branch: already
     outside the simulation's scope (divergence no. 6);
   - the renaming `opentyrian2000` → `opentyrian`, the `marketing/` site,
     the `docs/validation/*` reports, the changelog: no effect on the
     rewrite;
   - the "Records" menu of `title_screen.c`/`menu_tree.c`: C
     interface, the rewrite has its own Godot menu.

## Consequences

- The "mods" batch has the list, the paths and the state of each
  pack; it does not depend on merging the branch into `master`.
- The branch was not touched; the evaluation copies in WSL are
  deleted after use.
