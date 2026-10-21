# Tests: the gate before every push

The maintainer found within two minutes of play that you could neither buy a weapon
in the shop nor see the chosen option in the pause menu, even though all the
checks passed. Those checks compared images (a hovered line lights up = pass) and replayed golden masters that never go through
the menus. Nothing verified that a player action has its effect.

Rule: **every push to `master`, and every build handed to the maintainer, first passes
the player journeys (all PASS) then `player-smoke.sh`**, locally
(WSL/Linux), on the exported Linux package.

```bash
rewrite/scripts/package.sh --only=linux          # or --build on the next command
rewrite/scripts/player-journeys.sh --core        # before every push: about 2 min 40 s
rewrite/scripts/player-journeys.sh               # full set, before a version is handed to the maintainer
rewrite/scripts/player-smoke.sh                  # images, golden masters, lint.sh; requires the run above
```

`rewrite/scripts/lint.sh` (called by `player-smoke.sh`, or on its own) checks
the formatting and warnings of all the maintained code, without rewriting anything:
cargo fmt and clippy `-D warnings` (Rust), gdlint, gdformat and Godot's
"unused" warnings treated as errors (GDScript), ruff (Python), shellcheck,
PSScriptAnalyzer (PowerShell, if `pwsh` is installed).
Configuration: `rewrite/rustfmt.toml`, `rewrite/godot/gdlintrc`,
`rewrite/godot/gdformatrc`, `ruff.toml`, `rewrite/scripts/PSScriptAnalyzerSettings.psd1`.
Tools: `uv tool install ruff==0.17.0`, `uv tool install gdtoolkit==4.5.0`,
`apt install shellcheck`, PowerShell 7 then `Install-Module PSScriptAnalyzer`.

Two games run at the same time (two Xvfb screens, never more). The core
(`--core`) plays nine lightened journeys at 1280x684; the full set plays all the
journeys in full, and the core also at 1920x1080.

`player-smoke.sh` fails (`player-journeys` check) if `player-journeys.sh`
did not pass everything on the same git head (commit and uncommitted
changes): the file `build/player-journeys.pass` proves it.

## What a journey verifies

Each journey is played like a player: real keys and real X clicks
(xdotool, mouse moved in small steps), once with the keyboard only and once
with the mouse only, 1280x684 window (game ×3 letterboxed) then 1920x1080 for
the core. After each action, the effect is read back in the game itself:

- the `JourneyState` autoload (`rewrite/godot/tools/journey_state.gd`), active
  only with `--journey-state=<file>`, writes four times per second the
  scene, the open menu (chosen line, clickable areas), the shop (selected
  item, owned, money, lines marked on screen), the equipment, the level
  (pause, pause menu, F3/F4/F6 overlays, armor), the settings;
- the files written by the game: `saves/slot_NN.json`, `setup.cfg`,
  `endless_leaderboard.json`, F12 captures.

A PASS always rests on an effect: money = money − price + trade-in, equipment
slot changed, equipment mark on the right line of the image,
option changed then kept when reopening and after restart, save read back
identically. An image alone never makes a PASS.

| Journey | Core | Content (full set; the core keeps the essentials) |
|---|:-:|---|
| shop | yes | new game (Normal with keyboard, Hard with mouse), purchase in each category, sale of the rear gun, Ship Specs, data cube (core: front gun and left sidekick) |
| shop-easy | | same journey in Easy |
| pause | yes | 20 s of play, each pause option and each value, speed applied on resume, settings saved, F2-F12, Quit box No then Yes (core: one value per option, without F2-F12) |
| finish-level | | purchase, level finished (Ctrl+Shift+N, `--devmode`), debriefing, next shop: equipment and money kept |
| save-load | yes | purchase, Options > Save Game, name entered, back to the title, Load Game: same money, equipment, section |
| title | yes | each title entry, Setup (Graphics, Sound, Gameplay: value changed and kept; Jukebox, Mods, Keyboard), Quit, restart: settings kept (core: Setup > Gameplay, Quit, restart) |
| arcade-1p, arcade-2p | | Arcade Game Menu: each line, Options, Start Level, Quit |
| endless | | 25 s of Endless, Quit, name, summary, leaderboard: final score recorded |
| maintainer-profile | yes | copy of the maintainer's actual user folder (`--profile`): his save loads, the shop buys (core: with keyboard) |

Results: `build/player-journeys/report.md` (table, each FAIL with its
capture), `steps.jsonl`, `shots/`.

## Adding a journey

A function `journey_<name>(g, rep, mode)` in
`rewrite/scripts/player_journeys.py`, registered in `JOURNEYS`. Each action
is followed by a `rep.check(action, expected, observed, condition)` whose
condition reads the state (`g.state()`) or a game file. If the effect to verify
is not in the state, add it to `journey_state.gd` (read-only).
