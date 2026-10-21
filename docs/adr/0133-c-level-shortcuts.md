# ADR 0133 — Level shortcuts: the C table, slow motion on Left Shift

Status: accepted (2026-10-08, decisions by the maintainer: "Oui", then "Go" for Left Shift). Replaces the F8 key of gap no. 30 and the display shortcuts F2, F3, F4, F9, F10 of the rewrite **in a level**.

## Context
Audit B (gaps B2 and B3) measured that, in a level, the C version's keys did not have their function in the rewrite: Tab did nothing, F3 changed the graphics mode, F4 triggered slow motion, F6 and F12 did nothing, and pressing Shift alone triggered slow motion although the C version does not do that.

## Decision
In-level table (`input_map.c:enhanced_keys[]`, `input_map_bindings[]`):

| Key | Function | C source |
|---|---|---|
| F1 | in-game help (unchanged) | `mainint.c:JE_inGameHelp` |
| F2 | pause (P always works) | `INPUT_PAUSE`, `mainint.c` |
| F3 | achievement list as an overlay | `achievements.c:achievements_display` |
| F4 | STATISTICS table | `stats.c:stats_display` |
| F5 / F9 | quick save / load (Y / N question) | `input_action_quick_save`, `quick_load_*` |
| F6 | DPS counter | `enemy_render.c`, `dps.c` |
| F7 | damage numbers (unchanged) | `input_action_toggle_damage_numbers` |
| F10 | replay recording (cancels gap no. 30: F8 is freed) | `EKEY_REPLAY` |
| F11 | gamma, four levels | `JE_gammaCheck` |
| F12 | screenshot `screenshot_NNNN.bmp` in the user folder | `video.c:JE_saveScreenshot` |
| **Left Shift** | **slow motion (4 s, then 15 s cooldown)** | `bullet_time.c` — **intended gap: the C version uses Tab** |

1. **Slow motion on Left Shift** (the maintainer's choice): the gamepad's left trigger and mouse side button 4 do the same. A player's gameplay key placed on Left Shift takes precedence. Right Shift does nothing. The left trigger is therefore no longer used for fire switching (button B keeps it). The messages, the "Bullet Time" achievement and the 50-use counter are those of the C version.
2. **All these keys are reconfigurable** in the controls screen (shared key rows). Conflicts follow `input_scancode_in_use`: a key already taken by another shortcut, or a player's gameplay key, is refused; P remains reserved.
3. **Removed in a level**: F2 (CRT), F3 (graphics rendering), F4 (slow motion), F9 (controls screen), F10 (Display screen). **Outside a level, nothing changes**: these keys keep their function there, since the C version assigns nothing to them. The CRT filter, the rendering and the controls remain reachable through Setup > Graphics and Setup > Key Bindings.
4. **Simulation intact**: everything is drawn over the image (interface layer) or handled on the Godot side; no `tyrian-sim` state changes, strict parity stays at 76/76.
5. **Counters**: the lifetime and run statistics (`stats.c`) are held by the achievement tracking of `tyrian-sim` and saved in `opentyrian.dat` (OTYE block, extensions OTY2 and OTY3), readable by the C version.

## Consequences
- The player guide (EN and FR) and `divergences.md` carry the new table.
- The gdUnit tests go through real keyboard events; the on-screen proof compares the Linux package with the `c-final` C version under Xvfb.
