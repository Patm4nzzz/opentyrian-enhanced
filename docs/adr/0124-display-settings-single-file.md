# ADR 0124 — Display settings: a single file

Date: 2026-10-04 (block 20.4, refactoring approved by the maintainer)

## Context

Two scripts carried the same responsibility: `display_settings.gd` (static class
`DisplaySettings`: reading/writing `user://display.cfg`, the `--hd-sprites` option,
the folder of upscaled PNGs) and `display_options.gd` (the F10 screen,
which called `DisplaySettings` then `Effects.apply_display_settings()`).

## Decision

- `display_options.gd` is deleted; `display_settings.gd` contains the
  persistence, the application (`toggle_hd_sprites()` = save then
  `Effects.apply_display_settings()`) and the screen. The class inherits from `Node2D`
  to serve as the script of `scenes/display_options.tscn`; its setting functions
  remain `static`, so all the callers (`assets.gd`, `effects.gd`,
  `level.gd`) are unchanged.
- The scene path (`res://scenes/display_options.tscn`, used by F10 and
  the capture tool) and the `[display] hd_sprites` key of `user://display.cfg`
  are kept: an existing settings file is read back as is.
- CRT and smoothing remain non-persistent (behavior unchanged); making
  them persistent would be a behavior change, outside the refactoring.

## Consequences

A single place to read to know what a display setting is, how it is
saved and applied. Options screen capture: 0 pixels different
before/after.

## Addendum (block 20 integration)

Since `display_settings.gd` is loaded by `TyrianAssets`, the level scene generator
no longer compiled with `--script` (autoload name `Effects`/`Game`
unknown outside a scene). The generator is now launched as a scene
(`res://editor/build_levels.tscn`), where the autoloads exist; build scripts
and `docs/build.md` updated. Verified: 70 scenes generated without errors.

## Addendum (2026-10-05, review point 17): graphics rendering

- The saved setting becomes `[display] art_mode` (0 Original, 1 Sharp pixel art, 2 HD (AI)), replacing
  `hd_sprites` (a single format version; the old key is not read back: falls back to Original). F10 screen: row
  "Graphics: …"; F3 toggles the same value (smoothing is no longer a separate, unsaved setting).
- **Sharp pixel art** = xBRZ "freescale" shader from libretro/glsl-shaders (`xbrz/shaders/xbrz-freescale.glsl`, commit
  `c6a82efb`), ported without changing the algorithm into `shaders/xbrz_freescale.gdshaderinc` (adaptations: Godot
  syntax, neighbors renamed `pA..pI`). It replaces the former home-made "xBR-style" filter of `post.gdshader`. Computed at
  display time, at the window size: no file, no dedicated GPU, no effect on the simulation (golden masters
  unchanged). License: MIT header (Hyllian) and GPL v3 with exception (Zenju, xBRZ); compatible with the project's GPL v2 "or
  later", the combination being distributed under GPL v3. Mentioned in the package's `DATA-NOTICES.txt`.
- **HD (AI)** remains in the enumeration (`--hd-sprites` for captures) but is not in `SCREEN_ART_MODES`: the backdrop
  grid is only downscaled (ADR 0100, point 16). the maintainer's rule: no HD option on screen as long as that remains the case.
- Test: `test/display_settings_test.gd` (persistence, read-back, filter applied, HD absent from the toggle, invalid
  value). Captures of the three renderings on the same scene: `docs/screenshots/export/art-modes-1500.png`.
