# ADR 0129 — Native campaign save

Status: accepted (2026-10-05, card t_f20c39df, point 3). Replaces, for the campaign, the rewrite's reading and
writing of `opentyrian.dat` (GAME section, `tyrian-formats/src/save.rs`).

## Context
The rewrite read the slots of the C version's save (`opentyrian.dat`, 22
encrypted slots of 109 bytes, raw item indices) and only knew how to write them through the
campaign bot: the player could not save their game. the maintainer wants a format specific to
the rewrite, readable, by name and versioned, with no dependency on the C version's file.

## Decision
- **One JSON file per slot**: `user://saves/slot_NN.json` (NN = 01..11). One file per
  slot rather than a single file: a write can only damage one slot, and a damaged
  file does not prevent reading the others.
- **11 one-player slots**, like the "One Player Saved Games" page of the C version. Slot 11 is
  the "LAST LEVEL" automatic save, written before each campaign level
  (`tyrian2.c:JE_main`, "Save backup game", `saveLevel` section); the player can load it but
  cannot overwrite it (`mainint.c:JE_operation`, `slot % 11 != 0`). The two-player campaign
  is not ported: the two-player page of "Load Game" remains empty.
- **Content** (`tyrian-formats/src/campaign_save.rs`, `CampaignSaveFile`): `format`
  (`"opentyrian-rewrite-campaign-save"`), `format_version` (1), `slot`, `name` (14 capital letters at
  most), `saved_at`, `players`, `episode`, `section` (section of `levelsN.dat` where to resume, its
  shop), `last_level`, `difficulty` and `initial_difficulty` by name (`wimp` … `nortaneous`),
  `cash`, `data_cubes`, then the equipment **by name**: `ship`, `front_weapon {name, power}`,
  `rear_weapon` (or `null`), `shield`, `generator`, `left_sidekick` / `right_sidekick` / `special`
  (or `null`). The names are those of the item tables of the saved episode (`tyrianN.lvl` or
  `tyrian.hdt`, the same ones the shop displays). A name carried by several items of a table
  ("Multi-Cannon" front and rear…) is written `"Multi-Cannon #3"`; an item outside the original
  tables (mod item) is written `"#40"`. No index array. Optional field
  `super_arcade_mode` (`pItems[2]` of the C version: 1..9 Super Arcade ship, 254 Super Tyrian, 255
  arcade; absent = full game): "Load Game" restores the mode of the saved game.
- **Atomic write**: file `slot_NN.json.tmp` in the same folder, `sync_all`, then `rename`.
- **Strict reading, never a crash**: label and `format_version` checked before the rest
  (a file from a future version is refused for its version, even if it has new fields),
  unknown fields rejected, unknown names, powers outside 1..11, episode, section, difficulty and
  slot checked. A rejected slot is displayed as "Unreadable" in both loading
  screens, cannot be loaded, and the detailed message goes to the log (`push_warning`).
- **A single version**: any incompatible change will increment `format_version` and add
  the conversion from version 1 in `campaign_save.rs`.
- **Menus** (shared engine, ADR 0121): shop "Options" entry →
  `save_menu.gd` (Load Game, Save Game, Done; then the slot list with the geometry of
  `MENU_LOAD_SAVE`, name entry of `JE_operation`, confirmation of `JE_saveRequest`); title "Load
  Game" (`load_screen.gd`). The other rows of the C Options menu (volumes, keyboard,
  gamepad, mouse) are in Setup and are not carried over.
- **No more dependency on the C version's file for the campaign**: `TyrianCampaign.load_save_slots` and
  `save_slot` (reading / writing the GAME section) are removed. Only a **one-time
  import** remains (`TyrianCampaign.import_c_save`): on first access, if `user://saves` does not exist and
  a `user://opentyrian.dat` is present, its one-player slots 1 to 11 are converted;
  after that the C file is no longer read for the campaign. `save.rs` remains in `tyrian-formats`
  for this import, the achievements (OTYE block) and the comparison tools with the C version.
- **High scores**: the rewrite did not read them from the `.sav` (randomly drawn tables
  like `JE_loadConfiguration` without a save, `high_scores.gd`) and does not write them yet;
  nothing to move. Their saving, when ported, will go to a native file in the same
  spirit, not to `opentyrian.dat`.

## Evidence
- `cargo test -p tyrian-formats campaign_save`: round trip by names (duplicates and mod items
  included), unknown versions and other formats rejected, truncated file, non-object JSON, unknown
  names, powers, difficulty, episode, players, unknown fields rejected; atomic write
  with no `.tmp` left; file copied into another slot rejected; non-UTF-8 bytes rejected.
- gdUnit4 `save_test.gd`: save through the real shop keys (Options → Save Game →
  slot 3 → name → confirmation), file read back (names, power, difficulty), return to the
  shop without loss, then title "Load Game" → same episode, section, equipment, money,
  difficulty; automatic save before the level; truncated file, version 99, unknown name
  and non-JSON marked unreadable, loading refused without a screen change. `campaign_test.gd`
  `test_c_save_import`: import of a real C save, with no overwrite on the second import.
- Campaign bot: native save at each shop, reload through `Game.campaign_slots`
  then `Game.load_campaign_slot` at the 2nd shop of each episode.

## Consequences
Saves made by the rewrite are not readable by the C version (and conversely,
apart from the one-time import). A mod that changes the names of the original items makes a save
that cites them unreadable: "Unreadable" message, never a crash.
