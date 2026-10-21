# ADR 0082 — Creation kit: mod template and editor dock

Status: accepted (mods batch, block 17). Format superseded by ADR 0083 then 0128 (named events) (single format, items designated by name):
the mentions of `data/mods`, `"format": 2`, `new_enemies` and the numbers below are historical.

## Context
We need a short path "Godot level editor → events → Play from here → package" that
produces a mod readable by the same loader as the C mods (ADR 0080).

## Decision
- Ready-to-copy template: `rewrite/mod_template/` (commented format 2 `mod.json`, `README.md`).
- Mod levels: `levels/epN_NN.json` (same fields as `LevelEventResource`), key `events`
  (replaces the original list) or `add_events` (stable insertion after the events with the same
  `eventtime`). The map remains the game's. Reading: `tyrian-mods/src/levels.rs`.
- "Tyrian" dock of the `addons/tyrian_level_editor` plugin: "Add an enemy here" (event
  type 15), "Export to mod" (copy of the template + `levels/*.json` into `user://mods/<name>`),
  "Create the .zip package", "Play from here" (exports first if needed).
- The export writes the COMPLETE list (`events`): simpler to review than diffs, and
  it reflects exactly what the author sees in the editor.

## Consequences
Tutorial `docs/FIRST_LEVEL_15_MIN.md` (replaces `FIRST_MOD.md` for the rewrite), followed under
Xvfb with `rewrite/scripts/editor_session.sh`, captures `docs/screenshots/mods/01..10`.
Limitation: a mod cannot yet modify a level's map (tiles).
