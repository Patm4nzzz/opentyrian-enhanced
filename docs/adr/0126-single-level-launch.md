# ADR 0126 — A single path to launch a level at a position

Date: 2026-10-04 (block 20.7, refactoring approved by the maintainer). Updated on 2026-10-05 (review m5):
list of callers and exceptions re-read in the code.

## Context

Four places set `Game.episode`, `Game.lvl_file_num`,
`Game.start_cur_loc` themselves and then loaded `scenes/level.tscn`:

- the "Level editor" screen (`level_select.gd`), through the private function
  `Game._parse_level_id`;
- "Play from here" in the editor plugin, which wrote
  `user://play_from_here.json` by hand (path hard-coded because the `Game` autoload
  does not exist in the editor), read back by `Game._consume_play_from_here`;
- the campaign (`Game.advance_campaign`, "level" stop of JE_loadMap);
- Endless mode (`EndlessSession._goto_next`).

## Decision

- `Game.start_level_at(episode, lvl_file_num, cur_loc, options)` is the only
  point that sets the level and the starting position and loads the level
  scene. `options.load_scene = false` when the scene is already the main
  scene (launch from the editor) or will be loaded by the caller (Endless
  mode prepared for the capture tool).
- The editor's request file is read and written by two static functions
  of `LevelMap` (`@tool` class, hence available in the editor and
  in the game): `write_start_request` / `take_start_request`. Same path
  (`user://play_from_here.json`) and same JSON keys as before.
- `Game.parse_level_id` becomes public and pure (returns `[episode, level]`).
- The rest of the game state (difficulty, equipment, modes, campaign) remains
  set by the caller before the call: grouping it into `options` would have
  moved campaign logic around without simplifying it.
- Current callers of `start_level_at`: `level_select.gd` (Level
  editor screen), the editor request read back at startup (`game.gd`), the campaign
  (`Game.advance_campaign`), Endless mode (`endless_session.gd`) and the
  side modes menu (`modes_menu.gd`, migrated since). The title screen
  (`title.gd`) launches no level itself: "New Game" goes through
  `Game.start_new_game`, hence through the campaign.
- The only exceptions are tools that prepare the state of a replay and then load
  the scene themselves: `tools/bench.gd`, `tools/capture.gd` and
  `tools/video_frames.gd`.

## Consequences

A single function to follow to know how a level starts. Captures
(golden e1-tyrian-autopilot tick 4221, level selection, real launch
"Play from here" through the dropped file) pixel-identical; launch
from the Level editor screen verified before/after (same level, same cur_loc).

## Update 2026-10-10

The Level editor screen (`level_select.gd`, scene and constant `SCENE_LEVEL_SELECT`) is deleted: the menu no longer reached it since the title screen redesign (no caller, see `git log -S SCENE_LEVEL_SELECT`). Launching at a position is still handled by the campaign, the editor's "Play from here", Endless mode and the side modes menu.
