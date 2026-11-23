# Robustness of the `tyrian-sim` core (block 19)

Crate `rewrite/crates/tyrian-robustness`: a bot plays seeded random games on
the Rust core, without Godot. Each game depends only on a `u64` identifier:
level (70 levels of episodes 1 to 5), difficulty (1 to 10, `config.h`),
equipment, simulation seed, autopilot assists, inputs. A failing game is
replayed identically with `--only <id>`.

## What is checked

- **panics** during `Sim::new` or `Sim::step` (captured per game, with
  `file:line`; grouped by location in the summary); the `robustness` profile
  (workspace Cargo.toml) compiles in `release` with `overflow-checks` and
  `debug-assertions`: any overflow not explicitly `wrapping_*` becomes a
  detected panic;
- **non-reproducibility**: each game is played twice, in two passes in
  reverse order and with a different number of threads; the trace (FNV-1a of
  `frame_record()` at each tick + `state_hash()` every 32 ticks and at the
  end) must be identical;
- **API invariants**: `state().tick` advances by exactly 1 per `step`,
  constant `snapshot()` size.

Player 2 also receives inputs (mask derived from player 1's): the solo game
must ignore them with no effect on the trace.

`--wild-loadout` draws each equipment field over 0..=255, including outside
the tables (corrupt save or mod).

## Commands (from `rewrite/`)

```sh
# Short version (CI, ~1 s of computation):
cargo run --profile robustness -p tyrian-robustness -- --games 300 --max-ticks 3000
cargo run --profile robustness -p tyrian-robustness -- --games 300 --max-ticks 1000 --wild-loadout --first 1000000
# Long version (local):
cargo run --profile robustness -p tyrian-robustness -- --games 20000 --max-ticks 20000 --json /tmp/rob-long.json
cargo run --profile robustness -p tyrian-robustness -- --games 20000 --max-ticks 5000 --wild-loadout --json /tmp/rob-wild.json
# Replay one game (JSON detail, exit code 1 on a fault):
cargo run --profile robustness -p tyrian-robustness -- --only 1234 [--wild-loadout]
```

`cargo test` additionally runs `tests/short.rs` (24 normal games and 24
"wild" ones, 600 ticks, each replayed twice, then 12 games of 2,500 ticks with
the player's effects); it is skipped if `data/` is absent.

## Player effects (`--player-fx`)

The golden masters, the replays and the tools run with the effects that only the
player's game enables turned off (`tyrian_sim::fx::FxConfig`: freeze frames, Modern
explosions, effect events), because the C records its golden masters that way. These
paths were therefore played by no automated check, and two defects of the freeze
frames reached the players in v1.0-rc1 (docs/divergences.md, entries 64 and 65).
`--player-fx` turns them on for every game of the run:

```sh
cargo run --profile robustness -p tyrian-robustness -- --games 4000 --max-ticks 20000 --player-fx
```

Result (2026-11-22, Windows, 32 threads): 4,000 games, 41,390,604 ticks over the two
passes, 3,700 levels completed, 70 levels out of 70 drawn; 0 panic, 0 invariant
violation, 0 non-reproducible game.

The bot detects panics, overflows and non-reproducibility, not a wrong game rule (a
level that ends too early does not panic): those are covered by the tests of
`tyrian-sim/src/fx.rs` (`cargo test -p tyrian-sim hitstop_tests`).

The campaign bot of the Godot game (`rewrite/godot/tools/campaign_bot.gd`) has the same switch:
`--campaign-bot-player-fx` plays the five episodes through the real screens with the player's
Setup effects instead of the golden master settings.

```sh
godot --headless --path rewrite/godot -- --campaign-bot=<log.jsonl> --campaign-bot-player-fx
```

Result (2026-11-23, Windows, headless, speed 20): 45 levels ended, 5 episodes completed,
47 shops, 52 saves, 5 reloads checked, 0 failure and no error in the output, with and
without the switch (about 7 minutes each).

## Actual results (2026-10-04, WSL, 32 cores)

| campaign | games | ticks (2 passes) | levels completed | duration | panics | non-reproducible |
|---|---:|---:|---:|---:|---:|---:|
| table equipment, 20,000 ticks max (before fix) | 20,000 | 212,924,362 | 18,181 | 56.7 s | 0 | 0 |
| `--wild-loadout`, 5,000 ticks max (**before** fix) | 2,000 | 46,584 | 7 | 0.1 s | **1,991** | 0 |
| `--wild-loadout`, 5,000 ticks max (after fix) | 2,000 | 11,211,668 | 1,150 | 2.2 s | 0 | 0 |

All 70 levels were drawn (at least 237 games per level in the long campaign).

Final campaign (definitive code, after the fix and the change of the enemy
shot capacity to `enemy_shots.len()`, block 12):

| campaign | games | ticks (2 passes) | levels completed | duration | panics | invariants | non-reproducible |
|---|---:|---:|---:|---:|---:|---:|---:|
| table equipment, 20,000 ticks max, ids 0..20,000 | 20,000 | 212,924,362 | 18,181 | 45.8 s | 0 | 0 | 0 |
| `--wild-loadout`, 5,000 ticks max, ids 5,000,000..5,020,000 | 20,000 | 115,427,304 | 11,071 | 20.3 s | 0 | 0 | 0 |

Total: 40,000 games, 328,351,666 ticks played, 70 levels out of 70 covered
(at least 237 games per level).

## Defects found and fixed

Equipment outside the tables: `Sim::new` panicked (`index out of bounds`):

- `level.rs` (`World::start_level`): `tables.ships[players[i].items.ship]` —
  1,738 games;
- `player.rs` (`draw_options`, and the two other reads of
  `tables.options[sidekick]`) — 253 games;
- `shots.rs` (`player_shot_create`): `tables.ports[port_num]`, same cause
  (spotted on reading while fixing the first two).

Fix: clamping with `.min(*_NUM)` like the neighboring accesses to the same
tables (`tables.rs`: `weapon()` and `enemy()` already clamp). The C has no
direct equivalent (unchecked array overrun, `varz.c:JE_getShipInfo`); with
valid data the indices stay within the tables, so no effect on the golden
masters (10/10, 52,957 ticks identical after the fix).

No non-reproducibility or invariant violation observed.
