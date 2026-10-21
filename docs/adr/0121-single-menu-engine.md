# ADR 0121 — A single menu engine for the 8-bit screens (block 20.1)

Status: accepted (block 20, refactoring approved by the maintainer).

## Context

The C version has two menu systems: the legacy loops (`menus.c`, `game_menu.c`,
`mainint.c:JE_loadScreen`, `title_screen.c`…) that read `lastkey_scan` and draw by themselves, and
`menu_tree.c` (declarative tree: `MenuItemKind`, `run_menu`, `picker_popup`) for Setup and its
sub-menus. The Godot rewrite had copied this duality, and more: a `run_menu` stack in
`setup.gd`, and a hand-written navigation loop in every other screen (selection,
wrapping, scrolling window, sounds).

## Decision

- A single engine: `rewrite/godot/scripts/menu_engine.gd` (`MenuEngine`, a `RefCounted` attached to a
  `TyrianScreen`). Model of `menu_tree.h`: ACTION, TOGGLE, PICKER (with popup), BACK,
  KEYBIND, CUSTOM (drawing / activation / adjustment provided by the screen), LABEL rows; menu stack;
  navigation `menu_tree.c:move_selection` / `activate` / `adjust_horizontal`; `picker_popup`
  popups.
- The presentation differences of the C version become **drawing styles**, not engines:
  `MENU_TREE` (menu_tree.c:run_menu), `TITLE` (title_screen.c), `TOPICS` (mainint.c:JE_helpSystem),
  `GAME_MENU` (game_menu.c:JE_drawMenuHeader / JE_drawMenuChoices), `ROWS` (lists of CUSTOM rows:
  JE_loadScreen, replay_browser, achievements_display).
- The navigation differences of the legacy loops become menu **options**: `accept`
  (Space accepted or not), `move_sfx`, `wrap` (wrapping or bounded cursor), `redraw_on_other` (image
  re-presented on a key with no effect), `redraw_on_horizontal`, `shortcuts` (screen-specific actions:
  M/E/2 on the title, loading pages, achievement categories), `on_activate`, `on_back`,
  `window` (window centered on the cursor), `external` (navigation handled elsewhere: shop,
  `TyrianShop` in Rust).
- `render()` draws without `present()` (screens whose first image goes through a fade); `draw()`
  draws then presents, and opens the deferred popup like `run_menu`.

## Migrated screens

Setup and all its sub-menus (Graphics + popups, Sound, Gameplay, Mutators, Keys, Menu/Game Keys,
Gamepad), title, help (Instructions), Load Game, replays, achievements, shop (root, Upgrade Ship,
item list of all the categories including those with a weapon demonstration, Next Level),
side modes (`modes_menu.gd`) and mods (`mods_menu.gd`). The volume of the Sound menu, specific to
`setup.gd`, became a CUSTOM row as in `sound_menu.c`.

The side modes and the mods are screens specific to the rewrite (no C image to
reproduce) drawn with `TyrianText` nodes: they go through the engine for the model and
navigation (PICKER / TOGGLE / CUSTOM rows, wrapping, sounds) and provide their drawing through
the `render` option; their captures are pixel-identical before and after the migration.

Outside the engine, for good reasons:

- scores (`high_scores.gd`) and debriefing (`debrief.gd`): no list of choices (Left/Right
  changes the episode; a key continues);
- Endless (`endless_prompt.gd`, `endless_end.gd`): name entry and card choice, not
  row-based menus;
- key capture on the Controls screen (`controls_options.gd`): the key itself is read,
  not an action;
- shop sub-screens without a list (Data Cubes, Ship Specs, Options, animated firing simulation
  of `shop_preview.c`, animated map of `JE_updateNavScreen`): not ported, covered by the full shop
  card; their lists will go through this engine;
- `level_select.gd` and `display_settings.gd`: screens held by other batches (editor, display).

## Consequences

- One more menu = a table of rows + a style; no navigation loop to rewrite.
- Pitfall: `_draw()` of a `TyrianScreen` is also the virtual `CanvasItem._draw`, called by the
  engine before `_run`; a screen that lazily opens its menu in `_draw()` opens it too early
  (empty list). Open the menu in `_run`.
- Verification: `rewrite/scripts/ui_regression.sh` — 16 sequences, 0 rows different from the C version and from
  the Godot reference taken before the refactoring.
  Since the sorting of the Setup options (docs/setup-options.md), the seven sequences of the Setup
  sub-menus are no longer compared with the C version (deliberately different screen): 9 sequences remain.
