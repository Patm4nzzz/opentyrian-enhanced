# ADR 0005 — The Godot editor as the level editor

Date: 2026-10-04 — status: accepted (decision made alone).

## Decision
Each imported level becomes a generated Godot scene: three `TileMapLayer` nodes (layer 1: 14 columns × 300 rows; layer 2: 14 × 600; layer 3: 15 × 600; 24×28 tiles, `level_load.c:JE_loadMap`) and a `LevelEventResource` resource that carries the event list (time, type, `eventdat`…`eventdat6`). An editor plugin adds the "Play from here" command, which launches the game scene at the chosen scroll position.

`TileMapLayer` replaces the old `TileMap` node, deprecated since Godot 4.3.

## Consequences
Editing goes through the standard Godot tools; a reverse exporter (scene → `tyrianN.lvl`) is possible because `tyrian-formats` can rewrite the original format, but it is not in the scope of this rewrite.
