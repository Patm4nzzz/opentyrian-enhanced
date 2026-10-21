# ADR 0051 — Controls through shared InputMap actions, player 2, mouse, Steam Deck

Status: accepted (modes batch, blocks 5, 14 and 15).

## Context

The C version reads the keyboard (`config.c:keySettings`, editable through `keybindings_menu.c`), the mouse
(`keyboard.c`, `config.c:defaultMouseSettings` = {1, 4, 5}) and gamepads (`joystick.c`,
`player_movement.c:JE_playerMovement`: `action[0]` fire, `action_pressed[1]` fire mode switch,
`action[2]`/`action[3]` sidekicks), filtered by each player's `inputDevice[]`. The two players
share the same keys: in local play, player 2 plays with the mouse or a gamepad.
The Godot rewrite hard-coded player 1 (`game.gd`, `KEY_*`) and passed 0 for player 2.

## Decision

- `scripts/controls.gd` (`TyrianControls`) creates **all** the actions: `tyrian_<command>` (player 1,
  historical names kept), `tyrian_p2_<command>`, and the menu actions `tyrian_menu_*` and
  `tyrian_level_exit`. Upcoming screens (block 20.2) bind events to these actions instead of
  testing `KEY_*`.
- Player 1: keys from `config.c:defaultKeySettings` + gamepad 0. Player 2: gamepad 1 and dedicated
  keys (physical WASD, F fire, G switch, R/T sidekicks) — **a rewrite choice**, the C version has none;
  it allows two players on one keyboard.
- Gamepad: d-pad and left stick (dead zone 0.35), A fire, B fire mode switch, X/L1 left sidekick,
  Y/R1 right sidekick, R2 fire, L2 fire mode switch, Start exits the level. Analog directions
  are converted to a mask (all-or-nothing), which stays within the input domain of `tyrian-sim`
  (`PlayerInput`); the C version's fine analog movement (`joystick_axis_reduce`) is not reproduced
  (divergences.md, entry 12).
- Mouse: relative motion accumulated between two ticks, then `TyrianSim.set_mouse(mask, dx, dy)`;
  buttons per `defaultMouseSettings` (left fire, right both sidekicks, middle fire mode switch).
  The simulation only applies it to the player whose device is 0 or 2 (like the C version).
- Existing menus receive the gamepad as an equivalent key (`effects.gd`:
  `TyrianControls.menu_key_for`), without rewriting their `match KEY_*`.
- Gamepad analog (Setup > Joystick > Analog): **off by default for a new profile,
  like the C version** (`joystick.c:507`, `analog = false` with no config key); the d-pad and stick
  then give all-or-nothing directions at the keyboard's step. An existing `controls.cfg` that enabled it
  keeps it (`[joystick] analog_N`). Revision 2026-10-09 (the maintainer's "like the C version" rule):
  analog used to be on by default.
- Bindings are saved to `user://controls.cfg` (ConfigFile) by the controls screen (F9).
- Steam Deck: profile `res://config/steam_deck_controls.cfg`, loaded on first launch if
  `SteamDeck=1` (variable set by Steam) and there is no `user://controls.cfg`.

## Consequences

- The golden masters do not go through this layer: parity remains proven by `tyrian-golden`;
  the Godot captures replay the inputs (mask + mouse) from the file.
- Verification: `godot --headless --path . -s tools/check_controls.gd` (14 checks);
  now `test/controls_test.gd` (gdUnit4, ADR 0084).
