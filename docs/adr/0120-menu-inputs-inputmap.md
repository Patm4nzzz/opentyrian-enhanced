# ADR 0120 — Screen inputs through shared InputMap actions (block 20.2)

Status: accepted (block 20, refactoring approved by the maintainer).

## Context

Each Godot screen tested `KEY_*` in its own `_unhandled_input` (title, Setup, shop,
scores, help, loading, achievements, replays, controls, mods, modes, Endless end and resume). The gamepad
only reached them through a translation into a key (`TyrianControls.menu_key_for`, ADR 0051) limited to the six
keys of `MENU_KEY`. The C version has a menu action table (`input_map.h`: `INPUT_MENU_UP` …
`INPUT_MENU_NO`, defaults in `input_map.c:input_map_init`), but its legacy screens still read
`lastkey_scan` (Space = Enter in `title_screen.c`, `JE_loadScreen`, `JE_highScoreScreen`,
`JE_helpSystem`; Tab in `game_menu.c:JE_quitRequestMsg`).

## Decision

- `TyrianControls.MENU_ACTIONS` takes over the eleven actions of `input_map.h` (up, down, left, right,
  accept, back, delete, page_up, page_down, yes, no) with the C defaults, and names the keys that the
  legacy screens test directly: `select` (Space), `toggle` (Tab), `reset` (Delete, controls screen),
  `modes` / `endless` / `endless_2p` (M, E, 2 on the title, ADR 0050 and 0071), `retry` (R,
  `endless.c:endless_leaderboard_show`). Space remains a separate action from Enter: some screens
  do not accept it (Setup, `menu_tree.c:run_menu`; achievements).
- `TyrianControls.menu_action(event)` returns the action of a keyboard or gamepad event (`&""` otherwise).
- `TyrianScreen._unhandled_input` routes every event to `handle_action(action)` when the screen
  defines it; a key with no action arrives as `&""` (some C loops re-present the image on any
  key). `Node2D` screens (controls, mods, modes) call `menu_action` themselves.
- The capture tool (`--keys`) goes through the same path (`handle_event(key_event(code))`): the
  visual regression therefore exercises the InputMap table itself.
- The gamepad reaches the screens with `handle_action` (`effects.gd`) as is; `menu_key_for` is
  now only used by screens belonging to another batch (`level_select.gd`, `display_options.gd`).
- Exceptions kept as raw reading: capturing a new binding (controls screen, the key
  itself is what is requested) and the typed character of the Endless name (`unicode`).
- In-level technical help (F1) and capture cancellation (Esc): InputMap actions
  `tyrian_help` and `tyrian_capture_cancel` (`TyrianControls.GLOBAL_COMMANDS`, review m3), shared by
  both players, keyboard only, set by the last two rows of the controls screen and
  saved in `controls.cfg` (`[global]` section). So that no setting can lock the player out:
  - any key or button ends a capture (it cannot stay pending);
  - the cancel key refuses Enter, Up, Down, Tab, Delete (starting, choosing or resetting a
    capture), any gameplay key of either player and the help key; the help key refuses
    gameplay keys and the cancel key; a gameplay key refuses the help key. A refusal
    leaves the old key and displays "Key refused: already used";
  - Delete also restores F1 and Esc.
  Esc also remains the "back" menu action (`tyrian_menu_back`), not reconfigurable.

## Consequences

- Rebinding a menu action (InputMap) affects all screens at once.
- Verification: `tools/check_controls.gd` (now `test/controls_test.gd`, ADR 0084) (8 checks added: Enter / keypad Enter / Space /
  Esc / Tab / key without action / gamepad down / gamepad B), `rewrite/scripts/ui_regression.sh`
  (0 rows different across the 16 sequences), golden masters unchanged. Reconfigurable help and
  cancel: 3 cases in `test/controls_test.gd` (`controls.cfg` round trip, refused keys,
  capture on the controls screen with a changed cancel key).
